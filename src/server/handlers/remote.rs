use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::events::{self, AppEvent, RemoteManagedChannelCreated};
use super::newapi as newapi_handlers;
use crate::newapi as newapi_client;
use crate::server::AppState;
use crate::server::error::{ApiError, map_storage_unit_no_content_err};
use crate::storage::{self, RechargeCurrency};
use crate::sub2api as sub2api_client;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(in crate::server) enum RemoteAccountProvider {
    Newapi,
    Sub2Api,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(in crate::server) enum RemoteAccountCheckinMode {
    Disabled,
    SystemApi,
    PageOpen,
}

#[derive(Debug, Clone, Serialize)]
pub(in crate::server) struct RemoteAccountCommonResponse {
    pub id: String,
    pub base_url: String,
    pub api_url: Option<String>,
    pub user_id: String,
    pub user_token_configured: bool,
    pub page_checkin_url: Option<String>,
    pub checkin_mode: RemoteAccountCheckinMode,
    pub auto_checkin_enabled: bool,
    pub auto_checkin_time: String,
    pub low_balance_alert_threshold: f64,
    pub recharge_currency: RechargeCurrency,
    pub remote_username: Option<String>,
    pub remote_display_name: Option<String>,
    pub last_balance_amount: Option<f64>,
    pub last_sync_error: Option<String>,
    pub last_synced_at_ms: Option<i64>,
    pub low_balance_alert_notified: bool,
    pub last_balance_alert_at_ms: Option<i64>,
    pub sort_order: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub(in crate::server) struct NewapiRemoteAccountResponse {
    pub remote_role: Option<i64>,
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
}

#[derive(Debug, Clone, Serialize)]
pub(in crate::server) struct Sub2ApiRemoteAccountResponse {
    pub remote_role_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub(in crate::server) enum RemoteAccountResponse {
    Newapi {
        #[serde(flatten)]
        common: RemoteAccountCommonResponse,
        #[serde(flatten)]
        newapi: NewapiRemoteAccountResponse,
    },
    Sub2Api {
        #[serde(flatten)]
        common: RemoteAccountCommonResponse,
        #[serde(flatten)]
        sub2api: Sub2ApiRemoteAccountResponse,
    },
}

impl RemoteAccountResponse {
    fn sort_order(&self) -> i64 {
        match self {
            Self::Newapi { common, .. } | Self::Sub2Api { common, .. } => common.sort_order,
        }
    }

    fn created_at_ms(&self) -> i64 {
        match self {
            Self::Newapi { common, .. } | Self::Sub2Api { common, .. } => common.created_at_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(in crate::server) struct RemoteGroupResponse {
    pub id: Option<i64>,
    pub name: String,
    pub ratio: Option<f64>,
    pub description: Option<String>,
    pub platform: Option<String>,
    pub managed_channel_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(in crate::server) struct RemoteAccountDetectionResponse {
    pub provider: RemoteAccountProvider,
    pub normalized_base_url: String,
    pub recommended_api_url: Option<String>,
    pub suggested_page_checkin_url: Option<String>,
    pub supported_checkin_modes: Vec<RemoteAccountCheckinMode>,
}

#[derive(Debug, Clone, Serialize)]
pub(in crate::server) struct RemoteKeyResponse {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub group_id: Option<i64>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct DetectRemoteAccountInput {
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct ReorderRemoteAccountsInput {
    account_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct CreateRemoteAccountInput {
    provider: RemoteAccountProvider,
    base_url: String,
    api_url: Option<String>,
    user_id: Option<String>,
    user_token: Option<String>,
    bearer_token: Option<String>,
    page_checkin_url: Option<String>,
    checkin_mode: Option<RemoteAccountCheckinMode>,
    auto_checkin_time: Option<String>,
    low_balance_alert_threshold: Option<f64>,
    recharge_currency: Option<RechargeCurrency>,
}

#[derive(Debug, Deserialize, Default)]
pub(in crate::server) struct UpdateRemoteAccountInput {
    provider: Option<RemoteAccountProvider>,
    base_url: Option<String>,
    api_url: Option<String>,
    user_id: Option<String>,
    user_token: Option<String>,
    bearer_token: Option<String>,
    page_checkin_url: Option<String>,
    checkin_mode: Option<RemoteAccountCheckinMode>,
    auto_checkin_time: Option<String>,
    low_balance_alert_threshold: Option<f64>,
    recharge_currency: Option<RechargeCurrency>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct CreateRemoteKeyInput {
    name: String,
    group_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct CreateRemoteManagedChannelInput {
    name: String,
    protocol: Option<storage::Protocol>,
    group_name: String,
    group_id: Option<i64>,
    base_url_override: Option<String>,
    priority: Option<i64>,
    enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(in crate::server) struct CreateRemoteManagedChannelResponse {
    pub channel: storage::Channel,
}

#[derive(Debug, Deserialize, Default)]
pub(in crate::server) struct DeleteRemoteAccountInput {
    pub delete_managed_channels: Option<bool>,
    pub sync_remote_delete: Option<bool>,
}

fn validate_managed_channel_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request(
            "remote_managed_name_required",
            "name is required",
        ));
    }
    Ok(name.to_string())
}

fn managed_channel_base_url_for_newapi(
    account: &storage::NewApiAccount,
    base_url_override: Option<String>,
) -> String {
    base_url_override
        .or_else(|| account.api_url.clone())
        .unwrap_or_else(|| account.base_url.clone())
}

fn managed_channel_base_url_for_sub2api(
    account: &storage::RemoteAccount,
    base_url_override: Option<String>,
) -> String {
    base_url_override
        .or_else(|| account.api_url.clone())
        .unwrap_or_else(|| format!("{}/v1", account.base_url.trim_end_matches('/')))
}

pub(super) async fn delete_remote_managed_channel_resources(
    state: &AppState,
    channel: &storage::Channel,
) -> anyhow::Result<()> {
    let Some(provider) = channel.managed_provider() else {
        return Ok(());
    };
    let account_id = channel.managed_account_id().ok_or_else(|| {
        anyhow::anyhow!("managed channel {} missing linked remote account id", channel.id)
    })?;
    match provider {
        storage::ManagedRemoteProvider::Newapi => {
            let account =
                storage::get_newapi_account_with_secret(state.db_path(), account_id.to_string())
                    .await?;
            if let Some(remote_channel_id) = channel.newapi_channel_id {
                newapi_client::delete_channel(&state.http_client, &account, remote_channel_id)
                    .await?;
            }
            if let Some(remote_token_id) = channel
                .managed_resource_id()
                .as_deref()
                .and_then(|value| value.parse::<i64>().ok())
                .or(channel.newapi_token_id)
            {
                newapi_client::delete_token(&state.http_client, &account, remote_token_id).await?;
            }
        }
        storage::ManagedRemoteProvider::Sub2Api => {
            let account =
                storage::get_remote_account_with_secret(state.db_path(), account_id.to_string())
                    .await?;
            let token = account
                .access_token
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("sub2api account missing access token"))?;
            let key_id = channel
                .managed_resource_id()
                .as_deref()
                .and_then(|value| value.parse::<i64>().ok())
                .ok_or_else(|| anyhow::anyhow!("managed channel {} missing remote key id", channel.id))?;
            sub2api_client::delete_key(&state.http_client, &account.base_url, token, key_id).await?;
        }
    }
    Ok(())
}

fn detect_provider_from_base_url(raw: &str) -> Result<String, ApiError> {
    let value = newapi_handlers::validate_http_url(raw, "remote_base_url_required")?;
    let parsed = reqwest::Url::parse(&value).map_err(|e| {
        ApiError::bad_request("remote_base_url_invalid", format!("Invalid base_url: {e}"))
    })?;
    let host = parsed.host_str().ok_or_else(|| {
        ApiError::bad_request("remote_base_url_invalid", "base_url must include a host")
    })?;
    let mut out = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        out.push(':');
        out.push_str(&port.to_string());
    }
    Ok(out)
}

fn detect_recommended_api_url(
    provider: RemoteAccountProvider,
    base_url: &str,
    detected_api_url: Option<String>,
) -> Option<String> {
    match provider {
        RemoteAccountProvider::Newapi => detected_api_url,
        RemoteAccountProvider::Sub2Api => Some(format!("{}/v1", base_url.trim_end_matches('/'))),
    }
}

fn detect_suggested_page_checkin_url(
    provider: RemoteAccountProvider,
    base_url: &str,
) -> Option<String> {
    match provider {
        RemoteAccountProvider::Newapi => {
            Some(format!("{}/user/checkin", base_url.trim_end_matches('/')))
        }
        RemoteAccountProvider::Sub2Api => {
            Some(format!("{}/dashboard", base_url.trim_end_matches('/')))
        }
    }
}

#[derive(Debug)]
enum ResolvedRemoteAccount {
    Newapi(storage::NewApiAccount),
    Sub2Api(storage::RemoteAccount),
}

async fn resolve_remote_account_with_secret(
    state: &AppState,
    account_id: &str,
) -> Result<ResolvedRemoteAccount, ApiError> {
    let (newapi_account, remote_account) = tokio::try_join!(
        storage::get_newapi_account_with_secret_optional(state.db_path(), account_id.to_string()),
        storage::get_remote_account_with_secret_optional(state.db_path(), account_id.to_string()),
    )?;
    match (newapi_account, remote_account) {
        (Some(_), Some(_)) => Err(ApiError::Internal(anyhow::anyhow!(
            "account id {account_id} exists in both newapi_accounts and remote_accounts"
        ))),
        (Some(account), None) => Ok(ResolvedRemoteAccount::Newapi(account)),
        (None, Some(account)) => Ok(ResolvedRemoteAccount::Sub2Api(account)),
        (None, None) => Err(ApiError::not_found(
            "remote_account_not_found",
            "Remote account not found",
        )),
    }
}

fn resolve_newapi_checkin_mode(account: &storage::NewApiAccount) -> RemoteAccountCheckinMode {
    if matches!(
        account.checkin_mode,
        storage::NewApiAccountCheckinMode::PageOpen
    ) {
        RemoteAccountCheckinMode::PageOpen
    } else if account.auto_checkin_enabled {
        RemoteAccountCheckinMode::SystemApi
    } else {
        RemoteAccountCheckinMode::Disabled
    }
}

fn map_newapi_common(account: &storage::NewApiAccount) -> RemoteAccountCommonResponse {
    RemoteAccountCommonResponse {
        id: account.id.clone(),
        base_url: account.base_url.clone(),
        api_url: account.api_url.clone(),
        user_id: account.user_id.clone(),
        user_token_configured: account.user_token_configured,
        page_checkin_url: account.page_checkin_url.clone(),
        checkin_mode: resolve_newapi_checkin_mode(account),
        auto_checkin_enabled: account.auto_checkin_enabled,
        auto_checkin_time: account.auto_checkin_time.clone(),
        low_balance_alert_threshold: account.low_balance_alert_threshold,
        recharge_currency: account.recharge_currency,
        remote_username: account.remote_username.clone(),
        remote_display_name: account.remote_display_name.clone(),
        last_balance_amount: account.last_balance_amount,
        last_sync_error: account.last_sync_error.clone(),
        last_synced_at_ms: account.last_synced_at_ms,
        low_balance_alert_notified: account.low_balance_alert_notified,
        last_balance_alert_at_ms: account.last_balance_alert_at_ms,
        sort_order: account.sort_order,
        created_at_ms: account.created_at_ms,
        updated_at_ms: account.updated_at_ms,
    }
}

fn map_newapi_account(account: storage::NewApiAccount) -> RemoteAccountResponse {
    let common = map_newapi_common(&account);
    RemoteAccountResponse::Newapi {
        common,
        newapi: NewapiRemoteAccountResponse {
            remote_role: account.remote_role,
            remote_group: account.remote_group,
            quota_display_type: account.quota_display_type,
            quota_per_unit: account.quota_per_unit,
            usd_exchange_rate: account.usd_exchange_rate,
            custom_currency_symbol: account.custom_currency_symbol,
            custom_currency_exchange_rate: account.custom_currency_exchange_rate,
            remote_checkin_enabled: account.remote_checkin_enabled,
            remote_turnstile_check_enabled: account.remote_turnstile_check_enabled,
            last_quota: account.last_quota,
            last_used_quota: account.last_used_quota,
        },
    }
}

fn map_remote_account(account: storage::RemoteAccount) -> RemoteAccountResponse {
    let common = RemoteAccountCommonResponse {
        id: account.id,
        base_url: account.base_url,
        api_url: account.api_url,
        user_id: account.remote_user_id.unwrap_or_default(),
        user_token_configured: account.access_token_configured,
        page_checkin_url: account.page_checkin_url,
        checkin_mode: match account.checkin_mode {
            storage::RemoteAccountCheckinMode::Disabled => RemoteAccountCheckinMode::Disabled,
            storage::RemoteAccountCheckinMode::PageOpen => RemoteAccountCheckinMode::PageOpen,
        },
        auto_checkin_enabled: false,
        auto_checkin_time: account.auto_checkin_time,
        low_balance_alert_threshold: account.low_balance_alert_threshold,
        recharge_currency: account.recharge_currency,
        remote_username: account.remote_username,
        remote_display_name: account.remote_display_name,
        last_balance_amount: account.last_balance_amount,
        last_sync_error: account.last_sync_error,
        last_synced_at_ms: account.last_synced_at_ms,
        low_balance_alert_notified: account.low_balance_alert_notified,
        last_balance_alert_at_ms: account.last_balance_alert_at_ms,
        sort_order: account.sort_order,
        created_at_ms: account.created_at_ms,
        updated_at_ms: account.updated_at_ms,
    };
    RemoteAccountResponse::Sub2Api {
        common,
        sub2api: Sub2ApiRemoteAccountResponse {
            remote_role_text: account.remote_role,
        },
    }
}

fn validate_threshold(threshold: f64) -> Result<(), ApiError> {
    if threshold.is_finite() && threshold >= 0.0 {
        return Ok(());
    }
    Err(ApiError::bad_request(
        "remote_low_balance_threshold_invalid",
        "low_balance_alert_threshold must be a finite number >= 0",
    ))
}

fn validate_checkin_time(value: &str) -> Result<(), ApiError> {
    let time_format = time::format_description::parse("[hour]:[minute]:[second]").map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("parse checkin time format failed: {e}"))
    })?;
    time::Time::parse(value.trim(), &time_format).map_err(|e| {
        ApiError::bad_request(
            "remote_auto_checkin_time_invalid",
            format!("Invalid auto_checkin_time: {e}"),
        )
    })?;
    Ok(())
}

fn validate_sub2api_candidate(
    base_url: &str,
    page_checkin_url: Option<&str>,
    checkin_mode: RemoteAccountCheckinMode,
    auto_checkin_time: &str,
    low_balance_alert_threshold: f64,
    has_token: bool,
) -> Result<(), ApiError> {
    let _ = newapi_handlers::validate_http_url(base_url, "remote_base_url_required")?;
    validate_threshold(low_balance_alert_threshold)?;
    validate_checkin_time(auto_checkin_time)?;
    match checkin_mode {
        RemoteAccountCheckinMode::Disabled => {}
        RemoteAccountCheckinMode::PageOpen => {
            newapi_handlers::validate_optional_http_url(
                page_checkin_url,
                "remote_page_checkin_url_invalid",
            )?
            .ok_or_else(|| {
                ApiError::bad_request(
                    "remote_page_checkin_url_required",
                    "page_checkin_url is required when checkin_mode is page_open",
                )
            })?;
        }
        RemoteAccountCheckinMode::SystemApi => {
            return Err(ApiError::bad_request(
                "remote_checkin_mode_invalid",
                "sub2api does not support system_api check-in",
            ));
        }
    }
    if !has_token {
        return Err(ApiError::bad_request(
            "remote_credentials_required",
            "bearer_token is required for sub2api accounts",
        ));
    }
    Ok(())
}

async fn fetch_and_persist_sub2api_snapshot(
    state: &AppState,
    account_id: String,
) -> Result<storage::RemoteAccount, ApiError> {
    let account =
        storage::get_remote_account_with_secret(state.db_path(), account_id.clone()).await?;
    let token = account
        .access_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ApiError::bad_request("remote_credentials_required", "bearer_token is required")
        })?;
    let overview =
        sub2api_client::fetch_account_overview(&state.http_client, &account.base_url, token)
            .await
            .map_err(|err| {
                ApiError::bad_gateway(
                    "remote_sync_failed",
                    format!("Failed to sync sub2api account: {err}"),
                )
            })?;
    storage::apply_remote_account_sync_success(
        state.db_path(),
        account_id.clone(),
        storage::RemoteAccountRemoteSnapshot {
            remote_user_id: Some(overview.remote_user_id.to_string()),
            remote_role: overview.remote_role.clone(),
            remote_username: overview.remote_username.clone(),
            remote_display_name: overview.remote_display_name.clone(),
            last_balance_amount: overview.balance,
            last_synced_at_ms: Some(storage::now_ms()),
        },
    )
    .await?;
    storage::get_remote_account_without_secret(state.db_path(), account_id)
        .await
        .map_err(ApiError::Internal)
}

async fn record_sub2api_sync_failure(
    state: &AppState,
    account_id: String,
    err: &anyhow::Error,
) -> Result<(), ApiError> {
    storage::apply_remote_account_sync_failure(
        state.db_path(),
        account_id,
        err.to_string(),
        Some(storage::now_ms()),
    )
    .await?;
    Ok(())
}

pub(in crate::server) async fn detect_remote_account(
    State(state): State<AppState>,
    Json(input): Json<DetectRemoteAccountInput>,
) -> Result<impl IntoResponse, ApiError> {
    let base_url = detect_provider_from_base_url(&input.base_url)?;
    let (newapi_detect, sub2api_detect) = tokio::join!(
        newapi_client::probe_instance(&state.http_client, &base_url),
        sub2api_client::fetch_public_settings(&state.http_client, &base_url),
    );

    match (newapi_detect, sub2api_detect) {
        (Ok(()), Ok(_)) => Err(ApiError::bad_request(
            "remote_provider_detect_ambiguous",
            "Base URL matches multiple account providers",
        )),
        (Ok(()), Err(_)) => Ok(Json(RemoteAccountDetectionResponse {
            provider: RemoteAccountProvider::Newapi,
            normalized_base_url: base_url.clone(),
            recommended_api_url: None,
            suggested_page_checkin_url: detect_suggested_page_checkin_url(
                RemoteAccountProvider::Newapi,
                &base_url,
            ),
            supported_checkin_modes: vec![
                RemoteAccountCheckinMode::Disabled,
                RemoteAccountCheckinMode::SystemApi,
                RemoteAccountCheckinMode::PageOpen,
            ],
        })),
        (Err(_), Ok(settings)) => {
            let _ = settings.site_name;
            let _ = settings.backend_mode_enabled;
            Ok(Json(RemoteAccountDetectionResponse {
                provider: RemoteAccountProvider::Sub2Api,
                normalized_base_url: base_url.clone(),
                recommended_api_url: detect_recommended_api_url(
                    RemoteAccountProvider::Sub2Api,
                    &base_url,
                    settings.api_base_url,
                ),
                suggested_page_checkin_url: detect_suggested_page_checkin_url(
                    RemoteAccountProvider::Sub2Api,
                    &base_url,
                ),
                supported_checkin_modes: vec![
                    RemoteAccountCheckinMode::Disabled,
                    RemoteAccountCheckinMode::PageOpen,
                ],
            }))
        }
        (Err(_), Err(_)) => Err(ApiError::bad_request(
            "remote_provider_detect_failed",
            "Unable to detect account provider from base_url",
        )),
    }
}

pub(in crate::server) async fn list_remote_accounts(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let mut items = Vec::new();
    items.extend(
        storage::list_newapi_accounts(state.db_path())
            .await?
            .into_iter()
            .map(map_newapi_account),
    );
    items.extend(
        storage::list_remote_accounts(state.db_path())
            .await?
            .into_iter()
            .map(map_remote_account),
    );
    items.sort_by(|a, b| {
        a.sort_order()
            .cmp(&b.sort_order())
            .then_with(|| a.created_at_ms().cmp(&b.created_at_ms()))
    });
    Ok(Json(items))
}

pub(in crate::server) async fn reorder_remote_accounts(
    State(state): State<AppState>,
    Json(input): Json<ReorderRemoteAccountsInput>,
) -> Result<impl IntoResponse, ApiError> {
    let mut seen = HashSet::<String>::new();
    for id in &input.account_ids {
        if !seen.insert(id.clone()) {
            return Err(ApiError::bad_request(
                "remote_account_ids_duplicate",
                "account_ids contains duplicates",
            ));
        }
    }

    let newapi_accounts = storage::list_newapi_accounts(state.db_path()).await?;
    let remote_accounts = storage::list_remote_accounts(state.db_path()).await?;
    let total = newapi_accounts.len() + remote_accounts.len();
    if total != input.account_ids.len() {
        return Err(ApiError::bad_request(
            "remote_account_ids_mismatch",
            "account_ids must cover all accounts",
        ));
    }

    let newapi_ids = newapi_accounts
        .into_iter()
        .map(|item| item.id)
        .collect::<HashSet<_>>();
    let remote_ids = remote_accounts
        .into_iter()
        .map(|item| item.id)
        .collect::<HashSet<_>>();

    let mut newapi_orders = Vec::new();
    let mut remote_orders = Vec::new();
    for (index, account_id) in input.account_ids.iter().enumerate() {
        let sort_order = index as i64;
        if newapi_ids.contains(account_id) {
            newapi_orders.push((account_id.clone(), sort_order));
        } else if remote_ids.contains(account_id) {
            remote_orders.push((account_id.clone(), sort_order));
        } else {
            return Err(ApiError::bad_request(
                "remote_account_ids_mismatch",
                "account_ids contains unknown account",
            ));
        }
    }

    if !newapi_orders.is_empty() {
        storage::assign_newapi_account_sort_orders(state.db_path(), newapi_orders).await?;
    }
    if !remote_orders.is_empty() {
        storage::assign_remote_account_sort_orders(state.db_path(), remote_orders).await?;
    }
    newapi_handlers::notify_background_tasks(&state);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(in crate::server) async fn remote_account_checkins_today(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let legacy = storage::get_newapi_accounts_checkins_today(state.db_path()).await?;
    let remote = storage::get_remote_accounts_checkins_today(state.db_path()).await?;
    let mut ids = legacy.completed_account_ids;
    ids.extend(remote.completed_account_ids);
    ids.sort();
    ids.dedup();
    Ok(Json(storage::RemoteAccountCheckinsToday {
        date: remote.date,
        completed_account_ids: ids,
    }))
}

pub(in crate::server) async fn create_remote_account(
    State(state): State<AppState>,
    Json(input): Json<CreateRemoteAccountInput>,
) -> Result<impl IntoResponse, ApiError> {
    match input.provider {
        RemoteAccountProvider::Newapi => {
            let user_id = input.user_id.unwrap_or_default();
            let user_token = input.user_token.unwrap_or_default();
            let request_checkin_mode = match input
                .checkin_mode
                .unwrap_or(RemoteAccountCheckinMode::Disabled)
            {
                RemoteAccountCheckinMode::Disabled | RemoteAccountCheckinMode::SystemApi => {
                    storage::NewApiAccountCheckinMode::SystemApi
                }
                RemoteAccountCheckinMode::PageOpen => storage::NewApiAccountCheckinMode::PageOpen,
            };
            let auto_checkin_enabled = matches!(
                input.checkin_mode,
                Some(RemoteAccountCheckinMode::SystemApi)
            );
            let create_input = storage::CreateNewApiAccount {
                base_url: input.base_url,
                api_url: input.api_url,
                user_id,
                user_token,
                page_checkin_url: input.page_checkin_url,
                checkin_mode: Some(request_checkin_mode),
                auto_checkin_enabled: Some(auto_checkin_enabled),
                auto_checkin_time: input.auto_checkin_time,
                low_balance_alert_threshold: input.low_balance_alert_threshold,
                recharge_currency: input.recharge_currency,
            };
            let candidate = newapi_handlers::build_candidate_from_create(&create_input)?;
            let account = storage::create_newapi_account(state.db_path(), create_input)
                .await
                .map_err(|e| match e.downcast_ref::<storage::StorageError>() {
                    Some(storage::StorageError::NewApiAccountAlreadyExists { .. }) => {
                        ApiError::conflict(
                            "newapi_account_exists",
                            "New API account already exists",
                        )
                    }
                    _ => ApiError::Internal(e),
                })?;
            let account =
                newapi_handlers::sync_account_if_possible(&state, account.id.clone(), &candidate)
                    .await?;
            newapi_handlers::notify_background_tasks(&state);
            return Ok((
                axum::http::StatusCode::CREATED,
                Json(map_newapi_account(account)),
            ));
        }
        RemoteAccountProvider::Sub2Api => {}
    }

    let base_url = detect_provider_from_base_url(&input.base_url)?;
    let access_token = input.bearer_token.unwrap_or_default();
    let checkin_mode = input
        .checkin_mode
        .unwrap_or(RemoteAccountCheckinMode::Disabled);
    let auto_checkin_time = input
        .auto_checkin_time
        .clone()
        .unwrap_or_else(|| "00:05:00".to_string());
    validate_sub2api_candidate(
        &base_url,
        input.page_checkin_url.as_deref(),
        checkin_mode,
        &auto_checkin_time,
        input.low_balance_alert_threshold.unwrap_or(0.0),
        !access_token.trim().is_empty(),
    )?;
    let _overview =
        sub2api_client::fetch_account_overview(&state.http_client, &base_url, &access_token)
            .await
            .map_err(|err| {
                ApiError::bad_gateway(
                    "remote_sync_failed",
                    format!("Failed to validate sub2api account: {err}"),
                )
            })?;
    let account = storage::create_remote_account(
        state.db_path(),
        storage::CreateRemoteAccount {
            provider: storage::RemoteAccountProvider::Sub2Api,
            base_url,
            api_url: input.api_url,
            access_token,
            page_checkin_url: input.page_checkin_url,
            checkin_mode: Some(match checkin_mode {
                RemoteAccountCheckinMode::Disabled => storage::RemoteAccountCheckinMode::Disabled,
                RemoteAccountCheckinMode::PageOpen => storage::RemoteAccountCheckinMode::PageOpen,
                RemoteAccountCheckinMode::SystemApi => storage::RemoteAccountCheckinMode::Disabled,
            }),
            auto_checkin_time: Some(auto_checkin_time),
            low_balance_alert_threshold: input.low_balance_alert_threshold,
            recharge_currency: input.recharge_currency,
        },
    )
    .await
    .map_err(|e| match e.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::RemoteAccountAlreadyExists { .. }) => {
            ApiError::conflict("remote_account_exists", "Remote account already exists")
        }
        _ => ApiError::Internal(e),
    })?;
    let account = fetch_and_persist_sub2api_snapshot(&state, account.id.clone()).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(map_remote_account(account)),
    ))
}

