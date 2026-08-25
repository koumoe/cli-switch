use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::newapi as newapi_handlers;
use crate::bearer_token::normalize_optional_bearer_token;
use crate::events::{self, AppEvent, RemoteManagedChannelCreated};
use crate::newapi as newapi_client;
use crate::server::AppState;
use crate::server::error::{ApiError, map_storage_unit_no_content_err};
use crate::server::sub2api_auth;
use crate::storage::{self, RechargeCurrency};
use crate::sub2api as sub2api_client;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(in crate::server) enum RemoteAccountProvider {
    Newapi,
    #[serde(rename = "sub2api")]
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
    pub reauth_required: bool,
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
    #[serde(rename = "sub2api")]
    Sub2Api {
        #[serde(flatten)]
        common: RemoteAccountCommonResponse,
        #[serde(flatten)]
        sub2api: Sub2ApiRemoteAccountResponse,
    },
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
    refresh_token: Option<String>,
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
    refresh_token: Option<String>,
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

#[derive(Debug, Clone)]
struct PreparedCreateRemoteManagedChannelInput {
    name: String,
    protocol: storage::Protocol,
    group_name: String,
    group_id: Option<i64>,
    base_url_override: Option<String>,
    priority: i64,
    enabled: bool,
}

#[derive(Debug, Clone, Copy)]
struct DeleteRemoteAccountOptions {
    delete_managed_channels: bool,
    sync_remote_delete: bool,
}

fn validate_required_trimmed_text(
    value: &str,
    code: &'static str,
    message: &'static str,
) -> Result<String, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ApiError::bad_request(code, message));
    }
    Ok(value.to_string())
}

fn validate_managed_channel_name(name: &str) -> Result<String, ApiError> {
    validate_required_trimmed_text(name, "remote_managed_name_required", "name is required")
}

fn validate_remote_key_name(name: &str) -> Result<String, ApiError> {
    validate_required_trimmed_text(name, "remote_key_name_required", "name is required")
}

fn validate_remote_group_name(name: &str) -> Result<String, ApiError> {
    validate_required_trimmed_text(name, "remote_group_required", "group_name is required")
}

fn prepare_create_remote_managed_channel_input(
    input: CreateRemoteManagedChannelInput,
) -> Result<PreparedCreateRemoteManagedChannelInput, ApiError> {
    Ok(PreparedCreateRemoteManagedChannelInput {
        name: validate_managed_channel_name(&input.name)?,
        protocol: input.protocol.ok_or_else(|| {
            ApiError::bad_request("remote_protocol_required", "protocol is required")
        })?,
        group_name: validate_remote_group_name(&input.group_name)?,
        group_id: input.group_id,
        base_url_override: newapi_handlers::validate_optional_http_url(
            input.base_url_override.as_deref(),
            "remote_managed_base_url_invalid",
        )?,
        priority: input.priority.unwrap_or(0),
        enabled: input.enabled.unwrap_or(true),
    })
}

fn parse_delete_remote_account_options(
    input: DeleteRemoteAccountInput,
) -> Result<DeleteRemoteAccountOptions, ApiError> {
    let options = DeleteRemoteAccountOptions {
        delete_managed_channels: input.delete_managed_channels.unwrap_or(false),
        sync_remote_delete: input.sync_remote_delete.unwrap_or(false),
    };
    if options.sync_remote_delete && !options.delete_managed_channels {
        return Err(ApiError::bad_request(
            "remote_delete_remote_requires_channel_delete",
            "sync_remote_delete requires delete_managed_channels=true",
        ));
    }
    Ok(options)
}

