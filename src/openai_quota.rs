use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use serde_json::Value;

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

fn parse_window(value: &Value, now_ms: i64) -> Option<OpenAiQuotaWindow> {
    let object = value.as_object()?;
    let used_percent = object.get("used_percent")?.as_f64()?;
    if !used_percent.is_finite() {
        return None;
    }
    let window_minutes = object
        .get("limit_window_seconds")
        .and_then(Value::as_i64)
        .map(|seconds| seconds / 60)
        .or_else(|| object.get("window_minutes").and_then(Value::as_i64))?;
    if window_minutes <= 0 {
        return None;
    }
    let resets_at_ms = object
        .get("reset_at")
        .and_then(Value::as_i64)
        .map(|seconds| seconds.saturating_mul(1000))
        .or_else(|| {
            object
                .get("reset_after_seconds")
                .and_then(Value::as_i64)
                .map(|seconds| now_ms.saturating_add(seconds.saturating_mul(1000)))
        });
    Some(OpenAiQuotaWindow {
        used_percent,
        window_minutes,
        resets_at_ms,
    })
}

fn snapshot_from_payload(payload: Value, now_ms: i64) -> OpenAiQuotaSnapshot {
    let mut windows = Vec::new();
    let mut push = |value: Option<&Value>| {
        if let Some(value) = value.and_then(|value| parse_window(value, now_ms))
            && !windows.iter().any(|existing: &OpenAiQuotaWindow| {
                existing.window_minutes == value.window_minutes
                    && existing.used_percent.to_bits() == value.used_percent.to_bits()
                    && existing.resets_at_ms == value.resets_at_ms
            })
        {
            windows.push(value);
        }
    };
    if let Some(rate_limit) = payload.get("rate_limit") {
        push(rate_limit.get("primary_window"));
        push(rate_limit.get("secondary_window"));
    }
    if let Some(additional) = payload
        .get("additional_rate_limits")
        .and_then(Value::as_array)
    {
        for item in additional {
            let rate_limit = item.get("rate_limit").unwrap_or(item);
            push(rate_limit.get("primary_window"));
            push(rate_limit.get("secondary_window"));
        }
    }
    if windows.is_empty() {
        fn walk(value: &Value, now_ms: i64, output: &mut Vec<OpenAiQuotaWindow>) {
            if let Some(parsed) = parse_window(value, now_ms)
                && !output.iter().any(|existing| {
                    existing.window_minutes == parsed.window_minutes
                        && existing.used_percent.to_bits() == parsed.used_percent.to_bits()
                        && existing.resets_at_ms == parsed.resets_at_ms
                })
            {
                output.push(parsed);
            }
            match value {
                Value::Object(object) => object
                    .values()
                    .for_each(|child| walk(child, now_ms, output)),
                Value::Array(values) => values.iter().for_each(|child| walk(child, now_ms, output)),
                _ => {}
            }
        }
        walk(&payload, now_ms, &mut windows);
    }
    let primary = windows.first().cloned();
    let secondary = windows.get(1).cloned();
    let additional = windows.into_iter().skip(2).collect();
    OpenAiQuotaSnapshot {
        primary,
        secondary,
        additional,
        synced_at_ms: Some(now_ms),
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
    let payload: Value = response
        .json()
        .await
        .map_err(OpenAiQuotaError::InvalidResponse)?;
    let now = crate::storage::now_ms();
    Ok(snapshot_from_payload(payload, now))
}

pub fn from_headers(headers: &HeaderMap, now_ms: i64) -> Option<OpenAiQuotaSnapshot> {
    fn float(headers: &HeaderMap, key: &str) -> Option<f64> {
        headers.get(key)?.to_str().ok()?.parse().ok()
    }
    fn integer(headers: &HeaderMap, key: &str) -> Option<i64> {
        headers.get(key)?.to_str().ok()?.parse().ok()
    }
    fn read(headers: &HeaderMap, prefix: &str, now_ms: i64) -> Option<OpenAiQuotaWindow> {
        let used_percent = float(headers, &format!("x-codex-{prefix}-used-percent"))?;
        let window_minutes = integer(headers, &format!("x-codex-{prefix}-window-minutes"))?;
        let resets_at_ms = integer(headers, &format!("x-codex-{prefix}-reset-at"))
            .map(|seconds| seconds.saturating_mul(1000))
            .or_else(|| {
                integer(headers, &format!("x-codex-{prefix}-reset-after-seconds"))
                    .map(|seconds| now_ms.saturating_add(seconds.saturating_mul(1000)))
            });
        Some(OpenAiQuotaWindow {
            used_percent,
            window_minutes,
            resets_at_ms,
        })
    }
    let primary = read(headers, "primary", now_ms);
    let secondary = read(headers, "secondary", now_ms);
    (primary.is_some() || secondary.is_some()).then_some(OpenAiQuotaSnapshot {
        primary,
        secondary,
        additional: Vec::new(),
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
                        "secondary_window": {
                            "used_percent": 25,
                            "limit_window_seconds": 604800,
                            "reset_after_seconds": 120
                        }
                    },
                    "additional_rate_limits": [
                        {
                            "limit_name": "monthly",
                            "metered_feature": "codex_monthly",
                            "rate_limit": {
                                "primary_window": {
                                    "used_percent": 9,
                                    "limit_window_seconds": 2592000,
                                    "reset_at": 1800000000
                                }
                            }
                        }
                    ]
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
        assert_eq!(quota.secondary.unwrap().window_minutes, 10_080);
        assert_eq!(quota.additional.len(), 1);
        assert_eq!(quota.additional[0].used_percent, 9.0);
        assert_eq!(quota.additional[0].window_minutes, 43_200);
    }
}
