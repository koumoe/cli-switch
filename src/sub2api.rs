use anyhow::Context as _;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::Value;

use crate::bearer_token::normalize_bearer_token;

#[derive(Debug, Clone)]
pub struct Sub2ApiAccountOverview {
    pub remote_user_id: i64,
    pub remote_username: Option<String>,
    pub remote_display_name: Option<String>,
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
    email: Option<String>,
    username: Option<String>,
    role: Option<String>,
    balance: Option<f64>,
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

fn join_url(base_url: &str, path: &str) -> anyhow::Result<String> {
    let mut base =
        reqwest::Url::parse(base_url).with_context(|| format!("无效 URL：{base_url}"))?;
    base.set_query(None);
    base.set_fragment(None);

    let base_dir = match base.path().trim_end_matches('/') {
        "" => "/".to_string(),
        path => format!("{path}/"),
    };
    base.set_path(&base_dir);

    let joined = base
        .join(path.trim_start_matches('/'))
        .with_context(|| format!("拼接 sub2api URL 失败：base={base_url}, path={path}"))?;
    Ok(joined.to_string().trim_end_matches('/').to_string())
}

fn auth_headers(access_token: &str) -> anyhow::Result<HeaderMap> {
    let token = normalize_bearer_token(access_token);
    if token.is_empty() {
        anyhow::bail!("missing sub2api access token");
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).context("构建 Bearer Token 失败")?,
    );
    Ok(headers)
}

async fn send_json<T>(request: reqwest::RequestBuilder) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let response = request.send().await.context("发送 sub2api 请求失败")?;
    let status = response.status();
    let text = response.text().await.context("读取 sub2api 响应失败")?;
    if !status.is_success() {
        anyhow::bail!("sub2api 响应失败：HTTP {} {}", status.as_u16(), text);
    }
    let parsed: ApiEnvelope<T> =
        serde_json::from_str(&text).with_context(|| format!("解析 sub2api 响应失败：{text}"))?;
    if parsed.code != 0 {
        let message = parsed.message.trim();
        anyhow::bail!(
            "{}",
            if message.is_empty() {
                "sub2api 请求失败"
            } else {
                message
            }
        );
    }
    parsed.data.context("sub2api 响应缺少 data")
}

async fn send_no_data(request: reqwest::RequestBuilder) -> anyhow::Result<()> {
    let response = request.send().await.context("发送 sub2api 请求失败")?;
    let status = response.status();
    let text = response.text().await.context("读取 sub2api 响应失败")?;
    if !status.is_success() {
        anyhow::bail!("sub2api 响应失败：HTTP {} {}", status.as_u16(), text);
    }
    let parsed: ApiEnvelope<Value> =
        serde_json::from_str(&text).with_context(|| format!("解析 sub2api 响应失败：{text}"))?;
    if parsed.code != 0 {
        let message = parsed.message.trim();
        anyhow::bail!(
            "{}",
            if message.is_empty() {
                "sub2api 请求失败"
            } else {
                message
            }
        );
    }
    Ok(())
}

pub async fn fetch_public_settings(
    http_client: &reqwest::Client,
    base_url: &str,
) -> anyhow::Result<Sub2ApiPublicSettings> {
    let url = join_url(base_url, "/api/v1/settings/public")?;
    let data = send_json::<PublicSettingsData>(http_client.get(url)).await?;
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
) -> anyhow::Result<Sub2ApiAccountOverview> {
    let url = join_url(base_url, "/api/v1/auth/me")?;
    let headers = auth_headers(access_token)?;
    let data = send_json::<AuthMeData>(http_client.get(url).headers(headers)).await?;
    let remote_username = data.username.filter(|value| !value.trim().is_empty());
    Ok(Sub2ApiAccountOverview {
        remote_user_id: data.id,
        remote_username: remote_username.clone(),
        remote_display_name: data
            .email
            .filter(|value| !value.trim().is_empty())
            .or(remote_username),
        remote_role: data.role.filter(|value| !value.trim().is_empty()),
        balance: data.balance,
    })
}

pub async fn list_groups(
    http_client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> anyhow::Result<Vec<Sub2ApiGroupOption>> {
    let url = join_url(base_url, "/api/v1/groups/available")?;
    let headers = auth_headers(access_token)?;
    let data = send_json::<Vec<GroupData>>(http_client.get(url).headers(headers)).await?;
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
) -> anyhow::Result<Sub2ApiKey> {
    #[derive(serde::Serialize)]
    struct CreateKeyBody<'a> {
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        group_id: Option<i64>,
    }

    let url = join_url(base_url, "/api/v1/keys")?;
    let headers = auth_headers(access_token)?;
    let data = send_json::<KeyData>(http_client.post(url).headers(headers).json(&CreateKeyBody {
        name: request.name.trim(),
        group_id: request.group_id,
    }))
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
) -> anyhow::Result<Vec<Sub2ApiKey>> {
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

pub async fn delete_key(
    http_client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    key_id: i64,
) -> anyhow::Result<()> {
    let url = join_url(base_url, &format!("/api/v1/keys/{key_id}"))?;
    let headers = auth_headers(access_token)?;
    send_no_data(http_client.delete(url).headers(headers)).await
}

#[cfg(test)]
mod tests {
    use super::join_url;

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
}