fn normalize_http_url_preserving_path(raw: &str, field: &'static str) -> Result<String, ApiError> {
    let value = newapi_handlers::validate_http_url(raw, field)?;
    let mut parsed = reqwest::Url::parse(&value)
        .map_err(|e| ApiError::bad_request(field, format!("Invalid {field}: {e}")))?;
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

fn resolve_remote_url_from_base(base_url: &str, raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(parsed) = reqwest::Url::parse(value) {
        return match parsed.scheme() {
            "http" | "https" => Some(parsed.to_string().trim_end_matches('/').to_string()),
            _ => None,
        };
    }

    let mut base = reqwest::Url::parse(base_url).ok()?;
    base.set_query(None);
    base.set_fragment(None);

    if value.starts_with('/') {
        base.set_path(value);
        return Some(base.to_string().trim_end_matches('/').to_string());
    }

    let base_dir = match base.path().trim_end_matches('/') {
        "" => "/".to_string(),
        path => format!("{path}/"),
    };
    base.set_path(&base_dir);

    base.join(value)
        .ok()
        .map(|url| url.to_string().trim_end_matches('/').to_string())
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

fn channel_sort_rank(protocol: storage::Protocol) -> i32 {
    match protocol {
        storage::Protocol::Openai => 0,
        storage::Protocol::Anthropic => 1,
        storage::Protocol::Gemini => 2,
    }
}

fn add_channel_to_cache(state: &AppState, channel: &storage::Channel) {
    state.channels_cache.send_modify(|cur| {
        let mut next = (**cur).clone();
        next.push(channel.clone());
        next.sort_by(|a, b| {
            channel_sort_rank(a.protocol)
                .cmp(&channel_sort_rank(b.protocol))
                .then_with(|| b.priority.cmp(&a.priority))
                .then_with(|| a.name.cmp(&b.name))
        });
        *cur = std::sync::Arc::new(next);
    });
}

fn remove_channels_from_cache(state: &AppState, deleted_channel_ids: &[String]) {
    state.channels_cache.send_modify(|cur| {
        let mut next = (**cur).clone();
        next.retain(|channel| !deleted_channel_ids.contains(&channel.id));
        *cur = std::sync::Arc::new(next);
    });
}

fn clear_managed_links_in_cache(
    state: &AppState,
    provider: storage::ManagedRemoteProvider,
    account_id: &str,
) {
    state.channels_cache.send_modify(|cur| {
        let mut next = (**cur).clone();
        for channel in &mut next {
            if channel.is_managed_by_account(provider, account_id) {
                channel.clear_managed_remote_link();
            }
        }
        *cur = std::sync::Arc::new(next);
    });
}

fn publish_remote_managed_channel_created_if_enabled(
    state: &AppState,
    provider: storage::ManagedRemoteProvider,
    account_id: &str,
    account_base_url: &str,
    channel: &storage::Channel,
) {
    if !state
        .settings_snapshot()
        .remote_managed_channel_missing_prompt_enabled
    {
        return;
    }
    events::publish(AppEvent::RemoteManagedChannelCreated(
        RemoteManagedChannelCreated {
            channel_id: channel.id.clone(),
            channel_name: channel.name.clone(),
            account_id: account_id.to_string(),
            account_base_url: account_base_url.to_string(),
            provider,
            group_name: channel.managed_group_name().map(ToOwned::to_owned),
            resource_name: channel.managed_resource_name().map(ToOwned::to_owned),
        },
    ));
}

async fn list_managed_channels_for_account(
    state: &AppState,
    provider: storage::ManagedRemoteProvider,
    account_id: &str,
) -> Result<Vec<storage::Channel>, ApiError> {
    Ok(storage::list_channels(state.db_path())
        .await?
        .into_iter()
        .filter(|channel| channel.is_managed_by_account(provider, account_id))
        .collect())
}

pub(super) async fn delete_remote_managed_channel_resources(
    state: &AppState,
    channel: &storage::Channel,
) -> anyhow::Result<()> {
    let Some(provider) = channel.managed_provider() else {
        return Ok(());
    };
    let account_id = channel.managed_account_id().ok_or_else(|| {
        anyhow::anyhow!(
            "managed channel {} missing linked remote account id",
            channel.id
        )
    })?;
    match provider {
        storage::ManagedRemoteProvider::Newapi => {
            let account =
                storage::get_newapi_account_with_secret(state.db_path(), account_id.to_string())
                    .await?;
            if let Some(remote_token_id) = channel
                .managed_resource_id()
                .as_deref()
                .and_then(|value| value.parse::<i64>().ok())
            {
                newapi_client::delete_token(&state.http_client, &account, remote_token_id).await?;
            }
        }
        storage::ManagedRemoteProvider::Sub2Api => {
            let mut account =
                storage::get_remote_account_with_secret(state.db_path(), account_id.to_string())
                    .await?;
            let key_id = channel
                .managed_resource_id()
                .as_deref()
                .and_then(|value| value.parse::<i64>().ok())
                .ok_or_else(|| {
                    anyhow::anyhow!("managed channel {} missing remote key id", channel.id)
                })?;
            sub2api_auth::run_with_persisted_session(
                state.db_path(),
                &state.http_client,
                &mut account,
                |http_client, base_url, access_token| {
                    Box::pin(sub2api_client::delete_key(
                        http_client,
                        base_url,
                        access_token,
                        key_id,
                    ))
                },
            )
            .await?;
        }
    }
    Ok(())
}

async fn sync_delete_managed_channels_for_account(
    state: &AppState,
    provider: storage::ManagedRemoteProvider,
    account_id: &str,
) -> Result<(), ApiError> {
    let channels = list_managed_channels_for_account(state, provider, account_id).await?;
    let mut deleted_channel_ids = Vec::new();
    let mut failures = Vec::new();
    let mut relogin_required = false;
    for channel in &channels {
        match delete_remote_managed_channel_resources(state, channel).await {
            Ok(()) => deleted_channel_ids.push(channel.id.clone()),
            Err(err) => {
                if sub2api_auth::relogin_required_message(&err).is_some() {
                    relogin_required = true;
                }
                failures.push(format!("{} ({}): {err}", channel.name, channel.id));
            }
        }
    }
    if !deleted_channel_ids.is_empty() && !failures.is_empty() {
        for channel_id in &deleted_channel_ids {
            storage::delete_channel(state.db_path(), channel_id.clone()).await?;
        }
        remove_channels_from_cache(state, &deleted_channel_ids);
    }
    if !failures.is_empty() {
        if relogin_required && deleted_channel_ids.is_empty() {
            return Err(ApiError::bad_gateway(
                "remote_relogin_required",
                "sub2api login expired, please sign in again",
            ));
        }
        return Err(ApiError::bad_gateway(
            "remote_delete_partial_failed",
            format!(
                "Remote delete failed for some managed channels; account was kept. {}",
                failures.join(" ; ")
            ),
        ));
    }
    Ok(())
}

fn detect_provider_from_base_url(raw: &str) -> Result<String, ApiError> {
    normalize_http_url_preserving_path(raw, "remote_base_url_required").map_err(|err| match err {
        ApiError::BadRequest { code: _, message } => {
            ApiError::bad_request("remote_base_url_invalid", message)
        }
        other => other,
    })
}

fn detect_recommended_api_url(
    provider: RemoteAccountProvider,
    base_url: &str,
    detected_api_url: Option<String>,
) -> Option<String> {
    let detected = detected_api_url
        .as_deref()
        .and_then(|value| resolve_remote_url_from_base(base_url, value));
    match provider {
        RemoteAccountProvider::Newapi => detected,
        RemoteAccountProvider::Sub2Api => None,
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
        RemoteAccountProvider::Sub2Api => None,
    }
}

async fn resolve_remote_account_with_secret(
    state: &AppState,
    account_id: &str,
) -> Result<storage::UnifiedRemoteAccount, ApiError> {
    storage::get_unified_remote_account_with_secret_optional(
        state.db_path(),
        account_id.to_string(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("remote_account_not_found", "Remote account not found"))
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
        reauth_required: false,
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

impl From<storage::NewApiAccount> for RemoteAccountResponse {
    fn from(account: storage::NewApiAccount) -> Self {
        let common = map_newapi_common(&account);
        Self::Newapi {
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
}

impl From<storage::RemoteAccount> for RemoteAccountResponse {
    fn from(account: storage::RemoteAccount) -> Self {
        let common = RemoteAccountCommonResponse {
            id: account.id,
            base_url: account.base_url,
            api_url: account.api_url,
            user_id: account.remote_user_id.unwrap_or_default(),
            user_token_configured: account.access_token_configured,
            reauth_required: account.reauth_required,
            page_checkin_url: account.page_checkin_url,
            checkin_mode: normalize_sub2api_checkin_mode_from_storage(account.checkin_mode),
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
        Self::Sub2Api {
            common,
            sub2api: Sub2ApiRemoteAccountResponse {
                remote_role_text: account.remote_role,
            },
        }
    }
}

impl From<storage::UnifiedRemoteAccount> for RemoteAccountResponse {
    fn from(account: storage::UnifiedRemoteAccount) -> Self {
        match account {
            storage::UnifiedRemoteAccount::Newapi(account) => account.into(),
            storage::UnifiedRemoteAccount::Sub2Api(account) => account.into(),
        }
    }
}

fn remote_provider_mismatch_error() -> ApiError {
    ApiError::bad_request(
        "remote_provider_mismatch",
        "provider does not match account type",
    )
}

fn remote_credentials_required_error() -> ApiError {
    ApiError::bad_request("remote_credentials_required", "bearer_token is required")
}

fn newapi_credentials_required_error() -> ApiError {
    ApiError::bad_request(
        "newapi_credentials_required",
        "user_id and user_token are required for this action",
    )
}

fn remote_key_unsupported_provider_error() -> ApiError {
    ApiError::bad_request(
        "remote_key_unsupported_provider",
        "Only sub2api accounts support key creation",
    )
}

fn remote_group_not_found_error() -> ApiError {
    ApiError::bad_request(
        "remote_group_not_found",
        "selected remote group was not found",
    )
}

fn remote_system_checkin_unsupported_provider_error() -> ApiError {
    ApiError::bad_request(
        "remote_checkin_unsupported_provider",
        "System API check-in is only supported for newapi accounts",
    )
}

fn ensure_provider_matches(
    requested: Option<RemoteAccountProvider>,
    actual: RemoteAccountProvider,
) -> Result<(), ApiError> {
    if requested.unwrap_or(actual) != actual {
        return Err(remote_provider_mismatch_error());
    }
    Ok(())
}

fn normalize_newapi_create_checkin_mode(
    mode: Option<RemoteAccountCheckinMode>,
) -> (storage::NewApiAccountCheckinMode, bool) {
    match mode.unwrap_or(RemoteAccountCheckinMode::Disabled) {
        RemoteAccountCheckinMode::PageOpen => (storage::NewApiAccountCheckinMode::PageOpen, false),
        RemoteAccountCheckinMode::Disabled => (storage::NewApiAccountCheckinMode::SystemApi, false),
        RemoteAccountCheckinMode::SystemApi => (storage::NewApiAccountCheckinMode::SystemApi, true),
    }
}

fn normalize_newapi_update_checkin_mode(
    mode: Option<RemoteAccountCheckinMode>,
) -> (Option<storage::NewApiAccountCheckinMode>, Option<bool>) {
    match mode {
        Some(RemoteAccountCheckinMode::PageOpen) => (
            Some(storage::NewApiAccountCheckinMode::PageOpen),
            Some(false),
        ),
        Some(RemoteAccountCheckinMode::Disabled) => (
            Some(storage::NewApiAccountCheckinMode::SystemApi),
            Some(false),
        ),
        Some(RemoteAccountCheckinMode::SystemApi) => (
            Some(storage::NewApiAccountCheckinMode::SystemApi),
            Some(true),
        ),
        None => (None, None),
    }
}

fn normalize_sub2api_checkin_mode_to_storage(
    mode: RemoteAccountCheckinMode,
) -> storage::RemoteAccountCheckinMode {
    match mode {
        RemoteAccountCheckinMode::Disabled | RemoteAccountCheckinMode::SystemApi => {
            storage::RemoteAccountCheckinMode::Disabled
        }
        RemoteAccountCheckinMode::PageOpen => storage::RemoteAccountCheckinMode::PageOpen,
    }
}

fn normalize_sub2api_checkin_mode_from_storage(
    mode: storage::RemoteAccountCheckinMode,
) -> RemoteAccountCheckinMode {
    match mode {
        storage::RemoteAccountCheckinMode::Disabled => RemoteAccountCheckinMode::Disabled,
        storage::RemoteAccountCheckinMode::PageOpen => RemoteAccountCheckinMode::PageOpen,
    }
}

fn require_sub2api_access_token(account: &storage::RemoteAccount) -> Result<&str, ApiError> {
    account
        .access_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(remote_credentials_required_error)
}

fn map_create_newapi_account_error(err: anyhow::Error) -> ApiError {
    match err.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::NewApiAccountAlreadyExists { .. }) => {
            ApiError::conflict("newapi_account_exists", "New API account already exists")
        }
        _ => ApiError::Internal(err),
    }
}

fn map_update_newapi_account_error(err: anyhow::Error) -> ApiError {
    match err.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::NewApiAccountNotFound { .. }) => {
            ApiError::not_found("newapi_account_not_found", "New API account not found")
        }
        Some(storage::StorageError::NewApiAccountAlreadyExists { .. }) => {
            ApiError::conflict("newapi_account_exists", "New API account already exists")
        }
        _ => ApiError::Internal(err),
    }
}

fn map_delete_newapi_account_error(err: anyhow::Error) -> ApiError {
    match err.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::NewApiAccountNotFound { .. }) => {
            ApiError::not_found("newapi_account_not_found", "New API account not found")
        }
        _ => ApiError::Internal(err),
    }
}

fn map_complete_newapi_checkin_error(err: &anyhow::Error) -> Option<ApiError> {
    matches!(
        err.downcast_ref::<storage::StorageError>(),
        Some(storage::StorageError::NewApiAccountNotFound { .. })
    )
    .then(|| ApiError::not_found("newapi_account_not_found", "New API account not found"))
}

fn map_complete_sub2api_checkin_error(err: &anyhow::Error) -> Option<ApiError> {
    matches!(
        err.downcast_ref::<storage::StorageError>(),
        Some(storage::StorageError::RemoteAccountNotFound { .. })
    )
    .then(|| ApiError::not_found("remote_account_not_found", "Remote account not found"))
}

fn map_create_sub2api_account_error(err: anyhow::Error) -> ApiError {
    match err.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::RemoteAccountAlreadyExists { .. }) => {
            ApiError::conflict("remote_account_exists", "Remote account already exists")
        }
        _ => ApiError::Internal(err),
    }
}