pub(in crate::server) async fn update_remote_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    Json(input): Json<UpdateRemoteAccountInput>,
) -> Result<impl IntoResponse, ApiError> {
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        ResolvedRemoteAccount::Newapi(current) => {
            let provider = input.provider.unwrap_or(RemoteAccountProvider::Newapi);
            if provider != RemoteAccountProvider::Newapi {
                return Err(ApiError::bad_request(
                    "remote_provider_mismatch",
                    "provider does not match account type",
                ));
            }
            let request_checkin_mode = match input.checkin_mode {
                Some(RemoteAccountCheckinMode::PageOpen) => {
                    Some(storage::NewApiAccountCheckinMode::PageOpen)
                }
                Some(RemoteAccountCheckinMode::Disabled | RemoteAccountCheckinMode::SystemApi) => {
                    Some(storage::NewApiAccountCheckinMode::SystemApi)
                }
                None => None,
            };
            let auto_checkin_enabled = match input.checkin_mode {
                Some(RemoteAccountCheckinMode::SystemApi) => Some(true),
                Some(RemoteAccountCheckinMode::Disabled | RemoteAccountCheckinMode::PageOpen) => {
                    Some(false)
                }
                None => None,
            };
            let update_input = storage::UpdateNewApiAccount {
                base_url: input.base_url,
                api_url: input.api_url,
                user_id: input.user_id,
                user_token: input.user_token,
                page_checkin_url: input.page_checkin_url,
                checkin_mode: request_checkin_mode,
                auto_checkin_enabled,
                auto_checkin_time: input.auto_checkin_time,
                low_balance_alert_threshold: input.low_balance_alert_threshold,
                recharge_currency: input.recharge_currency,
            };
            let candidate = newapi_handlers::build_candidate_from_update(&current, &update_input)?;
            storage::update_newapi_account(state.db_path(), account_id.clone(), update_input)
                .await
                .map_err(|e| match e.downcast_ref::<storage::StorageError>() {
                    Some(storage::StorageError::NewApiAccountNotFound { .. }) => {
                        ApiError::not_found("newapi_account_not_found", "New API account not found")
                    }
                    Some(storage::StorageError::NewApiAccountAlreadyExists { .. }) => {
                        ApiError::conflict(
                            "newapi_account_exists",
                            "New API account already exists",
                        )
                    }
                    _ => ApiError::Internal(e),
                })?;
            let account =
                newapi_handlers::sync_account_if_possible(&state, account_id, &candidate).await?;
            newapi_handlers::notify_background_tasks(&state);
            Ok(Json(map_newapi_account(account)))
        }
        ResolvedRemoteAccount::Sub2Api(current) => {
            let provider = input.provider.unwrap_or(RemoteAccountProvider::Sub2Api);
            if provider != RemoteAccountProvider::Sub2Api {
                return Err(ApiError::bad_request(
                    "remote_provider_mismatch",
                    "provider does not match account type",
                ));
            }
            let effective_base_url = input
                .base_url
                .as_deref()
                .map(detect_provider_from_base_url)
                .transpose()?
                .unwrap_or_else(|| current.base_url.clone());
            let effective_token = input
                .bearer_token
                .clone()
                .or_else(|| current.access_token.clone())
                .unwrap_or_default();
            let effective_checkin_mode = input.checkin_mode.unwrap_or(match current.checkin_mode {
                storage::RemoteAccountCheckinMode::Disabled => RemoteAccountCheckinMode::Disabled,
                storage::RemoteAccountCheckinMode::PageOpen => RemoteAccountCheckinMode::PageOpen,
            });
            let effective_auto_checkin_time = input
                .auto_checkin_time
                .clone()
                .unwrap_or_else(|| current.auto_checkin_time.clone());
            let effective_threshold = input
                .low_balance_alert_threshold
                .unwrap_or(current.low_balance_alert_threshold);
            validate_sub2api_candidate(
                &effective_base_url,
                input
                    .page_checkin_url
                    .as_deref()
                    .or(current.page_checkin_url.as_deref()),
                effective_checkin_mode,
                &effective_auto_checkin_time,
                effective_threshold,
                !effective_token.trim().is_empty(),
            )?;
            let _overview = sub2api_client::fetch_account_overview(
                &state.http_client,
                &effective_base_url,
                &effective_token,
            )
            .await
            .map_err(|err| {
                ApiError::bad_gateway(
                    "remote_sync_failed",
                    format!("Failed to validate sub2api account: {err}"),
                )
            })?;
            storage::update_remote_account(
                state.db_path(),
                account_id.clone(),
                storage::UpdateRemoteAccount {
                    base_url: Some(effective_base_url),
                    api_url: input.api_url,
                    access_token: Some(effective_token),
                    page_checkin_url: input.page_checkin_url,
                    checkin_mode: Some(match effective_checkin_mode {
                        RemoteAccountCheckinMode::Disabled => {
                            storage::RemoteAccountCheckinMode::Disabled
                        }
                        RemoteAccountCheckinMode::PageOpen => {
                            storage::RemoteAccountCheckinMode::PageOpen
                        }
                        RemoteAccountCheckinMode::SystemApi => {
                            storage::RemoteAccountCheckinMode::Disabled
                        }
                    }),
                    auto_checkin_time: Some(effective_auto_checkin_time),
                    low_balance_alert_threshold: Some(effective_threshold),
                    recharge_currency: input.recharge_currency,
                },
            )
            .await
            .map_err(|e| match e.downcast_ref::<storage::StorageError>() {
                Some(storage::StorageError::RemoteAccountNotFound { .. }) => {
                    ApiError::not_found("remote_account_not_found", "Remote account not found")
                }
                Some(storage::StorageError::RemoteAccountAlreadyExists { .. }) => {
                    ApiError::conflict("remote_account_exists", "Remote account already exists")
                }
                _ => ApiError::Internal(e),
            })?;
            let account = fetch_and_persist_sub2api_snapshot(&state, account_id).await?;
            Ok(Json(map_remote_account(account)))
        }
    }
}

