use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::proxy::ProxyError;

#[derive(Serialize)]
pub(crate) struct ErrorBody {
    pub(crate) error: String,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadGateway(String),
    #[error("{0}")]
    Unavailable(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ApiError::Unavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            ApiError::Internal(err) => {
                tracing::error!(err = %err, "api internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error".to_string(),
                )
            }
        };
        (status, Json(ErrorBody { error: msg })).into_response()
    }
}

pub(crate) fn map_proxy_error(e: ProxyError) -> ApiError {
    match e {
        ProxyError::NoEnabledChannel(p) => {
            ApiError::Unavailable(format!("未配置启用的 {} 渠道", p.as_str()))
        }
        ProxyError::NoAvailableChannel(p) => {
            ApiError::Unavailable(format!("无可用的 {} 渠道（可能被自动禁用）", p.as_str()))
        }
        ProxyError::InvalidBaseUrl(msg) => ApiError::Internal(anyhow::anyhow!(msg)),
        ProxyError::ReadBody(msg) => ApiError::BadRequest(msg),
        ProxyError::Upstream(msg) => ApiError::BadGateway(msg),
        ProxyError::Storage(e) => ApiError::Internal(e),
    }
}

pub(crate) fn map_storage_unit_no_content(
    res: anyhow::Result<()>,
    classify: impl FnOnce(&str) -> Option<ApiError>,
) -> Result<StatusCode, ApiError> {
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            let msg = e.to_string();
            if let Some(api_err) = classify(&msg) {
                Err(api_err)
            } else {
                Err(ApiError::Internal(e))
            }
        }
    }
}