fn map_update_sub2api_account_error(err: anyhow::Error) -> ApiError {
    match err.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::RemoteAccountNotFound { .. }) => {
            ApiError::not_found("remote_account_not_found", "Remote account not found")
        }
        Some(storage::StorageError::RemoteAccountAlreadyExists { .. }) => {
            ApiError::conflict("remote_account_exists", "Remote account already exists")
        }
        _ => ApiError::Internal(err),
    }
}

fn map_sub2api_action_error(
    err: anyhow::Error,
    code: &'static str,
    action: &'static str,
) -> ApiError {
    if let Some(message) = sub2api_auth::relogin_required_message(&err) {
        return ApiError::bad_gateway("remote_relogin_required", message.to_string());
    }
    if let Some(request_err) = err.downcast_ref::<sub2api_client::Sub2ApiRequestError>() {
        return ApiError::bad_gateway(
            code,
            format!("Failed to {action}: {}", request_err.message()),
        );
    }
    ApiError::Internal(err)
}

fn count_managed_channels_by_group_name(channels: &[storage::Channel]) -> HashMap<String, usize> {
    channels.iter().fold(HashMap::new(), |mut acc, channel| {
        if let Some(group_name) = channel.managed_group_name() {
            *acc.entry(group_name.to_string()).or_default() += 1;
        }
        acc
    })
}

fn count_sub2api_managed_channels(
    channels: &[storage::Channel],
) -> (HashMap<i64, usize>, HashMap<String, usize>) {
    let mut counts_by_id = HashMap::<i64, usize>::new();
    let mut counts_by_name = HashMap::<String, usize>::new();
    for channel in channels {
        if let Some(group_id) = channel.managed_group_id() {
            *counts_by_id.entry(group_id).or_default() += 1;
        } else if let Some(group_name) = channel.managed_group_name() {
            *counts_by_name.entry(group_name.to_string()).or_default() += 1;
        }
    }
    (counts_by_id, counts_by_name)
}

fn map_sub2api_key_response(key: sub2api_client::Sub2ApiKey) -> RemoteKeyResponse {
    RemoteKeyResponse {
        id: key.id,
        key: key.key,
        name: key.name,
        group_id: key.group_id,
        status: key.status,
    }
}

fn find_sub2api_group<'a>(
    groups: &'a [sub2api_client::Sub2ApiGroupOption],
    input: &PreparedCreateRemoteManagedChannelInput,
) -> Result<&'a sub2api_client::Sub2ApiGroupOption, ApiError> {
    let selected_group = if let Some(group_id) = input.group_id {
        groups
            .iter()
            .find(|item| item.id == group_id)
            .or_else(|| groups.iter().find(|item| item.name == input.group_name))
    } else {
        groups.iter().find(|item| item.name == input.group_name)
    };
    selected_group.ok_or_else(remote_group_not_found_error)
}

async fn sync_delete_managed_channels_if_requested(
    state: &AppState,
    provider: storage::ManagedRemoteProvider,
    account_id: &str,
    options: DeleteRemoteAccountOptions,
) -> Result<(), ApiError> {
    if options.delete_managed_channels && options.sync_remote_delete {
        sync_delete_managed_channels_for_account(state, provider, account_id).await?;
    }
    Ok(())
}

fn finalize_remote_account_delete(
    state: &AppState,
    provider: storage::ManagedRemoteProvider,
    account_id: &str,
    options: DeleteRemoteAccountOptions,
    result: &storage::DeleteNewApiAccountResult,
) {
    if options.delete_managed_channels {
        remove_channels_from_cache(state, &result.deleted_managed_channel_ids);
    } else {
        clear_managed_links_in_cache(state, provider, account_id);
    }
    newapi_handlers::notify_background_tasks(state);
}

async fn create_newapi_remote_account_impl(
    state: &AppState,
    input: CreateRemoteAccountInput,
) -> Result<RemoteAccountResponse, ApiError> {
    let user_id = input.user_id.unwrap_or_default();
    let user_token = input.user_token.unwrap_or_default();
    let (request_checkin_mode, auto_checkin_enabled) =
        normalize_newapi_create_checkin_mode(input.checkin_mode);
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
        .map_err(map_create_newapi_account_error)?;
    let account =
        newapi_handlers::sync_account_if_possible(state, account.id.clone(), &candidate).await?;
    newapi_handlers::notify_background_tasks(state);
    Ok(RemoteAccountResponse::from(account))
}

async fn create_sub2api_remote_account_impl(
    state: &AppState,
    input: CreateRemoteAccountInput,
) -> Result<RemoteAccountResponse, ApiError> {
    let base_url = detect_provider_from_base_url(&input.base_url)?;
    let mut session = sub2api_auth::InMemorySub2ApiSession {
        access_token: input.bearer_token.unwrap_or_default(),
        refresh_token: normalize_optional_bearer_token(input.refresh_token),
    };
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
        !session.access_token.trim().is_empty(),
    )?;
    let overview = sub2api_auth::run_with_inmemory_session(
        &state.http_client,
        &base_url,
        &mut session,
        |http_client, base_url, access_token| {
            Box::pin(sub2api_client::fetch_account_overview(
                http_client,
                base_url,
                access_token,
            ))
        },
    )
    .await
    .map_err(|err| {
        map_sub2api_action_error(err, "remote_sync_failed", "validate sub2api account")
    })?;
    let account = storage::create_remote_account(
        state.db_path(),
        storage::CreateRemoteAccount {
            provider: storage::RemoteAccountProvider::Sub2Api,
            base_url,
            api_url: input.api_url,
            access_token: session.access_token,
            refresh_token: session.refresh_token,
            page_checkin_url: input.page_checkin_url,
            checkin_mode: Some(normalize_sub2api_checkin_mode_to_storage(checkin_mode)),
            auto_checkin_time: Some(auto_checkin_time),
            low_balance_alert_threshold: input.low_balance_alert_threshold,
            recharge_currency: input.recharge_currency,
        },
    )
    .await
    .map_err(map_create_sub2api_account_error)?;
    let account = persist_sub2api_overview(state, account, &overview).await?;
    Ok(RemoteAccountResponse::from(account))
}

async fn update_newapi_remote_account_impl(
    state: &AppState,
    account_id: String,
    current: storage::NewApiAccount,
    input: UpdateRemoteAccountInput,
) -> Result<RemoteAccountResponse, ApiError> {
    ensure_provider_matches(input.provider, RemoteAccountProvider::Newapi)?;
    let (request_checkin_mode, auto_checkin_enabled) =
        normalize_newapi_update_checkin_mode(input.checkin_mode);
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
        .map_err(map_update_newapi_account_error)?;
    let account = newapi_handlers::sync_account_if_possible(state, account_id, &candidate).await?;
    newapi_handlers::notify_background_tasks(state);
    Ok(RemoteAccountResponse::from(account))
}

