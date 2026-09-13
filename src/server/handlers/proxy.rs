use axum::body::Body;
use axum::extract::FromRequestParts;
use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::Request;

use crate::proxy;
use crate::server::AppState;
use crate::server::error::{ApiError, map_proxy_error};
use crate::storage;

async fn reject_unsupported_websocket_if_needed(
    state: &AppState,
    is_websocket: bool,
    path: String,
    supported_path: Option<&str>,
) -> Option<axum::response::Response> {
    let unsupported = is_websocket && supported_path.is_none_or(|supported| path != supported);
    if unsupported {
        Some(proxy::websocket::reject_unsupported(state, path).await)
    } else {
        None
    }
}

pub(in crate::server) async fn proxy_openai(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    if proxy::websocket::is_websocket_request(&req) {
        if let Some(response) = reject_unsupported_websocket_if_needed(
            &state,
            true,
            req.uri().path().trim_end_matches('/').to_string(),
            Some(proxy::websocket::CODEX_RESPONSES_PATH),
        )
        .await
        {
            return Ok(response);
        }
        let (mut parts, _body) = req.into_parts();
        let ws = match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
            Ok(ws) => ws,
            Err(_) => {
                proxy::websocket::record_local_failure(
                    &state,
                    parts.uri.path().to_string(),
                    "local_handshake_rejected",
                )
                .await;
                return Err(ApiError::bad_request(
                    "proxy_websocket_handshake_invalid",
                    "Invalid WebSocket handshake",
                ));
            }
        };
        return proxy::websocket::upgrade(&state, ws, parts.headers, parts.uri)
            .await
            .map_err(map_proxy_error);
    }
    proxy::forward_with_config(
        &state.proxy_http_client,
        Some(&state.openai_oauth_client_pool),
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
    if let Some(response) = reject_unsupported_websocket_if_needed(
        &state,
        proxy::websocket::is_websocket_request(&req),
        req.uri().path().trim_end_matches('/').to_string(),
        None,
    )
    .await
    {
        return Ok(response);
    }
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
    if let Some(response) = reject_unsupported_websocket_if_needed(
        &state,
        proxy::websocket::is_websocket_request(&req),
        req.uri().path().trim_end_matches('/').to_string(),
        None,
    )
    .await
    {
        return Ok(response);
    }
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
