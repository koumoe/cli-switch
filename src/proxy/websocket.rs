use axum::body::Body;
use axum::extract::ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade};
use axum::http::{HeaderMap, Request, Response, StatusCode, header};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use reqwest::Url;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::{
    ProxyError, apply_auth, build_upstream_url, clear_channel_scoped_headers, filtered_headers,
    list_available_channels,
};
use crate::server::AppState;
use crate::storage::{self, Protocol};

pub(crate) const CODEX_RESPONSES_PATH: &str = "/v1/responses";

pub(crate) fn is_websocket_request(req: &Request<Body>) -> bool {
    let upgrade = req
        .headers()
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    let connection = req
        .headers()
        .get(header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .any(|v| v.trim().eq_ignore_ascii_case("upgrade"))
        });
    upgrade && connection
}

pub(crate) async fn reject_unsupported(state: &AppState, path: String) -> Response<Body> {
    record_local_failure(state, path, "local_endpoint_not_supported").await;
    (StatusCode::NOT_FOUND, "websocket endpoint not supported").into_response()
}

pub(crate) async fn record_local_failure(state: &AppState, path: String, reason: &str) {
    let endpoint = format!("ws://*{path}");
    if let Err(error) = storage::record_endpoint_failure(
        state.db_path(),
        endpoint,
        "ws".to_string(),
        reason.to_string(),
    )
    .await
    {
        tracing::warn!(%error, "record local websocket endpoint failure failed");
    }
}

type UpstreamSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct HandshakeFailure {
    error: ProxyError,
    retry_same_channel: bool,
    record_channel_failure: bool,
}

impl HandshakeFailure {
    fn retry(error: ProxyError) -> Self {
        Self {
            error,
            retry_same_channel: true,
            record_channel_failure: true,
        }
    }

    fn skip_channel(error: ProxyError) -> Self {
        Self {
            error,
            retry_same_channel: false,
            record_channel_failure: false,
        }
    }
}

