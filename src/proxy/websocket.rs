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
    let total_channels = channels.len();

    for (channel_index, channel) in channels.into_iter().enumerate() {
        let channel_total = super::channel_attempt_budget(&channel, settings.as_ref(), false);
        for channel_attempt in 1..=channel_total {
            let attempt = channel_index + channel_attempt;
            let has_more_attempts = channel_attempt < channel_total;
            let has_more_channels = channel_index + 1 < total_channels;
            let is_openai_oauth =
                channel.managed_provider() == Some(storage::ManagedRemoteProvider::Openai);

            let mut url = if is_openai_oauth {
                match Url::parse(&crate::codex_upstream::responses_url(Some(
                    &channel.base_url,
                ))) {
                    Ok(url) => url,
                    Err(error) => {
                        let error = ProxyError::InvalidBaseUrl(error.to_string());
                        let auto_disabled = record_handshake_failure(
                            state,
                            settings.as_ref(),
                            &channel,
                            &error,
                            attempt,
                            total_channels,
                        )
                        .await;
                        last_error = Some(error);
                        if auto_disabled || !has_more_attempts {
                            break;
                        }
                        continue;
                    }
                }
            } else {
                match build_upstream_url(&channel.base_url, &uri, "/v1") {
                    Ok(url) => url,
                    Err(error) => {
                        let auto_disabled = record_handshake_failure(
                            state,
                            settings.as_ref(),
                            &channel,
                            &error,
                            attempt,
                            total_channels,
                        )
                        .await;
                        last_error = Some(error);
                        if auto_disabled || !has_more_attempts {
                            break;
                        }
                        continue;
                    }
                }
            };

            let scheme = match url.scheme() {
                "http" => "ws".to_string(),
                "https" => "wss".to_string(),
                "ws" | "wss" => url.scheme().to_string(),
                other => {
                    let error = ProxyError::InvalidBaseUrl(format!(
                        "unsupported websocket scheme: {other}"
                    ));
                    let auto_disabled = record_handshake_failure(
                        state,
                        settings.as_ref(),
                        &channel,
                        &error,
                        attempt,
                        total_channels,
                    )
                    .await;
                    last_error = Some(error);
                    if auto_disabled || !has_more_attempts {
                        break;
                    }
                    continue;
                }
            };
            if url.set_scheme(&scheme).is_err() {
                let error = ProxyError::InvalidBaseUrl(url.to_string());
                let auto_disabled = record_handshake_failure(
                    state,
                    settings.as_ref(),
                    &channel,
                    &error,
                    attempt,
                    total_channels,
                )
                .await;
                last_error = Some(error);
                if auto_disabled || !has_more_attempts {
                    break;
                }
                continue;
            }

            let mut forwarded = filtered_headers(&headers);
            clear_channel_scoped_headers(&mut forwarded);
            if is_openai_oauth {
                let Some(account_id) = channel.managed_account_id() else {
                    last_error = Some(ProxyError::Upstream(
                        "OpenAI managed channel is missing account id".into(),
                    ));
                    break;
                };
                let account = match storage::get_openai_account_with_secret(
                    state.db_path(),
                    account_id.to_string(),
                )
                .await
                {
                    Ok(account) => account,
                    Err(error) => {
                        last_error = Some(ProxyError::Storage(error));
                        break;
                    }
                };
                if account.reauth_required {
                    last_error = Some(ProxyError::Upstream("OpenAI account requires login".into()));
                    break;
                }
                if let Err(error) = crate::codex_upstream::apply_oauth_credentials(
                    &mut forwarded,
                    crate::codex_upstream::CodexCredentials {
                        access_token: account.access_token.as_deref().unwrap_or_default(),
                        account_id: &account.remote_user_id,
                    },
                ) {
                    last_error = Some(ProxyError::Upstream(error.to_string()));
                    break;
                }
            } else if let Err(error) =
                apply_auth(&channel, Protocol::Openai, &mut url, &mut forwarded)
            {
                let auto_disabled = record_handshake_failure(
                    state,
                    settings.as_ref(),
                    &channel,
                    &error,
                    attempt,
                    total_channels,
                )
                .await;
                last_error = Some(error);
                if auto_disabled || !has_more_attempts {
                    break;
                }
                continue;
            }

            let mut request = match url.as_str().into_client_request() {
                Ok(request) => request,
                Err(error) => {
                    let error =
                        ProxyError::Upstream(format!("build websocket request failed: {error}"));
                    let auto_disabled = record_handshake_failure(
                        state,
                        settings.as_ref(),
                        &channel,
                        &error,
                        attempt,
                        total_channels,
                    )
                    .await;
                    last_error = Some(error);
                    if auto_disabled || !has_more_attempts {
                        break;
                    }
                    continue;
                }
            };
            for (name, value) in forwarded {
                if let Some(name) = name {
                    request.headers_mut().insert(name, value);
                }
            }

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
                        attempt,
                        total_channels,
                    )
                    .await;
                    last_error = Some(error);
                    if auto_disabled || !has_more_attempts {
                        break;
                    }
                }
            }
            if !has_more_attempts && !has_more_channels {
                break;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ProxyError::Upstream("websocket handshake failed".into())))
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