pub(in crate::server) async fn refresh_remote_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        ResolvedRemoteAccount::Newapi(account) => {
            if !newapi_client::account_has_user_api_credentials(&account) {
                let cleared =
                    newapi_handlers::clear_account_remote_state(&state, account_id.clone()).await?;
                return Ok(Json(map_newapi_account(cleared)));
            }
            let overview =
                match newapi_client::fetch_account_overview(&state.http_client, &account).await {
                    Ok(overview) => overview,
                    Err(err) => {
                        let _ = newapi_handlers::record_account_sync_failure(
                            &state,
                            account_id.clone(),
                            &err,
                        )
                        .await;
                        return Err(newapi_handlers::sync_error(err));
                    }
                };
            let account =
                newapi_handlers::apply_account_overview(&state, account.id.clone(), &overview)
                    .await?;
            Ok(Json(map_newapi_account(account)))
        }
        ResolvedRemoteAccount::Sub2Api(_) => {
            let result = fetch_and_persist_sub2api_snapshot(&state, account_id.clone()).await;
            match result {
                Ok(account) => Ok(Json(map_remote_account(account))),
                Err(err) => {
                    if let ApiError::BadGateway { message, .. } = &err {
                        let anyhow_err = anyhow::anyhow!(message.clone());
                        let _ = record_sub2api_sync_failure(&state, account_id, &anyhow_err).await;
                    }
                    Err(err)
                }
            }
        }
    }
}

