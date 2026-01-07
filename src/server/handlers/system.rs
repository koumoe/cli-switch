use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

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
        return Err(ApiError::BadRequest("url 不能为空".to_string()));
    }
    let parsed =
        reqwest::Url::parse(url).map_err(|e| ApiError::BadRequest(format!("url 无效：{e}")))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(ApiError::BadRequest(format!(
                "仅支持 http/https url，当前 scheme={other}"
            )));
        }
    }

    crate::server::open_in_browser(parsed.as_str())
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    Ok(StatusCode::NO_CONTENT)
}
