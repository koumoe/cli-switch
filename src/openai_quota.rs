use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde::Deserialize;

use crate::storage::{OpenAiAccount, OpenAiQuotaSnapshot, OpenAiQuotaWindow};

pub(crate) const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Debug, thiserror::Error)]
pub(crate) enum OpenAiQuotaError {
    #[error("missing OpenAI access token")]
    MissingAccessToken,
    #[error("OpenAI usage request failed: {0}")]
    Request(#[source] reqwest::Error),
    #[error("OpenAI usage endpoint returned HTTP {0}")]
    Http(StatusCode),
    #[error("invalid OpenAI usage response: {0}")]
    InvalidResponse(#[source] reqwest::Error),
}

impl OpenAiQuotaError {
    pub(crate) fn is_auth_failure(&self) -> bool {
        matches!(
            self,
            Self::MissingAccessToken | Self::Http(StatusCode::UNAUTHORIZED)
        )
    }
}

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

pub(crate) async fn fetch(
    client: &reqwest::Client,
    account: &OpenAiAccount,
    usage_url: Option<&str>,
) -> Result<OpenAiQuotaSnapshot, OpenAiQuotaError> {
    let usage_url = usage_url.unwrap_or(USAGE_URL);
    let access_token = account
        .access_token
        .as_deref()
        .ok_or(OpenAiQuotaError::MissingAccessToken)?;
    let response = client
        .get(usage_url)
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
        .await
        .map_err(OpenAiQuotaError::Request)?;
    let status = response.status();
    if !status.is_success() {
        return Err(OpenAiQuotaError::Http(status));
    }
    let payload: UsageEnvelope = response
        .json()
        .await
        .map_err(OpenAiQuotaError::InvalidResponse)?;
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
    use axum::{Json, Router, routing::get};
    use serde_json::json;
    use uuid::Uuid;

    fn account(access_token: Option<&str>) -> OpenAiAccount {
        OpenAiAccount {
            id: Uuid::new_v4().to_string(),
            name: "OpenAI".to_string(),
            base_url: "https://chatgpt.com".to_string(),
            access_token: access_token.map(str::to_string),
            refresh_token: None,
            id_token: None,
            access_token_configured: access_token.is_some(),
            refresh_token_configured: false,
            remote_user_id: "account-1".to_string(),
            remote_username: None,
            remote_display_name: None,
            plan_type: None,
            token_expires_at_ms: None,
            last_refresh_at_ms: None,
            quota: OpenAiQuotaSnapshot::default(),
            last_sync_error: None,
            reauth_required: false,
            last_synced_at_ms: None,
            sort_order: 0,
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }
    #[test]
    fn parses_codex_rate_limit_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-codex-primary-used-percent", "34".parse().unwrap());
        headers.insert("x-codex-primary-window-minutes", "10080".parse().unwrap());
        headers.insert("x-codex-primary-reset-after-seconds", "60".parse().unwrap());
        let quota = from_headers(&headers, 1_000).unwrap();
        assert_eq!(quota.primary.unwrap().resets_at_ms, Some(61_000));
    }

    #[tokio::test]
    async fn reports_usage_auth_failures() {
        let app = Router::new().route(
            "/usage",
            get(|| async { (StatusCode::UNAUTHORIZED, "expired") }),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let error = fetch(
            &reqwest::Client::new(),
            &account(Some("expired")),
            Some(&format!("http://{address}/usage")),
        )
        .await
        .unwrap_err();
        assert!(error.is_auth_failure());
    }

    #[tokio::test]
    async fn parses_usage_payload() {
        let app = Router::new().route(
            "/usage",
            get(|| async {
                Json(json!({
                    "rate_limit": {
                        "primary_window": {
                            "used_percent": 12.5,
                            "limit_window_seconds": 18_000,
                            "reset_after_seconds": 60
                        },
                        "secondary_window": null
                    }
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let quota = fetch(
            &reqwest::Client::new(),
            &account(Some("valid")),
            Some(&format!("http://{address}/usage")),
        )
        .await
        .unwrap();
        let primary = quota.primary.unwrap();
        assert_eq!(primary.used_percent, 12.5);
        assert_eq!(primary.window_minutes, 300);
    }
}
