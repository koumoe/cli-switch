use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;
use serde::Serialize;

use crate::changelog;
use crate::events::{self, AppEvent};
use crate::server::AppState;
use crate::server::error::ApiError;
use crate::{storage, update};

pub(in crate::server) async fn update_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = storage::get_app_settings(state.db_path()).await?;
    let data_dir = state.data_dir();
    let db_path = state.db_path();
    Ok(Json(
        update::get_status(
            state.update_runtime.clone(),
            db_path,
            &data_dir,
            settings.app_auto_update_enabled,
        )
        .await,
    ))
}

pub(in crate::server) async fn update_check(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let data_dir = state.data_dir();
    let db_path = state.db_path();
    let res = update::check_latest(
        &state.http_client,
        state.update_runtime.clone(),
        db_path,
        &data_dir,
    )
    .await;
    Ok(Json(res))
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateChangelogQuery {
    pub version: String,
    pub locale: Option<String>,
}

pub(in crate::server) async fn update_changelog(
    State(state): State<AppState>,
    Query(q): Query<UpdateChangelogQuery>,
) -> Result<impl IntoResponse, ApiError> {
    if q.version.trim().is_empty() {
        return Err(ApiError::bad_request(
            "update.version_required",
            "version is required",
        ));
    }
    let locale = q.locale.unwrap_or_else(|| "en".to_string());
    let res = changelog::fetch_update_overview(&state.http_client, &q.version, &locale).await?;
    Ok(Json(res))
}

#[derive(Serialize)]
struct UpdateDownloadResponse {
    started: bool,
    status: update::UpdateStatus,
}

pub(in crate::server) async fn update_download(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = storage::get_app_settings(state.db_path()).await?;
    let data_dir = state.data_dir();
    let db_path = state.db_path();
    let started = update::spawn_download_latest(
        state.http_client.clone(),
        state.update_runtime.clone(),
        db_path.clone(),
        data_dir.clone(),
    )
    .await;
    let status = update::get_status(
        state.update_runtime.clone(),
        db_path,
        &data_dir,
        settings.app_auto_update_enabled,
    )
    .await;
    Ok(Json(UpdateDownloadResponse { started, status }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateIgnoreInput {
    pub version: String,
}

pub(in crate::server) async fn update_ignore(
    State(state): State<AppState>,
    Json(input): Json<UpdateIgnoreInput>,
) -> Result<impl IntoResponse, ApiError> {
    if input.version.trim().is_empty() {
        return Err(ApiError::bad_request(
            "update.version_required",
            "version is required",
        ));
    }
    storage::ignore_update_version(state.db_path(), &input.version).await?;

    let settings = storage::get_app_settings(state.db_path()).await?;
    let data_dir = state.data_dir();
    let status = update::get_status(
        state.update_runtime.clone(),
        state.db_path(),
        &data_dir,
        settings.app_auto_update_enabled,
    )
    .await;
    events::publish(AppEvent::UpdateStatus(status.clone()));
    Ok(Json(status))
}
