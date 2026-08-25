use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bearer_token::normalize_bearer_token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sub2ApiAuthContext {
    AccessToken,
    RefreshToken,
}

#[derive(Debug, thiserror::Error)]
pub enum Sub2ApiRequestError {
    #[error("{message}")]
    Auth {
        context: Sub2ApiAuthContext,
        reason: Option<String>,
        message: String,
    },
    #[error("{message}")]
    Request { message: String },
}

impl Sub2ApiRequestError {
    pub fn is_access_token_invalid(&self) -> bool {
        matches!(
            self,
            Self::Auth {
                context: Sub2ApiAuthContext::AccessToken,
                ..
            }
        )
    }

    pub fn is_refresh_token_invalid(&self) -> bool {
        matches!(
            self,
            Self::Auth {
                context: Sub2ApiAuthContext::RefreshToken,
                ..
            }
        )
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Auth { message, .. } | Self::Request { message } => message,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sub2ApiTokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Clone)]
pub struct Sub2ApiAccountOverview {
    pub remote_user_id: i64,
    pub remote_username: Option<String>,
    pub remote_role: Option<String>,
    pub balance: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Sub2ApiPublicSettings {
    pub api_base_url: Option<String>,
    pub site_name: Option<String>,
    pub backend_mode_enabled: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct Sub2ApiGroupOption {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub platform: Option<String>,
    pub rate_multiplier: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Sub2ApiKey {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub group_id: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct CreateSub2ApiKeyRequest {
    pub name: String,
    pub group_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    code: i64,
    #[serde(default)]
    message: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct PublicSettingsData {
    api_base_url: Option<String>,
    site_name: Option<String>,
    backend_mode_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AuthMeData {
    id: i64,
    username: Option<String>,
    role: Option<String>,
    balance: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SubscriptionProgressItemData {
    progress: SubscriptionProgressData,
}

#[derive(Debug, Deserialize)]
struct SubscriptionProgressData {
    daily: Option<SubscriptionWindowData>,
    weekly: Option<SubscriptionWindowData>,
    monthly: Option<SubscriptionWindowData>,
}

#[derive(Debug, Deserialize)]
struct SubscriptionWindowData {
    remaining_usd: f64,
}

#[derive(Debug, Deserialize)]
struct GroupData {
    id: i64,
    name: String,
    description: Option<String>,
    platform: Option<String>,
    rate_multiplier: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct KeyData {
    id: i64,
    key: String,
    name: String,
    group_id: Option<i64>,
    status: String,
}

#[derive(Debug, Deserialize)]
struct KeyListData {
    items: Vec<KeyData>,
    pages: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RefreshTokenData {
    access_token: String,
    refresh_token: String,
}

fn join_url(base_url: &str, path: &str) -> Result<String, Sub2ApiRequestError> {
    let mut base = reqwest::Url::parse(base_url).map_err(|err| Sub2ApiRequestError::Request {
        message: format!("无效 URL：{base_url} ({err})"),
    })?;
    base.set_query(None);
    base.set_fragment(None);

    let base_dir = match base.path().trim_end_matches('/') {
        "" => "/".to_string(),
        path => format!("{path}/"),
    };
    base.set_path(&base_dir);

    let joined =
        base.join(path.trim_start_matches('/'))
            .map_err(|err| Sub2ApiRequestError::Request {
                message: format!("拼接 sub2api URL 失败：base={base_url}, path={path}, err={err}"),
            })?;
    Ok(joined.to_string().trim_end_matches('/').to_string())
}

fn auth_headers(access_token: &str) -> Result<HeaderMap, Sub2ApiRequestError> {
    let token = normalize_bearer_token(access_token);
    if token.is_empty() {
        return Err(Sub2ApiRequestError::Request {
            message: "missing sub2api access token".to_string(),
        });
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).map_err(|err| {
            Sub2ApiRequestError::Request {
                message: format!("构建 Bearer Token 失败：{err}"),
            }
        })?,
    );
    Ok(headers)
}

fn known_auth_reason(reason: &str, context: Sub2ApiAuthContext) -> bool {
    match context {
        Sub2ApiAuthContext::AccessToken => matches!(
            reason,
            "TOKEN_EXPIRED" | "INVALID_TOKEN" | "TOKEN_REVOKED" | "ACCESS_TOKEN_EXPIRED"
        ),
        Sub2ApiAuthContext::RefreshToken => matches!(
            reason,
            "REFRESH_TOKEN_INVALID"
                | "REFRESH_TOKEN_EXPIRED"
                | "REFRESH_TOKEN_REUSED"
                | "TOKEN_REVOKED"
                | "INVALID_TOKEN"
        ),
    }
}

fn parse_error_reason(text: &str) -> Option<String> {
    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
        for pointer in ["/reason", "/code", "/error/reason", "/error/code"] {
            if let Some(value) = parsed
                .pointer(pointer)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Some(value.to_string());
            }
        }
    }

    [
        "TOKEN_EXPIRED",
        "INVALID_TOKEN",
        "TOKEN_REVOKED",
        "ACCESS_TOKEN_EXPIRED",
        "REFRESH_TOKEN_INVALID",
        "REFRESH_TOKEN_EXPIRED",
        "REFRESH_TOKEN_REUSED",
    ]
    .into_iter()
    .find(|marker| text.contains(marker))
    .map(str::to_string)
}

fn parse_error_message(text: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
        for pointer in ["/message", "/error/message"] {
            if let Some(value) = parsed
                .pointer(pointer)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return value.to_string();
            }
        }
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        "sub2api 请求失败".to_string()
    } else {
        trimmed.to_string()
    }
}

fn map_failed_response(
    status: reqwest::StatusCode,
    text: &str,
    auth_context: Option<Sub2ApiAuthContext>,
) -> Sub2ApiRequestError {
    let reason = parse_error_reason(text);
    let message = parse_error_message(text);
    if let Some(context) = auth_context
        && let Some(reason_value) = reason.clone()
        && known_auth_reason(&reason_value, context)
    {
        return Sub2ApiRequestError::Auth {
            context,
            reason: Some(reason_value),
            message,
        };
    }

    Sub2ApiRequestError::Request {
        message: format!("sub2api 响应失败：HTTP {} {}", status.as_u16(), message),
    }
}

async fn send_json<T>(
    request: reqwest::RequestBuilder,
    auth_context: Option<Sub2ApiAuthContext>,
) -> Result<T, Sub2ApiRequestError>
where
    T: for<'de> Deserialize<'de>,
{
    let response = request
        .send()
        .await
        .map_err(|err| Sub2ApiRequestError::Request {
            message: format!("发送 sub2api 请求失败：{err}"),
        })?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| Sub2ApiRequestError::Request {
            message: format!("读取 sub2api 响应失败：{err}"),
        })?;
    if !status.is_success() {
        return Err(map_failed_response(status, &text, auth_context));
    }
    let parsed: ApiEnvelope<T> =
        serde_json::from_str(&text).map_err(|err| Sub2ApiRequestError::Request {
            message: format!("解析 sub2api 响应失败：{err}; body={text}"),
        })?;
    if parsed.code != 0 {
        let message = parsed.message.trim();
        return Err(Sub2ApiRequestError::Request {
            message: if message.is_empty() {
                "sub2api 请求失败".to_string()
            } else {
                message.to_string()
            },
        });
    }
    parsed.data.ok_or_else(|| Sub2ApiRequestError::Request {
        message: "sub2api 响应缺少 data".to_string(),
    })
}

async fn send_no_data(
    request: reqwest::RequestBuilder,
    auth_context: Option<Sub2ApiAuthContext>,
) -> Result<(), Sub2ApiRequestError> {
    let response = request
        .send()
        .await
        .map_err(|err| Sub2ApiRequestError::Request {
            message: format!("发送 sub2api 请求失败：{err}"),
        })?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| Sub2ApiRequestError::Request {
            message: format!("读取 sub2api 响应失败：{err}"),
        })?;
    if !status.is_success() {
        return Err(map_failed_response(status, &text, auth_context));
    }
    let parsed: ApiEnvelope<Value> =
        serde_json::from_str(&text).map_err(|err| Sub2ApiRequestError::Request {
            message: format!("解析 sub2api 响应失败：{err}; body={text}"),
        })?;
    if parsed.code != 0 {
        let message = parsed.message.trim();
        return Err(Sub2ApiRequestError::Request {
            message: if message.is_empty() {
                "sub2api 请求失败".to_string()
            } else {
                message.to_string()
            },
        });
    }
    Ok(())
}

pub async fn fetch_public_settings(
    http_client: &reqwest::Client,
    base_url: &str,
) -> Result<Sub2ApiPublicSettings, Sub2ApiRequestError> {
    let url = join_url(base_url, "/api/v1/settings/public")?;
    let data = send_json::<PublicSettingsData>(http_client.get(url), None).await?;
    Ok(Sub2ApiPublicSettings {
        api_base_url: data.api_base_url.filter(|value| !value.trim().is_empty()),
        site_name: data.site_name.filter(|value| !value.trim().is_empty()),
        backend_mode_enabled: data.backend_mode_enabled,
    })
}

pub async fn fetch_account_overview(
    http_client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<Sub2ApiAccountOverview, Sub2ApiRequestError> {
    let url = join_url(base_url, "/api/v1/auth/me")?;
    let headers = auth_headers(access_token)?;
    let data = send_json::<AuthMeData>(
        http_client.get(url).headers(headers),
        Some(Sub2ApiAuthContext::AccessToken),
    )
    .await?;
    let subscription_balance =
        fetch_subscription_remaining_balance(http_client, base_url, access_token)
            .await
            .ok()
            .flatten();
    let remote_username = data.username.filter(|value| !value.trim().is_empty());
    Ok(Sub2ApiAccountOverview {
        remote_user_id: data.id,
        remote_username,
        remote_role: data.role.filter(|value| !value.trim().is_empty()),
        balance: subscription_balance.or(data.balance),
    })
}

async fn fetch_subscription_remaining_balance(
    http_client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<Option<f64>, Sub2ApiRequestError> {
    let url = join_url(base_url, "/api/v1/subscriptions/progress")?;
    let headers = auth_headers(access_token)?;
    let items = send_json::<Vec<SubscriptionProgressItemData>>(
        http_client.get(url).headers(headers),
        Some(Sub2ApiAuthContext::AccessToken),
    )
    .await?;
    Ok(total_subscription_remaining_balance(&items))
}

fn total_subscription_remaining_balance(items: &[SubscriptionProgressItemData]) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;

    for item in items {
        if let Some(remaining) = effective_subscription_remaining(&item.progress) {
            total += remaining;
            found = true;
        }
    }

    found.then_some(total)
}

fn effective_subscription_remaining(progress: &SubscriptionProgressData) -> Option<f64> {
    [
        progress.daily.as_ref(),
        progress.weekly.as_ref(),
        progress.monthly.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|window| window.remaining_usd)
    .filter(|value| value.is_finite() && *value >= 0.0)
    .reduce(f64::min)
}

pub async fn list_groups(
    http_client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<Vec<Sub2ApiGroupOption>, Sub2ApiRequestError> {
    let url = join_url(base_url, "/api/v1/groups/available")?;
    let headers = auth_headers(access_token)?;
    let data = send_json::<Vec<GroupData>>(
        http_client.get(url).headers(headers),
        Some(Sub2ApiAuthContext::AccessToken),
    )
    .await?;
    Ok(data
        .into_iter()
        .map(|item| Sub2ApiGroupOption {
            id: item.id,
            name: item.name,
            description: item.description.filter(|value| !value.trim().is_empty()),
            platform: item.platform.filter(|value| !value.trim().is_empty()),
            rate_multiplier: item.rate_multiplier,
        })
        .collect())
}

pub async fn create_key(
    http_client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    request: &CreateSub2ApiKeyRequest,
) -> Result<Sub2ApiKey, Sub2ApiRequestError> {
    #[derive(serde::Serialize)]
    struct CreateKeyBody<'a> {
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        group_id: Option<i64>,
    }

    let url = join_url(base_url, "/api/v1/keys")?;
    let headers = auth_headers(access_token)?;
    let data = send_json::<KeyData>(
        http_client.post(url).headers(headers).json(&CreateKeyBody {
            name: request.name.trim(),
            group_id: request.group_id,
        }),
        Some(Sub2ApiAuthContext::AccessToken),
    )
    .await?;
    Ok(Sub2ApiKey {
        id: data.id,
        key: data.key,
        name: data.name,
        group_id: data.group_id,
        status: data.status,
    })
}

pub async fn list_keys(
    http_client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<Vec<Sub2ApiKey>, Sub2ApiRequestError> {
    const PAGE_SIZE: usize = 100;
    const MAX_PAGES: usize = 100;

    let url = join_url(base_url, "/api/v1/keys")?;
    let headers = auth_headers(access_token)?;
    let page_size = PAGE_SIZE.to_string();
    let mut out = Vec::new();

    for page in 1..=MAX_PAGES {
        let page_s = page.to_string();
        let data = send_json::<KeyListData>(
            http_client
                .get(url.clone())
                .headers(headers.clone())
                .query(&[("page", page_s.as_str()), ("page_size", page_size.as_str())]),
            Some(Sub2ApiAuthContext::AccessToken),
        )
        .await?;
        let reached_last_page = data
            .pages
            .and_then(|value| usize::try_from(value).ok())
            .is_some_and(|pages| page >= pages);
        out.extend(data.items.into_iter().map(|item| Sub2ApiKey {
            id: item.id,
            key: item.key,
            name: item.name,
            group_id: item.group_id,
            status: item.status,
        }));
        if reached_last_page || out.len() < page.saturating_mul(PAGE_SIZE) {
            break;
        }
    }

    Ok(out)
}

pub async fn refresh_access_token(
    http_client: &reqwest::Client,
    base_url: &str,
    refresh_token: &str,
) -> Result<Sub2ApiTokenPair, Sub2ApiRequestError> {
    #[derive(Debug, Serialize)]
    struct RefreshTokenRequest<'a> {
        refresh_token: &'a str,
    }

    let refresh_token = normalize_bearer_token(refresh_token);
    if refresh_token.is_empty() {
        return Err(Sub2ApiRequestError::Request {
            message: "missing sub2api refresh token".to_string(),
        });
    }

    let url = join_url(base_url, "/api/v1/auth/refresh")?;
    let data = send_json::<RefreshTokenData>(
        http_client.post(url).json(&RefreshTokenRequest {
            refresh_token: &refresh_token,
        }),
        Some(Sub2ApiAuthContext::RefreshToken),
    )
    .await?;
    Ok(Sub2ApiTokenPair {
        access_token: normalize_bearer_token(&data.access_token),
        refresh_token: normalize_bearer_token(&data.refresh_token),
    })
}

pub async fn delete_key(
    http_client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    key_id: i64,
) -> Result<(), Sub2ApiRequestError> {
    let url = join_url(base_url, &format!("/api/v1/keys/{key_id}"))?;
    let headers = auth_headers(access_token)?;
    send_no_data(
        http_client.delete(url).headers(headers),
        Some(Sub2ApiAuthContext::AccessToken),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        Sub2ApiAuthContext, Sub2ApiRequestError, SubscriptionProgressData,
        SubscriptionProgressItemData, SubscriptionWindowData, join_url, map_failed_response,
        total_subscription_remaining_balance,
    };

    #[test]
    fn join_url_preserves_base_path_for_api_endpoints() {
        let joined =
            join_url("https://example.com/tenant", "/api/v1/settings/public").expect("join url");
        assert_eq!(joined, "https://example.com/tenant/api/v1/settings/public");
    }

    #[test]
    fn join_url_keeps_root_instances_stable() {
        let joined = join_url("https://example.com", "/api/v1/auth/me").expect("join url");
        assert_eq!(joined, "https://example.com/api/v1/auth/me");
    }

    #[test]
    fn subscription_remaining_balance_uses_active_subscription_quota() {
        let items = vec![SubscriptionProgressItemData {
            progress: SubscriptionProgressData {
                daily: Some(SubscriptionWindowData {
                    remaining_usd: 84.472642,
                }),
                weekly: None,
                monthly: None,
            },
        }];

        let remaining =
            total_subscription_remaining_balance(&items).expect("subscription balance exists");
        assert!((remaining - 84.472642).abs() < 1e-9);
    }

    #[test]
    fn subscription_remaining_balance_uses_strictest_window_per_subscription() {
        let items = vec![
            SubscriptionProgressItemData {
                progress: SubscriptionProgressData {
                    daily: Some(SubscriptionWindowData {
                        remaining_usd: 80.0,
                    }),
                    weekly: Some(SubscriptionWindowData {
                        remaining_usd: 60.0,
                    }),
                    monthly: None,
                },
            },
            SubscriptionProgressItemData {
                progress: SubscriptionProgressData {
                    daily: None,
                    weekly: None,
                    monthly: Some(SubscriptionWindowData {
                        remaining_usd: 12.5,
                    }),
                },
            },
        ];

        let remaining =
            total_subscription_remaining_balance(&items).expect("subscription balance exists");
        assert!((remaining - 72.5).abs() < 1e-9);
    }

    #[test]
    fn map_failed_response_recognizes_access_token_failures() {
        let err = map_failed_response(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"code":"TOKEN_EXPIRED","message":"Token has expired"}"#,
            Some(Sub2ApiAuthContext::AccessToken),
        );
        assert!(matches!(
            err,
            Sub2ApiRequestError::Auth {
                context: Sub2ApiAuthContext::AccessToken,
                ..
            }
        ));
    }

    #[test]
    fn map_failed_response_recognizes_refresh_token_failures() {
        let err = map_failed_response(
            reqwest::StatusCode::UNAUTHORIZED,
            r#"{"code":401,"reason":"REFRESH_TOKEN_EXPIRED","message":"refresh token has expired"}"#,
            Some(Sub2ApiAuthContext::RefreshToken),
        );
        assert!(matches!(
            err,
            Sub2ApiRequestError::Auth {
                context: Sub2ApiAuthContext::RefreshToken,
                ..
            }
        ));
    }
}
