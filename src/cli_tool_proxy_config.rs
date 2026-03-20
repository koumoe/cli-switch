use anyhow::Context as _;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use crate::cli_tools::CliToolId;
use crate::i18n::{AppLocale, UserFacingIssue, UserFacingIssuePayload};

#[derive(Debug, Clone)]
struct ProxyUrls {
    /// Root base URL, e.g. http://127.0.0.1:3210
    base: String,
    /// OpenAI-compatible base URL, e.g. http://127.0.0.1:3210/v1
    openai: String,
}

impl ProxyUrls {
    fn from_listen_addr(addr: SocketAddr) -> Self {
        // Always force 127.0.0.1 (even if the server listens on 0.0.0.0 for LAN access).
        let port = addr.port();
        let base = format!("http://127.0.0.1:{port}");
        let openai = format!("{base}/v1");
        Self { base, openai }
    }
}

fn normalize_url(s: &str) -> String {
    s.trim().trim_end_matches('/').to_string()
}

fn home_dir() -> anyhow::Result<PathBuf> {
    let ud = directories::UserDirs::new().context("无法定位用户目录（UserDirs）")?;
    Ok(ud.home_dir().to_path_buf())
}

fn claude_config_dir() -> anyhow::Result<PathBuf> {
    // Claude Code supports overriding this via CLAUDE_CONFIG_DIR. Keep compatible.
    if let Some(v) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(v));
    }
    Ok(home_dir()?.join(".claude"))
}

fn claude_settings_path() -> anyhow::Result<PathBuf> {
    Ok(claude_config_dir()?.join("settings.json"))
}

fn claude_user_state_path() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?.join(".claude.json"))
}

fn codex_config_path() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?.join(".codex").join("config.toml"))
}

fn gemini_env_path() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?.join(".gemini").join(".env"))
}

#[derive(Debug, Clone, Serialize)]
pub struct CliToolProxyConfigCheck {
    pub id: String,
    pub ok: bool,
    pub file: String,
    pub key: String,
    pub expected: String,
    pub current: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliToolProxyConfigToolStatus {
    pub id: CliToolId,
    pub name: &'static str,
    pub ok: bool,
    pub checks: Vec<CliToolProxyConfigCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliToolProxyConfigStatusResponse {
    pub tools: Vec<CliToolProxyConfigToolStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliToolProxyConfigApplyItem {
    pub id: CliToolId,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<UserFacingIssuePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliToolProxyConfigApplyResponse {
    pub ok: bool,
    pub applied: Vec<CliToolProxyConfigApplyItem>,
    pub status: CliToolProxyConfigStatusResponse,
}

fn read_text_if_exists(path: &Path) -> anyhow::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("读取文件失败：{}", path.display())),
    }
}

fn write_text(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建目录失败：{}", parent.display()))?;
    }
    std::fs::write(path, content).with_context(|| format!("写入文件失败：{}", path.display()))?;
    Ok(())
}

fn parse_dotenv(s: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        // Support "export KEY=VALUE" (best-effort).
        let t = t.strip_prefix("export ").unwrap_or(t).trim();
        let Some((k, v)) = t.split_once('=') else {
            continue;
        };
        let key = k.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let mut val = v.trim().to_string();
        // Strip simple quotes to avoid common .env quoting issues.
        if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
            || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
        {
            val = val[1..val.len() - 1].to_string();
        }
        out.insert(key, val);
    }
    out
}

