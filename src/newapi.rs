use anyhow::Context as _;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::Value;

use crate::storage::{NewApiAccount, NewApiAccountCheckinMode, NewApiAccountRemoteSnapshot};

const NEW_API_USER_HEADER: &str = "New-Api-User";

#[derive(Debug, Clone)]
pub struct NewApiAccountOverview {
    pub remote_role: Option<i64>,
    pub remote_username: Option<String>,
    pub remote_display_name: Option<String>,
    pub remote_group: Option<String>,
    pub quota_display_type: String,
    pub quota_per_unit: f64,
    pub usd_exchange_rate: f64,
    pub custom_currency_symbol: Option<String>,
    pub custom_currency_exchange_rate: f64,
    pub remote_checkin_enabled: bool,
    pub remote_turnstile_check_enabled: bool,
    pub last_quota: Option<i64>,
    pub last_used_quota: Option<i64>,
    pub last_balance_amount: Option<f64>,
    pub checked_in_today: bool,
}

pub fn account_has_user_api_credentials(account: &NewApiAccount) -> bool {
    !account.user_id.trim().is_empty()
        && account
            .user_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
}

pub fn build_remote_snapshot(overview: &NewApiAccountOverview) -> NewApiAccountRemoteSnapshot {
    NewApiAccountRemoteSnapshot {
        replace_remote_state: true,
        remote_role: overview.remote_role,
        remote_username: overview.remote_username.clone(),
        remote_display_name: overview.remote_display_name.clone(),
        remote_group: overview.remote_group.clone(),
        quota_display_type: Some(overview.quota_display_type.clone()),
        quota_per_unit: Some(overview.quota_per_unit),
        usd_exchange_rate: Some(overview.usd_exchange_rate),
        custom_currency_symbol: overview.custom_currency_symbol.clone(),
        custom_currency_exchange_rate: Some(overview.custom_currency_exchange_rate),
        remote_checkin_enabled: Some(overview.remote_checkin_enabled),
        remote_turnstile_check_enabled: Some(overview.remote_turnstile_check_enabled),
        last_quota: overview.last_quota,
        last_used_quota: overview.last_used_quota,
        last_balance_amount: overview.last_balance_amount,
        last_sync_error: None,
        last_synced_at_ms: Some(crate::storage::now_ms()),
    }
}

