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
        return Err(ApiError::bad_request(
            "system_url_required",
            "url is required",
        ));
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| ApiError::bad_request("system_url_invalid", format!("Invalid url: {e}")))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(ApiError::bad_request(
                "system_url_scheme_not_supported",
                format!("Only http/https is supported, scheme={other}"),
            ));
        }
    }

    crate::server::open_in_browser(parsed.as_str())
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    Ok(StatusCode::NO_CONTENT)
}