async fn update_sub2api_remote_account_impl(
    state: &AppState,
    account_id: String,
    current: storage::RemoteAccount,
    input: UpdateRemoteAccountInput,
) -> Result<RemoteAccountResponse, ApiError> {
    ensure_provider_matches(input.provider, RemoteAccountProvider::Sub2Api)?;
    let effective_base_url = input
        .base_url
        .as_deref()
        .map(detect_provider_from_base_url)
        .transpose()?
        .unwrap_or_else(|| current.base_url.clone());
    let mut session = sub2api_auth::InMemorySub2ApiSession {
        access_token: input
            .bearer_token
            .clone()
            .or_else(|| current.access_token.clone())
            .unwrap_or_default(),
        refresh_token: if input.refresh_token.is_some() {
            normalize_optional_bearer_token(input.refresh_token.clone())
        } else {
            current.refresh_token.clone()
        },
    };
    let effective_checkin_mode =
        input
            .checkin_mode
            .unwrap_or(normalize_sub2api_checkin_mode_from_storage(
                current.checkin_mode,
            ));
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
        !session.access_token.trim().is_empty(),
    )?;
    let overview = sub2api_auth::run_with_inmemory_session(
        &state.http_client,
        &effective_base_url,
        &mut session,
        |http_client, base_url, access_token| {
            Box::pin(sub2api_client::fetch_account_overview(
                http_client,
                base_url,
                access_token,
            ))
        },
    )
    .await
    .map_err(|err| {
        map_sub2api_action_error(err, "remote_sync_failed", "validate sub2api account")
    })?;
    let account = storage::update_remote_account(
        state.db_path(),
        account_id,
        storage::UpdateRemoteAccount {
            base_url: Some(effective_base_url),
            api_url: input.api_url,
            access_token: Some(session.access_token.clone()),
            refresh_token: Some(session.refresh_token.clone().unwrap_or_default()),
            page_checkin_url: input.page_checkin_url,
            checkin_mode: Some(normalize_sub2api_checkin_mode_to_storage(
                effective_checkin_mode,
            )),
            auto_checkin_time: Some(effective_auto_checkin_time),
            low_balance_alert_threshold: Some(effective_threshold),
            recharge_currency: input.recharge_currency,
        },
    )
    .await
    .map_err(map_update_sub2api_account_error)?;
    let account = persist_sub2api_overview(state, account, &overview).await?;
    Ok(RemoteAccountResponse::from(account))
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

fn build_sub2api_snapshot(
    overview: &sub2api_client::Sub2ApiAccountOverview,
    synced_at_ms: i64,
) -> storage::RemoteAccountRemoteSnapshot {
    storage::RemoteAccountRemoteSnapshot {
        remote_user_id: Some(overview.remote_user_id.to_string()),
        remote_role: overview.remote_role.clone(),
        remote_username: overview.remote_username.clone(),
        remote_display_name: overview.remote_display_name.clone(),
        last_balance_amount: overview.balance,
        last_synced_at_ms: Some(synced_at_ms),
    }
}

fn apply_sub2api_overview_to_account(
    account: &mut storage::RemoteAccount,
    overview: &sub2api_client::Sub2ApiAccountOverview,
    synced_at_ms: i64,
) {
    account.remote_user_id = Some(overview.remote_user_id.to_string());
    account.remote_role = overview.remote_role.clone();
    account.remote_username = overview.remote_username.clone();
    account.remote_display_name = overview.remote_display_name.clone();
    account.last_balance_amount = overview.balance;
    account.last_sync_error = None;
    account.reauth_required = false;
    account.last_synced_at_ms = Some(synced_at_ms);
    account.updated_at_ms = synced_at_ms;
}

async fn persist_sub2api_overview(
    state: &AppState,
    mut account: storage::RemoteAccount,
    overview: &sub2api_client::Sub2ApiAccountOverview,
) -> Result<storage::RemoteAccount, ApiError> {
    let synced_at_ms = storage::now_ms();
    storage::apply_remote_account_sync_success(
        state.db_path(),
        account.id.clone(),
        build_sub2api_snapshot(overview, synced_at_ms),
    )
    .await?;
    apply_sub2api_overview_to_account(&mut account, overview, synced_at_ms);
    Ok(account)
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
        false,
    )
    .await?;
    Ok(())
}

async fn refresh_newapi_remote_account_impl(
    state: &AppState,
    account: storage::NewApiAccount,
) -> Result<RemoteAccountResponse, ApiError> {
    if !newapi_client::account_has_user_api_credentials(&account) {
        let cleared =
            newapi_handlers::clear_account_remote_state(state, account.id.clone()).await?;
        return Ok(RemoteAccountResponse::from(cleared));
    }
    let overview = match newapi_client::fetch_account_overview(&state.http_client, &account).await {
        Ok(overview) => overview,
        Err(err) => {
            let _ =
                newapi_handlers::record_account_sync_failure(state, account.id.clone(), &err).await;
            return Err(newapi_handlers::sync_error(err));
        }
    };
    let account =
        newapi_handlers::apply_account_overview(state, account.id.clone(), &overview).await?;
    Ok(RemoteAccountResponse::from(account))
}

async fn refresh_sub2api_remote_account_impl(
    state: &AppState,
    mut account: storage::RemoteAccount,
) -> Result<RemoteAccountResponse, ApiError> {
    let account_id = account.id.clone();
    require_sub2api_access_token(&account)?;
    let overview = sub2api_auth::run_with_persisted_session(
        state.db_path(),
        &state.http_client,
        &mut account,
        |http_client, base_url, access_token| {
            Box::pin(sub2api_client::fetch_account_overview(
                http_client,
                base_url,
                access_token,
            ))
        },
    )
    .await;
    match overview {
        Ok(overview) => {
            let account = persist_sub2api_overview(state, account, &overview).await?;
            Ok(RemoteAccountResponse::from(account))
        }
        Err(err) => {
            let anyhow_err = err;
            if sub2api_auth::relogin_required_message(&anyhow_err).is_none() {
                let _ = record_sub2api_sync_failure(state, account_id, &anyhow_err).await;
            }
            Err(map_sub2api_action_error(
                anyhow_err,
                "remote_sync_failed",
                "sync sub2api account",
            ))
        }
    }
}

async fn list_newapi_remote_account_groups_impl(
    state: &AppState,
    account: storage::NewApiAccount,
) -> Result<Vec<RemoteGroupResponse>, ApiError> {
    if !newapi_client::account_has_user_api_credentials(&account) {
        return Err(newapi_credentials_required_error());
    }
    let groups = newapi_client::list_groups(&state.http_client, &account)
        .await
        .map_err(newapi_handlers::sync_error)?;
    let channels = list_managed_channels_for_account(
        state,
        storage::ManagedRemoteProvider::Newapi,
        &account.id,
    )
    .await?;
    let managed_counts = count_managed_channels_by_group_name(&channels);
    Ok(groups
        .into_iter()
        .map(|item| RemoteGroupResponse {
            id: None,
            name: item.name.clone(),
            ratio: item.ratio,
            description: item.description,
            platform: None,
            managed_channel_count: managed_counts.get(&item.name).copied().unwrap_or(0),
        })
        .collect())
}

async fn list_sub2api_remote_account_groups_impl(
    state: &AppState,
    mut account: storage::RemoteAccount,
) -> Result<Vec<RemoteGroupResponse>, ApiError> {
    require_sub2api_access_token(&account)?;
    let groups = sub2api_auth::run_with_persisted_session(
        state.db_path(),
        &state.http_client,
        &mut account,
        |http_client, base_url, access_token| {
            Box::pin(sub2api_client::list_groups(
                http_client,
                base_url,
                access_token,
            ))
        },
    )
    .await
    .map_err(|err| {
        map_sub2api_action_error(err, "remote_groups_load_failed", "load sub2api groups")
    })?;
    let channels = list_managed_channels_for_account(
        state,
        storage::ManagedRemoteProvider::Sub2Api,
        &account.id,
    )
    .await?;
    let (managed_counts_by_id, managed_counts_by_name) = count_sub2api_managed_channels(&channels);
    Ok(groups
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
        .collect())
}

async fn create_sub2api_remote_account_key_impl(
    state: &AppState,
    mut account: storage::RemoteAccount,
    input: CreateRemoteKeyInput,
) -> Result<RemoteKeyResponse, ApiError> {
    require_sub2api_access_token(&account)?;
    let name = validate_remote_key_name(&input.name)?;
    let request = sub2api_client::CreateSub2ApiKeyRequest {
        name,
        group_id: input.group_id,
    };
    let key = sub2api_auth::run_with_persisted_session(
        state.db_path(),
        &state.http_client,
        &mut account,
        |http_client, base_url, access_token| {
            let request = request.clone();
            Box::pin(async move {
                sub2api_client::create_key(http_client, base_url, access_token, &request).await
            })
        },
    )
    .await
    .map_err(|err| {
        map_sub2api_action_error(err, "remote_key_create_failed", "create remote key")
    })?;
    Ok(map_sub2api_key_response(key))
}