pub(crate) async fn upgrade(
    state: &AppState,
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Result<Response<Body>, ProxyError> {
    let settings = state.settings_snapshot();
    let channels = list_available_channels(
        state.channels_snapshot().as_ref(),
        Protocol::Openai,
        storage::now_ms(),
        settings.as_ref(),
    )?;
    let mut last_error = None;
    let total_attempts = channels
        .iter()
        .map(|channel| super::channel_attempt_budget(channel, settings.as_ref(), false))
        .sum::<usize>();
    let mut overall_attempt = 0usize;

    for channel in channels {
        let channel_total = super::channel_attempt_budget(&channel, settings.as_ref(), false);
        for channel_attempt in 1..=channel_total {
            overall_attempt += 1;
            let has_more_attempts = channel_attempt < channel_total;
            let request = match build_websocket_request(state, &channel, &headers, &uri).await {
                Ok(request) => request,
                Err(failure) => {
                    let auto_disabled = if failure.record_channel_failure {
                        record_handshake_failure(
                            state,
                            settings.as_ref(),
                            &channel,
                            &failure.error,
                            overall_attempt,
                            total_attempts,
                        )
                        .await
                    } else {
                        false
                    };
                    last_error = Some(failure.error);
                    if !failure.retry_same_channel || auto_disabled || !has_more_attempts {
                        break;
                    }
                    continue;
                }
            };

            match tokio_tungstenite::connect_async(request).await {
                Ok((upstream, _response)) => {
                    return Ok(ws
                        .on_upgrade(move |downstream| async move {
                            proxy_socket(downstream, upstream).await
                        })
                        .into_response());
                }
                Err(error) => {
                    let error = ProxyError::Upstream(format!(
                        "upstream websocket handshake failed: {error}"
                    ));
                    let auto_disabled = record_handshake_failure(
                        state,
                        settings.as_ref(),
                        &channel,
                        &error,
                        overall_attempt,
                        total_attempts,
                    )
                    .await;
                    last_error = Some(error);
                    if auto_disabled || !has_more_attempts {
                        break;
                    }
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ProxyError::Upstream("websocket handshake failed".into())))
}

async fn build_websocket_request(
    state: &AppState,
    channel: &storage::Channel,
    headers: &HeaderMap,
    uri: &axum::http::Uri,
) -> Result<http::Request<()>, HandshakeFailure> {
    let is_openai_oauth =
        channel.managed_provider() == Some(storage::ManagedRemoteProvider::Openai);
    let mut url = if is_openai_oauth {
        Url::parse(&crate::codex_upstream::responses_url(Some(
            &channel.base_url,
        )))
        .map_err(|error| HandshakeFailure::retry(ProxyError::InvalidBaseUrl(error.to_string())))?
    } else {
        build_upstream_url(&channel.base_url, uri, "/v1").map_err(HandshakeFailure::retry)?
    };
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        "ws" | "wss" => url.scheme(),
        other => {
            return Err(HandshakeFailure::retry(ProxyError::InvalidBaseUrl(
                format!("unsupported websocket scheme: {other}"),
            )));
        }
    };
    let scheme = scheme.to_string();
    url.set_scheme(&scheme)
        .map_err(|_| HandshakeFailure::retry(ProxyError::InvalidBaseUrl(url.to_string())))?;

    let mut forwarded = filtered_headers(headers);
    clear_channel_scoped_headers(&mut forwarded);
    if is_openai_oauth {
        let Some(account_id) = channel.managed_account_id() else {
            return Err(HandshakeFailure::skip_channel(ProxyError::Upstream(
                "OpenAI managed channel is missing account id".into(),
            )));
        };
        let account =
            storage::get_openai_account_with_secret(state.db_path(), account_id.to_string())
                .await
                .map_err(|error| HandshakeFailure::skip_channel(ProxyError::Storage(error)))?;
        if account.reauth_required {
            return Err(HandshakeFailure::skip_channel(ProxyError::Upstream(
                "OpenAI account requires login".into(),
            )));
        }
        crate::codex_upstream::apply_oauth_credentials(
            &mut forwarded,
            crate::codex_upstream::CodexCredentials {
                access_token: account.access_token.as_deref().unwrap_or_default(),
                account_id: &account.remote_user_id,
            },
        )
        .map_err(|error| HandshakeFailure::retry(ProxyError::Upstream(error.to_string())))?;
    } else {
        apply_auth(channel, Protocol::Openai, &mut url, &mut forwarded)
            .map_err(HandshakeFailure::retry)?;
    }

    let mut request = url.as_str().into_client_request().map_err(|error| {
        HandshakeFailure::retry(ProxyError::Upstream(format!(
            "build websocket request failed: {error}"
        )))
    })?;
    for (name, value) in forwarded {
        if let Some(name) = name {
            request.headers_mut().insert(name, value);
        }
    }
    Ok(request)
}

async fn record_handshake_failure(
    state: &AppState,
    settings: &storage::AppSettings,
    channel: &storage::Channel,
    error: &ProxyError,
    attempt: usize,
    total: usize,
) -> bool {
    let auto_disabled = super::record_failure_and_maybe_disable(
        &state.db_path(),
        settings,
        channel,
        Some(state.channels_cache.clone()),
    )
    .await;
    tracing::warn!(
        protocol = Protocol::Openai.as_str(),
        channel_id = %channel.id,
        attempt,
        total,
        err = %error,
        "websocket handshake attempt failed"
    );
    auto_disabled
}

async fn proxy_socket(mut downstream: WebSocket, mut upstream: UpstreamSocket) {
    loop {
        tokio::select! {
            incoming = downstream.next() => match incoming {
                Some(Ok(message)) => {
                    if let Some(message) = to_tungstenite(message) && upstream.send(message).await.is_err() { break; }
                }
                _ => break,
            },
            incoming = upstream.next() => match incoming {
                Some(Ok(message)) => {
                    if let Some(message) = to_axum(message) && downstream.send(message).await.is_err() { break; }
                }
                _ => break,
            },
        }
    }
}

fn to_tungstenite(message: AxumMessage) -> Option<Message> {
    match message {
        AxumMessage::Text(text) => Some(Message::Text(text.to_string())),
        AxumMessage::Binary(bytes) => Some(Message::Binary(bytes.to_vec())),
        AxumMessage::Ping(bytes) => Some(Message::Ping(bytes.to_vec())),
        AxumMessage::Pong(bytes) => Some(Message::Pong(bytes.to_vec())),
        AxumMessage::Close(frame) => Some(Message::Close(frame.map(|frame| {
            tokio_tungstenite::tungstenite::protocol::CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            }
        }))),
    }
}

fn to_axum(message: Message) -> Option<AxumMessage> {
    match message {
        Message::Text(text) => Some(AxumMessage::Text(text.to_string().into())),
        Message::Binary(bytes) => Some(AxumMessage::Binary(bytes.to_vec().into())),
        Message::Ping(bytes) => Some(AxumMessage::Ping(bytes.to_vec().into())),
        Message::Pong(bytes) => Some(AxumMessage::Pong(bytes.to_vec().into())),
        Message::Close(frame) => Some(AxumMessage::Close(frame.map(|frame| {
            axum::extract::ws::CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            }
        }))),
        Message::Frame(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::is_websocket_request;
    use axum::body::Body;
    use axum::http::Request;

    fn request(upgrade: Option<&str>, connection: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder();
        if let Some(value) = upgrade {
            builder = builder.header("upgrade", value);
        }
        if let Some(value) = connection {
            builder = builder.header("connection", value);
        }
        builder.body(Body::empty()).unwrap()
    }

    #[test]
    fn recognizes_case_insensitive_websocket_upgrade() {
        let req = request(Some("WebSocket"), Some("keep-alive, Upgrade"));
        assert!(is_websocket_request(&req));
    }

    #[test]
    fn requires_both_upgrade_and_connection_tokens() {
        assert!(!is_websocket_request(&request(Some("websocket"), None)));
        assert!(!is_websocket_request(&request(None, Some("Upgrade"))));
        assert!(!is_websocket_request(&request(
            Some("websocket"),
            Some("keep-alive")
        )));
    }
}
