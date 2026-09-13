use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Json, http::StatusCode};
use serde::Deserialize;

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

#[derive(Debug, Deserialize)]
pub(in crate::server) struct EndpointFailuresQuery {
    limit: Option<usize>,
}

pub(in crate::server) async fn endpoint_failures(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<EndpointFailuresQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let items = storage::list_endpoint_failures(state.db_path(), query.limit.unwrap_or(5)).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

pub(in crate::server) async fn clear_endpoint_failures(
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    storage::clear_endpoint_failures(state.db_path()).await?;
    Ok(StatusCode::NO_CONTENT)
}
