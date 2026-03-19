use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

fn map_chat_bridge_storage_error(err: &anyhow::Error) -> Option<ApiError> {
    match err.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::ChatPairingTokenInvalid) => Some(ApiError::bad_request(
            "chat_bridge_pairing_token_invalid",
            "Pairing token is invalid",
        )),
        Some(storage::StorageError::ChatPairingTokenExpired) => Some(ApiError::bad_request(
            "chat_bridge_pairing_token_expired",
            "Pairing token has expired",
        )),
        Some(storage::StorageError::ChatPairingTokenUsed) => Some(ApiError::conflict(
            "chat_bridge_pairing_token_used",
            "Pairing token has already been used",
        )),
        Some(storage::StorageError::ChatPairingTokenPlatformMismatch { .. }) => {
            Some(ApiError::bad_request(
                "chat_bridge_pairing_token_platform_mismatch",
                "Pairing token platform mismatch",
            ))
        }
        Some(storage::StorageError::ChatBindingAlreadyExists { .. }) => Some(ApiError::conflict(
            "chat_bridge_binding_exists",
            "Chat binding already exists",
        )),
        Some(storage::StorageError::ChatBindingNotFound { .. }) => Some(ApiError::not_found(
            "chat_bridge_binding_not_found",
            "Chat binding not found",
        )),
        _ => None,
    }
}

pub(in crate::server) async fn list_chat_bridge_bindings(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let items = storage::list_chat_bindings(state.db_path())
        .await
        .map_err(|err| map_chat_bridge_storage_error(&err).unwrap_or(ApiError::Internal(err)))?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct CreatePairingTokenInput {
    platform: storage::ChatPlatform,
    expires_in_minutes: Option<i64>,
}

pub(in crate::server) async fn create_chat_bridge_pairing_token(
    State(state): State<AppState>,
    Json(input): Json<CreatePairingTokenInput>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(v) = input.expires_in_minutes
        && !(1..=storage::MAX_PAIRING_TOKEN_EXPIRES_MINUTES).contains(&v)
    {
        return Err(ApiError::bad_request(
            "chat_bridge_pairing_token_expires_out_of_range",
            format!(
                "expires_in_minutes must be between 1 and {}",
                storage::MAX_PAIRING_TOKEN_EXPIRES_MINUTES
            ),
        ));
    }

    let token = storage::create_pairing_token(
        state.db_path(),
        storage::CreatePairingTokenInput {
            platform: input.platform,
            expires_in_minutes: input.expires_in_minutes,
        },
    )
    .await
    .map_err(|err| map_chat_bridge_storage_error(&err).unwrap_or(ApiError::Internal(err)))?;

    Ok((StatusCode::CREATED, Json(token)))
}

pub(in crate::server) async fn deactivate_chat_bridge_binding(
    State(state): State<AppState>,
    Path(binding_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    storage::deactivate_chat_binding(state.db_path(), binding_id)
        .await
        .map_err(|err| map_chat_bridge_storage_error(&err).unwrap_or(ApiError::Internal(err)))?;
    Ok(StatusCode::NO_CONTENT)
}
