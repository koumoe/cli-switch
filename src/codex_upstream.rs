use axum::http::{HeaderMap, HeaderValue, header};
use serde_json::{Value, json};

pub const DEFAULT_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
pub const RESPONSES_PATH: &str = "/responses";
const RESPONSES_LITE_HEADER: &str = "x-openai-internal-codex-responses-lite";

pub fn is_responses_lite(headers: &HeaderMap) -> bool {
    headers
        .get(RESPONSES_LITE_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

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

/// Remove invalid IDs from replayed Responses reasoning items.
///
/// Some compatible upstreams emit generic `item_*` IDs for reasoning output.
/// When a client replays that history, OpenAI rejects the item because reasoning
/// IDs must begin with `rs`. Removing only the invalid ID lets the upstream treat
/// the preserved reasoning payload as an input item without changing its
/// encrypted content, summary, other fields, or array position.
pub fn sanitize_responses_reasoning_item_ids(body: &[u8]) -> Option<Vec<u8>> {
    let mut value = serde_json::from_slice::<Value>(body).ok()?;
    let input = value.get_mut("input").and_then(Value::as_array_mut)?;

    let mut changed = false;
    for item in input {
        let Some(item) = item.as_object_mut() else {
            continue;
        };
        if item.get("type").and_then(Value::as_str) != Some("reasoning") {
            continue;
        }

        let has_valid_id = item
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("rs"));
        if item.contains_key("id") && !has_valid_id {
            item.remove("id");
            changed = true;
        }
    }

    if !changed {
        return None;
    }
    serde_json::to_vec(&value).ok()
}

/// Apply only account credentials for a managed OpenAI OAuth request.
/// Client identity and request context headers come from the real client and
/// must pass through unchanged.
pub fn apply_oauth_credentials(
    headers: &mut HeaderMap,
    credentials: CodexCredentials<'_>,
) -> anyhow::Result<()> {
    let bearer = HeaderValue::from_str(&format!("Bearer {}", credentials.access_token))?;
    let account_id = HeaderValue::from_str(credentials.account_id)?;

    headers.insert(header::AUTHORIZATION, bearer);
    headers.insert("chatgpt-account-id", account_id);
    Ok(())
}

/// Apply only the compatibility fields currently required by the Codex ChatGPT
/// backend. A compliant payload is returned byte-for-byte unchanged. This does
/// not translate Chat Completions, Anthropic, or Gemini payloads.
pub fn normalize_responses_body(
    body: &[u8],
    model: Option<&str>,
    responses_lite: bool,
) -> anyhow::Result<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(body)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Codex Responses 请求体必须是 JSON 对象"))?;
    let mut changed = false;

    if let Some(model) = model.map(str::trim).filter(|value| !value.is_empty())
        && !object.contains_key("model")
    {
        object.insert("model".to_string(), Value::String(model.to_string()));
        changed = true;
    }
    if object.get("stream").and_then(Value::as_bool) != Some(true) {
        object.insert("stream".to_string(), Value::Bool(true));
        changed = true;
    }
    if object.get("store").and_then(Value::as_bool) != Some(false) {
        object.insert("store".to_string(), Value::Bool(false));
        changed = true;
    }
    if responses_lite {
        if object.get("parallel_tool_calls").and_then(Value::as_bool) != Some(false) {
            object.insert("parallel_tool_calls".to_string(), Value::Bool(false));
            changed = true;
        }
    } else if !object.contains_key("parallel_tool_calls") {
        object
            .entry("parallel_tool_calls".to_string())
            .or_insert(Value::Bool(true));
        changed = true;
    }
    let includes_encrypted_reasoning =
        object
            .get("include")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.as_str() == Some("reasoning.encrypted_content"))
            });
    if !includes_encrypted_reasoning {
        let include = match object.remove("include") {
            Some(Value::Array(mut items)) => {
                items.push(Value::String("reasoning.encrypted_content".to_string()));
                Value::Array(items)
            }
            _ => json!(["reasoning.encrypted_content"]),
        };
        object.insert("include".to_string(), include);
        changed = true;
    }
    if !object.contains_key("instructions") {
        object.insert("instructions".to_string(), Value::String(String::new()));
        changed = true;
    }

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
        changed |= object.remove(unsupported).is_some();
    }

    if !changed {
        return Ok(body.to_vec());
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
    fn applies_oauth_credentials_without_overwriting_client_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("codex_cli_rs/0.21.0"),
        );
        headers.insert("version", HeaderValue::from_static("0.21.0"));
        headers.insert("originator", HeaderValue::from_static("codex_cli_rs"));
        headers.insert("session-id", HeaderValue::from_static("session-1"));
        headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert("openai-beta", HeaderValue::from_static("client-beta"));
        apply_oauth_credentials(
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
        assert_eq!(headers["openai-beta"], "client-beta");
        assert_eq!(headers[header::ACCEPT], "application/json");
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(headers["version"], "0.21.0");
        assert_eq!(headers[header::USER_AGENT], "codex_cli_rs/0.21.0");
        assert_eq!(headers["session-id"], "session-1");
        assert!(!headers.contains_key("session_id"));
    }

    #[test]
    fn detects_responses_lite_header_case_insensitively() {
        let mut headers = HeaderMap::new();
        headers.insert(RESPONSES_LITE_HEADER, HeaderValue::from_static("TRUE"));
        assert!(is_responses_lite(&headers));

        headers.insert(RESPONSES_LITE_HEADER, HeaderValue::from_static("false"));
        assert!(!is_responses_lite(&headers));

        headers.insert(RESPONSES_LITE_HEADER, HeaderValue::from_static("1"));
        assert!(!is_responses_lite(&headers));
    }

    #[test]
    fn strips_only_invalid_reasoning_item_ids() {
        let body = br#"{
            "input":[
                {"type":"message","id":"item_message","role":"assistant"},
                {"type":"reasoning","id":"item_reasoning","encrypted_content":"opaque","summary":[{"type":"summary_text","text":"kept"}],"content":[{"type":"reasoning_text","text":"kept"}]},
                {"type":"reasoning","id":"rs_valid","summary":[]},
                {"type":"reasoning","id":null,"summary":[]},
                {"type":"reasoning","summary":[]},
                {"type":"function_call","id":"item_call","call_id":"call_1","name":"tool","arguments":"{}"}
            ]
        }"#;

        let sanitized = sanitize_responses_reasoning_item_ids(body).unwrap();
        let value: Value = serde_json::from_slice(&sanitized).unwrap();
        let input = value["input"].as_array().unwrap();

        assert_eq!(input.len(), 6);
        assert_eq!(input[0]["id"], "item_message");
        assert!(input[1].get("id").is_none());
        assert_eq!(input[1]["encrypted_content"], "opaque");
        assert_eq!(input[1]["summary"][0]["text"], "kept");
        assert_eq!(input[1]["content"][0]["text"], "kept");
        assert_eq!(input[2]["id"], "rs_valid");
        assert!(input[3].get("id").is_none());
        assert!(input[4].get("id").is_none());
        assert_eq!(input[5]["id"], "item_call");
        assert_eq!(input[5]["call_id"], "call_1");
    }

    #[test]
    fn keeps_unmatched_responses_bodies_byte_for_byte() {
        for body in [
            br#"{ "input": "hello" }"#.as_slice(),
            br#"{ "input": [{"type":"reasoning","id":"rs_valid","summary":[]}] }"#.as_slice(),
            br#"{ "input": [{"type":"message","id":"item_legacy"}] }"#.as_slice(),
            b"not-json".as_slice(),
        ] {
            assert!(sanitize_responses_reasoning_item_ids(body).is_none());
        }
    }

    #[test]
    fn normalizes_responses_payload_for_codex_backend() {
        let normalized = normalize_responses_body(
            br#"{"model":"old","input":"hello","temperature":0.2,"previous_response_id":"r"}"#,
            Some("gpt-5-codex"),
            false,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&normalized).unwrap();

        assert_eq!(value["model"], "old");
        assert_eq!(value["stream"], true);
        assert_eq!(value["store"], false);
        assert_eq!(value["parallel_tool_calls"], true);
        assert_eq!(value["instructions"], "");
        assert_eq!(value["include"], json!(["reasoning.encrypted_content"]));
        assert!(value.get("temperature").is_none());
        assert!(value.get("previous_response_id").is_none());
    }

    #[test]
    fn keeps_compliant_responses_body_byte_for_byte() {
        let body = br#"{"model":"gpt-5.6-sol","input":"hello","stream":true,"store":false,"parallel_tool_calls":false,"include":["reasoning.encrypted_content"],"instructions":""}"#;
        assert_eq!(
            normalize_responses_body(body, Some("gpt-5.6-sol"), false).unwrap(),
            body
        );
    }

    #[test]
    fn adds_required_include_without_dropping_existing_includes() {
        let normalized =
            normalize_responses_body(br#"{"input":"hello","include":["foo"]}"#, None, false)
                .unwrap();
        let value: Value = serde_json::from_slice(&normalized).unwrap();
        assert_eq!(
            value["include"],
            json!(["foo", "reasoning.encrypted_content"])
        );
    }

    #[test]
    fn preserves_disabled_parallel_tool_calls_for_non_lite_requests() {
        let normalized = normalize_responses_body(
            br#"{"input":"hello","parallel_tool_calls":false}"#,
            None,
            false,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&normalized).unwrap();

        assert_eq!(value["parallel_tool_calls"], false);
    }

    #[test]
    fn pins_parallel_tool_calls_to_false_for_responses_lite() {
        for body in [
            br#"{"input":"hello"}"#.as_slice(),
            br#"{"input":"hello","parallel_tool_calls":true}"#.as_slice(),
            br#"{"input":"hello","parallel_tool_calls":false}"#.as_slice(),
            br#"{"input":"hello","parallel_tool_calls":"false"}"#.as_slice(),
        ] {
            let normalized = normalize_responses_body(body, None, true).unwrap();
            let value: Value = serde_json::from_slice(&normalized).unwrap();
            assert_eq!(value["parallel_tool_calls"], false);
        }
    }
}