async fn create_newapi_remote_managed_channel_impl(
    state: &AppState,
    account: storage::NewApiAccount,
    input: PreparedCreateRemoteManagedChannelInput,
) -> Result<CreateRemoteManagedChannelResponse, ApiError> {
    if !newapi_client::account_has_user_api_credentials(&account) {
        return Err(newapi_credentials_required_error());
    }
    let PreparedCreateRemoteManagedChannelInput {
        name,
        protocol,
        group_name,
        group_id: _,
        base_url_override,
        priority,
        enabled,
    } = input;
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
        priority,
        retry_times: 1,
        ignore_channel_protection: false,
        recharge_currency: Some(account.recharge_currency),
        real_multiplier: Some(remote.group_ratio),
        enabled,
        managed_by_remote: Some(true),
        managed_remote_provider: Some(storage::ManagedRemoteProvider::Newapi),
        managed_remote_account_id: Some(account.id.clone()),
        managed_remote_resource_id: Some(remote.token_id.to_string()),
        managed_remote_resource_name: Some(remote.token_name.clone()),
        managed_remote_group_name: Some(remote.group_name.clone()),
        managed_remote_group_id: None,
    };
    let channel = match storage::create_channel(state.db_path(), create_local).await {
        Ok(channel) => channel,
        Err(err) => {
            let _ =
                newapi_client::delete_token(&state.http_client, &account, remote.token_id).await;
            return Err(ApiError::Internal(err));
        }
    };
    add_channel_to_cache(state, &channel);
    publish_remote_managed_channel_created_if_enabled(
        state,
        storage::ManagedRemoteProvider::Newapi,
        &account.id,
        &account.base_url,
        &channel,
    );
    Ok(CreateRemoteManagedChannelResponse { channel })
}

async fn create_sub2api_remote_managed_channel_impl(
    state: &AppState,
    mut account: storage::RemoteAccount,
    input: PreparedCreateRemoteManagedChannelInput,
) -> Result<CreateRemoteManagedChannelResponse, ApiError> {
    require_sub2api_access_token(&account)?;
    let groups = sub2api_auth::run_with_persisted_session(
        state.db_path(),
        &state.http_client,
        &mut account,
        |http_client, base_url, access_token| {
            Box::pin(sub2api_client::list_groups(
                http_client,
                base_url,
                access_token,
            ))
        },
    )
    .await
    .map_err(|err| {
        map_sub2api_action_error(err, "remote_groups_load_failed", "load sub2api groups")
    })?;
    let selected_group = find_sub2api_group(&groups, &input)?;
    let selected_group_id = selected_group.id;
    let selected_group_name = selected_group.name.clone();
    let selected_group_ratio = selected_group.rate_multiplier.unwrap_or(1.0);
    let PreparedCreateRemoteManagedChannelInput {
        name,
        protocol,
        group_name: _,
        group_id: _,
        base_url_override,
        priority,
        enabled,
    } = input;
    let request = sub2api_client::CreateSub2ApiKeyRequest {
        name: name.clone(),
        group_id: Some(selected_group_id),
    };
    let created_key = sub2api_auth::run_with_persisted_session(
        state.db_path(),
        &state.http_client,
        &mut account,
        |http_client, base_url, access_token| {
            let request = request.clone();
            Box::pin(async move {
                sub2api_client::create_key(http_client, base_url, access_token, &request).await
            })
        },
    )
    .await
    .map_err(|err| {
        map_sub2api_action_error(
            err,
            "remote_managed_channel_create_failed",
            "create remote managed channel",
        )
    })?;
    let create_local = storage::CreateChannel {
        name,
        protocol,
        base_url: managed_channel_base_url_for_sub2api(&account, base_url_override),
        auth_type: Some("auto".to_string()),
        auth_ref: created_key.key.clone(),
        checkin_url: None,
        priority,
        retry_times: 1,
        ignore_channel_protection: false,
        recharge_currency: Some(account.recharge_currency),
        real_multiplier: Some(selected_group_ratio),
        enabled,
        managed_by_remote: Some(true),
        managed_remote_provider: Some(storage::ManagedRemoteProvider::Sub2Api),
        managed_remote_account_id: Some(account.id.clone()),
        managed_remote_resource_id: Some(created_key.id.to_string()),
        managed_remote_resource_name: Some(created_key.name.clone()),
        managed_remote_group_name: Some(selected_group_name),
        managed_remote_group_id: Some(selected_group_id),
    };
    let channel = match storage::create_channel(state.db_path(), create_local).await {
        Ok(channel) => channel,
        Err(err) => {
            let _ = sub2api_auth::run_with_persisted_session(
                state.db_path(),
                &state.http_client,
                &mut account,
                |http_client, base_url, access_token| {
                    Box::pin(sub2api_client::delete_key(
                        http_client,
                        base_url,
                        access_token,
                        created_key.id,
                    ))
                },
            )
            .await;
            return Err(ApiError::Internal(err));
        }
    };
    add_channel_to_cache(state, &channel);
    publish_remote_managed_channel_created_if_enabled(
        state,
        storage::ManagedRemoteProvider::Sub2Api,
        &account.id,
        &account.base_url,
        &channel,
    );
    Ok(CreateRemoteManagedChannelResponse { channel })
}

async fn delete_newapi_remote_account_impl(
    state: &AppState,
    account_id: String,
    options: DeleteRemoteAccountOptions,
) -> Result<storage::DeleteNewApiAccountResult, ApiError> {
    sync_delete_managed_channels_if_requested(
        state,
        storage::ManagedRemoteProvider::Newapi,
        &account_id,
        options,
    )
    .await?;
    let result = storage::delete_newapi_account(
        state.db_path(),
        account_id.clone(),
        options.delete_managed_channels,
    )
    .await
    .map_err(map_delete_newapi_account_error)?;
    finalize_remote_account_delete(
        state,
        storage::ManagedRemoteProvider::Newapi,
        &account_id,
        options,
        &result,
    );
    Ok(result)
}

