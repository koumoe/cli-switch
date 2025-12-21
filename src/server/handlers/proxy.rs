use axum::body::Body;
use axum::extract::State;
use axum::http::Request;

use crate::proxy;
use crate::server::AppState;
use crate::server::error::{ApiError, map_proxy_error};
use crate::storage;

pub(in crate::server) async fn proxy_openai(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    proxy::forward(
        &state.http_client,
        state.db_path(),
        storage::Protocol::Openai,
        "/v1",
        req,
    )
    .await
    .map_err(map_proxy_error)
}

pub(in crate::server) async fn proxy_anthropic(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    proxy::forward(
        &state.http_client,
        state.db_path(),
        storage::Protocol::Anthropic,
        "/v1",
        req,
    )
    .await
    .map_err(map_proxy_error)
}

pub(in crate::server) async fn proxy_gemini(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    proxy::forward(
        &state.http_client,
        state.db_path(),
        storage::Protocol::Gemini,
        "/v1beta",
        req,
    )
    .await
    .map_err(map_proxy_error)
}