#[derive(Debug, Clone)]
pub struct NewApiGroupOption {
    pub name: String,
    pub ratio: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewApiSystemCheckinResult {
    pub quota_awarded: Option<i64>,
    pub checkin_date: Option<String>,
    pub already_checked_in: bool,
}

#[derive(Debug, Clone)]
pub struct CreateManagedChannelRequest {
    pub name: String,
    pub group_name: String,
}

#[derive(Debug, Clone)]
pub struct CreateManagedChannelResult {
    pub token_id: i64,
    pub token_name: String,
    pub token_key: String,
    pub group_name: String,
    pub group_ratio: f64,
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct NewApiEnvelope<T> {
    success: bool,
    #[serde(default)]
    message: String,
    #[serde(default)]
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct StatusData {
    quota_per_unit: Option<f64>,
    quota_display_type: Option<String>,
    usd_exchange_rate: Option<f64>,
    custom_currency_symbol: Option<String>,
    custom_currency_exchange_rate: Option<f64>,
    checkin_enabled: Option<bool>,
    turnstile_check: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SelfData {
    role: Option<i64>,
    username: Option<String>,
    display_name: Option<String>,
    group: Option<String>,
    quota: Option<i64>,
    used_quota: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CheckinStats {
    checked_in_today: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CheckinStatusData {
    stats: Option<CheckinStats>,
}

#[derive(Debug, Deserialize)]
struct TokenListData<T> {
    items: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenSearchItem {
    pub id: i64,
    pub name: String,
    pub key: Option<String>,
    pub created_time: Option<i64>,
    pub group: Option<String>,
}

#[derive(Debug, Clone)]
struct CreatedToken {
    id: i64,
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenKeyData {
    key: String,
}

#[derive(Debug, Deserialize)]
struct CheckinPostData {
    quota_awarded: Option<i64>,
    checkin_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GroupValueData {
    ratio: Value,
    desc: Option<String>,
}

fn auth_headers(account: &NewApiAccount) -> anyhow::Result<HeaderMap> {
    let token = account
        .user_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("newapi 账号缺少 user token")?;
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(token)?);
    headers.insert(
        NEW_API_USER_HEADER,
        HeaderValue::from_str(account.user_id.trim())?,
    );
    Ok(headers)
}

fn join_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

async fn send_json<T: for<'de> Deserialize<'de>>(
    req: reqwest::RequestBuilder,
) -> anyhow::Result<T> {
    let res = req.send().await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        anyhow::bail!("newapi 请求失败：HTTP {} {}", status.as_u16(), text.trim());
    }
    let parsed: NewApiEnvelope<T> = serde_json::from_str(&text)
        .with_context(|| format!("解析 newapi 响应失败：{}", text.trim()))?;
    if !parsed.success {
        let raw_message = parsed.message.trim();
        let message = raw_message
            .strip_prefix("Error: ")
            .unwrap_or(raw_message)
            .trim();
        anyhow::bail!(
            "{}",
            if message.is_empty() {
                "newapi 请求失败"
            } else {
                message
            }
        );
    }
    parsed.data.context("newapi 响应缺少 data 字段")
}

async fn send_no_data(req: reqwest::RequestBuilder) -> anyhow::Result<()> {
    let res = req.send().await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        anyhow::bail!("newapi 请求失败：HTTP {} {}", status.as_u16(), text.trim());
    }
    let parsed: NewApiEnvelope<Value> = serde_json::from_str(&text)
        .with_context(|| format!("解析 newapi 响应失败：{}", text.trim()))?;
    if !parsed.success {
        let message = parsed.message.trim();
        anyhow::bail!(
            "{}",
            if message.is_empty() {
                "newapi 请求失败"
            } else {
                message
            }
        );
    }
    Ok(())
}

pub async fn fetch_account_overview(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
) -> anyhow::Result<NewApiAccountOverview> {
    let headers = auth_headers(account)?;
    let status_url = join_url(&account.base_url, "/api/status");
    let user_url = join_url(&account.base_url, "/api/user/self");

    let status_fut = send_json::<StatusData>(http_client.get(status_url));
    let user_fut = send_json::<SelfData>(http_client.get(user_url).headers(headers.clone()));
    let (status, user) = tokio::try_join!(status_fut, user_fut)?;
    let quota_display_type = status
        .quota_display_type
        .unwrap_or_else(|| "USD".to_string())
        .trim()
        .to_string();
    let quota_per_unit = status.quota_per_unit.unwrap_or(500_000.0).max(1.0);
    let usd_exchange_rate = status.usd_exchange_rate.unwrap_or(1.0).max(0.0);
    let custom_currency_exchange_rate =
        status.custom_currency_exchange_rate.unwrap_or(1.0).max(0.0);
    let last_quota = user.quota;
    let last_used_quota = user.used_quota;
    let last_balance_amount = last_quota.map(|quota| {
        convert_quota_to_amount(
            quota,
            &quota_display_type,
            quota_per_unit,
            usd_exchange_rate,
            custom_currency_exchange_rate,
        )
    });
    let remote_checkin_enabled = status.checkin_enabled.unwrap_or(false);
    let should_probe_checkin = remote_checkin_enabled
        && (account.auto_checkin_enabled
            || matches!(account.checkin_mode, NewApiAccountCheckinMode::PageOpen));
    let checked_in_today = if should_probe_checkin {
        let month = {
            let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
            let now = time::OffsetDateTime::now_utc().to_offset(offset);
            let fmt = time::format_description::parse("[year]-[month]")?;
            now.format(&fmt)?
        };
        let checkin_url = join_url(
            &account.base_url,
            &format!("/api/user/checkin?month={month}"),
        );
        let checkin =
            send_json::<CheckinStatusData>(http_client.get(checkin_url).headers(headers)).await?;
        checkin
            .stats
            .and_then(|stats| stats.checked_in_today)
            .unwrap_or(false)
    } else {
        false
    };

    Ok(NewApiAccountOverview {
        remote_role: user.role,
        remote_username: user.username.filter(|value| !value.trim().is_empty()),
        remote_display_name: user.display_name.filter(|value| !value.trim().is_empty()),
        remote_group: user.group.filter(|value| !value.trim().is_empty()),
        quota_display_type,
        quota_per_unit,
        usd_exchange_rate,
        custom_currency_symbol: status
            .custom_currency_symbol
            .filter(|value| !value.trim().is_empty()),
        custom_currency_exchange_rate,
        remote_checkin_enabled,
        remote_turnstile_check_enabled: status.turnstile_check.unwrap_or(false),
        last_quota,
        last_used_quota,
        last_balance_amount,
        checked_in_today,
    })
}

pub fn convert_quota_to_amount(
    quota: i64,
    quota_display_type: &str,
    quota_per_unit: f64,
    usd_exchange_rate: f64,
    custom_currency_exchange_rate: f64,
) -> f64 {
    let quota_per_unit = quota_per_unit.max(1.0);
    if quota_display_type == "TOKENS" {
        return quota as f64;
    }
    let result_usd = (quota as f64) / quota_per_unit;
    match quota_display_type {
        "CNY" => result_usd * usd_exchange_rate,
        "CUSTOM" => result_usd * custom_currency_exchange_rate,
        _ => result_usd,
    }
}

pub fn format_balance_amount(
    amount: f64,
    quota_display_type: &str,
    custom_symbol: Option<&str>,
) -> String {
    let symbol = match quota_display_type {
        "CNY" => "¥",
        "CUSTOM" => custom_symbol.unwrap_or("¤"),
        "TOKENS" => "",
        _ => "$",
    };
    if quota_display_type == "TOKENS" {
        return format!("{amount:.0}");
    }
    format!("{symbol}{amount:.2}")
}

pub async fn list_groups(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
) -> anyhow::Result<Vec<NewApiGroupOption>> {
    let headers = auth_headers(account)?;
    let url = join_url(&account.base_url, "/api/user/self/groups");
    let raw =
        send_json::<serde_json::Map<String, Value>>(http_client.get(url).headers(headers)).await?;
    let mut groups = Vec::new();
    for (name, value) in raw {
        let parsed: GroupValueData = serde_json::from_value(Value::Object(match value {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        }))?;
        let ratio = match parsed.ratio {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.parse::<f64>().ok(),
            _ => None,
        };
        groups.push(NewApiGroupOption {
            name,
            ratio,
            description: parsed.desc.filter(|value| !value.trim().is_empty()),
        });
    }
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(groups)
}

pub async fn list_tokens(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
) -> anyhow::Result<Vec<TokenSearchItem>> {
    const PAGE_SIZE: usize = 100;
    const MAX_PAGES: usize = 100;

    let headers = auth_headers(account)?;
    let url = join_url(&account.base_url, "/api/token/search");
    let mut all = Vec::new();
    let page_size = PAGE_SIZE.to_string();

    for page in 1..=MAX_PAGES {
        let page_s = page.to_string();
        let data = send_json::<TokenListData<TokenSearchItem>>(
            http_client
                .get(url.clone())
                .headers(headers.clone())
                .query(&[
                    ("keyword", ""),
                    ("p", page_s.as_str()),
                    ("page_size", page_size.as_str()),
                    ("size", page_size.as_str()),
                ]),
        )
        .await?;

        let fetched = data.items.len();
        all.extend(data.items);
        if fetched < PAGE_SIZE {
            break;
        }
    }

    Ok(all)
}

pub async fn perform_system_checkin_with_overview(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
    overview: Option<&NewApiAccountOverview>,
) -> anyhow::Result<NewApiSystemCheckinResult> {
    let fetched_overview = if overview.is_none() {
        Some(fetch_account_overview(http_client, account).await?)
    } else {
        None
    };
    let overview = overview
        .or(fetched_overview.as_ref())
        .context("missing overview for system checkin")?;
    if !overview.remote_checkin_enabled {
        anyhow::bail!("目标 New API 未启用系统签到");
    }
    if overview.remote_turnstile_check_enabled {
        anyhow::bail!("目标 New API 已启用 Turnstile，系统签到不可用，请改用页面打开签到");
    }
    if overview.checked_in_today {
        return Ok(NewApiSystemCheckinResult {
            quota_awarded: None,
            checkin_date: None,
            already_checked_in: true,
        });
    }

    let headers = auth_headers(account)?;
    let url = join_url(&account.base_url, "/api/user/checkin");
    let result = send_json::<CheckinPostData>(http_client.post(url).headers(headers)).await?;
    Ok(NewApiSystemCheckinResult {
        quota_awarded: result.quota_awarded,
        checkin_date: result.checkin_date,
        already_checked_in: false,
    })
}

pub async fn create_managed_channel(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
    request: &CreateManagedChannelRequest,
) -> anyhow::Result<CreateManagedChannelResult> {
    let groups = list_groups(http_client, account).await?;
    let group = groups
        .iter()
        .find(|item| item.name == request.group_name)
        .with_context(|| format!("远端分组不存在：{}", request.group_name))?;
    let group_ratio = group.ratio.with_context(|| {
        format!(
            "分组 {} 没有可用倍率，当前流程不支持自动组",
            request.group_name
        )
    })?;

    let created_token = create_token(
        http_client,
        account,
        &request.name,
        Some(&request.group_name),
    )
    .await?;
    let token_key = match usable_token_key(created_token.key.as_deref()) {
        Some(key) => key.to_owned(),
        None => get_token_key(http_client, account, created_token.id).await?,
    };

    Ok(CreateManagedChannelResult {
        token_id: created_token.id,
        token_name: request.name.clone(),
        token_key,
        group_name: request.group_name.clone(),
        group_ratio,
    })
}

async fn create_token(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
    token_name: &str,
    group_name: Option<&str>,
) -> anyhow::Result<CreatedToken> {
    let request_started_at = time::OffsetDateTime::now_utc()
        .unix_timestamp()
        .saturating_sub(1);
    let headers = auth_headers(account)?;
    let url = join_url(&account.base_url, "/api/token");
    let requested_group = group_name.unwrap_or("").trim().to_string();
    let payload = serde_json::json!({
        "name": token_name,
        "expired_time": -1,
        "unlimited_quota": true,
        "remain_quota": 0,
        "group": requested_group.clone(),
        "cross_group_retry": false,
        "model_limits_enabled": false,
        "model_limits": "",
        "allow_ips": "",
    });
    send_no_data(
        http_client
            .post(url)
            .headers(headers.clone())
            .json(&payload),
    )
    .await?;

    let search_url = join_url(&account.base_url, "/api/token/search");
    let data = send_json::<TokenListData<TokenSearchItem>>(
        http_client.get(search_url).headers(headers).query(&[
            ("keyword", token_name),
            ("p", "1"),
            ("page_size", "100"),
            ("size", "100"),
        ]),
    )
    .await?;

    let exact_matches = data
        .items
        .into_iter()
        .filter(|item| {
            item.name == token_name
                && item.group.as_deref().map(str::trim).unwrap_or("") == requested_group
        })
        .collect::<Vec<_>>();

    exact_matches
        .iter()
        .filter(|item| item.created_time.unwrap_or(i64::MIN) >= request_started_at)
        .max_by_key(|item| (item.created_time.unwrap_or(i64::MIN), item.id))
        .or_else(|| {
            exact_matches
                .iter()
                .max_by_key(|item| (item.created_time.unwrap_or(i64::MIN), item.id))
        })
        .map(|item| CreatedToken {
            id: item.id,
            key: item.key.clone(),
        })
        .with_context(|| format!("创建 token 后未找到：{token_name}"))
}

fn usable_token_key(key: Option<&str>) -> Option<&str> {
    let key = key?.trim();
    if key.is_empty() || key.contains('*') {
        return None;
    }
    Some(key)
}

async fn get_token_key(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
    token_id: i64,
) -> anyhow::Result<String> {
    let headers = auth_headers(account)?;
    let url = join_url(&account.base_url, &format!("/api/token/{token_id}/key"));
    let data = send_json::<TokenKeyData>(http_client.post(url).headers(headers)).await?;
    Ok(data.key)
}

pub async fn delete_channel(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
    channel_id: i64,
) -> anyhow::Result<()> {
    let headers = auth_headers(account)?;
    let url = join_url(&account.base_url, &format!("/api/channel/{channel_id}"));
    send_no_data(http_client.delete(url).headers(headers)).await
}

pub async fn delete_token(
    http_client: &reqwest::Client,
    account: &NewApiAccount,
    token_id: i64,
) -> anyhow::Result<()> {
    let headers = auth_headers(account)?;
    let url = join_url(&account.base_url, &format!("/api/token/{token_id}"));
    send_no_data(http_client.delete(url).headers(headers)).await
}

#[cfg(test)]
mod tests {
    use super::usable_token_key;

    #[test]
    fn accepts_full_token_key() {
        assert_eq!(
            usable_token_key(Some("sk-test-key-1234567890")),
            Some("sk-test-key-1234567890")
        );
    }

    #[test]
    fn rejects_empty_or_blank_token_key() {
        assert_eq!(usable_token_key(None), None);
        assert_eq!(usable_token_key(Some("")), None);
        assert_eq!(usable_token_key(Some("   ")), None);
    }

    #[test]
    fn rejects_masked_token_key() {
        assert_eq!(usable_token_key(Some("QKJv**********ie8s")), None);
        assert_eq!(usable_token_key(Some("  a4Q1**********jOUN  ")), None);
    }
}
