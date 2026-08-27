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
    proxy::forward_with_config(
        &state.proxy_http_client,
        Some(&state.openai_proxy_http_client),
        state.db_path(),
        storage::Protocol::Openai,
        "/v1",
        req,
        proxy::ProxyConfigSnapshot {
            settings: state.settings_snapshot(),
            channels: state.channels_snapshot(),
            channels_cache: Some(state.channels_cache.clone()),
        },
    )
    .await
    .map_err(map_proxy_error)
}

pub(in crate::server) async fn proxy_anthropic(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    proxy::forward_with_config(
        &state.proxy_http_client,
        None,
        state.db_path(),
        storage::Protocol::Anthropic,
        "/v1",
        req,
        proxy::ProxyConfigSnapshot {
            settings: state.settings_snapshot(),
            channels: state.channels_snapshot(),
            channels_cache: Some(state.channels_cache.clone()),
        },
    )
    .await
    .map_err(map_proxy_error)
}

pub(in crate::server) async fn proxy_gemini(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    proxy::forward_with_config(
        &state.proxy_http_client,
        None,
        state.db_path(),
        storage::Protocol::Gemini,
        "/v1beta",
        req,
        proxy::ProxyConfigSnapshot {
            settings: state.settings_snapshot(),
            channels: state.channels_snapshot(),
            channels_cache: Some(state.channels_cache.clone()),
        },
    )
    .await
    .map_err(map_proxy_error)
}