pub(in crate::server) async fn list_remote_account_groups(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        ResolvedRemoteAccount::Newapi(account) => {
            if !newapi_client::account_has_user_api_credentials(&account) {
                return Err(ApiError::bad_request(
                    "newapi_credentials_required",
                    "user_id and user_token are required for this action",
                ));
            }
            let groups = newapi_client::list_groups(&state.http_client, &account)
                .await
                .map_err(newapi_handlers::sync_error)?;
            let managed_counts = storage::list_channels(state.db_path())
                .await?
                .into_iter()
                .filter(|channel| {
                    channel.is_managed_by_account(
                        storage::ManagedRemoteProvider::Newapi,
                        account.id.as_str(),
                    )
                })
                .fold(HashMap::<String, usize>::new(), |mut acc, channel| {
                    if let Some(group) = channel.managed_group_name() {
                        *acc.entry(group.to_string()).or_default() += 1;
                    }
                    acc
                });
            let out = groups
                .into_iter()
                .map(|item| RemoteGroupResponse {
                    id: None,
                    name: item.name.clone(),
                    ratio: item.ratio,
                    description: item.description,
                    platform: None,
                    managed_channel_count: managed_counts.get(&item.name).copied().unwrap_or(0),
                })
                .collect::<Vec<_>>();
            Ok(Json(out))
        }
        ResolvedRemoteAccount::Sub2Api(account) => {
            let token = account
                .access_token
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request("remote_credentials_required", "bearer_token is required")
                })?;
            let groups = sub2api_client::list_groups(&state.http_client, &account.base_url, token)
                .await
                .map_err(|err| {
                    ApiError::bad_gateway("remote_groups_load_failed", err.to_string())
                })?;
            let channels = storage::list_channels(state.db_path()).await?;
            let mut managed_counts_by_id = HashMap::<i64, usize>::new();
            let mut managed_counts_by_name = HashMap::<String, usize>::new();
            for channel in channels.into_iter().filter(|channel| {
                channel.is_managed_by_account(
                    storage::ManagedRemoteProvider::Sub2Api,
                    account.id.as_str(),
                )
            }) {
                if let Some(group_id) = channel.managed_group_id() {
                    *managed_counts_by_id.entry(group_id).or_default() += 1;
                } else if let Some(group_name) = channel.managed_group_name() {
                    *managed_counts_by_name
                        .entry(group_name.to_string())
                        .or_default() += 1;
                }
            }
            Ok(Json(
                groups
                    .into_iter()
                    .map(|item| RemoteGroupResponse {
                        id: Some(item.id),
                        managed_channel_count: managed_counts_by_id
                            .get(&item.id)
                            .copied()
                            .or_else(|| managed_counts_by_name.get(item.name.as_str()).copied())
                            .unwrap_or(0),
                        name: item.name,
                        ratio: item.rate_multiplier,
                        description: item.description,
                        platform: item.platform,
                    })
                    .collect::<Vec<_>>(),
            ))
        }
    }
}

