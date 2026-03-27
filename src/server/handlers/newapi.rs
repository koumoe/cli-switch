use anyhow::Context as _;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::events::{self, AppEvent, NewApiManagedChannelCreated};
use crate::newapi as newapi_client;
use crate::server::AppState;
use crate::server::error::{ApiError, map_storage_unit_no_content_err};
use crate::storage::{self, RechargeCurrency};

fn validate_http_url(raw: &str, field: &'static str) -> Result<String, ApiError> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(ApiError::bad_request(field, format!("{field} is required")));
    }
    let parsed = reqwest::Url::parse(value)
        .map_err(|e| ApiError::bad_request(field, format!("Invalid {field}: {e}")))?;
    match parsed.scheme() {
        "http" | "https" => Ok(value.to_string()),
        other => Err(ApiError::bad_request(
            field,
            format!("Only http/https is supported, scheme={other}"),
        )),
    }
}

fn validate_optional_http_url(
    raw: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, ApiError> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => validate_http_url(value, field).map(Some),
        None => Ok(None),
    }
}

fn validate_account_candidate(account: &storage::NewApiAccount) -> Result<(), ApiError> {
    let _ = validate_http_url(&account.base_url, "newapi_base_url_required")?;
    if !account.low_balance_alert_threshold.is_finite() || account.low_balance_alert_threshold < 0.0
    {
        return Err(ApiError::bad_request(
            "newapi_low_balance_threshold_invalid",
            "low_balance_alert_threshold must be a finite number >= 0",
        ));
    }
    let time_format = time::format_description::parse("[hour]:[minute]:[second]").map_err(|e| {
        ApiError::Internal(anyhow::anyhow!("parse checkin time format failed: {e}"))
    })?;
    time::Time::parse(account.auto_checkin_time.trim(), &time_format).map_err(|e| {
        ApiError::bad_request(
            "newapi_auto_checkin_time_invalid",
            format!("Invalid auto_checkin_time: {e}"),
        )
    })?;
    let has_credentials = newapi_client::account_has_user_api_credentials(account);
    if !has_credentials
        && account.auto_checkin_enabled
        && matches!(
            account.checkin_mode,
            storage::NewApiAccountCheckinMode::SystemApi
        )
    {
        return Err(ApiError::bad_request(
            "newapi_credentials_required_for_system_checkin",
            "user_id and user_token are required when system_api auto check-in is enabled",
        ));
    }
    if matches!(
        account.checkin_mode,
        storage::NewApiAccountCheckinMode::PageOpen
    ) {
        validate_optional_http_url(
            account.page_checkin_url.as_deref(),
            "newapi_page_checkin_url_invalid",
        )?
        .ok_or_else(|| {
            ApiError::bad_request(
                "newapi_page_checkin_url_required",
                "page_checkin_url is required when checkin_mode is page_open",
            )
        })?;
    }
    Ok(())
}

fn build_candidate_from_create(
    input: &storage::CreateNewApiAccount,
) -> Result<storage::NewApiAccount, ApiError> {
    let page_checkin_url = validate_optional_http_url(
        input.page_checkin_url.as_deref(),
        "newapi_page_checkin_url_invalid",
    )?;
    let api_url = validate_optional_http_url(input.api_url.as_deref(), "newapi_api_url_invalid")?;
    let checkin_mode = input
        .checkin_mode
        .unwrap_or(storage::NewApiAccountCheckinMode::SystemApi);
    let auto_checkin_enabled = input.auto_checkin_enabled.unwrap_or(false)
        && matches!(checkin_mode, storage::NewApiAccountCheckinMode::SystemApi);
    let account = storage::NewApiAccount {
        id: "<candidate>".to_string(),
        base_url: validate_http_url(&input.base_url, "newapi_base_url_required")?,
        api_url,
        user_id: input.user_id.trim().to_string(),
        user_token: Some(input.user_token.trim().to_string()).filter(|value| !value.is_empty()),
        user_token_configured: !input.user_token.trim().is_empty(),
        page_checkin_url,
        checkin_mode,
        auto_checkin_enabled,
        auto_checkin_time: input
            .auto_checkin_time
            .clone()
            .unwrap_or_else(|| "00:05:00".to_string()),
        low_balance_alert_threshold: input.low_balance_alert_threshold.unwrap_or(0.0),
        recharge_currency: input.recharge_currency.unwrap_or(RechargeCurrency::Cny),
        remote_role: None,
        remote_username: None,
        remote_display_name: None,
        remote_group: None,
        quota_display_type: "USD".to_string(),
        quota_per_unit: 500_000.0,
        usd_exchange_rate: 1.0,
        custom_currency_symbol: None,
        custom_currency_exchange_rate: 1.0,
        remote_checkin_enabled: false,
        remote_turnstile_check_enabled: false,
        last_quota: None,
        last_used_quota: None,
        last_balance_amount: None,
        last_sync_error: None,
        last_synced_at_ms: None,
        low_balance_alert_notified: false,
        last_balance_alert_at_ms: None,
        created_at_ms: 0,
        updated_at_ms: 0,
    };
    validate_account_candidate(&account)?;
    Ok(account)
}

