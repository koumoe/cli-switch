use anyhow::Context as _;
use serde::Serialize;

const GITHUB_OWNER: &str = "koumoe";
const GITHUB_REPO: &str = "cli-switch";

#[derive(Debug, Clone, Serialize)]
pub struct ChangelogSection {
    pub title: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangelogOverview {
    pub version: String,
    pub locale: String,
    pub sections: Vec<ChangelogSection>,
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

fn tag_for_version(version: &str) -> String {
    format!("v{}", normalize_version(version))
}

fn locale_candidates(locale: &str) -> Vec<String> {
    let raw = locale.trim().to_lowercase();
    let base = raw
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    let mut out = Vec::new();
    if !raw.is_empty() {
        out.push(raw.clone());
    }
    if !base.is_empty() && base != raw {
        out.push(base.clone());
    }

    // 项目内部约定：简体中文文件使用 cn.md
    if base == "zh" {
        out.push("cn".to_string());
    }
    if base == "en" {
        out.push("en".to_string());
    }

    out.push("en".to_string());

    // 去重（保序）
    let mut seen = std::collections::HashSet::<String>::new();
    out.retain(|v| seen.insert(v.clone()));
    out
}

async fn download_text(client: &reqwest::Client, url: &str) -> anyhow::Result<reqwest::Response> {
    client
        .get(url)
        .send()
        .await
        .with_context(|| format!("download failed: {url}"))
}

fn extract_version_block(markdown: &str, version: &str) -> Option<String> {
    let target = normalize_version(version);
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;

    for (idx, line) in markdown.lines().enumerate() {
        if !line.starts_with("## ") {
            continue;
        }
        if start.is_none() {
            if line.contains(&format!("[{target}]")) || line.contains(&format!(" {target}")) {
                start = Some(idx);
            }
            continue;
        }
        end = Some(idx);
        break;
    }

    let start = start?;
    let end = end.unwrap_or_else(|| markdown.lines().count());
    let block = markdown
        .lines()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string();
    Some(block)
}

fn simplify_item_text(line: &str) -> String {
    let s = line
        .trim()
        .trim_start_matches('*')
        .trim_start_matches('-')
        .trim();
    if let Some(idx) = s.find(" ([") {
        return s[..idx].trim().to_string();
    }
    s.to_string()
}

fn parse_sections(block: &str) -> Vec<ChangelogSection> {
    let mut sections: Vec<ChangelogSection> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_items: Vec<String> = Vec::new();

    let flush = |sections: &mut Vec<ChangelogSection>,
                 title: &mut Option<String>,
                 items: &mut Vec<String>| {
        if items.is_empty() {
            *title = None;
            return;
        }
        sections.push(ChangelogSection {
            title: title.take().unwrap_or_else(|| "更新内容".to_string()),
            items: std::mem::take(items),
        });
    };

    for line in block.lines() {
        let line = line.trim_end();
        if let Some(h) = line.strip_prefix("### ") {
            flush(&mut sections, &mut current_title, &mut current_items);
            current_title = Some(h.trim().to_string());
            continue;
        }

        let t = line.trim_start();
        if t.starts_with("* ") || t.starts_with("- ") {
            current_items.push(simplify_item_text(t));
        }
    }

    flush(&mut sections, &mut current_title, &mut current_items);
    sections
}

pub async fn fetch_update_overview(
    client: &reqwest::Client,
    version: &str,
    locale: &str,
) -> anyhow::Result<ChangelogOverview> {
    let version_norm = normalize_version(version);
    let tag = tag_for_version(&version_norm);
    let mut last_err: Option<anyhow::Error> = None;

    for cand in locale_candidates(locale) {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/docs/changelog/{}.md",
            GITHUB_OWNER, GITHUB_REPO, tag, cand
        );
        let resp = match download_text(client, &url).await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            continue;
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            last_err = Some(anyhow::anyhow!("download failed: {status} {body}"));
            continue;
        }

        let text = resp
            .text()
            .await
            .with_context(|| format!("read changelog response failed: {url}"))?;

        let block = extract_version_block(&text, &version_norm)
            .with_context(|| format!("version block not found: v{version_norm}"))?;

        return Ok(ChangelogOverview {
            version: version_norm,
            locale: cand,
            sections: parse_sections(&block),
        });
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("changelog not found")))
}
