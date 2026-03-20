use regex::{Captures, Regex};
use std::sync::OnceLock;

pub(in crate::chat_bridge) fn redact_sensitive_text(text: &str) -> String {
    let mut redacted = text.to_string();

    redacted = replace_with_groups(&redacted, bearer_regex(), 2, Some(bearer_prefix));
    redacted = replace_with_groups(&redacted, key_value_regex(), 3, Some(key_value_prefix));

    for regex in token_regexes() {
        redacted = replace_with_groups(&redacted, regex, 0, None::<fn(&Captures<'_>) -> String>);
    }

    redacted
}

fn replace_with_groups<F>(
    text: &str,
    regex: &Regex,
    token_group: usize,
    prefix: Option<F>,
) -> String
where
    F: Fn(&Captures<'_>) -> String,
{
    regex
        .replace_all(text, |caps: &Captures<'_>| {
            let token = caps
                .get(token_group)
                .map(|m| m.as_str())
                .unwrap_or_default()
                .to_string();
            let prefix = prefix.as_ref().map(|f| f(caps)).unwrap_or_default();
            format!("{prefix}{}", mask_secret(&token))
        })
        .into_owned()
}

fn bearer_prefix(caps: &Captures<'_>) -> String {
    let scheme = caps.get(1).map(|m| m.as_str()).unwrap_or("Bearer");
    format!("{scheme} ")
}

fn key_value_prefix(caps: &Captures<'_>) -> String {
    let key = caps.get(1).map(|m| m.as_str()).unwrap_or("secret");
    let sep = caps.get(2).map(|m| m.as_str()).unwrap_or("=");
    format!("{key}{sep}")
}

fn mask_secret(secret: &str) -> String {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return "[REDACTED]".to_string();
    }

    let char_count = trimmed.chars().count();
    if char_count <= 8 {
        return "[REDACTED]".to_string();
    }

    let head_len = if char_count >= 24 { 6 } else { 4 };
    let tail_len = 4usize.min(char_count.saturating_sub(head_len));
    let head: String = trimmed.chars().take(head_len).collect();
    let tail: String = trimmed
        .chars()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}...[REDACTED]...{tail}")
}

fn bearer_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(Bearer)\s+([A-Za-z0-9._~+/=-]{8,})").expect("compile bearer regex")
    })
}

fn key_value_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(api[_-]?key|access[_-]?token|refresh[_-]?token|token|secret|password)\b(\s*[:=]\s*)([^\s"'`]{8,})"#,
        )
        .expect("compile key-value regex")
    })
}

fn token_regexes() -> &'static [Regex] {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        vec![
            Regex::new(r"\bsk-[A-Za-z0-9]{12,}\b").expect("compile openai token regex"),
            Regex::new(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}\b")
                .expect("compile github token regex"),
            Regex::new(r"\bgithub_pat_[A-Za-z0-9_]{20,}\b").expect("compile github pat regex"),
            Regex::new(r"\bglpat-[A-Za-z0-9\-_]{20,}\b").expect("compile gitlab token regex"),
            Regex::new(r"\bAIza[0-9A-Za-z\-_]{20,}\b").expect("compile google api key regex"),
            Regex::new(r"\b\d{6,12}:[A-Za-z0-9_-]{20,}\b")
                .expect("compile telegram bot token regex"),
            Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{20,}\b").expect("compile slack token regex"),
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::redact_sensitive_text;

    #[test]
    fn redacts_common_tokens() {
        let input = "Bearer sk-1234567890abcdefghijklmn ghp_abcdefghijklmnopqrstuvwxyz123456";
        let output = redact_sensitive_text(input);
        assert!(output.contains("Bearer sk-123...[REDACTED]"));
        assert!(!output.contains("ghp_abcdefghijklmnopqrstuvwxyz123456"));
    }

    #[test]
    fn redacts_key_value_pairs() {
        let input = "api_key = supersecretvalue123456\npassword: hunter2butlong";
        let output = redact_sensitive_text(input);
        assert!(output.contains("api_key = supe...[REDACTED]"));
        assert!(output.contains("password: hunt...[REDACTED]"));
    }
}