pub(in crate::server) async fn complete_remote_account_checkin_today(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        ResolvedRemoteAccount::Newapi(_) => {
            let res = storage::complete_newapi_account_checkin_today(
                state.db_path(),
                account_id,
                "manual_page",
            )
            .await;
            map_storage_unit_no_content_err(res, |e| {
                matches!(
                    e.downcast_ref::<storage::StorageError>(),
                    Some(storage::StorageError::NewApiAccountNotFound { .. })
                )
                .then(|| {
                    ApiError::not_found("newapi_account_not_found", "New API account not found")
                })
            })
        }
        ResolvedRemoteAccount::Sub2Api(_) => {
            let res =
                storage::complete_remote_account_checkin_today(state.db_path(), account_id).await;
            map_storage_unit_no_content_err(res, |e| {
                matches!(
                    e.downcast_ref::<storage::StorageError>(),
                    Some(storage::StorageError::RemoteAccountNotFound { .. })
                )
                .then(|| {
                    ApiError::not_found("remote_account_not_found", "Remote account not found")
                })
            })
        }
    }
}

pub(in crate::server) async fn create_remote_account_key(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    Json(input): Json<CreateRemoteKeyInput>,
) -> Result<impl IntoResponse, ApiError> {
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        ResolvedRemoteAccount::Newapi(_) => Err(ApiError::bad_request(
            "remote_key_unsupported_provider",
            "Only sub2api accounts support key creation",
        )),
        ResolvedRemoteAccount::Sub2Api(account) => {
            let token = account
                .access_token
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request("remote_credentials_required", "bearer_token is required")
                })?;
            let name = input.name.trim();
            if name.is_empty() {
                return Err(ApiError::bad_request(
                    "remote_key_name_required",
                    "name is required",
                ));
            }
            let key = sub2api_client::create_key(
                &state.http_client,
                &account.base_url,
                token,
                &sub2api_client::CreateSub2ApiKeyRequest {
                    name: name.to_string(),
                    group_id: input.group_id,
                },
            )
            .await
            .map_err(|err| ApiError::bad_gateway("remote_key_create_failed", err.to_string()))?;
            Ok(Json(RemoteKeyResponse {
                id: key.id,
                key: key.key,
                name: key.name,
                group_id: key.group_id,
                status: key.status,
            }))
        }
    }
}

