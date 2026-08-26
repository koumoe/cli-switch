use axum::Json;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::server::handlers::remote::RemoteAccountResponse;
use crate::server::openai_auth;

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
    let response =
        openai_auth::refresh_persisted_account(&state.http_client, state.db_path(), account_id)
            .await?;
    let secret =
        crate::storage::get_openai_account_with_secret(state.db_path(), response.id.clone())
            .await?;
    if let Ok(quota) = crate::openai_quota::fetch(&state.http_client, &secret).await {
        crate::storage::update_openai_account_quota(state.db_path(), response.id.clone(), quota)
            .await?;
    }
    let response =
        crate::storage::get_openai_account_without_secret(state.db_path(), response.id).await?;
    Ok(Json(RemoteAccountResponse::from(response)))
}
