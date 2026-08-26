use axum::http::{HeaderMap, HeaderValue, header};
use serde_json::{Value, json};
use uuid::Uuid;

pub const DEFAULT_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
pub const RESPONSES_PATH: &str = "/responses";
pub const DEFAULT_VERSION: &str = "0.21.0";
pub const DEFAULT_USER_AGENT: &str = "codex_cli_rs/0.50.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCredentials<'a> {
    pub access_token: &'a str,
    pub account_id: &'a str,
}

pub fn responses_url(base_url: Option<&str>) -> String {
    let base_url = base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_BASE_URL)
        .trim_end_matches('/');
    format!("{base_url}{RESPONSES_PATH}")
}

/// Apply the headers used by CLIProxyAPI's Codex executor for OAuth accounts.
/// A fresh session id is generated for every upstream request unless the caller
/// already supplied one.
pub fn apply_headers(
    headers: &mut HeaderMap,
    credentials: CodexCredentials<'_>,
) -> anyhow::Result<()> {
    let bearer = HeaderValue::from_str(&format!("Bearer {}", credentials.access_token))?;
    let account_id = HeaderValue::from_str(credentials.account_id)?;

    headers.insert(header::AUTHORIZATION, bearer);
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(
        "openai-beta",
        HeaderValue::from_static("responses=experimental"),
    );
    headers.insert("originator", HeaderValue::from_static("codex_cli_rs"));
    headers.insert("chatgpt-account-id", account_id);
    headers
        .entry("version")
        .or_insert(HeaderValue::from_static(DEFAULT_VERSION));
    headers
        .entry(header::USER_AGENT)
        .or_insert(HeaderValue::from_static(DEFAULT_USER_AGENT));
    headers
        .entry("session_id")
        .or_insert(HeaderValue::from_str(&Uuid::new_v4().to_string())?);
    Ok(())
}

/// Normalize an OpenAI Responses payload to the subset accepted by the Codex
/// ChatGPT backend. This intentionally does not translate Chat Completions,
/// Anthropic, or Gemini payloads; those require protocol-aware translators.
pub fn normalize_responses_body(body: &[u8], model: Option<&str>) -> anyhow::Result<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(body)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Codex Responses 请求体必须是 JSON 对象"))?;

    if let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) {
        object.insert("model".to_string(), Value::String(model.to_string()));
    }
    object.insert("stream".to_string(), Value::Bool(true));
    object.insert("store".to_string(), Value::Bool(false));
    object.insert("parallel_tool_calls".to_string(), Value::Bool(true));
    object.insert(
        "include".to_string(),
        json!(["reasoning.encrypted_content"]),
    );
    object
        .entry("instructions".to_string())
        .or_insert_with(|| Value::String(String::new()));

    for unsupported in [
        "max_output_tokens",
        "max_completion_tokens",
        "temperature",
        "top_p",
        "service_tier",
        "previous_response_id",
        "prompt_cache_retention",
        "safety_identifier",
    ] {
        object.remove(unsupported);
    }

    Ok(serde_json::to_vec(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_default_and_custom_responses_urls() {
        assert_eq!(
            responses_url(None),
            "https://chatgpt.com/backend-api/codex/responses"
        );
        assert_eq!(
            responses_url(Some("https://example.test/codex/")),
            "https://example.test/codex/responses"
        );
    }

    #[test]
    fn applies_oauth_account_headers_without_overwriting_client_identity() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("codex-cli-test"),
        );
        apply_headers(
            &mut headers,
            CodexCredentials {
                access_token: "access-token",
                account_id: "account-id",
            },
        )
        .unwrap();

        assert_eq!(headers[header::AUTHORIZATION], "Bearer access-token");
        assert_eq!(headers["chatgpt-account-id"], "account-id");
        assert_eq!(headers["originator"], "codex_cli_rs");
        assert_eq!(headers["openai-beta"], "responses=experimental");
        assert_eq!(headers[header::ACCEPT], "text/event-stream");
        assert_eq!(headers[header::USER_AGENT], "codex-cli-test");
        assert!(headers.contains_key("session_id"));
    }

    #[test]
    fn normalizes_responses_payload_for_codex_backend() {
        let normalized = normalize_responses_body(
            br#"{"model":"old","input":"hello","temperature":0.2,"previous_response_id":"r"}"#,
            Some("gpt-5-codex"),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&normalized).unwrap();

        assert_eq!(value["model"], "gpt-5-codex");
        assert_eq!(value["stream"], true);
        assert_eq!(value["store"], false);
        assert_eq!(value["parallel_tool_calls"], true);
        assert_eq!(value["instructions"], "");
        assert_eq!(value["include"], json!(["reasoning.encrypted_content"]));
        assert!(value.get("temperature").is_none());
        assert!(value.get("previous_response_id").is_none());
    }
}