async fn delete_sub2api_remote_account_impl(
    state: &AppState,
    account_id: String,
    options: DeleteRemoteAccountOptions,
) -> Result<storage::DeleteNewApiAccountResult, ApiError> {
    let linked_channel_ids = storage::list_channel_ids_by_managed_account(
        state.db_path(),
        storage::ManagedRemoteProvider::Sub2Api,
        account_id.clone(),
    )
    .await?;
    sync_delete_managed_channels_if_requested(
        state,
        storage::ManagedRemoteProvider::Sub2Api,
        &account_id,
        options,
    )
    .await?;
    let result = if options.delete_managed_channels {
        for channel_id in &linked_channel_ids {
            storage::delete_channel(state.db_path(), channel_id.clone()).await?;
        }
        storage::delete_remote_account(state.db_path(), account_id.clone()).await?;
        storage::DeleteNewApiAccountResult {
            deleted_managed_channel_ids: linked_channel_ids,
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
    finalize_remote_account_delete(
        state,
        storage::ManagedRemoteProvider::Sub2Api,
        &account_id,
        options,
        &result,
    );
    Ok(result)
}

async fn complete_newapi_remote_account_checkin_today_impl(
    state: &AppState,
    account_id: String,
) -> Result<axum::http::StatusCode, ApiError> {
    let res =
        storage::complete_newapi_account_checkin_today(state.db_path(), account_id, "manual_page")
            .await;
    map_storage_unit_no_content_err(res, map_complete_newapi_checkin_error)
}

async fn complete_sub2api_remote_account_checkin_today_impl(
    state: &AppState,
    account_id: String,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = storage::complete_remote_account_checkin_today(state.db_path(), account_id).await;
    map_storage_unit_no_content_err(res, map_complete_sub2api_checkin_error)
}

async fn perform_newapi_remote_account_system_checkin_impl(
    state: AppState,
    account_id: String,
) -> Result<axum::response::Response, ApiError> {
    newapi_handlers::perform_newapi_account_system_checkin(
        State(state),
        axum::extract::Path(account_id),
    )
    .await
    .map(IntoResponse::into_response)
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
    let items = storage::list_all_remote_accounts(state.db_path())
        .await?
        .into_iter()
        .map(RemoteAccountResponse::from)
        .collect::<Vec<_>>();
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

    let accounts = storage::list_all_remote_accounts(state.db_path()).await?;
    if accounts.len() != input.account_ids.len() {
        return Err(ApiError::bad_request(
            "remote_account_ids_mismatch",
            "account_ids must cover all accounts",
        ));
    }

    let accounts_by_id = accounts
        .into_iter()
        .map(|account| (account.id().to_string(), account.managed_provider()))
        .collect::<HashMap<_, _>>();

    let mut newapi_orders = Vec::new();
    let mut remote_orders = Vec::new();
    for (index, account_id) in input.account_ids.iter().enumerate() {
        let sort_order = index as i64;
        match accounts_by_id.get(account_id).copied() {
            Some(storage::ManagedRemoteProvider::Newapi) => {
                newapi_orders.push((account_id.clone(), sort_order));
            }
            Some(storage::ManagedRemoteProvider::Sub2Api) => {
                remote_orders.push((account_id.clone(), sort_order));
            }
            None => {
                return Err(ApiError::bad_request(
                    "remote_account_ids_mismatch",
                    "account_ids contains unknown account",
                ));
            }
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
    let checkins = storage::get_all_remote_accounts_checkins_today(state.db_path()).await?;
    Ok(Json(checkins))
}

pub(in crate::server) async fn create_remote_account(
    State(state): State<AppState>,
    Json(input): Json<CreateRemoteAccountInput>,
) -> Result<impl IntoResponse, ApiError> {
    let response = match input.provider {
        RemoteAccountProvider::Newapi => create_newapi_remote_account_impl(&state, input).await?,
        RemoteAccountProvider::Sub2Api => create_sub2api_remote_account_impl(&state, input).await?,
    };
    Ok((axum::http::StatusCode::CREATED, Json(response)))
}

pub(in crate::server) async fn update_remote_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    Json(input): Json<UpdateRemoteAccountInput>,
) -> Result<impl IntoResponse, ApiError> {
    let response = match resolve_remote_account_with_secret(&state, &account_id).await? {
        storage::UnifiedRemoteAccount::Newapi(current) => {
            update_newapi_remote_account_impl(&state, account_id, current, input).await?
        }
        storage::UnifiedRemoteAccount::Sub2Api(current) => {
            update_sub2api_remote_account_impl(&state, account_id, current, input).await?
        }
    };
    Ok(Json(response))
}

pub(in crate::server) async fn refresh_remote_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let response = match resolve_remote_account_with_secret(&state, &account_id).await? {
        storage::UnifiedRemoteAccount::Newapi(account) => {
            refresh_newapi_remote_account_impl(&state, account).await?
        }
        storage::UnifiedRemoteAccount::Sub2Api(account) => {
            refresh_sub2api_remote_account_impl(&state, account).await?
        }
    };
    Ok(Json(response))
}

pub(in crate::server) async fn list_remote_account_groups(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let groups = match resolve_remote_account_with_secret(&state, &account_id).await? {
        storage::UnifiedRemoteAccount::Newapi(account) => {
            list_newapi_remote_account_groups_impl(&state, account).await?
        }
        storage::UnifiedRemoteAccount::Sub2Api(account) => {
            list_sub2api_remote_account_groups_impl(&state, account).await?
        }
    };
    Ok(Json(groups))
}

pub(in crate::server) async fn complete_remote_account_checkin_today(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        storage::UnifiedRemoteAccount::Newapi(_) => {
            complete_newapi_remote_account_checkin_today_impl(&state, account_id).await
        }
        storage::UnifiedRemoteAccount::Sub2Api(_) => {
            complete_sub2api_remote_account_checkin_today_impl(&state, account_id).await
        }
    }
}

pub(in crate::server) async fn perform_remote_account_system_checkin(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    match resolve_remote_account_with_secret(&state, &account_id).await? {
        storage::UnifiedRemoteAccount::Newapi(_) => {
            perform_newapi_remote_account_system_checkin_impl(state, account_id).await
        }
        storage::UnifiedRemoteAccount::Sub2Api(_) => {
            Err(remote_system_checkin_unsupported_provider_error())
        }
    }
}

pub(in crate::server) async fn create_remote_account_key(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    Json(input): Json<CreateRemoteKeyInput>,
) -> Result<impl IntoResponse, ApiError> {
    let response = match resolve_remote_account_with_secret(&state, &account_id).await? {
        storage::UnifiedRemoteAccount::Newapi(_) => {
            return Err(remote_key_unsupported_provider_error());
        }
        storage::UnifiedRemoteAccount::Sub2Api(account) => {
            create_sub2api_remote_account_key_impl(&state, account, input).await?
        }
    };
    Ok(Json(response))
}

pub(in crate::server) async fn create_remote_managed_channel(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    Json(input): Json<CreateRemoteManagedChannelInput>,
) -> Result<impl IntoResponse, ApiError> {
    let input = prepare_create_remote_managed_channel_input(input)?;
    let response = match resolve_remote_account_with_secret(&state, &account_id).await? {
        storage::UnifiedRemoteAccount::Newapi(account) => {
            create_newapi_remote_managed_channel_impl(&state, account, input).await?
        }
        storage::UnifiedRemoteAccount::Sub2Api(account) => {
            create_sub2api_remote_managed_channel_impl(&state, account, input).await?
        }
    };
    Ok((axum::http::StatusCode::CREATED, Json(response)))
}

pub(in crate::server) async fn delete_remote_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    input: Option<Json<DeleteRemoteAccountInput>>,
) -> Result<impl IntoResponse, ApiError> {
    let options =
        parse_delete_remote_account_options(input.map(|Json(input)| input).unwrap_or_default())?;
    let result = match resolve_remote_account_with_secret(&state, &account_id).await? {
        storage::UnifiedRemoteAccount::Newapi(_) => {
            delete_newapi_remote_account_impl(&state, account_id, options).await?
        }
        storage::UnifiedRemoteAccount::Sub2Api(_) => {
            delete_sub2api_remote_account_impl(&state, account_id, options).await?
        }
    };
    Ok(Json(result).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_bridge::weixin::{WeixinControl, WeixinStatus};
    use crate::chat_bridge::whatsapp_web::{WhatsAppWebControl, WhatsAppWebStatus};
    use crate::update;
    use axum::body::to_bytes;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::{get, post};
    use axum::{Json as AxumJson, Router};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{mpsc, watch};

    fn remove_sqlite_artifacts(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    }

    fn temp_db_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-test-remote-handler-{}.sqlite",
            uuid::Uuid::new_v4()
        ))
    }

    async fn build_test_state(db_path: PathBuf) -> AppState {
        let settings = Arc::new(
            storage::get_app_settings(db_path.clone())
                .await
                .expect("load settings"),
        );
        let channels = Arc::new(
            storage::list_channels(db_path.clone())
                .await
                .expect("list channels"),
        );

        let (settings_notify, _) = watch::channel(0_u64);
        let (settings_cache, settings_cache_rx) = watch::channel(settings);
        let (channels_cache, channels_cache_rx) = watch::channel(channels);
        let (whatsapp_control_tx, _) = mpsc::channel::<WhatsAppWebControl>(1);
        let (_, whatsapp_status_rx) = watch::channel(WhatsAppWebStatus::default());
        let (weixin_control_tx, _) = mpsc::channel::<WeixinControl>(1);
        let (_, weixin_status_rx) = watch::channel(WeixinStatus::default());

        AppState {
            listen_addr: "127.0.0.1:0".parse().expect("listen addr"),
            db_path: Arc::new(db_path),
            http_client: reqwest::Client::new(),
            proxy_http_client: reqwest::Client::new(),
            settings_notify,
            settings_cache,
            settings_cache_rx,
            channels_cache,
            channels_cache_rx,
            update_runtime: Arc::new(tokio::sync::Mutex::new(update::UpdateRuntime::default())),
            whatsapp_control_tx,
            whatsapp_status_rx,
            weixin_control_tx,
            weixin_status_rx,
        }
    }

    async fn spawn_sub2api_detect_server() -> String {
        let app = Router::new().route(
            "/tenant/api/v1/settings/public",
            get(|| async {
                AxumJson(serde_json::json!({
                    "code": 0,
                    "message": "",
                    "data": {
                        "api_base_url": "openapi/v1",
                        "site_name": "Demo",
                        "backend_mode_enabled": true
                    }
                }))
            }),
        );

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind detect server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://127.0.0.1:{}/tenant/", addr.port())
    }

    async fn spawn_sub2api_overview_server() -> (String, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_handler = calls.clone();
        let app = Router::new().route(
            "/api/v1/auth/me",
            get(move || {
                let calls = calls_for_handler.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    AxumJson(serde_json::json!({
                        "code": 0,
                        "message": "",
                        "data": {
                            "id": 42,
                            "email": "demo@example.com",
                            "username": "demo-user",
                            "role": "admin",
                            "balance": 12.5
                        }
                    }))
                }
            }),
        );

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind overview server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://127.0.0.1:{}", addr.port()), calls)
    }

    fn authorization_value(headers: &HeaderMap) -> Option<&str> {
        headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        serde_json::from_slice(&body).expect("parse json")
    }

    async fn spawn_sub2api_refresh_success_server() -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>)
    {
        let auth_me_calls = Arc::new(AtomicUsize::new(0));
        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let auth_me_calls_for_handler = auth_me_calls.clone();
        let refresh_calls_for_handler = refresh_calls.clone();
        let app = Router::new()
            .route(
                "/api/v1/auth/me",
                get(move |headers: HeaderMap| {
                    let auth_me_calls = auth_me_calls_for_handler.clone();
                    async move {
                        auth_me_calls.fetch_add(1, Ordering::SeqCst);
                        match authorization_value(&headers) {
                            Some("Bearer expired-access") => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "code": "TOKEN_EXPIRED",
                                    "message": "Token has expired"
                                })),
                            )
                                .into_response(),
                            Some("Bearer rotated-access") => (
                                StatusCode::OK,
                                AxumJson(serde_json::json!({
                                    "code": 0,
                                    "message": "",
                                    "data": {
                                        "id": 42,
                                        "email": "demo@example.com",
                                        "username": "demo-user",
                                        "role": "admin",
                                        "balance": 1.25
                                    }
                                })),
                            )
                                .into_response(),
                            Some(other) => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "code": "INVALID_TOKEN",
                                    "message": format!("unexpected authorization header: {other}")
                                })),
                            )
                                .into_response(),
                            None => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "code": "INVALID_TOKEN",
                                    "message": "missing authorization header"
                                })),
                            )
                                .into_response(),
                        }
                    }
                }),
            )
            .route(
                "/api/v1/subscriptions/progress",
                get(|headers: HeaderMap| async move {
                    match authorization_value(&headers) {
                        Some("Bearer rotated-access") => (
                            StatusCode::OK,
                            AxumJson(serde_json::json!({
                                "code": 0,
                                "message": "",
                                "data": [
                                    {
                                        "progress": {
                                            "daily": {
                                                "remaining_usd": 9.5
                                            }
                                        }
                                    }
                                ]
                            })),
                        )
                            .into_response(),
                        _ => (
                            StatusCode::UNAUTHORIZED,
                            AxumJson(serde_json::json!({
                                "code": "TOKEN_EXPIRED",
                                "message": "Token has expired"
                            })),
                        )
                            .into_response(),
                    }
                }),
            )
            .route(
                "/api/v1/auth/refresh",
                post(move |AxumJson(payload): AxumJson<serde_json::Value>| {
                    let refresh_calls = refresh_calls_for_handler.clone();
                    async move {
                        refresh_calls.fetch_add(1, Ordering::SeqCst);
                        match payload
                            .get("refresh_token")
                            .and_then(|value| value.as_str())
                        {
                            Some("good-refresh") => (
                                StatusCode::OK,
                                AxumJson(serde_json::json!({
                                    "code": 0,
                                    "message": "",
                                    "data": {
                                        "access_token": "rotated-access",
                                        "refresh_token": "rotated-refresh"
                                    }
                                })),
                            )
                                .into_response(),
                            Some(other) => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "reason": "REFRESH_TOKEN_INVALID",
                                    "message": format!("unexpected refresh token: {other}")
                                })),
                            )
                                .into_response(),
                            None => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "reason": "REFRESH_TOKEN_INVALID",
                                    "message": "missing refresh token"
                                })),
                            )
                                .into_response(),
                        }
                    }
                }),
            );

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind refresh success server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (
            format!("http://127.0.0.1:{}", addr.port()),
            auth_me_calls,
            refresh_calls,
        )
    }

    async fn spawn_sub2api_refresh_invalid_server() -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>)
    {
        let auth_me_calls = Arc::new(AtomicUsize::new(0));
        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let auth_me_calls_for_handler = auth_me_calls.clone();
        let refresh_calls_for_handler = refresh_calls.clone();
        let app = Router::new()
            .route(
                "/api/v1/auth/me",
                get(move |headers: HeaderMap| {
                    let auth_me_calls = auth_me_calls_for_handler.clone();
                    async move {
                        auth_me_calls.fetch_add(1, Ordering::SeqCst);
                        match authorization_value(&headers) {
                            Some("Bearer expired-access") => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "code": "TOKEN_EXPIRED",
                                    "message": "Token has expired"
                                })),
                            )
                                .into_response(),
                            Some(other) => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "code": "INVALID_TOKEN",
                                    "message": format!("unexpected authorization header: {other}")
                                })),
                            )
                                .into_response(),
                            None => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "code": "INVALID_TOKEN",
                                    "message": "missing authorization header"
                                })),
                            )
                                .into_response(),
                        }
                    }
                }),
            )
            .route(
                "/api/v1/auth/refresh",
                post(move |AxumJson(payload): AxumJson<serde_json::Value>| {
                    let refresh_calls = refresh_calls_for_handler.clone();
                    async move {
                        refresh_calls.fetch_add(1, Ordering::SeqCst);
                        match payload
                            .get("refresh_token")
                            .and_then(|value| value.as_str())
                        {
                            Some("stale-refresh") => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "code": 401,
                                    "reason": "REFRESH_TOKEN_EXPIRED",
                                    "message": "refresh token has expired"
                                })),
                            )
                                .into_response(),
                            Some(other) => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "reason": "REFRESH_TOKEN_INVALID",
                                    "message": format!("unexpected refresh token: {other}")
                                })),
                            )
                                .into_response(),
                            None => (
                                StatusCode::UNAUTHORIZED,
                                AxumJson(serde_json::json!({
                                    "reason": "REFRESH_TOKEN_INVALID",
                                    "message": "missing refresh token"
                                })),
                            )
                                .into_response(),
                        }
                    }
                }),
            );

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind refresh invalid server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (
            format!("http://127.0.0.1:{}", addr.port()),
            auth_me_calls,
            refresh_calls,
        )
    }

    async fn spawn_newapi_system_checkin_server() -> (String, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_checkin = calls.clone();
        let app = Router::new()
            .route(
                "/api/status",
                get(|| async {
                    AxumJson(serde_json::json!({
                        "success": true,
                        "message": "",
                        "data": {
                            "quota_display_type": "USD",
                            "quota_per_unit": 500000.0,
                            "usd_exchange_rate": 1.0,
                            "checkin_enabled": true,
                            "turnstile_check": false
                        }
                    }))
                }),
            )
            .route(
                "/api/user/self",
                get(|| async {
                    AxumJson(serde_json::json!({
                        "success": true,
                        "message": "",
                        "data": {
                            "role": 100,
                            "username": "demo-user",
                            "display_name": "Demo User",
                            "group": "default",
                            "quota": 2000000,
                            "used_quota": 500000
                        }
                    }))
                }),
            )
            .route(
                "/api/user/checkin",
                post(move || {
                    let calls = calls_for_checkin.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        AxumJson(serde_json::json!({
                            "success": true,
                            "message": "",
                            "data": {
                                "quota_awarded": 777,
                                "checkin_date": "2026-03-30"
                            }
                        }))
                    }
                }),
            );

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind newapi checkin server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://127.0.0.1:{}", addr.port()), calls)
    }

    #[tokio::test]
    async fn detect_remote_account_preserves_base_path_and_uses_public_api_base_url() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        let state = build_test_state(db_path.clone()).await;
        let base_url = spawn_sub2api_detect_server().await;

        let response = detect_remote_account(
            State(state),
            Json(DetectRemoteAccountInput {
                base_url: base_url.clone(),
            }),
        )
        .await
        .expect("detect should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("parse json");

        assert_eq!(
            payload["provider"],
            serde_json::Value::String("sub2api".to_string())
        );
        assert_eq!(
            payload["normalized_base_url"],
            serde_json::Value::String(base_url.trim_end_matches('/').to_string())
        );
        assert_eq!(payload["recommended_api_url"], serde_json::Value::Null);

        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn create_sub2api_account_uses_single_remote_fetch_and_persists_snapshot() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        let state = build_test_state(db_path.clone()).await;
        let (base_url, calls) = spawn_sub2api_overview_server().await;

        let response = create_remote_account(
            State(state),
            Json(CreateRemoteAccountInput {
                provider: RemoteAccountProvider::Sub2Api,
                base_url: base_url.clone(),
                api_url: None,
                user_id: None,
                user_token: None,
                bearer_token: Some("Bearer secret-token".to_string()),
                refresh_token: None,
                page_checkin_url: None,
                checkin_mode: Some(RemoteAccountCheckinMode::Disabled),
                auto_checkin_time: None,
                low_balance_alert_threshold: Some(3.0),
                recharge_currency: Some(RechargeCurrency::Cny),
            }),
        )
        .await
        .expect("create should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let accounts = storage::list_remote_accounts(db_path.clone())
            .await
            .expect("list accounts");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].remote_user_id.as_deref(), Some("42"));
        assert_eq!(accounts[0].remote_username.as_deref(), Some("demo-user"));
        assert_eq!(
            accounts[0].remote_display_name.as_deref(),
            Some("demo@example.com")
        );
        assert_eq!(accounts[0].last_balance_amount, Some(12.5));
        assert_eq!(accounts[0].last_sync_error, None);

        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn update_sub2api_account_uses_single_remote_fetch_and_persists_snapshot() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        let state = build_test_state(db_path.clone()).await;
        let (base_url, calls) = spawn_sub2api_overview_server().await;

        let account = storage::create_remote_account(
            db_path.clone(),
            storage::CreateRemoteAccount {
                provider: storage::RemoteAccountProvider::Sub2Api,
                base_url: base_url.clone(),
                api_url: None,
                access_token: "Bearer secret-token".to_string(),
                refresh_token: None,
                page_checkin_url: None,
                checkin_mode: Some(storage::RemoteAccountCheckinMode::Disabled),
                auto_checkin_time: None,
                low_balance_alert_threshold: Some(0.0),
                recharge_currency: Some(RechargeCurrency::Cny),
            },
        )
        .await
        .expect("seed remote account");

        let response = update_remote_account(
            State(state),
            axum::extract::Path(account.id.clone()),
            Json(UpdateRemoteAccountInput {
                low_balance_alert_threshold: Some(5.0),
                ..Default::default()
            }),
        )
        .await
        .expect("update should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let updated = storage::get_remote_account_without_secret(db_path.clone(), account.id)
            .await
            .expect("reload updated account");
        assert_eq!(updated.low_balance_alert_threshold, 5.0);
        assert_eq!(updated.remote_user_id.as_deref(), Some("42"));
        assert_eq!(updated.remote_username.as_deref(), Some("demo-user"));
        assert_eq!(
            updated.remote_display_name.as_deref(),
            Some("demo@example.com")
        );
        assert_eq!(updated.last_balance_amount, Some(12.5));
        assert_eq!(updated.last_sync_error, None);

        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn refresh_sub2api_account_rotates_tokens_after_access_token_expired() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        let state = build_test_state(db_path.clone()).await;
        let (base_url, auth_me_calls, refresh_calls) = spawn_sub2api_refresh_success_server().await;

        let account = storage::create_remote_account(
            db_path.clone(),
            storage::CreateRemoteAccount {
                provider: storage::RemoteAccountProvider::Sub2Api,
                base_url,
                api_url: None,
                access_token: "Bearer expired-access".to_string(),
                refresh_token: Some("Bearer good-refresh".to_string()),
                page_checkin_url: None,
                checkin_mode: Some(storage::RemoteAccountCheckinMode::Disabled),
                auto_checkin_time: None,
                low_balance_alert_threshold: Some(0.0),
                recharge_currency: Some(RechargeCurrency::Cny),
            },
        )
        .await
        .expect("seed sub2api account");

        let response =
            refresh_remote_account(State(state), axum::extract::Path(account.id.clone()))
                .await
                .expect("refresh should succeed")
                .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(auth_me_calls.load(Ordering::SeqCst), 2);
        assert_eq!(refresh_calls.load(Ordering::SeqCst), 1);

        let payload = response_json(response).await;
        assert_eq!(
            payload["provider"],
            serde_json::Value::String("sub2api".to_string())
        );
        assert_eq!(payload["reauth_required"], serde_json::Value::Bool(false));
        assert_eq!(
            payload["remote_username"],
            serde_json::Value::String("demo-user".to_string())
        );

        let updated = storage::get_remote_account_with_secret(db_path.clone(), account.id)
            .await
            .expect("reload updated account");
        assert_eq!(updated.access_token.as_deref(), Some("rotated-access"));
        assert_eq!(updated.refresh_token.as_deref(), Some("rotated-refresh"));
        assert!(!updated.reauth_required);
        assert_eq!(updated.last_sync_error, None);
        assert_eq!(updated.remote_user_id.as_deref(), Some("42"));
        assert_eq!(updated.remote_username.as_deref(), Some("demo-user"));
        assert_eq!(updated.last_balance_amount, Some(9.5));

        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn refresh_sub2api_account_marks_relogin_required_after_refresh_token_expired() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        let state = build_test_state(db_path.clone()).await;
        let (base_url, auth_me_calls, refresh_calls) = spawn_sub2api_refresh_invalid_server().await;

        let account = storage::create_remote_account(
            db_path.clone(),
            storage::CreateRemoteAccount {
                provider: storage::RemoteAccountProvider::Sub2Api,
                base_url,
                api_url: None,
                access_token: "Bearer expired-access".to_string(),
                refresh_token: Some("Bearer stale-refresh".to_string()),
                page_checkin_url: None,
                checkin_mode: Some(storage::RemoteAccountCheckinMode::Disabled),
                auto_checkin_time: None,
                low_balance_alert_threshold: Some(0.0),
                recharge_currency: Some(RechargeCurrency::Cny),
            },
        )
        .await
        .expect("seed sub2api account");

        let response =
            match refresh_remote_account(State(state), axum::extract::Path(account.id.clone()))
                .await
            {
                Ok(_) => panic!("refresh should require relogin"),
                Err(err) => err.into_response(),
            };

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(auth_me_calls.load(Ordering::SeqCst), 1);
        assert_eq!(refresh_calls.load(Ordering::SeqCst), 1);

        let payload = response_json(response).await;
        assert_eq!(
            payload["code"],
            serde_json::Value::String("remote_relogin_required".to_string())
        );

        let updated = storage::get_remote_account_with_secret(db_path.clone(), account.id)
            .await
            .expect("reload updated account");
        assert_eq!(updated.access_token.as_deref(), Some("expired-access"));
        assert_eq!(updated.refresh_token.as_deref(), Some("stale-refresh"));
        assert!(updated.reauth_required);
        assert_eq!(
            updated.last_sync_error.as_deref(),
            Some("sub2api login expired, please sign in again")
        );
        assert!(updated.last_synced_at_ms.is_some());

        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn remote_system_checkin_delegates_to_newapi_accounts() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        let state = build_test_state(db_path.clone()).await;
        let (base_url, calls) = spawn_newapi_system_checkin_server().await;

        let account = storage::create_newapi_account(
            db_path.clone(),
            storage::CreateNewApiAccount {
                base_url,
                api_url: None,
                user_id: "demo-user-id".to_string(),
                user_token: "demo-user-token".to_string(),
                page_checkin_url: None,
                checkin_mode: Some(storage::NewApiAccountCheckinMode::SystemApi),
                auto_checkin_enabled: Some(false),
                auto_checkin_time: None,
                low_balance_alert_threshold: Some(0.0),
                recharge_currency: Some(RechargeCurrency::Cny),
            },
        )
        .await
        .expect("seed newapi account");

        let response = perform_remote_account_system_checkin(
            State(state),
            axum::extract::Path(account.id.clone()),
        )
        .await
        .expect("system checkin should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("parse json");
        assert_eq!(
            payload["already_checked_in"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(
            payload["quota_awarded"],
            serde_json::Value::Number(777.into())
        );
        assert_eq!(
            payload["checkin_date"],
            serde_json::Value::String("2026-03-30".to_string())
        );

        let updated =
            storage::get_newapi_account_without_secret(db_path.clone(), account.id.clone())
                .await
                .expect("reload newapi account");
        assert_eq!(updated.remote_username.as_deref(), Some("demo-user"));
        assert_eq!(updated.remote_display_name.as_deref(), Some("Demo User"));
        assert_eq!(updated.last_sync_error, None);

        let checkins = storage::get_newapi_accounts_checkins_today(db_path.clone())
            .await
            .expect("load checkins");
        assert!(checkins.completed_account_ids.contains(&account.id));

        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn remote_system_checkin_rejects_sub2api_accounts() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        let state = build_test_state(db_path.clone()).await;

        let account = storage::create_remote_account(
            db_path.clone(),
            storage::CreateRemoteAccount {
                provider: storage::RemoteAccountProvider::Sub2Api,
                base_url: "http://127.0.0.1:65535".to_string(),
                api_url: None,
                access_token: "Bearer demo-token".to_string(),
                refresh_token: None,
                page_checkin_url: None,
                checkin_mode: Some(storage::RemoteAccountCheckinMode::Disabled),
                auto_checkin_time: None,
                low_balance_alert_threshold: Some(0.0),
                recharge_currency: Some(RechargeCurrency::Cny),
            },
        )
        .await
        .expect("seed sub2api account");

        let response = match perform_remote_account_system_checkin(
            State(state),
            axum::extract::Path(account.id),
        )
        .await
        {
            Ok(_) => panic!("sub2api should be rejected"),
            Err(err) => err.into_response(),
        };

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("parse json");
        assert_eq!(
            payload["code"],
            serde_json::Value::String("remote_checkin_unsupported_provider".to_string())
        );

        remove_sqlite_artifacts(&db_path);
    }
}