fn build_candidate_from_update(
    current: &storage::NewApiAccount,
    input: &storage::UpdateNewApiAccount,
) -> Result<storage::NewApiAccount, ApiError> {
    let mut next = current.clone();
    if let Some(value) = input.base_url.as_deref() {
        next.base_url = validate_http_url(value, "newapi_base_url_required")?;
    }
    if input.api_url.is_some() {
        next.api_url =
            validate_optional_http_url(input.api_url.as_deref(), "newapi_api_url_invalid")?;
    }
    if let Some(value) = input.user_id.as_deref() {
        next.user_id = value.trim().to_string();
    }
    if let Some(value) = input.user_token.as_deref() {
        next.user_token = Some(value.trim().to_string()).filter(|value| !value.is_empty());
        next.user_token_configured = next.user_token.is_some();
    }
    if input.page_checkin_url.is_some() {
        next.page_checkin_url = validate_optional_http_url(
            input.page_checkin_url.as_deref(),
            "newapi_page_checkin_url_invalid",
        )?;
    }
    if let Some(value) = input.checkin_mode {
        next.checkin_mode = value;
        if matches!(
            next.checkin_mode,
            storage::NewApiAccountCheckinMode::PageOpen
        ) {
            next.auto_checkin_enabled = false;
        }
    }
    if let Some(value) = input.auto_checkin_enabled {
        next.auto_checkin_enabled = value
            && matches!(
                next.checkin_mode,
                storage::NewApiAccountCheckinMode::SystemApi
            );
    }
    if let Some(value) = input.auto_checkin_time.as_deref() {
        next.auto_checkin_time = value.trim().to_string();
    }
    if let Some(value) = input.low_balance_alert_threshold {
        next.low_balance_alert_threshold = value;
    }
    if let Some(value) = input.recharge_currency {
        next.recharge_currency = value;
    }
    validate_account_candidate(&next)?;
    Ok(next)
}

async fn apply_account_overview(
    state: &AppState,
    account_id: String,
    overview: &newapi_client::NewApiAccountOverview,
) -> Result<storage::NewApiAccount, ApiError> {
    storage::update_newapi_account_remote_snapshot(
        state.db_path(),
        account_id.clone(),
        newapi_client::build_remote_snapshot(overview),
    )
    .await?;
    if overview.checked_in_today {
        storage::complete_newapi_account_checkin_today(
            state.db_path(),
            account_id.clone(),
            "remote_detected",
        )
        .await?;
    }
    let account = storage::get_newapi_account_without_secret(state.db_path(), account_id).await?;
    Ok(account)
}

async fn clear_account_remote_state(
    state: &AppState,
    account_id: String,
) -> Result<storage::NewApiAccount, ApiError> {
    storage::update_newapi_account_remote_snapshot(
        state.db_path(),
        account_id.clone(),
        storage::NewApiAccountRemoteSnapshot {
            replace_remote_state: true,
            quota_display_type: Some("USD".to_string()),
            quota_per_unit: Some(500_000.0),
            usd_exchange_rate: Some(1.0),
            custom_currency_exchange_rate: Some(1.0),
            remote_checkin_enabled: Some(false),
            remote_turnstile_check_enabled: Some(false),
            last_sync_error: None,
            last_synced_at_ms: None,
            ..Default::default()
        },
    )
    .await?;
    storage::set_newapi_account_balance_alert_notified(
        state.db_path(),
        account_id.clone(),
        false,
        None,
    )
    .await?;
    storage::get_newapi_account_without_secret(state.db_path(), account_id)
        .await
        .map_err(ApiError::Internal)
}