pub(in crate::server) async fn create_remote_managed_channel(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    Json(input): Json<CreateRemoteManagedChannelInput>,
) -> Result<impl IntoResponse, ApiError> {
    let protocol = input
        .protocol
        .ok_or_else(|| ApiError::bad_request("remote_protocol_required", "protocol is required"))?;
    let name = validate_managed_channel_name(&input.name)?;
    let base_url_override = newapi_handlers::validate_optional_http_url(
        input.base_url_override.as_deref(),
        "remote_managed_base_url_invalid",
    )?;
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        ResolvedRemoteAccount::Newapi(account) => {
            if !newapi_client::account_has_user_api_credentials(&account) {
                return Err(ApiError::bad_request(
                    "newapi_credentials_required",
                    "user_id and user_token are required for this action",
                ));
            }
            let group_name = input.group_name.trim().to_string();
            if group_name.is_empty() {
                return Err(ApiError::bad_request(
                    "remote_group_required",
                    "group_name is required",
                ));
            }
            let remote = newapi_client::create_managed_channel(
                &state.http_client,
                &account,
                &newapi_client::CreateManagedChannelRequest {
                    name: name.clone(),
                    group_name: group_name.clone(),
                },
            )
            .await
            .map_err(|err| {
                ApiError::bad_gateway("remote_managed_channel_create_failed", err.to_string())
            })?;
            let create_local = storage::CreateChannel {
                name,
                protocol,
                base_url: managed_channel_base_url_for_newapi(&account, base_url_override),
                auth_type: Some("auto".to_string()),
                auth_ref: remote.token_key.clone(),
                checkin_url: None,
                priority: input.priority.unwrap_or(0),
                recharge_currency: Some(account.recharge_currency),
                real_multiplier: Some(remote.group_ratio),
                enabled: input.enabled.unwrap_or(true),
                managed_by_remote: Some(true),
                managed_remote_provider: Some(storage::ManagedRemoteProvider::Newapi),
                managed_remote_account_id: Some(account_id.clone()),
                managed_remote_resource_id: Some(remote.token_id.to_string()),
                managed_remote_resource_name: Some(remote.token_name.clone()),
                managed_remote_group_name: Some(remote.group_name.clone()),
                managed_remote_group_id: None,
                managed_by_newapi: Some(true),
                newapi_account_id: Some(account_id.clone()),
                newapi_channel_id: None,
                newapi_token_id: Some(remote.token_id),
                newapi_token_name: Some(remote.token_name.clone()),
                newapi_group: Some(remote.group_name.clone()),
            };
            let channel = match storage::create_channel(state.db_path(), create_local).await {
                Ok(channel) => channel,
                Err(err) => {
                    let _ = newapi_client::delete_token(&state.http_client, &account, remote.token_id)
                        .await;
                    return Err(ApiError::Internal(err));
                }
            };
            state.channels_cache.send_modify(|cur| {
                let mut next = (**cur).clone();
                next.push(channel.clone());
                next.sort_by(|a, b| {
                    let rank = |p: storage::Protocol| match p {
                        storage::Protocol::Openai => 0,
                        storage::Protocol::Anthropic => 1,
                        storage::Protocol::Gemini => 2,
                    };
                    rank(a.protocol)
                        .cmp(&rank(b.protocol))
                        .then_with(|| b.priority.cmp(&a.priority))
                        .then_with(|| a.name.cmp(&b.name))
                });
                *cur = std::sync::Arc::new(next);
            });
            if state
                .settings_snapshot()
                .newapi_managed_channel_missing_prompt_enabled
            {
                events::publish(AppEvent::RemoteManagedChannelCreated(
                    RemoteManagedChannelCreated {
                        channel_id: channel.id.clone(),
                        channel_name: channel.name.clone(),
                        account_id: account.id.clone(),
                        account_base_url: account.base_url.clone(),
                        provider: storage::ManagedRemoteProvider::Newapi,
                        group_name: channel.managed_group_name().map(ToOwned::to_owned),
                        resource_name: channel.managed_resource_name().map(ToOwned::to_owned),
                    },
                ));
            }
            Ok((
                axum::http::StatusCode::CREATED,
                Json(CreateRemoteManagedChannelResponse { channel }),
            ))
        }
        ResolvedRemoteAccount::Sub2Api(account) => {
            let token = account
                .access_token
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    ApiError::bad_request("remote_credentials_required", "bearer_token is required")
                })?;
            let groups = sub2api_client::list_groups(&state.http_client, &account.base_url, token)
                .await
                .map_err(|err| {
                    ApiError::bad_gateway("remote_groups_load_failed", err.to_string())
                })?;
            let requested_group_name = input.group_name.trim();
            let selected_group = if let Some(group_id) = input.group_id {
                groups
                    .iter()
                    .find(|item| item.id == group_id)
                    .or_else(|| groups.iter().find(|item| item.name == requested_group_name))
            } else {
                groups.iter().find(|item| item.name == requested_group_name)
            }
            .ok_or_else(|| {
                ApiError::bad_request("remote_group_not_found", "selected remote group was not found")
            })?;
            let created_key = sub2api_client::create_key(
                &state.http_client,
                &account.base_url,
                token,
                &sub2api_client::CreateSub2ApiKeyRequest {
                    name: name.clone(),
                    group_id: Some(selected_group.id),
                },
            )
            .await
            .map_err(|err| {
                ApiError::bad_gateway("remote_managed_channel_create_failed", err.to_string())
            })?;
            let create_local = storage::CreateChannel {
                name,
                protocol,
                base_url: managed_channel_base_url_for_sub2api(&account, base_url_override),
                auth_type: Some("auto".to_string()),
                auth_ref: created_key.key.clone(),
                checkin_url: None,
                priority: input.priority.unwrap_or(0),
                recharge_currency: Some(account.recharge_currency),
                real_multiplier: Some(selected_group.rate_multiplier.unwrap_or(1.0)),
                enabled: input.enabled.unwrap_or(true),
                managed_by_remote: Some(true),
                managed_remote_provider: Some(storage::ManagedRemoteProvider::Sub2Api),
                managed_remote_account_id: Some(account_id.clone()),
                managed_remote_resource_id: Some(created_key.id.to_string()),
                managed_remote_resource_name: Some(created_key.name.clone()),
                managed_remote_group_name: Some(selected_group.name.clone()),
                managed_remote_group_id: Some(selected_group.id),
                managed_by_newapi: Some(false),
                newapi_account_id: None,
                newapi_channel_id: None,
                newapi_token_id: None,
                newapi_token_name: None,
                newapi_group: None,
            };
            let channel = match storage::create_channel(state.db_path(), create_local).await {
                Ok(channel) => channel,
                Err(err) => {
                    let _ = sub2api_client::delete_key(
                        &state.http_client,
                        &account.base_url,
                        token,
                        created_key.id,
                    )
                    .await;
                    return Err(ApiError::Internal(err));
                }
            };
            state.channels_cache.send_modify(|cur| {
                let mut next = (**cur).clone();
                next.push(channel.clone());
                next.sort_by(|a, b| {
                    let rank = |p: storage::Protocol| match p {
                        storage::Protocol::Openai => 0,
                        storage::Protocol::Anthropic => 1,
                        storage::Protocol::Gemini => 2,
                    };
                    rank(a.protocol)
                        .cmp(&rank(b.protocol))
                        .then_with(|| b.priority.cmp(&a.priority))
                        .then_with(|| a.name.cmp(&b.name))
                });
                *cur = std::sync::Arc::new(next);
            });
            if state
                .settings_snapshot()
                .newapi_managed_channel_missing_prompt_enabled
            {
                events::publish(AppEvent::RemoteManagedChannelCreated(
                    RemoteManagedChannelCreated {
                        channel_id: channel.id.clone(),
                        channel_name: channel.name.clone(),
                        account_id: account.id.clone(),
                        account_base_url: account.base_url.clone(),
                        provider: storage::ManagedRemoteProvider::Sub2Api,
                        group_name: channel.managed_group_name().map(ToOwned::to_owned),
                        resource_name: channel.managed_resource_name().map(ToOwned::to_owned),
                    },
                ));
            }
            Ok((
                axum::http::StatusCode::CREATED,
                Json(CreateRemoteManagedChannelResponse { channel }),
            ))
        }
    }
}

