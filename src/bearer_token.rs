pub(crate) fn normalize_bearer_token(raw: &str) -> String {
    raw.trim()
        .strip_prefix("Bearer ")
        .or_else(|| raw.trim().strip_prefix("bearer "))
        .unwrap_or(raw.trim())
        .trim()
        .to_string()
}

pub(crate) fn normalize_optional_bearer_token(raw: Option<String>) -> Option<String> {
    raw.map(|value| normalize_bearer_token(&value))
        .filter(|value| !value.is_empty())
}
