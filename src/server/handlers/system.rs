use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::server::AppState;
use crate::server::error::ApiError;

#[derive(Debug, Deserialize)]
pub(in crate::server) struct OpenInBrowserInput {
    url: String,
}

pub(in crate::server) async fn open_in_browser(
    Json(input): Json<OpenInBrowserInput>,
) -> Result<impl IntoResponse, ApiError> {
    let url = input.url.trim();
    if url.is_empty() {
        return Err(ApiError::bad_request(
            "system.url_required",
            "url is required",
        ));
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| ApiError::bad_request("system.url_invalid", format!("Invalid url: {e}")))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(ApiError::bad_request(
                "system.url_scheme_not_supported",
                format!("Only http/https is supported, scheme={other}"),
            ));
        }
    }

    crate::server::open_in_browser(parsed.as_str())
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(in crate::server) async fn open_data_dir(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(data_dir) = state.db_path.parent() else {
        return Err(ApiError::Internal(anyhow::anyhow!(
            "db path has no parent dir"
        )));
    };

    std::fs::create_dir_all(data_dir)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("create data dir failed: {e}")))?;
    crate::server::open_path(data_dir).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(in crate::server) struct PickFolderInput {
    title: Option<String>,
    directory: Option<String>,
}

#[derive(Debug, Serialize)]
pub(in crate::server) struct PickFolderResponse {
    path: Option<String>,
}

pub(in crate::server) async fn pick_folder(
    Json(input): Json<PickFolderInput>,
) -> Result<Json<PickFolderResponse>, ApiError> {
    #[cfg(not(feature = "desktop"))]
    {
        let _ = input;
        Err(ApiError::bad_request(
            "system.dialog_not_supported",
            "folder picker not supported",
        ))
    }

    #[cfg(feature = "desktop")]
    {
        let title = input.title.unwrap_or_else(|| "Select folder".to_string());
        let directory = input.directory.unwrap_or_default();
        let res = tokio::task::spawn_blocking(move || {
            let mut dlg = rfd::FileDialog::new().set_title(&title);
            let dir = std::path::PathBuf::from(directory.trim());
            if dir.is_dir() {
                dlg = dlg.set_directory(dir);
            }
            dlg.pick_folder()
        })
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("pick folder join failed: {e}")))?;

        Ok(Json(PickFolderResponse {
            path: res.map(|p| p.to_string_lossy().to_string()),
        }))
    }
}
