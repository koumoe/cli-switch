use axum::Json;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::openai_quota::OpenAiQuotaResetOutcome;
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

#[derive(Debug, Deserialize)]
pub(in crate::server) struct ResetOpenAiQuotaInput {
    idempotency_key: String,
}

#[derive(Debug, Serialize)]
struct ResetOpenAiQuotaResponse {
    outcome: OpenAiQuotaResetOutcome,
    account: RemoteAccountResponse,
    quota_refresh_error: Option<String>,
}

pub(in crate::server) async fn reset_openai_account_quota(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    Json(input): Json<ResetOpenAiQuotaInput>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        reset_openai_account_quota_at(
            state.db_path(),
            &state.http_client,
            account_id,
            &input.idempotency_key,
            None,
            None,
        )
        .await?,
    ))
}

async fn reset_openai_account_quota_at(
    db_path: PathBuf,
    client: &reqwest::Client,
    account_id: String,
    idempotency_key: &str,
    reset_url: Option<&str>,
    usage_url: Option<&str>,
) -> Result<ResetOpenAiQuotaResponse, ApiError> {
    // Keep the caller's UUID unchanged across authentication and network retries.
    uuid::Uuid::parse_str(idempotency_key).map_err(|_| {
        ApiError::bad_request(
            "openai_quota_reset_invalid_key",
            "A UUID idempotency key is required",
        )
    })?;
    let mut account = storage::get_openai_account_with_secret(db_path.clone(), account_id.clone())
        .await
        .map_err(map_openai_storage_error)?;
    let outcome = match crate::openai_quota::consume_reset(
        client,
        &account,
        idempotency_key,
        reset_url,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(error) if error.is_auth_failure() => {
            account =
                openai_auth::refresh_persisted_account(client, db_path.clone(), account_id.clone())
                    .await
                    .map_err(map_openai_refresh_error)?;
            crate::openai_quota::consume_reset(client, &account, idempotency_key, reset_url)
                .await
                .map_err(map_openai_quota_reset_error)?
        }
        Err(error) => return Err(map_openai_quota_reset_error(error)),
    };
    // Once a redemption has a known outcome, never report it as an unknown
    // failure merely because the subsequent database/usage refresh failed.
    let refresh = match storage::invalidate_openai_account_quota_reset_count(
        db_path.clone(),
        account_id.clone(),
    )
    .await
    {
        Ok(()) => match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            refresh_openai_account_with_client(
                db_path.clone(),
                client,
                account_id.clone(),
                usage_url,
            ),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(ApiError::bad_gateway(
                "openai_quota_fetch_failed",
                "Quota refresh timed out after reset",
            )),
        },
        Err(error) => Err(map_openai_storage_error(error)),
    };
    match refresh {
        Ok(account) => Ok(ResetOpenAiQuotaResponse {
            outcome,
            account,
            quota_refresh_error: None,
        }),
        Err(error) => {
            let mut fallback = storage::get_openai_account_without_secret(db_path, account_id)
                .await
                .unwrap_or(account);
            fallback.quota.reset_available_count = None;
            Ok(ResetOpenAiQuotaResponse {
                outcome,
                account: fallback.into(),
                quota_refresh_error: Some(error.to_string()),
            })
        }
    }
}

fn map_openai_quota_reset_error(error: crate::openai_quota::OpenAiQuotaError) -> ApiError {
    ApiError::bad_gateway(
        "openai_quota_reset_failed",
        format!("Failed to reset OpenAI quota: {error}"),
    )
}

#[cfg(test)]
#[path = "openai_reset_tests.rs"]
mod reset_tests;

fn map_openai_storage_error(err: anyhow::Error) -> ApiError {
    match err.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::RemoteAccountNotFound { .. }) => {
            ApiError::not_found("remote_account_not_found", "Remote account not found")
        }
        _ => ApiError::Internal(err),
    }
}

fn map_openai_refresh_error(err: anyhow::Error) -> ApiError {
    if err.downcast_ref::<storage::StorageError>().is_some() {
        return map_openai_storage_error(err);
    }
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

pub(super) async fn refresh_openai_account_data_at(
    state: &AppState,
    account_id: String,
    usage_url: Option<&str>,
) -> Result<RemoteAccountResponse, ApiError> {
    refresh_openai_account_with_client(state.db_path(), &state.http_client, account_id, usage_url)
        .await
}

async fn refresh_openai_account_with_client(
    db_path: PathBuf,
    client: &reqwest::Client,
    account_id: String,
    usage_url: Option<&str>,
) -> Result<RemoteAccountResponse, ApiError> {
    let mut account = storage::get_openai_account_with_secret(db_path.clone(), account_id.clone())
        .await
        .map_err(map_openai_storage_error)?;
    let quota = match crate::openai_quota::fetch(client, &account, usage_url).await {
        Ok(quota) => quota,
        Err(error) if error.is_auth_failure() => {
            account =
                openai_auth::refresh_persisted_account(client, db_path.clone(), account_id.clone())
                    .await
                    .map_err(map_openai_refresh_error)?;
            match crate::openai_quota::fetch(client, &account, usage_url).await {
                Ok(quota) => quota,
                Err(error) => {
                    let _ = storage::mark_openai_account_auth_failure(
                        db_path.clone(),
                        account_id.clone(),
                        error.to_string(),
                        None,
                    )
                    .await;
                    return Err(map_openai_quota_error(error));
                }
            }
        }
        Err(error) => {
            let _ = storage::mark_openai_account_auth_failure(
                db_path.clone(),
                account_id.clone(),
                error.to_string(),
                None,
            )
            .await;
            return Err(map_openai_quota_error(error));
        }
    };
    storage::update_openai_account_quota(db_path.clone(), account_id.clone(), quota)
        .await
        .map_err(map_openai_storage_error)?;
    let response = storage::get_openai_account_without_secret(db_path, account_id)
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

    #[test]
    fn preserves_storage_errors_when_mapping_refresh_failures() {
        let error = anyhow::Error::new(storage::StorageError::RemoteAccountNotFound {
            account_id: "missing-account".to_string(),
        });
        let response = map_openai_refresh_error(error).into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