async fn sync_account_if_possible(
    state: &AppState,
    account_id: String,
    account: &storage::NewApiAccount,
) -> Result<storage::NewApiAccount, ApiError> {
    if !newapi_client::account_has_user_api_credentials(account) {
        return clear_account_remote_state(state, account_id).await;
    }
    let overview = newapi_client::fetch_account_overview(&state.http_client, account)
        .await
        .map_err(sync_error)?;
    apply_account_overview(state, account_id, &overview).await
}

async fn record_account_sync_failure(
    state: &AppState,
    account_id: String,
    err: &anyhow::Error,
) -> Result<(), ApiError> {
    storage::update_newapi_account_remote_snapshot(
        state.db_path(),
        account_id,
        storage::NewApiAccountRemoteSnapshot {
            last_sync_error: Some(err.to_string()),
            last_synced_at_ms: Some(storage::now_ms()),
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

fn sync_error(err: anyhow::Error) -> ApiError {
    ApiError::bad_gateway(
        "newapi_sync_failed",
        format!("Failed to sync New API account: {err}"),
    )
}

fn validate_managed_channel_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request(
            "newapi_managed_name_required",
            "name is required",
        ));
    }
    Ok(name.to_string())
}

fn managed_channel_recharge_currency(account: &storage::NewApiAccount) -> RechargeCurrency {
    account.recharge_currency
}

fn managed_channel_base_url(
    account: &storage::NewApiAccount,
    base_url_override: Option<String>,
) -> String {
    base_url_override
        .or_else(|| account.api_url.clone())
        .unwrap_or_else(|| account.base_url.clone())
}

fn notify_background_tasks(state: &AppState) {
    let next = *state.settings_notify.borrow() + 1;
    let _ = state.settings_notify.send(next);
}

async fn delete_remote_managed_channel_resources(
    state: &AppState,
    account: &storage::NewApiAccount,
    channel: &storage::Channel,
) -> anyhow::Result<()> {
    if let Some(remote_channel_id) = channel.newapi_channel_id {
        newapi_client::delete_channel(&state.http_client, account, remote_channel_id)
            .await
            .with_context(|| {
                format!(
                    "delete remote channel {} for local channel {} failed",
                    remote_channel_id, channel.id
                )
            })?;
    }
    if let Some(remote_token_id) = channel.newapi_token_id {
        newapi_client::delete_token(&state.http_client, account, remote_token_id)
            .await
            .with_context(|| {
                format!(
                    "delete remote token {} for local channel {} failed",
                    remote_token_id, channel.id
                )
            })?;
    }
    Ok(())
}