fn patch_dotenv_text(prev: &str, updates: &HashMap<&str, &str>) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut written: HashSet<&str> = HashSet::new();

    for line in prev.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || !t.contains('=') {
            out.push(line.to_string());
            continue;
        }

        let (k_raw, _v_raw) = t.split_once('=').unwrap_or((t, ""));
        let key = k_raw.trim().trim_start_matches("export ").trim();
        if let Some(new_v) = updates.get(key) {
            if written.contains(key) {
                // Drop duplicates for the keys we manage.
                continue;
            }
            out.push(format!("{key}={new_v}"));
            written.insert(key);
        } else {
            out.push(line.to_string());
        }
    }

    for (k, v) in updates {
        if !written.contains(k) {
            if !out.is_empty() && !out.last().unwrap().is_empty() {
                // Add a spacer line before appending our managed keys if the file isn't empty.
                out.push(String::new());
            }
            out.push(format!("{k}={v}"));
            written.insert(k);
        }
    }

    let mut s = out.join("\n");
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn read_json_if_exists(path: &Path) -> anyhow::Result<Option<serde_json::Value>> {
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(None);
    };
    let v: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("解析 JSON 失败：{}", path.display()))?;
    Ok(Some(v))
}

fn get_json_bool(v: &serde_json::Value, key: &str) -> Option<bool> {
    v.get(key).and_then(|x| x.as_bool())
}

