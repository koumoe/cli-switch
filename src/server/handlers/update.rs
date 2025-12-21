use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::{storage, update};

pub(in crate::server) async fn update_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = storage::get_app_settings(state.db_path()).await?;
    let data_dir = state.data_dir();
    Ok(Json(
        update::get_status(
            state.update_runtime.clone(),
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
    let res =
        update::check_latest(&state.http_client, state.update_runtime.clone(), &data_dir).await;
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
    let started = update::spawn_download_latest(
        state.http_client.clone(),
        state.update_runtime.clone(),
        data_dir.clone(),
    )
    .await;
    let status = update::get_status(
        state.update_runtime.clone(),
        &data_dir,
        settings.app_auto_update_enabled,
    )
    .await;
    Ok(Json(UpdateDownloadResponse { started, status }))
}
