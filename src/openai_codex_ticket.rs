use serde_json::json;
use uuid::Uuid;

use crate::storage::OpenAiAccount;

pub const TICKET_LENGTH: usize = 292;
pub const TICKET_TTL_MS: i64 = 55 * 60 * 1000;
pub const MIN_CODEX_VERSION: &str = "0.153.4";
const TICKET_ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";

pub fn is_supported_codex_version(version: &str) -> bool {
    let minimum = semver::Version::parse(MIN_CODEX_VERSION)
        .expect("MIN_CODEX_VERSION must be a valid semantic version");
    semver::Version::parse(version).is_ok_and(|version| version >= minimum)
}

pub fn is_valid_ticket_state(state: &str) -> bool {
    state.len() == TICKET_LENGTH && state.starts_with("gAAAAA")
}

pub async fn harvest_ticket(
    account: &OpenAiAccount,
    model: &str,
    proxy_url: &str,
    codex_version: &str,
) -> anyhow::Result<String> {
    let access_token = account
        .access_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("OpenAI account has no access token"))?;
    let builder = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(proxy_url.trim())?)
        .timeout(std::time::Duration::from_secs(45))
        .no_gzip()
        .no_brotli()
        .no_deflate();
    let client = builder.build()?;
    let session_id = Uuid::new_v4().to_string();
    let request = client
        .post(TICKET_ENDPOINT)
        .bearer_auth(access_token)
        .header("chatgpt-account-id", &account.remote_user_id)
        .header("openai-beta", "responses=experimental")
        .header("originator", "codex_cli_rs")
        .header("version", codex_version)
        .header("user-agent", format!("codex_cli_rs/{codex_version}"))
        .header("session_id", session_id)
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .json(&json!({
            "model": model,
            "store": false,
            "stream": true,
            "instructions": "Reply with exactly: pong",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "ping"}]}]
        }));
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "ticket harvest returned HTTP {}",
            response.status()
        ));
    }
    let state = response
        .headers()
        .get("x-codex-turn-state")
        .ok_or_else(|| anyhow::anyhow!("ticket response did not include x-codex-turn-state"))?
        .to_str()
        .map_err(|_| anyhow::anyhow!("ticket response header is not valid UTF-8"))?
        .trim()
        .to_string();
    if !is_valid_ticket_state(&state) {
        return Err(anyhow::anyhow!("ticket response header has invalid shape"));
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::is_supported_codex_version;

    #[test]
    fn requires_supported_semver() {
        assert!(is_supported_codex_version("0.153.4"));
        assert!(is_supported_codex_version("0.155.1"));
        assert!(!is_supported_codex_version("0.153.3"));
        assert!(!is_supported_codex_version("0.155"));
    }
}
