use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::newapi as newapi_client;
use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage::{self, RechargeCurrency};

pub(super) fn validate_http_url(raw: &str, field: &'static str) -> Result<String, ApiError> {
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

pub(super) fn validate_optional_http_url(
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

pub(super) fn build_candidate_from_create(
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
        name: input.name.clone().unwrap_or_default(),
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
        sort_order: 0,
        created_at_ms: 0,
        updated_at_ms: 0,
    };
    validate_account_candidate(&account)?;
    Ok(account)
}

pub(super) fn build_candidate_from_update(
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

pub(super) async fn apply_account_overview(
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

pub(super) async fn clear_account_remote_state(
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

pub(super) async fn sync_account_if_possible(
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

pub(super) async fn record_account_sync_failure(
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

pub(super) fn sync_error(err: anyhow::Error) -> ApiError {
    ApiError::bad_gateway(
        "newapi_sync_failed",
        format!("Failed to sync New API account: {err}"),
    )
}

pub(super) fn notify_background_tasks(state: &AppState) {
    let next = *state.settings_notify.borrow() + 1;
    let _ = state.settings_notify.send(next);
}

#[derive(Debug, Serialize)]
pub(in crate::server) struct NewApiCheckinResponse {
    pub account: storage::NewApiAccount,
    pub already_checked_in: bool,
    pub quota_awarded: Option<i64>,
    pub checkin_date: Option<String>,
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
