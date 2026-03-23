use serde_json::Value;

use crate::chat_bridge::MAX_DISPLAY_JSON_DEPTH;
use crate::chat_bridge::i18n::t;
use crate::i18n::AppLocale;

pub(super) fn build_safe_mode_notice(
    stdout: &str,
    stderr: &str,
    locale: AppLocale,
) -> Option<String> {
    let mut lines = Vec::new();
    collect_safe_mode_notice_lines(stdout, &mut lines);
    collect_safe_mode_notice_lines(stderr, &mut lines);
    if lines.is_empty() {
        return None;
    }

    let mut message = t(locale, "notice.safe_mode_header");
    for line in lines.iter().take(4) {
        message.push_str("\n- ");
        message.push_str(line);
    }
    if lines.len() > 4 {
        message.push_str("\n- ");
        message.push_str(&t(locale, "notice.safe_mode_more"));
    }
    message.push('\n');
    message.push_str(&t(locale, "notice.safe_mode_footer"));
    Some(message)
}

fn collect_safe_mode_notice_lines(raw: &str, out: &mut Vec<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        collect_safe_mode_notice_strings_from_value(&value, out, 0);
        return;
    }

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            collect_safe_mode_notice_strings_from_value(&value, out, 0);
            continue;
        }
        push_safe_mode_notice_line(out, trimmed);
    }
}

fn collect_safe_mode_notice_strings_from_value(value: &Value, out: &mut Vec<String>, depth: usize) {
    if depth >= MAX_DISPLAY_JSON_DEPTH {
        return;
    }

    match value {
        Value::String(text) => push_safe_mode_notice_line(out, text),
        Value::Array(items) => {
            for item in items {
                collect_safe_mode_notice_strings_from_value(item, out, depth + 1);
            }
        }
        Value::Object(map) => {
            if map
                .get("role")
                .and_then(Value::as_str)
                .is_some_and(|role| matches!(role.trim(), "assistant" | "user" | "system" | "tool"))
            {
                return;
            }

            for key in [
                "error", "detail", "message", "stderr", "reason", "text", "content", "output",
                "result",
            ] {
                if let Some(next) = map.get(key) {
                    collect_safe_mode_notice_strings_from_value(next, out, depth + 1);
                }
            }

            for next in map.values() {
                if next.is_array() || next.is_object() {
                    collect_safe_mode_notice_strings_from_value(next, out, depth + 1);
                }
            }
        }
        _ => {}
    }
}

fn push_safe_mode_notice_line(out: &mut Vec<String>, raw: &str) {
    let normalized = normalize_safe_mode_notice_line(raw);
    let Some(line) = normalized else {
        return;
    };
    if out
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&line))
    {
        return;
    }
    out.push(line);
}

fn normalize_safe_mode_notice_line(raw: &str) -> Option<String> {
    let squashed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = squashed.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if !["permission denied", "skipped", "sandbox blocked"]
        .iter()
        .any(|keyword| lower.contains(keyword))
    {
        return None;
    }

    let max_chars = 280usize;
    if trimmed.chars().count() <= max_chars {
        return Some(trimmed.to_string());
    }

    let cut_at = trimmed
        .char_indices()
        .nth(max_chars.saturating_sub(1))
        .map(|(index, _)| index)
        .unwrap_or(trimmed.len());
    Some(format!("{}...", &trimmed[..cut_at]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::AppLocale;

    #[test]
    fn build_safe_mode_notice_collects_keyword_lines() {
        let stdout =
            r#"{"error":{"message":"Sandbox blocked while deleting src/config/prod.env"}}"#;
        let stderr = "Permission denied when removing secrets.env";
        let notice = build_safe_mode_notice(stdout, stderr, AppLocale::ZhCN).expect("notice");

        assert!(notice.contains("Sandbox blocked while deleting src/config/prod.env"));
        assert!(notice.contains("Permission denied when removing secrets.env"));
        assert!(notice.contains("-yolo"));
    }

    #[test]
    fn build_safe_mode_notice_matches_snapshot_zh_cn() {
        let stdout =
            r#"{"error":{"message":"Sandbox blocked while deleting src/config/prod.env"}}"#;
        let stderr = "Permission denied when removing secrets.env";
        let notice = build_safe_mode_notice(stdout, stderr, AppLocale::ZhCN).expect("notice");

        assert_eq!(
            notice,
            "检测到安全模式拦截了部分操作：\n- Sandbox blocked while deleting src/config/prod.env\n- Permission denied when removing secrets.env\n如需允许写入等危险操作，请重新启动该会话并使用 -yolo。"
        );
    }

    #[test]
    fn build_safe_mode_notice_matches_snapshot_en_us() {
        let stdout =
            r#"{"error":{"message":"Sandbox blocked while deleting src/config/prod.env"}}"#;
        let stderr = "Permission denied when removing secrets.env";
        let notice = build_safe_mode_notice(stdout, stderr, AppLocale::EnUS).expect("notice");

        assert_eq!(
            notice,
            "Safe mode blocked part of the operation:\n- Sandbox blocked while deleting src/config/prod.env\n- Permission denied when removing secrets.env\nIf you need write access or other risky actions, restart this session with -yolo."
        );
    }
}
