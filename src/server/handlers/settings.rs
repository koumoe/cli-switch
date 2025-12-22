use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::{autostart, logging, storage};

pub(in crate::server) async fn get_settings(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = storage::get_app_settings(state.db_path()).await?;
    Ok(Json(settings))
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct UpdateSettingsInput {
    pricing_auto_update_enabled: Option<bool>,
    pricing_auto_update_interval_hours: Option<i64>,
    close_behavior: Option<storage::CloseBehavior>,
    auto_start_enabled: Option<bool>,
    auto_start_launch_mode: Option<storage::AutoStartLaunchMode>,
    app_auto_update_enabled: Option<bool>,
    auto_disable_enabled: Option<bool>,
    auto_disable_window_minutes: Option<i64>,
    auto_disable_failure_times: Option<i64>,
    auto_disable_disable_minutes: Option<i64>,
    log_level: Option<logging::LogLevel>,
    log_retention_days: Option<i64>,
}

pub(in crate::server) async fn update_settings(
    State(state): State<AppState>,
    Json(input): Json<UpdateSettingsInput>,
) -> Result<impl IntoResponse, ApiError> {
    let autostart_enabled_updated = input.auto_start_enabled.is_some();
    let autostart_mode_updated = input.auto_start_launch_mode.is_some();

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
            "app_auto_update_enabled",
            input.app_auto_update_enabled.is_some(),
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
        ("log_level", input.log_level.is_some()),
        ("log_retention_days", input.log_retention_days.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, is_changed)| is_changed.then_some(name))
    .collect();

    if let Some(h) = input.pricing_auto_update_interval_hours
        && !(1..=8760).contains(&h)
    {
        return Err(ApiError::BadRequest(
            "pricing_auto_update_interval_hours 必须在 1..=8760 之间".to_string(),
        ));
    }
    if let Some(v) = input.auto_disable_window_minutes
        && v < 1
    {
        return Err(ApiError::BadRequest(
            "auto_disable_window_minutes 必须 >= 1".to_string(),
        ));
    }
    if let Some(v) = input.auto_disable_failure_times
        && v < 1
    {
        return Err(ApiError::BadRequest(
            "auto_disable_failure_times 必须 >= 1".to_string(),
        ));
    }
    if let Some(v) = input.auto_disable_disable_minutes
        && v < 1
    {
        return Err(ApiError::BadRequest(
            "auto_disable_disable_minutes 必须 >= 1".to_string(),
        ));
    }
    if let Some(v) = input.log_retention_days
        && !(1..=3650).contains(&v)
    {
        return Err(ApiError::BadRequest(
            "log_retention_days 必须在 1..=3650 之间".to_string(),
        ));
    }

    let auto_start_enabled = input.auto_start_enabled;
    if let Some(enabled) = auto_start_enabled {
        let res = tokio::task::spawn_blocking(move || autostart::set_enabled(enabled)).await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(desired = enabled, err = %e, "set autostart failed");
                return Err(ApiError::BadRequest(format!("设置开机自启动失败：{e}")));
            }
            Err(e) => {
                tracing::warn!(desired = enabled, err = %e, "set autostart failed");
                return Err(ApiError::BadRequest(format!("设置开机自启动失败：{e}")));
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
            app_auto_update_enabled: input.app_auto_update_enabled,
            auto_disable_enabled: input.auto_disable_enabled,
            auto_disable_window_minutes: input.auto_disable_window_minutes,
            auto_disable_failure_times: input.auto_disable_failure_times,
            auto_disable_disable_minutes: input.auto_disable_disable_minutes,
            log_level: input.log_level,
            log_retention_days: input.log_retention_days,
        },
    )
    .await?;

    if autostart_mode_updated && settings.auto_start_enabled && !autostart_enabled_updated {
        let res = tokio::task::spawn_blocking(move || autostart::set_enabled(true)).await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(err = %e, "rewrite autostart args failed");
                return Err(ApiError::BadRequest(format!("更新开机自启动参数失败：{e}")));
            }
            Err(e) => {
                tracing::warn!(err = %e, "rewrite autostart args failed");
                return Err(ApiError::BadRequest(format!("更新开机自启动参数失败：{e}")));
            }
        }
    }

    if input.log_level.is_some() {
        let _ = logging::set_level(settings.log_level);
    }

    if !changed.is_empty() {
        tracing::info!(changed = ?changed, "settings updated");
    }

    let next = *state.settings_notify.borrow() + 1;
    let _ = state.settings_notify.send(next);

    Ok(Json(settings))
}
