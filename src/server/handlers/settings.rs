use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

use crate::events::{self, AppEvent, SystemNotificationSettings};
use crate::i18n::AppLocale;
use crate::server::AppState;
use crate::server::error::ApiError;
use crate::{autostart, logging, storage};

pub(in crate::server) async fn get_settings(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = storage::get_app_settings(state.db_path()).await?;
    let _ = state.settings_cache.send(Arc::new(settings.clone()));
    Ok(Json(settings))
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct UpdateSettingsInput {
    pricing_auto_update_enabled: Option<bool>,
    pricing_auto_update_interval_hours: Option<i64>,
    close_behavior: Option<storage::CloseBehavior>,
    auto_start_enabled: Option<bool>,
    auto_start_launch_mode: Option<storage::AutoStartLaunchMode>,
    server_lan_accessible: Option<bool>,
    ui_locale: Option<AppLocale>,
    app_auto_update_enabled: Option<bool>,
    gemini_cli_auto_update_enabled: Option<bool>,
    claude_code_auto_update_enabled: Option<bool>,
    codex_auto_update_enabled: Option<bool>,
    auto_disable_enabled: Option<bool>,
    auto_disable_window_minutes: Option<i64>,
    auto_disable_failure_times: Option<i64>,
    auto_disable_disable_minutes: Option<i64>,
    channel_retry_enabled: Option<bool>,
    anthropic_count_tokens_mock_enabled: Option<bool>,
    log_level: Option<logging::LogLevel>,
    log_retention_days: Option<i64>,
    chat_bridge_enabled: Option<bool>,
    chat_bridge_telegram_enabled: Option<bool>,
    chat_bridge_telegram_bot_token: Option<String>,
    chat_bridge_discord_enabled: Option<bool>,
    chat_bridge_discord_bot_token: Option<String>,
    chat_bridge_whatsapp_enabled: Option<bool>,
    chat_bridge_weixin_enabled: Option<bool>,
    chat_bridge_turn_timeout_minutes: Option<i64>,
    chat_bridge_allow_new_projects: Option<bool>,
    system_notifications_enabled: Option<bool>,
    remote_low_balance_system_notification_enabled: Option<bool>,
    remote_managed_channel_missing_system_notification_enabled: Option<bool>,
    remote_managed_channel_multiplier_system_notification_enabled: Option<bool>,
    remote_group_added_system_notification_enabled: Option<bool>,
    remote_managed_channel_missing_prompt_enabled: Option<bool>,
    remote_managed_channel_sync_multiplier_enabled: Option<bool>,
    remote_managed_channel_sync_free_multiplier_enabled: Option<bool>,
}

pub(in crate::server) async fn update_settings(
    State(state): State<AppState>,
    Json(input): Json<UpdateSettingsInput>,
) -> Result<impl IntoResponse, ApiError> {
    let autostart_enabled_updated = input.auto_start_enabled.is_some();
    let autostart_mode_updated = input.auto_start_launch_mode.is_some();
    let prev_notification_settings =
        SystemNotificationSettings::from_settings(state.settings_snapshot().as_ref());

    let changed: Vec<&'static str> = [
        (
            "pricing_auto_update_enabled",
            input.pricing_auto_update_enabled.is_some(),
        ),
        (
            "pricing_auto_update_interval_hours",
            input.pricing_auto_update_interval_hours.is_some(),
        ),
        ("close_behavior", input.close_behavior.is_some()),
        ("auto_start_enabled", input.auto_start_enabled.is_some()),
        (
            "auto_start_launch_mode",
            input.auto_start_launch_mode.is_some(),
        ),
        (
            "server_lan_accessible",
            input.server_lan_accessible.is_some(),
        ),
        ("ui_locale", input.ui_locale.is_some()),
        (
            "app_auto_update_enabled",
            input.app_auto_update_enabled.is_some(),
        ),
        (
            "gemini_cli_auto_update_enabled",
            input.gemini_cli_auto_update_enabled.is_some(),
        ),
        (
            "claude_code_auto_update_enabled",
            input.claude_code_auto_update_enabled.is_some(),
        ),
        (
            "codex_auto_update_enabled",
            input.codex_auto_update_enabled.is_some(),
        ),
        ("auto_disable_enabled", input.auto_disable_enabled.is_some()),
        (
            "auto_disable_window_minutes",
            input.auto_disable_window_minutes.is_some(),
        ),
        (
            "auto_disable_failure_times",
            input.auto_disable_failure_times.is_some(),
        ),
        (
            "auto_disable_disable_minutes",
            input.auto_disable_disable_minutes.is_some(),
        ),
        (
            "channel_retry_enabled",
            input.channel_retry_enabled.is_some(),
        ),
        (
            "anthropic_count_tokens_mock_enabled",
            input.anthropic_count_tokens_mock_enabled.is_some(),
        ),
        ("log_level", input.log_level.is_some()),
        ("log_retention_days", input.log_retention_days.is_some()),
        ("chat_bridge_enabled", input.chat_bridge_enabled.is_some()),
        (
            "chat_bridge_telegram_enabled",
            input.chat_bridge_telegram_enabled.is_some(),
        ),
        (
            "chat_bridge_telegram_bot_token",
            input.chat_bridge_telegram_bot_token.is_some(),
        ),
        (
            "chat_bridge_discord_enabled",
            input.chat_bridge_discord_enabled.is_some(),
        ),
        (
            "chat_bridge_discord_bot_token",
            input.chat_bridge_discord_bot_token.is_some(),
        ),
        (
            "chat_bridge_whatsapp_enabled",
            input.chat_bridge_whatsapp_enabled.is_some(),
        ),
        (
            "chat_bridge_weixin_enabled",
            input.chat_bridge_weixin_enabled.is_some(),
        ),
        (
            "chat_bridge_turn_timeout_minutes",
            input.chat_bridge_turn_timeout_minutes.is_some(),
        ),
        (
            "chat_bridge_allow_new_projects",
            input.chat_bridge_allow_new_projects.is_some(),
        ),
        (
            "system_notifications_enabled",
            input.system_notifications_enabled.is_some(),
        ),
        (
            "remote_low_balance_system_notification_enabled",
            input
                .remote_low_balance_system_notification_enabled
                .is_some(),
        ),
        (
            "remote_managed_channel_missing_system_notification_enabled",
            input
                .remote_managed_channel_missing_system_notification_enabled
                .is_some(),
        ),
        (
            "remote_managed_channel_multiplier_system_notification_enabled",
            input
                .remote_managed_channel_multiplier_system_notification_enabled
                .is_some(),
        ),
        (
            "remote_group_added_system_notification_enabled",
            input
                .remote_group_added_system_notification_enabled
                .is_some(),
        ),
        (
            "remote_managed_channel_missing_prompt_enabled",
            input
                .remote_managed_channel_missing_prompt_enabled
                .is_some(),
        ),
        (
            "remote_managed_channel_sync_multiplier_enabled",
            input
                .remote_managed_channel_sync_multiplier_enabled
                .is_some(),
        ),
        (
            "remote_managed_channel_sync_free_multiplier_enabled",
            input
                .remote_managed_channel_sync_free_multiplier_enabled
                .is_some(),
        ),
    ]
    .into_iter()
    .filter_map(|(name, is_changed)| is_changed.then_some(name))
    .collect();

    if let Some(h) = input.pricing_auto_update_interval_hours
        && !(1..=8760).contains(&h)
    {
        return Err(ApiError::bad_request(
            "settings_pricing_auto_update_interval_hours_out_of_range",
            "pricing_auto_update_interval_hours must be between 1 and 8760",
        ));
    }
    if let Some(v) = input.auto_disable_window_minutes
        && v < 1
    {
        return Err(ApiError::bad_request(
            "settings_auto_disable_window_minutes_out_of_range",
            "auto_disable_window_minutes must be >= 1",
        ));
    }
    if let Some(v) = input.auto_disable_failure_times
        && v < 1
    {
        return Err(ApiError::bad_request(
            "settings_auto_disable_failure_times_out_of_range",
            "auto_disable_failure_times must be >= 1",
        ));
    }
    if let Some(v) = input.auto_disable_disable_minutes
        && v < 1
    {
        return Err(ApiError::bad_request(
            "settings_auto_disable_disable_minutes_out_of_range",
            "auto_disable_disable_minutes must be >= 1",
        ));
    }
    if let Some(v) = input.log_retention_days
        && !(1..=3650).contains(&v)
    {
        return Err(ApiError::bad_request(
            "settings_log_retention_days_out_of_range",
            "log_retention_days must be between 1 and 3650",
        ));
    }
    if let Some(v) = input.chat_bridge_turn_timeout_minutes
        && v < 0
    {
        return Err(ApiError::bad_request(
            "settings_chat_bridge_turn_timeout_minutes_out_of_range",
            "chat_bridge_turn_timeout_minutes must be >= 0",
        ));
    }

    let auto_start_enabled = input.auto_start_enabled;
    if let Some(enabled) = auto_start_enabled {
        let res = tokio::task::spawn_blocking(move || autostart::set_enabled(enabled)).await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(desired = enabled, err = %e, "set autostart failed");
                return Err(ApiError::bad_request(
                    "settings_autostart_set_failed",
                    format!("Failed to update auto-start: {e}"),
                ));
            }
            Err(e) => {
                tracing::warn!(desired = enabled, err = %e, "set autostart failed");
                return Err(ApiError::bad_request(
                    "settings_autostart_set_failed",
                    format!("Failed to update auto-start: {e}"),
                ));
            }
        }
    }

    let settings = storage::update_app_settings(
        state.db_path(),
        storage::AppSettingsPatch {
            pricing_auto_update_enabled: input.pricing_auto_update_enabled,
            pricing_auto_update_interval_hours: input.pricing_auto_update_interval_hours,
            close_behavior: input.close_behavior,
            auto_start_enabled,
            auto_start_launch_mode: input.auto_start_launch_mode,
            server_lan_accessible: input.server_lan_accessible,
            ui_locale: input.ui_locale,
            app_auto_update_enabled: input.app_auto_update_enabled,
            gemini_cli_auto_update_enabled: input.gemini_cli_auto_update_enabled,
            claude_code_auto_update_enabled: input.claude_code_auto_update_enabled,
            codex_auto_update_enabled: input.codex_auto_update_enabled,
            auto_disable_enabled: input.auto_disable_enabled,
            auto_disable_window_minutes: input.auto_disable_window_minutes,
            auto_disable_failure_times: input.auto_disable_failure_times,
            auto_disable_disable_minutes: input.auto_disable_disable_minutes,
            channel_retry_enabled: input.channel_retry_enabled,
            anthropic_count_tokens_mock_enabled: input.anthropic_count_tokens_mock_enabled,
            log_level: input.log_level,
            log_retention_days: input.log_retention_days,
            chat_bridge_enabled: input.chat_bridge_enabled,
            chat_bridge_telegram_enabled: input.chat_bridge_telegram_enabled,
            chat_bridge_telegram_bot_token: input.chat_bridge_telegram_bot_token,
            chat_bridge_discord_enabled: input.chat_bridge_discord_enabled,
            chat_bridge_discord_bot_token: input.chat_bridge_discord_bot_token,
            chat_bridge_whatsapp_enabled: input.chat_bridge_whatsapp_enabled,
            chat_bridge_weixin_enabled: input.chat_bridge_weixin_enabled,
            chat_bridge_turn_timeout_minutes: input.chat_bridge_turn_timeout_minutes,
            chat_bridge_allow_new_projects: input.chat_bridge_allow_new_projects,
            system_notifications_enabled: input.system_notifications_enabled,
            remote_low_balance_system_notification_enabled: input
                .remote_low_balance_system_notification_enabled,
            remote_managed_channel_missing_system_notification_enabled: input
                .remote_managed_channel_missing_system_notification_enabled,
            remote_managed_channel_multiplier_system_notification_enabled: input
                .remote_managed_channel_multiplier_system_notification_enabled,
            remote_group_added_system_notification_enabled: input
                .remote_group_added_system_notification_enabled,
            remote_managed_channel_missing_prompt_enabled: input
                .remote_managed_channel_missing_prompt_enabled,
            remote_managed_channel_sync_multiplier_enabled: input
                .remote_managed_channel_sync_multiplier_enabled,
            remote_managed_channel_sync_free_multiplier_enabled: input
                .remote_managed_channel_sync_free_multiplier_enabled,
            ..Default::default()
        },
    )
    .await?;

    if autostart_mode_updated && settings.auto_start_enabled && !autostart_enabled_updated {
        let res = tokio::task::spawn_blocking(move || autostart::set_enabled(true)).await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(err = %e, "rewrite autostart args failed");
                return Err(ApiError::bad_request(
                    "settings_autostart_args_update_failed",
                    format!("Failed to update auto-start args: {e}"),
                ));
            }
            Err(e) => {
                tracing::warn!(err = %e, "rewrite autostart args failed");
                return Err(ApiError::bad_request(
                    "settings_autostart_args_update_failed",
                    format!("Failed to update auto-start args: {e}"),
                ));
            }
        }
    }

    if input.log_level.is_some() {
        let _ = logging::set_level(settings.log_level);
    }

    if !changed.is_empty() {
        tracing::info!(changed = ?changed, "settings updated");
    }

    let next_notification_settings = SystemNotificationSettings::from_settings(&settings);
    if prev_notification_settings != next_notification_settings {
        events::publish(AppEvent::SystemNotificationSettingsChanged(
            next_notification_settings,
        ));
    }
    let _ = state.settings_cache.send(Arc::new(settings.clone()));

    let next = *state.settings_notify.borrow() + 1;
    let _ = state.settings_notify.send(next);

    Ok(Json(settings))
}
