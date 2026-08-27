use axum::Json;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::server::handlers::remote::RemoteAccountResponse;
use crate::server::openai_auth;
use crate::storage;

#[derive(Debug, Deserialize, Default)]
pub(in crate::server) struct StartOpenAiOAuthInput {
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAiOAuthStatusResponse {
    request_id: String,
    status: openai_auth::OAuthPublicStatus,
    account: Option<RemoteAccountResponse>,
    error: Option<String>,
    expires_at_ms: i64,
}

pub(in crate::server) async fn start_openai_oauth(
    State(state): State<AppState>,
    input: Option<Json<StartOpenAiOAuthInput>>,
) -> Result<impl IntoResponse, ApiError> {
    let name = input.and_then(|Json(input)| input.name);
    let response =
        openai_auth::start_oauth(state.db_path(), state.http_client.clone(), name).await?;
    Ok(Json(response))
}

pub(in crate::server) async fn get_openai_oauth_status(
    Path(request_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(response) = openai_auth::oauth_status(&request_id).await? else {
        return Err(ApiError::not_found(
            "openai_oauth_session_not_found",
            "OpenAI OAuth session was not found",
        ));
    };
    Ok(Json(OpenAiOAuthStatusResponse {
        request_id: response.request_id,
        status: response.status,
        account: response.account.map(RemoteAccountResponse::from),
        error: response.error,
        expires_at_ms: response.expires_at_ms,
    }))
}

pub(in crate::server) async fn refresh_openai_account(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(refresh_openai_account_data(&state, account_id).await?))
}

fn map_openai_storage_error(err: anyhow::Error) -> ApiError {
    match err.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::RemoteAccountNotFound { .. }) => {
            ApiError::not_found("remote_account_not_found", "Remote account not found")
        }
        _ => ApiError::Internal(err),
    }
}

fn map_openai_refresh_error(err: anyhow::Error) -> ApiError {
    if let Some(message) = openai_auth::relogin_required_message(&err) {
        return ApiError::bad_gateway("openai_relogin_required", message);
    }
    ApiError::bad_gateway(
        "openai_refresh_failed",
        format!("Failed to refresh OpenAI account: {err}"),
    )
}

fn map_openai_quota_error(err: crate::openai_quota::OpenAiQuotaError) -> ApiError {
    ApiError::bad_gateway(
        "openai_quota_fetch_failed",
        format!("Failed to fetch OpenAI quota: {err}"),
    )
}

pub(super) async fn refresh_openai_account_data(
    state: &AppState,
    account_id: String,
) -> Result<RemoteAccountResponse, ApiError> {
    refresh_openai_account_data_at(state, account_id, None).await
}

async fn fetch_quota(
    state: &AppState,
    account: &storage::OpenAiAccount,
    usage_url: Option<&str>,
) -> Result<storage::OpenAiQuotaSnapshot, crate::openai_quota::OpenAiQuotaError> {
    match usage_url {
        Some(usage_url) => {
            crate::openai_quota::fetch_at(&state.http_client, account, usage_url).await
        }
        None => crate::openai_quota::fetch(&state.http_client, account).await,
    }
}

pub(super) async fn refresh_openai_account_data_at(
    state: &AppState,
    account_id: String,
    usage_url: Option<&str>,
) -> Result<RemoteAccountResponse, ApiError> {
    let mut account = storage::get_openai_account_with_secret(state.db_path(), account_id.clone())
        .await
        .map_err(map_openai_storage_error)?;
    let quota = match fetch_quota(state, &account, usage_url).await {
        Ok(quota) => quota,
        Err(error) if error.is_auth_failure() => {
            account = openai_auth::refresh_persisted_account(
                &state.http_client,
                state.db_path(),
                account_id.clone(),
            )
            .await
            .map_err(map_openai_refresh_error)?;
            match fetch_quota(state, &account, usage_url).await {
                Ok(quota) => quota,
                Err(error) => {
                    let reauth_required = error.is_auth_failure();
                    let _ = storage::mark_openai_account_auth_failure(
                        state.db_path(),
                        account_id.clone(),
                        error.to_string(),
                        reauth_required,
                    )
                    .await;
                    return Err(map_openai_quota_error(error));
                }
            }
        }
        Err(error) => {
            let _ = storage::mark_openai_account_auth_failure(
                state.db_path(),
                account_id.clone(),
                error.to_string(),
                false,
            )
            .await;
            return Err(map_openai_quota_error(error));
        }
    };
    storage::update_openai_account_quota(state.db_path(), account_id.clone(), quota)
        .await
        .map_err(map_openai_storage_error)?;
    let response = storage::get_openai_account_without_secret(state.db_path(), account_id)
        .await
        .map_err(map_openai_storage_error)?;
    Ok(RemoteAccountResponse::from(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn maps_oauth_upstream_rejection_to_bad_gateway() {
        let response = map_openai_refresh_error(anyhow::Error::msg(
            r#"OpenAI token endpoint returned HTTP 403: {"error":{"code":"unsupported_country_region_territory"}}"#
        ))
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read error body");
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("parse error body");
        assert_eq!(payload["code"], "openai_refresh_failed");
    }
}
