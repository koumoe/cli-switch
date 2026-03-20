use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use std::collections::BTreeMap;

use crate::i18n::{current_locale, render_optional};
use crate::proxy::ProxyError;

#[derive(Serialize)]
pub(crate) struct ErrorBody {
    pub(crate) code: String,
    pub(crate) message: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) args: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) detail: Option<String>,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum ApiError {
    #[error("{message}")]
    BadRequest { code: &'static str, message: String },
    #[error("{message}")]
    NotFound { code: &'static str, message: String },
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("{message}")]
    BadGateway { code: &'static str, message: String },
    #[error("{message}")]
    Unavailable { code: &'static str, message: String },
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    pub(crate) fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        ApiError::BadRequest {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        ApiError::NotFound {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        ApiError::Conflict {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn bad_gateway(code: &'static str, message: impl Into<String>) -> Self {
        ApiError::BadGateway {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn unavailable(code: &'static str, message: impl Into<String>) -> Self {
        ApiError::Unavailable {
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let locale = current_locale();
        let (status, code, fallback_message, detail) = match &self {
            ApiError::BadRequest { code, message } => (
                StatusCode::BAD_REQUEST,
                (*code).to_string(),
                message.clone(),
                Some(message.clone()),
            ),
            ApiError::NotFound { code, message } => (
                StatusCode::NOT_FOUND,
                (*code).to_string(),
                message.clone(),
                Some(message.clone()),
            ),
            ApiError::Conflict { code, message } => (
                StatusCode::CONFLICT,
                (*code).to_string(),
                message.clone(),
                Some(message.clone()),
            ),
            ApiError::BadGateway { code, message } => (
                StatusCode::BAD_GATEWAY,
                (*code).to_string(),
                message.clone(),
                Some(message.clone()),
            ),
            ApiError::Unavailable { code, message } => (
                StatusCode::SERVICE_UNAVAILABLE,
                (*code).to_string(),
                message.clone(),
                Some(message.clone()),
            ),
            ApiError::Internal(err) => {
                tracing::error!(err = %err, "api internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error".to_string(),
                    "Internal error".to_string(),
                    None,
                )
            }
        };
        let translated_message =
            render_optional(locale.unwrap_or_default(), &code, &BTreeMap::new())
                .or_else(|| {
                    render_optional(
                        locale.unwrap_or_default(),
                        &format!("errors.{code}"),
                        &BTreeMap::new(),
                    )
                })
                .unwrap_or(fallback_message);
        let detail = detail.filter(|value| value.trim() != translated_message.trim());
        (
            status,
            Json(ErrorBody {
                code,
                message: translated_message,
                args: BTreeMap::new(),
                detail,
            }),
        )
            .into_response()
    }
}

pub(crate) fn map_proxy_error(e: ProxyError) -> ApiError {
    match e {
        ProxyError::NoEnabledChannel(p) => ApiError::unavailable(
            "proxy_no_enabled_channel",
            format!("No enabled {} channel configured", p.as_str()),
        ),
        ProxyError::NoAvailableChannel(p) => ApiError::unavailable(
            "proxy_no_available_channel",
            format!("No available {} channel (may be auto-disabled)", p.as_str()),
        ),
        ProxyError::InvalidBaseUrl(_) => {
            ApiError::bad_request("proxy_invalid_base_url", "Invalid channel base_url")
        }
        ProxyError::ReadBody(_) => {
            ApiError::bad_request("proxy_read_body_failed", "Failed to read request body")
        }
        ProxyError::Upstream(_) => {
            ApiError::bad_gateway("proxy_upstream_failed", "Upstream request failed")
        }
        ProxyError::Storage(e) => ApiError::Internal(e),
    }
}

pub(crate) fn map_storage_unit_no_content_err(
    res: anyhow::Result<()>,
    classify: impl FnOnce(&anyhow::Error) -> Option<ApiError>,
) -> Result<StatusCode, ApiError> {
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            if let Some(api_err) = classify(&e) {
                Err(api_err)
            } else {
                Err(ApiError::Internal(e))
            }
        }
    }
}