pub(in crate::server) async fn delete_remote_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    input: Option<Json<DeleteRemoteAccountInput>>,
) -> Result<impl IntoResponse, ApiError> {
    let input = input.map(|Json(input)| input).unwrap_or_default();
    let delete_managed_channels = input.delete_managed_channels.unwrap_or(false);
    let sync_remote_delete = input.sync_remote_delete.unwrap_or(false);
    if sync_remote_delete && !delete_managed_channels {
        return Err(ApiError::bad_request(
            "remote_delete_remote_requires_channel_delete",
            "sync_remote_delete requires delete_managed_channels=true",
        ));
    }
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        ResolvedRemoteAccount::Newapi(_account) => {
            if delete_managed_channels && sync_remote_delete {
                let channels = storage::list_channels(state.db_path())
                    .await?
                    .into_iter()
                    .filter(|channel| {
                        channel.is_managed_by_account(
                            storage::ManagedRemoteProvider::Newapi,
                            account_id.as_str(),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut deleted_channel_ids = Vec::new();
                let mut failures = Vec::new();
                for channel in &channels {
                    match delete_remote_managed_channel_resources(&state, channel).await {
                        Ok(()) => deleted_channel_ids.push(channel.id.clone()),
                        Err(err) => {
                            failures.push(format!("{} ({}): {err}", channel.name, channel.id))
                        }
                    }
                }
                if !deleted_channel_ids.is_empty() && !failures.is_empty() {
                    for channel_id in &deleted_channel_ids {
                        storage::delete_channel(state.db_path(), channel_id.clone()).await?;
                    }
                    state.channels_cache.send_modify(|cur| {
                        let deleted = deleted_channel_ids.clone();
                        let mut next = (**cur).clone();
                        next.retain(|channel| !deleted.contains(&channel.id));
                        *cur = std::sync::Arc::new(next);
                    });
                }
                if !failures.is_empty() {
                    return Err(ApiError::bad_gateway(
                        "remote_delete_partial_failed",
                        format!(
                            "Remote delete failed for some managed channels; account was kept. {}",
                            failures.join(" ; ")
                        ),
                    ));
                }
            }
            let res = storage::delete_newapi_account(
                state.db_path(),
                account_id.clone(),
                delete_managed_channels,
            )
            .await;
            match res {
                Ok(result) => {
                    if delete_managed_channels {
                        let deleted = result.deleted_managed_channel_ids.clone();
                        state.channels_cache.send_modify(|cur| {
                            let deleted = deleted.clone();
                            let mut next = (**cur).clone();
                            next.retain(|channel| !deleted.contains(&channel.id));
                            *cur = std::sync::Arc::new(next);
                        });
                    } else {
                        let account_id = account_id.clone();
                        state.channels_cache.send_modify(|cur| {
                            let mut next = (**cur).clone();
                            for channel in &mut next {
                                if channel.is_managed_by_account(
                                    storage::ManagedRemoteProvider::Newapi,
                                    account_id.as_str(),
                                ) {
                                    channel.clear_managed_remote_link();
                                }
                            }
                            *cur = std::sync::Arc::new(next);
                        });
                    }
                    newapi_handlers::notify_background_tasks(&state);
                    Ok(Json(result).into_response())
                }
                Err(err) => Err(match err.downcast_ref::<storage::StorageError>() {
                    Some(storage::StorageError::NewApiAccountNotFound { .. }) => {
                        ApiError::not_found("newapi_account_not_found", "New API account not found")
                    }
                    _ => ApiError::Internal(err),
                }),
            }
        }
        ResolvedRemoteAccount::Sub2Api(account) => {
            let linked_channel_ids = storage::list_channel_ids_by_managed_account(
                state.db_path(),
                storage::ManagedRemoteProvider::Sub2Api,
                account_id.clone(),
            )
            .await?;
            if delete_managed_channels && sync_remote_delete {
                let channels = storage::list_channels(state.db_path())
                    .await?
                    .into_iter()
                    .filter(|channel| {
                        channel.is_managed_by_account(
                            storage::ManagedRemoteProvider::Sub2Api,
                            account_id.as_str(),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut deleted_channel_ids = Vec::new();
                let mut failures = Vec::new();
                for channel in &channels {
                    match delete_remote_managed_channel_resources(&state, channel).await {
                        Ok(()) => deleted_channel_ids.push(channel.id.clone()),
                        Err(err) => {
                            failures.push(format!("{} ({}): {err}", channel.name, channel.id))
                        }
                    }
                }
                if !deleted_channel_ids.is_empty() && !failures.is_empty() {
                    for channel_id in &deleted_channel_ids {
                        storage::delete_channel(state.db_path(), channel_id.clone()).await?;
                    }
                    state.channels_cache.send_modify(|cur| {
                        let deleted = deleted_channel_ids.clone();
                        let mut next = (**cur).clone();
                        next.retain(|channel| !deleted.contains(&channel.id));
                        *cur = std::sync::Arc::new(next);
                    });
                }
                if !failures.is_empty() {
                    return Err(ApiError::bad_gateway(
                        "remote_delete_partial_failed",
                        format!(
                            "Remote delete failed for some managed channels; account was kept. {}",
                            failures.join(" ; ")
                        ),
                    ));
                }
            }

            let result = if delete_managed_channels {
                for channel_id in &linked_channel_ids {
                    storage::delete_channel(state.db_path(), channel_id.clone()).await?;
                }
                storage::delete_remote_account(state.db_path(), account_id.clone()).await?;
                storage::DeleteNewApiAccountResult {
                    deleted_managed_channel_ids: linked_channel_ids.clone(),
                    detached_channel_ids: Vec::new(),
                }
            } else {
                let detached_channel_ids = storage::detach_channels_from_managed_account(
                    state.db_path(),
                    storage::ManagedRemoteProvider::Sub2Api,
                    account_id.clone(),
                )
                .await?;
                storage::delete_remote_account(state.db_path(), account_id.clone()).await?;
                storage::DeleteNewApiAccountResult {
                    deleted_managed_channel_ids: Vec::new(),
                    detached_channel_ids,
                }
            };
            if delete_managed_channels {
                let deleted = result.deleted_managed_channel_ids.clone();
                state.channels_cache.send_modify(|cur| {
                    let deleted = deleted.clone();
                    let mut next = (**cur).clone();
                    next.retain(|channel| !deleted.contains(&channel.id));
                    *cur = std::sync::Arc::new(next);
                });
            } else {
                let account_id = account_id.clone();
                state.channels_cache.send_modify(|cur| {
                    let mut next = (**cur).clone();
                    for channel in &mut next {
                        if channel.is_managed_by_account(
                            storage::ManagedRemoteProvider::Sub2Api,
                            account_id.as_str(),
                        ) {
                            channel.clear_managed_remote_link();
                        }
                    }
                    *cur = std::sync::Arc::new(next);
                });
            }
            newapi_handlers::notify_background_tasks(&state);
            let _ = account;
            Ok(Json(result).into_response())
        }
    }
}
