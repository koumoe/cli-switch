use reqwest::header::HeaderMap;
use serde::Deserialize;

use crate::storage::{OpenAiAccount, OpenAiQuotaSnapshot, OpenAiQuotaWindow};

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Debug, Deserialize)]
struct UsageEnvelope {
    rate_limit: Option<RateLimit>,
}

#[derive(Debug, Deserialize)]
struct RateLimit {
    primary_window: Option<UsageWindow>,
    secondary_window: Option<UsageWindow>,
}

#[derive(Debug, Deserialize)]
struct UsageWindow {
    used_percent: f64,
    limit_window_seconds: i64,
    reset_at: Option<i64>,
    reset_after_seconds: Option<i64>,
}

fn window(value: UsageWindow, now_ms: i64) -> OpenAiQuotaWindow {
    OpenAiQuotaWindow {
        used_percent: value.used_percent,
        window_minutes: value.limit_window_seconds / 60,
        resets_at_ms: value
            .reset_at
            .map(|value| value.saturating_mul(1000))
            .or_else(|| {
                value
                    .reset_after_seconds
                    .map(|value| now_ms.saturating_add(value.saturating_mul(1000)))
            }),
    }
}

pub async fn fetch(
    client: &reqwest::Client,
    account: &OpenAiAccount,
) -> anyhow::Result<OpenAiQuotaSnapshot> {
    let access_token = account
        .access_token
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing OpenAI access token"))?;
    let response = client
        .get(USAGE_URL)
        .bearer_auth(access_token)
        .header("chatgpt-account-id", &account.remote_user_id)
        .header("openai-beta", "codex-1")
        .header("oai-language", "zh-CN")
        .header("originator", "Codex Desktop")
        .header("accept", "application/json")
        .header("sec-fetch-site", "none")
        .header("sec-fetch-mode", "no-cors")
        .header("sec-fetch-dest", "empty")
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("OpenAI usage endpoint returned HTTP {}", status.as_u16());
    }
    let payload: UsageEnvelope = response.json().await?;
    let now = crate::storage::now_ms();
    Ok(OpenAiQuotaSnapshot {
        primary: payload
            .rate_limit
            .as_ref()
            .and_then(|rate| rate.primary_window.as_ref())
            .map(|value| {
                window(
                    UsageWindow {
                        used_percent: value.used_percent,
                        limit_window_seconds: value.limit_window_seconds,
                        reset_at: value.reset_at,
                        reset_after_seconds: value.reset_after_seconds,
                    },
                    now,
                )
            }),
        secondary: payload
            .rate_limit
            .and_then(|rate| rate.secondary_window)
            .map(|value| window(value, now)),
        synced_at_ms: Some(now),
    })
}

pub fn from_headers(headers: &HeaderMap, now_ms: i64) -> Option<OpenAiQuotaSnapshot> {
    fn float(headers: &HeaderMap, key: &str) -> Option<f64> {
        headers.get(key)?.to_str().ok()?.parse().ok()
    }
    fn integer(headers: &HeaderMap, key: &str) -> Option<i64> {
        headers.get(key)?.to_str().ok()?.parse().ok()
    }
    fn read(headers: &HeaderMap, prefix: &str, now_ms: i64) -> Option<OpenAiQuotaWindow> {
        Some(OpenAiQuotaWindow {
            used_percent: float(headers, &format!("x-codex-{prefix}-used-percent"))?,
            window_minutes: integer(headers, &format!("x-codex-{prefix}-window-minutes"))?,
            resets_at_ms: integer(headers, &format!("x-codex-{prefix}-reset-after-seconds"))
                .map(|seconds| now_ms.saturating_add(seconds.saturating_mul(1000))),
        })
    }
    let primary = read(headers, "primary", now_ms);
    let secondary = read(headers, "secondary", now_ms);
    (primary.is_some() || secondary.is_some()).then_some(OpenAiQuotaSnapshot {
        primary,
        secondary,
        synced_at_ms: Some(now_ms),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_codex_rate_limit_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-codex-primary-used-percent", "34".parse().unwrap());
        headers.insert("x-codex-primary-window-minutes", "10080".parse().unwrap());
        headers.insert("x-codex-primary-reset-after-seconds", "60".parse().unwrap());
        let quota = from_headers(&headers, 1_000).unwrap();
        assert_eq!(quota.primary.unwrap().resets_at_ms, Some(61_000));
    }
}
