use super::parse::extract_display_text;
use crate::chat_bridge::projects::{AggregatedProject, display_project_index};

pub(in crate::chat_bridge) fn format_projects_list(items: &[AggregatedProject]) -> String {
    if items.is_empty() {
        return "没有发现可用项目。先在本机运行对应 CLI，或在设置里开启“允许新项目”后直接使用绝对路径。".to_string();
    }

    let mut lines = vec!["📂 项目列表:".to_string()];
    for (index, item) in items.iter().enumerate() {
        lines.push(format!(
            "[{}] {} ({})",
            display_project_index(index),
            item.display_name,
            display_path(&item.path),
        ));
    }
    lines.join("\n")
}

fn display_path(path: &str) -> String {
    let trimmed = path.trim();
    if let Some(home) = directories::UserDirs::new().map(|item| item.home_dir().to_path_buf()) {
        let home_str = home.to_string_lossy().to_string();
        if trimmed == home_str {
            return "~".to_string();
        }
        let prefix = format!("{home_str}/");
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            return format!("~/{rest}");
        }
    }
    trimmed.to_string()
}

pub(super) fn build_live_output(display_chunks: &[String]) -> Option<String> {
    if display_chunks.is_empty() {
        return None;
    }
    Some(display_chunks.join("\n"))
}

pub(super) fn format_streaming_message(label: &str, body: &str, max_chars: usize) -> String {
    let normalized_body = body.trim();
    if normalized_body.is_empty() {
        return label.to_string();
    }

    let max_body_chars = max_chars
        .saturating_sub(label.chars().count())
        .saturating_sub(1)
        .max(256);
    let capped_body = cap_for_streaming(normalized_body, max_body_chars);
    format!("{label}\n{capped_body}")
}

pub(super) fn compose_final_output(
    success: bool,
    timed_out: bool,
    file_output: Option<&str>,
    stdout: &str,
    stderr: &str,
) -> String {
    let mut content = file_output
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            let extracted = extract_display_text(stdout, true);
            if extracted.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                extracted
            }
        });

    if !success {
        let stderr_text = {
            let extracted = extract_display_text(stderr, true);
            if extracted.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                extracted
            }
        };
        if !stderr_text.trim().is_empty() {
            if !content.trim().is_empty() {
                content.push_str("\n\n");
            }
            content.push_str("stderr:\n");
            content.push_str(stderr_text.trim());
        }
    }

    if timed_out {
        let timeout_text = "任务执行超时，进程已被强制终止。";
        if content.trim().is_empty() {
            return timeout_text.to_string();
        }
        content = format!("{timeout_text}\n\n{content}");
    }

    if !success && content.trim().is_empty() {
        return "任务执行失败，但没有捕获到可展示的输出。".to_string();
    }
    if success && content.trim().is_empty() {
        return "任务已完成，但 CLI 没有返回可展示的文本输出。".to_string();
    }
    content
}

pub(super) fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return vec!["(空输出)".to_string()];
    }
    if normalized.chars().count() <= max_chars {
        return vec![normalized.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    for ch in normalized.chars() {
        if current_len >= max_chars {
            chunks.push(current);
            current = String::new();
            current_len = 0;
        }
        current.push(ch);
        current_len += 1;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

pub(super) fn cap_for_streaming(text: &str, max_chars: usize) -> String {
    let total_chars = text.chars().count();
    if total_chars <= max_chars {
        return text.to_string();
    }

    let keep_chars = max_chars.saturating_sub(4);
    let skip_chars = total_chars.saturating_sub(keep_chars);
    let tail_start = text
        .char_indices()
        .nth(skip_chars)
        .map(|(index, _)| index)
        .unwrap_or(0);
    format!("...\n{}", &text[tail_start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_live_output_keeps_body_only() {
        assert_eq!(
            build_live_output(&["OK".to_string()]).as_deref(),
            Some("OK")
        );
    }

    #[test]
    fn format_streaming_message_keeps_session_label() {
        assert_eq!(
            format_streaming_message(
                "[alpha(#1)|claude|demo]",
                "OK",
                crate::chat_bridge::MESSAGE_CHAR_LIMIT,
            ),
            "[alpha(#1)|claude|demo]\nOK"
        );
    }
}