fn check_claude(urls: &ProxyUrls) -> anyhow::Result<CliToolProxyConfigToolStatus> {
    let settings_path = claude_settings_path()?;
    let settings_display = settings_path.to_string_lossy().to_string();

    let expected_base = urls.base.clone();
    let mut checks: Vec<CliToolProxyConfigCheck> = Vec::new();

    let (current_base, has_token, base_err) = match read_json_if_exists(&settings_path) {
        Ok(Some(v)) => {
            let env = v.get("env").and_then(|e| e.as_object());
            let cur = env
                .and_then(|m| m.get("ANTHROPIC_BASE_URL"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let token = env
                .map(|m| {
                    let a = m
                        .get("ANTHROPIC_AUTH_TOKEN")
                        .and_then(|x| x.as_str())
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false);
                    let b = m
                        .get("ANTHROPIC_API_KEY")
                        .and_then(|x| x.as_str())
                        .map(|s| !s.trim().is_empty())
                        .unwrap_or(false);
                    a || b
                })
                .unwrap_or(false);
            (cur, token, None)
        }
        Ok(None) => (None, false, None),
        Err(e) => (None, false, Some(e.to_string())),
    };

    let base_ok = current_base
        .as_deref()
        .map(normalize_url)
        .is_some_and(|v| v == normalize_url(&expected_base));
    checks.push(CliToolProxyConfigCheck {
        id: "claude_base_url".to_string(),
        ok: base_ok,
        file: settings_display.clone(),
        key: "env.ANTHROPIC_BASE_URL".to_string(),
        expected: expected_base.clone(),
        current: current_base,
        message: base_err.clone(),
    });

    checks.push(CliToolProxyConfigCheck {
        id: "claude_token_present".to_string(),
        ok: has_token,
        file: settings_display.clone(),
        key: "env.ANTHROPIC_AUTH_TOKEN / env.ANTHROPIC_API_KEY".to_string(),
        expected: "(non-empty)".to_string(),
        current: has_token.then_some("(set)".to_string()),
        message: base_err,
    });

    let state_path = claude_user_state_path()?;
    let state_display = state_path.to_string_lossy().to_string();
    let (current_onboarding, onboarding_err) = match read_json_if_exists(&state_path) {
        Ok(Some(v)) => (get_json_bool(&v, "hasCompletedOnboarding"), None),
        Ok(None) => (None, None),
        Err(e) => (None, Some(e.to_string())),
    };
    let onboarding_ok = current_onboarding.unwrap_or(false);
    checks.push(CliToolProxyConfigCheck {
        id: "claude_has_completed_onboarding".to_string(),
        ok: onboarding_ok,
        file: state_display,
        key: "hasCompletedOnboarding".to_string(),
        expected: "true".to_string(),
        current: current_onboarding.map(|v| v.to_string()),
        message: onboarding_err,
    });

    let ok = checks.iter().all(|c| c.ok);
    Ok(CliToolProxyConfigToolStatus {
        id: CliToolId::Claude,
        name: "Claude Code",
        ok,
        checks,
    })
}

fn check_codex(urls: &ProxyUrls) -> anyhow::Result<CliToolProxyConfigToolStatus> {
    let cfg_path = codex_config_path()?;
    let cfg_display = cfg_path.to_string_lossy().to_string();

    let expected_base_url = urls.openai.clone();

    let mut checks: Vec<CliToolProxyConfigCheck> = Vec::new();

    let content = match read_text_if_exists(&cfg_path) {
        Ok(v) => v,
        Err(e) => {
            checks.push(CliToolProxyConfigCheck {
                id: "codex_config_read".to_string(),
                ok: false,
                file: cfg_display.clone(),
                key: "(file)".to_string(),
                expected: "(readable)".to_string(),
                current: None,
                message: Some(e.to_string()),
            });
            return Ok(CliToolProxyConfigToolStatus {
                id: CliToolId::Codex,
                name: "Codex",
                ok: false,
                checks,
            });
        }
    };

    let doc = match content.as_deref() {
        Some(s) => match s.parse::<toml_edit::DocumentMut>() {
            Ok(doc) => doc,
            Err(e) => {
                checks.push(CliToolProxyConfigCheck {
                    id: "codex_config_parse".to_string(),
                    ok: false,
                    file: cfg_display.clone(),
                    key: "(file)".to_string(),
                    expected: "(valid TOML)".to_string(),
                    current: None,
                    message: Some(format!("解析 TOML 失败：{e}")),
                });
                return Ok(CliToolProxyConfigToolStatus {
                    id: CliToolId::Codex,
                    name: "Codex",
                    ok: false,
                    checks,
                });
            }
        },
        None => toml_edit::DocumentMut::new(),
    };

    let current_provider = doc
        .get("model_provider")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let provider_ok = current_provider
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty());
    checks.push(CliToolProxyConfigCheck {
        id: "codex_model_provider".to_string(),
        ok: provider_ok,
        file: cfg_display.clone(),
        key: "model_provider".to_string(),
        expected: "(non-empty)".to_string(),
        current: current_provider.clone(),
        message: None,
    });

    let provider_id = current_provider
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let current_base_url = provider_id.and_then(|pid| {
        doc.get("model_providers")
            .and_then(|v| v.get(pid))
            .and_then(|v| v.get("base_url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });
    let base_ok = current_base_url
        .as_deref()
        .map(normalize_url)
        .is_some_and(|v| v == normalize_url(&expected_base_url));
    checks.push(CliToolProxyConfigCheck {
        id: "codex_selected_provider_base_url".to_string(),
        ok: base_ok,
        file: cfg_display,
        key: provider_id
            .map(|pid| format!("model_providers.{pid}.base_url"))
            .unwrap_or_else(|| "model_providers.<model_provider>.base_url".to_string()),
        expected: expected_base_url,
        current: current_base_url,
        message: None,
    });

    let ok = checks.iter().all(|c| c.ok);
    Ok(CliToolProxyConfigToolStatus {
        id: CliToolId::Codex,
        name: "Codex",
        ok,
        checks,
    })
}

fn check_gemini(urls: &ProxyUrls) -> anyhow::Result<CliToolProxyConfigToolStatus> {
    let env_path = gemini_env_path()?;
    let env_display = env_path.to_string_lossy().to_string();

    let expected_base = urls.base.clone();
    let mut checks: Vec<CliToolProxyConfigCheck> = Vec::new();

    let (vars, read_err) = match read_text_if_exists(&env_path) {
        Ok(Some(s)) => (parse_dotenv(&s), None),
        Ok(None) => (HashMap::new(), None),
        Err(e) => (HashMap::new(), Some(e.to_string())),
    };

    let current_base = vars.get("GOOGLE_GEMINI_BASE_URL").cloned();
    let base_ok = current_base
        .as_deref()
        .map(normalize_url)
        .is_some_and(|v| v == normalize_url(&expected_base));
    checks.push(CliToolProxyConfigCheck {
        id: "gemini_base_url".to_string(),
        ok: base_ok,
        file: env_display.clone(),
        key: "GOOGLE_GEMINI_BASE_URL".to_string(),
        expected: expected_base,
        current: current_base,
        message: read_err,
    });

    // Gemini CLI non-interactive auth requires an API key env (unless user uses OAuth flows).
    // For CliSwitch proxy, a placeholder key is sufficient.
    let has_key = vars
        .get("GEMINI_API_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || vars
            .get("GOOGLE_API_KEY")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
    checks.push(CliToolProxyConfigCheck {
        id: "gemini_api_key_present".to_string(),
        ok: has_key,
        file: env_display,
        key: "GEMINI_API_KEY / GOOGLE_API_KEY".to_string(),
        expected: "(non-empty)".to_string(),
        current: has_key.then_some("(set)".to_string()),
        message: None,
    });

    let ok = checks.iter().all(|c| c.ok);
    Ok(CliToolProxyConfigToolStatus {
        id: CliToolId::Gemini,
        name: "Gemini CLI",
        ok,
        checks,
    })
}

pub fn get_status(listen_addr: SocketAddr) -> anyhow::Result<CliToolProxyConfigStatusResponse> {
    let urls = ProxyUrls::from_listen_addr(listen_addr);
    Ok(CliToolProxyConfigStatusResponse {
        tools: vec![
            check_claude(&urls)?,
            check_codex(&urls)?,
            check_gemini(&urls)?,
        ],
    })
}

fn apply_claude(urls: &ProxyUrls) -> anyhow::Result<()> {
    let settings_path = claude_settings_path()?;
    let mut root = match read_text_if_exists(&settings_path)? {
        Some(s) => serde_json::from_str::<serde_json::Value>(&s)
            .with_context(|| format!("解析 JSON 失败：{}", settings_path.display()))?,
        None => serde_json::Value::Object(serde_json::Map::new()),
    };

    if !root.is_object() {
        root = serde_json::Value::Object(serde_json::Map::new());
    }

    let obj = root.as_object_mut().expect("root must be object");
    if !obj.get("env").is_some_and(|v| v.is_object()) {
        obj.insert(
            "env".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }
    let env = obj
        .get_mut("env")
        .and_then(|v| v.as_object_mut())
        .expect("env must be object");

    env.insert(
        "ANTHROPIC_BASE_URL".to_string(),
        serde_json::Value::String(urls.base.clone()),
    );

    // Only fill a placeholder token when the user didn't configure any Anthropic token yet.
    let has_token = env
        .get("ANTHROPIC_AUTH_TOKEN")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty())
        || env
            .get("ANTHROPIC_API_KEY")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty());
    if !has_token {
        env.insert(
            "ANTHROPIC_AUTH_TOKEN".to_string(),
            serde_json::Value::String("cliswitch".to_string()),
        );
    }

    let mut out = serde_json::to_string_pretty(&root)?;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    write_text(&settings_path, &out)?;

    // hasCompletedOnboarding: true (Windows-specific known issue, but harmless to set generally).
    let state_path = claude_user_state_path()?;
    let mut state = match read_text_if_exists(&state_path)? {
        Some(s) => serde_json::from_str::<serde_json::Value>(&s)
            .with_context(|| format!("解析 JSON 失败：{}", state_path.display()))?,
        None => serde_json::Value::Object(serde_json::Map::new()),
    };
    if !state.is_object() {
        state = serde_json::Value::Object(serde_json::Map::new());
    }
    let obj = state.as_object_mut().expect("state must be object");
    obj.insert(
        "hasCompletedOnboarding".to_string(),
        serde_json::Value::Bool(true),
    );
    let mut out = serde_json::to_string_pretty(&state)?;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    write_text(&state_path, &out)?;

    Ok(())
}

fn apply_codex(urls: &ProxyUrls) -> anyhow::Result<()> {
    let path = codex_config_path()?;
    let prev = read_text_if_exists(&path)?;
    let mut doc = match prev.as_deref() {
        Some(s) => s
            .parse::<toml_edit::DocumentMut>()
            .with_context(|| format!("解析 TOML 失败：{}", path.display()))?,
        None => toml_edit::DocumentMut::new(),
    };

    // Prefer updating the currently selected provider to avoid overriding user setups.
    // If unset, create a default provider id.
    let provider_id = doc
        .get("model_provider")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "cliswitch".to_string());

    doc["model_provider"] = toml_edit::value(provider_id.as_str());
    doc["model_providers"][provider_id.as_str()]["base_url"] =
        toml_edit::value(urls.openai.clone());

    let mut out = doc.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    write_text(&path, &out)?;
    Ok(())
}

fn apply_gemini(urls: &ProxyUrls) -> anyhow::Result<()> {
    let path = gemini_env_path()?;
    let prev = read_text_if_exists(&path)?.unwrap_or_default();
    let current = parse_dotenv(&prev);

    let mut updates: HashMap<&str, &str> = HashMap::new();
    updates.insert("GOOGLE_GEMINI_BASE_URL", urls.base.as_str());

    let has_key = current
        .get("GEMINI_API_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || current
            .get("GOOGLE_API_KEY")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
    if !has_key {
        updates.insert("GEMINI_API_KEY", "cliswitch");
    }

    let next = patch_dotenv_text(&prev, &updates);
    write_text(&path, &next)?;
    Ok(())
}

pub fn apply(
    listen_addr: SocketAddr,
    tools: &[CliToolId],
    locale: AppLocale,
) -> anyhow::Result<CliToolProxyConfigApplyResponse> {
    let urls = ProxyUrls::from_listen_addr(listen_addr);

    let mut applied: Vec<CliToolProxyConfigApplyItem> = Vec::new();
    for id in tools {
        let res = match id {
            CliToolId::Claude => apply_claude(&urls),
            CliToolId::Codex => apply_codex(&urls),
            CliToolId::Gemini => apply_gemini(&urls),
        };
        match res {
            Ok(()) => applied.push(CliToolProxyConfigApplyItem {
                id: *id,
                ok: true,
                issue: None,
                error: None,
            }),
            Err(e) => {
                let issue = UserFacingIssue::new("tools_proxy_config_apply_item_failed")
                    .with_arg("tool", id.as_str())
                    .with_detail(e.to_string());
                applied.push(CliToolProxyConfigApplyItem {
                    id: *id,
                    ok: false,
                    issue: Some(issue.to_payload(locale)),
                    error: Some(issue.legacy_error_text(locale)),
                });
            }
        }
    }

    let ok = applied.iter().all(|x| x.ok);
    let status = get_status(listen_addr)?;
    Ok(CliToolProxyConfigApplyResponse {
        ok,
        applied,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_dotenv_updates_and_preserves_comments() {
        let prev = r#"# comment
FOO=1
GOOGLE_GEMINI_BASE_URL=https://example.com

"#;
        let mut updates = HashMap::new();
        updates.insert("GOOGLE_GEMINI_BASE_URL", "http://127.0.0.1:3210");
        updates.insert("GEMINI_API_KEY", "cliswitch");
        let next = patch_dotenv_text(prev, &updates);
        assert!(next.contains("# comment"));
        assert!(next.contains("FOO=1"));
        assert!(next.contains("GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:3210"));
        assert!(next.contains("GEMINI_API_KEY=cliswitch"));
        assert!(next.ends_with('\n'));
    }

    #[test]
    fn normalize_url_trims_trailing_slash() {
        assert_eq!(
            normalize_url(" http://127.0.0.1:3210/ "),
            "http://127.0.0.1:3210"
        );
    }
}