#[derive(Debug, Deserialize, Default)]
pub(in crate::server) struct DeleteNewApiAccountInput {
    pub delete_managed_channels: Option<bool>,
    pub sync_remote_delete: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct CreateManagedChannelInput {
    pub name: String,
    pub protocol: Option<storage::Protocol>,
    pub group_name: String,
    pub base_url_override: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct ReorderNewApiAccountsInput {
    pub account_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(in crate::server) struct NewApiGroupResponse {
    pub name: String,
    pub ratio: Option<f64>,
    pub description: Option<String>,
    pub managed_channel_count: usize,
}

#[derive(Debug, Serialize)]
pub(in crate::server) struct NewApiCheckinResponse {
    pub account: storage::NewApiAccount,
    pub already_checked_in: bool,
    pub quota_awarded: Option<i64>,
    pub checkin_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub(in crate::server) struct CreateManagedChannelResponse {
    pub channel: storage::Channel,
}

pub(in crate::server) async fn list_newapi_accounts(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let accounts = storage::list_newapi_accounts(state.db_path()).await?;
    Ok(Json(accounts))
}

pub(in crate::server) async fn reorder_newapi_accounts(
    State(state): State<AppState>,
    Json(input): Json<ReorderNewApiAccountsInput>,
) -> Result<impl IntoResponse, ApiError> {
    let mut seen = std::collections::HashSet::<String>::new();
    for id in &input.account_ids {
        if !seen.insert(id.clone()) {
            return Err(ApiError::bad_request(
                "newapi_account_ids_duplicate",
                "account_ids contains duplicates",
            ));
        }
    }
    storage::reorder_newapi_accounts(state.db_path(), input.account_ids)
        .await
        .map_err(|e| match e.downcast_ref::<storage::StorageError>() {
            Some(storage::StorageError::NewApiAccountReorderMismatch { .. }) => {
                ApiError::bad_request(
                    "newapi_account_ids_mismatch",
                    "account_ids must cover all accounts",
                )
            }
            _ => ApiError::Internal(e),
        })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(in crate::server) async fn newapi_account_checkins_today(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let checkins = storage::get_newapi_accounts_checkins_today(state.db_path()).await?;
    Ok(Json(checkins))
}

pub(in crate::server) async fn create_newapi_account(
    State(state): State<AppState>,
    Json(input): Json<storage::CreateNewApiAccount>,
) -> Result<impl IntoResponse, ApiError> {
    let candidate = build_candidate_from_create(&input)?;
    let account = storage::create_newapi_account(state.db_path(), input)
        .await
        .map_err(|e| match e.downcast_ref::<storage::StorageError>() {
            Some(storage::StorageError::NewApiAccountAlreadyExists { .. }) => {
                ApiError::conflict("newapi_account_exists", "New API account already exists")
            }
            _ => ApiError::Internal(e),
        })?;

    let account = sync_account_if_possible(&state, account.id.clone(), &candidate).await?;
    notify_background_tasks(&state);
    Ok((axum::http::StatusCode::CREATED, Json(account)))
}

pub(in crate::server) async fn update_newapi_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    Json(input): Json<storage::UpdateNewApiAccount>,
) -> Result<impl IntoResponse, ApiError> {
    let current =
        storage::get_newapi_account_with_secret(state.db_path(), account_id.clone()).await?;
    let candidate = build_candidate_from_update(&current, &input)?;

    storage::update_newapi_account(state.db_path(), account_id.clone(), input)
        .await
        .map_err(|e| match e.downcast_ref::<storage::StorageError>() {
            Some(storage::StorageError::NewApiAccountNotFound { .. }) => {
                ApiError::not_found("newapi_account_not_found", "New API account not found")
            }
            Some(storage::StorageError::NewApiAccountAlreadyExists { .. }) => {
                ApiError::conflict("newapi_account_exists", "New API account already exists")
            }
            _ => ApiError::Internal(e),
        })?;

    let account = sync_account_if_possible(&state, account_id, &candidate).await?;
    notify_background_tasks(&state);
    Ok(Json(account))
}

pub(in crate::server) async fn refresh_newapi_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let account =
        storage::get_newapi_account_with_secret(state.db_path(), account_id.clone()).await?;
    if !newapi_client::account_has_user_api_credentials(&account) {
        return clear_account_remote_state(&state, account_id)
            .await
            .map(Json);
    }
    let overview = match newapi_client::fetch_account_overview(&state.http_client, &account).await {
        Ok(overview) => overview,
        Err(err) => {
            let _ = record_account_sync_failure(&state, account_id, &err).await;
            return Err(sync_error(err));
        }
    };
    let account = apply_account_overview(&state, account.id.clone(), &overview).await?;
    Ok(Json(account))
}

pub(in crate::server) async fn list_newapi_account_groups(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let account = storage::get_newapi_account_with_secret(state.db_path(), account_id).await?;
    if !newapi_client::account_has_user_api_credentials(&account) {
        return Err(ApiError::bad_request(
            "newapi_credentials_required",
            "user_id and user_token are required for this action",
        ));
    }
    let groups = newapi_client::list_groups(&state.http_client, &account)
        .await
        .map_err(sync_error)?;
    let managed_counts = storage::list_channels(state.db_path())
        .await?
        .into_iter()
        .filter(|channel| {
            channel.managed_by_newapi
                && channel.newapi_account_id.as_deref() == Some(account.id.as_str())
        })
        .fold(
            std::collections::HashMap::<String, usize>::new(),
            |mut acc, channel| {
                if let Some(group) = channel.newapi_group.as_deref() {
                    *acc.entry(group.to_string()).or_default() += 1;
                }
                acc
            },
        );
    Ok(Json(
        groups
            .into_iter()
            .map(|item| NewApiGroupResponse {
                managed_channel_count: managed_counts.get(&item.name).copied().unwrap_or(0),
                name: item.name,
                ratio: item.ratio,
                description: item.description,
            })
            .collect::<Vec<_>>(),
    ))
}

pub(in crate::server) async fn complete_newapi_account_checkin_today(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let res =
        storage::complete_newapi_account_checkin_today(state.db_path(), account_id, "manual_page")
            .await;
    map_storage_unit_no_content_err(res, |e| {
        matches!(
            e.downcast_ref::<storage::StorageError>(),
            Some(storage::StorageError::NewApiAccountNotFound { .. })
        )
        .then(|| ApiError::not_found("newapi_account_not_found", "New API account not found"))
    })
}

pub(in crate::server) async fn perform_newapi_account_system_checkin(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let account =
        storage::get_newapi_account_with_secret(state.db_path(), account_id.clone()).await?;
    if !newapi_client::account_has_user_api_credentials(&account) {
        return Err(ApiError::bad_request(
            "newapi_credentials_required",
            "user_id and user_token are required for this action",
        ));
    }
    let overview_before =
        match newapi_client::fetch_account_overview(&state.http_client, &account).await {
            Ok(overview) => overview,
            Err(err) => {
                let _ = record_account_sync_failure(&state, account_id.clone(), &err).await;
                return Err(sync_error(err));
            }
        };
    let _ = apply_account_overview(&state, account_id.clone(), &overview_before).await?;
    let checkin_result = newapi_client::perform_system_checkin_with_overview(
        &state.http_client,
        &account,
        Some(&overview_before),
    )
    .await
    .map_err(|err| {
        let message = err.to_string();
        if message.contains("Turnstile") || message.contains("未启用系统签到") {
            ApiError::bad_request("newapi_checkin_unavailable", message)
        } else {
            ApiError::bad_gateway("newapi_checkin_failed", message)
        }
    })?;

    let overview_after = if checkin_result.already_checked_in {
        overview_before
    } else {
        match newapi_client::fetch_account_overview(&state.http_client, &account).await {
            Ok(overview) => overview,
            Err(err) => {
                let _ = record_account_sync_failure(&state, account_id.clone(), &err).await;
                return Err(sync_error(err));
            }
        }
    };
    let account = apply_account_overview(&state, account_id.clone(), &overview_after).await?;
    storage::complete_newapi_account_checkin_today(
        state.db_path(),
        account_id,
        if checkin_result.already_checked_in {
            "remote_detected"
        } else {
            "system_api"
        },
    )
    .await?;

    Ok(Json(NewApiCheckinResponse {
        account,
        already_checked_in: checkin_result.already_checked_in,
        quota_awarded: checkin_result.quota_awarded,
        checkin_date: checkin_result.checkin_date,
    }))
}

pub(in crate::server) async fn create_newapi_managed_channel(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    Json(input): Json<CreateManagedChannelInput>,
) -> Result<impl IntoResponse, ApiError> {
    let account =
        storage::get_newapi_account_with_secret(state.db_path(), account_id.clone()).await?;
    if !newapi_client::account_has_user_api_credentials(&account) {
        return Err(ApiError::bad_request(
            "newapi_credentials_required",
            "user_id and user_token are required for this action",
        ));
    }
    let group_name = input.group_name.trim().to_string();
    if group_name.is_empty() {
        return Err(ApiError::bad_request(
            "newapi_group_required",
            "group_name is required",
        ));
    }
    let protocol = input
        .protocol
        .ok_or_else(|| ApiError::bad_request("newapi_protocol_required", "protocol is required"))?;
    let name = validate_managed_channel_name(&input.name)?;
    let base_url_override = validate_optional_http_url(
        input.base_url_override.as_deref(),
        "newapi_managed_base_url_invalid",
    )?;

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
        ApiError::bad_gateway("newapi_managed_channel_create_failed", err.to_string())
    })?;

    let create_local = storage::CreateChannel {
        name,
        protocol,
        base_url: managed_channel_base_url(&account, base_url_override),
        auth_type: Some("auto".to_string()),
        auth_ref: remote.token_key.clone(),
        checkin_url: None,
        priority: input.priority.unwrap_or(0),
        recharge_currency: Some(managed_channel_recharge_currency(&account)),
        real_multiplier: Some(remote.group_ratio),
        enabled: input.enabled.unwrap_or(true),
        managed_by_newapi: Some(true),
        newapi_account_id: Some(account_id),
        newapi_channel_id: None,
        newapi_token_id: Some(remote.token_id),
        newapi_token_name: Some(remote.token_name),
        newapi_group: Some(remote.group_name),
    };

    let channel = match storage::create_channel(state.db_path(), create_local).await {
        Ok(channel) => channel,
        Err(err) => {
            let _ =
                newapi_client::delete_token(&state.http_client, &account, remote.token_id).await;
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
        events::publish(AppEvent::NewApiManagedChannelCreated(
            NewApiManagedChannelCreated {
                channel_id: channel.id.clone(),
                channel_name: channel.name.clone(),
                account_id: account.id.clone(),
                account_base_url: account.base_url.clone(),
                group_name: channel.newapi_group.clone(),
                token_name: channel.newapi_token_name.clone(),
            },
        ));
    }

    Ok((
        axum::http::StatusCode::CREATED,
        Json(CreateManagedChannelResponse { channel }),
    ))
}

pub(in crate::server) async fn delete_newapi_account(
    State(state): State<AppState>,
    axum::extract::Path(account_id): axum::extract::Path<String>,
    input: Option<Json<DeleteNewApiAccountInput>>,
) -> Result<impl IntoResponse, ApiError> {
    let input = input.map(|Json(input)| input).unwrap_or_default();
    let delete_managed_channels = input.delete_managed_channels.unwrap_or(false);
    let sync_remote_delete = input.sync_remote_delete.unwrap_or(false);
    if sync_remote_delete && !delete_managed_channels {
        return Err(ApiError::bad_request(
            "newapi_delete_remote_requires_channel_delete",
            "sync_remote_delete requires delete_managed_channels=true",
        ));
    }

    if delete_managed_channels && sync_remote_delete {
        let account =
            storage::get_newapi_account_with_secret(state.db_path(), account_id.clone()).await?;
        let channels = storage::list_channels(state.db_path())
            .await?
            .into_iter()
            .filter(|channel| {
                channel.managed_by_newapi
                    && channel.newapi_account_id.as_deref() == Some(account_id.as_str())
            })
            .collect::<Vec<_>>();
        let mut deleted_channel_ids = Vec::new();
        let mut failures = Vec::new();
        for channel in &channels {
            match delete_remote_managed_channel_resources(&state, &account, channel).await {
                Ok(()) => deleted_channel_ids.push(channel.id.clone()),
                Err(err) => failures.push(format!("{} ({}): {err}", channel.name, channel.id)),
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
                "newapi_remote_delete_partial_failed",
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
                        if channel.newapi_account_id.as_deref() == Some(account_id.as_str()) {
                            channel.managed_by_newapi = false;
                            channel.newapi_account_id = None;
                            channel.newapi_channel_id = None;
                            channel.newapi_token_id = None;
                            channel.newapi_token_name = None;
                            channel.newapi_group = None;
                        }
                    }
                    *cur = std::sync::Arc::new(next);
                });
            }
            notify_background_tasks(&state);
            Ok(Json(result))
        }
        Err(err) => Err(match err.downcast_ref::<storage::StorageError>() {
            Some(storage::StorageError::NewApiAccountNotFound { .. }) => {
                ApiError::not_found("newapi_account_not_found", "New API account not found")
            }
            _ => ApiError::Internal(err),
        }),
    }
}
