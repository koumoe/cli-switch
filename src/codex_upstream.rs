use axum::http::{HeaderMap, HeaderValue, header};
use semver::Version;
use serde_json::{Value, json};
use uuid::Uuid;

pub const DEFAULT_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
pub const RESPONSES_PATH: &str = "/responses";
pub const DEFAULT_VERSION: &str = "0.150.1";
pub const DEFAULT_ORIGINATOR: &str = "codex-tui";
pub const DEFAULT_USER_AGENT: &str =
    "codex-tui/0.150.1 (Mac OS 26.5.0; arm64) iTerm.app/3.6.10 (codex-tui; 0.150.1)";
pub const MIN_SUPPORTED_VERSION: &str = "0.144.0";
const RESPONSES_LITE_HEADER: &str = "x-openai-internal-codex-responses-lite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexClientIdentity {
    pub version: String,
    pub originator: String,
    pub user_agent: String,
}

impl CodexClientIdentity {
    pub fn for_version(version: Option<&str>) -> Self {
        identity_for_version(version)
    }
}

pub fn identity_for_version(version: Option<&str>) -> CodexClientIdentity {
    let version = normalize_client_version(version).unwrap_or_else(|| DEFAULT_VERSION.to_string());
    let user_agent = if version == DEFAULT_VERSION {
        DEFAULT_USER_AGENT.to_string()
    } else {
        format!(
            "{DEFAULT_ORIGINATOR}/{version} (Mac OS 26.5.0; arm64) iTerm.app/3.6.10 (codex-tui; {version})"
        )
    };
    CodexClientIdentity {
        version: version.clone(),
        originator: DEFAULT_ORIGINATOR.to_string(),
        user_agent,
    }
}

fn normalize_client_version(version: Option<&str>) -> Option<String> {
    let version = version?.trim().trim_start_matches('v');
    if version.is_empty() {
        return None;
    }
    let parsed = Version::parse(version).ok()?;
    let minimum = Version::parse(MIN_SUPPORTED_VERSION).expect("valid Codex minimum version");
    (parsed >= minimum).then(|| version.to_string())
}

pub fn default_identity() -> CodexClientIdentity {
    identity_for_version(Some(DEFAULT_VERSION))
}

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

/// Apply a self-consistent current Codex identity for OAuth accounts.
/// ChatGPT validates that `originator`, the User-Agent client name/version,
/// and the `version` header agree. Stale or mixed identities can be rejected
/// as an outdated Codex client even when the OAuth token is valid.
pub fn apply_headers_with_identity(
    headers: &mut HeaderMap,
    credentials: CodexCredentials<'_>,
    identity: &CodexClientIdentity,
) -> anyhow::Result<()> {
    let bearer = HeaderValue::from_str(&format!("Bearer {}", credentials.access_token))?;
    let account_id = HeaderValue::from_str(credentials.account_id)?;
    let originator = HeaderValue::from_str(&identity.originator)?;
    let version = HeaderValue::from_str(&identity.version)?;
    let user_agent = HeaderValue::from_str(&identity.user_agent)?;

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
    headers.insert("originator", originator);
    headers.insert("chatgpt-account-id", account_id);
    headers.insert("version", version);
    headers.insert(header::USER_AGENT, user_agent);
    headers
        .entry("session_id")
        .or_insert(HeaderValue::from_str(&Uuid::new_v4().to_string())?);
    Ok(())
}

/// Normalize an OpenAI Responses payload to the subset accepted by the Codex
/// ChatGPT backend. This intentionally does not translate Chat Completions,
/// Anthropic, or Gemini payloads; those require protocol-aware translators.
pub fn normalize_responses_body(
    body: &[u8],
    model: Option<&str>,
    responses_lite: bool,
) -> anyhow::Result<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(body)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("Codex Responses 请求体必须是 JSON 对象"))?;

    if let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) {
        object.insert("model".to_string(), Value::String(model.to_string()));
    }
    object.insert("stream".to_string(), Value::Bool(true));
    object.insert("store".to_string(), Value::Bool(false));
    if responses_lite {
        object.insert("parallel_tool_calls".to_string(), Value::Bool(false));
    } else {
        object
            .entry("parallel_tool_calls".to_string())
            .or_insert(Value::Bool(true));
    }
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
    fn applies_current_self_consistent_oauth_identity() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_static("codex_cli_rs/0.21.0"),
        );
        headers.insert("version", HeaderValue::from_static("0.21.0"));
        headers.insert("originator", HeaderValue::from_static("codex_cli_rs"));
        apply_headers_with_identity(
            &mut headers,
            CodexCredentials {
                access_token: "access-token",
                account_id: "account-id",
            },
            &default_identity(),
        )
        .unwrap();

        assert_eq!(headers[header::AUTHORIZATION], "Bearer access-token");
        assert_eq!(headers["chatgpt-account-id"], "account-id");
        assert_eq!(headers["originator"], DEFAULT_ORIGINATOR);
        assert_eq!(headers["openai-beta"], "responses=experimental");
        assert_eq!(headers[header::ACCEPT], "text/event-stream");
        assert_eq!(headers["version"], DEFAULT_VERSION);
        assert_eq!(headers[header::USER_AGENT], DEFAULT_USER_AGENT);
        assert!(headers.contains_key("session_id"));
    }

    #[test]
    fn normalizes_detected_versions_to_the_supported_codex_floor() {
        let detected = identity_for_version(Some("v0.149.1"));
        assert_eq!(detected.version, "0.149.1");
        assert_eq!(detected.originator, "codex-tui");
        assert!(detected.user_agent.starts_with("codex-tui/0.149.1 "));
        assert_eq!(
            identity_for_version(Some("0.140.0")).version,
            DEFAULT_VERSION
        );
        assert_eq!(
            identity_for_version(Some("not-a-version")).version,
            DEFAULT_VERSION
        );
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

        assert_eq!(value["model"], "gpt-5-codex");
        assert_eq!(value["stream"], true);
        assert_eq!(value["store"], false);
        assert_eq!(value["parallel_tool_calls"], true);
        assert_eq!(value["instructions"], "");
        assert_eq!(value["include"], json!(["reasoning.encrypted_content"]));
        assert!(value.get("temperature").is_none());
        assert!(value.get("previous_response_id").is_none());
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
