use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, Response, StatusCode};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt as _;
use futures_util::TryStreamExt as _;
use reqwest::Url;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch;
use uuid::Uuid;

use crate::storage::{self, Channel, Protocol};

mod limits;
mod stream;
mod usage_writer;

use stream::{InstrumentedStream, StreamRecordContext};

#[derive(Clone)]
pub struct ProxyConfigSnapshot {
    pub settings: Arc<storage::AppSettings>,
    pub channels: Arc<Vec<Channel>>,
    /// Optional: allows proxy to best-effort update in-memory channel state (e.g. auto-disable).
    pub channels_cache: Option<watch::Sender<Arc<Vec<Channel>>>>,
}

struct AttemptCtx<'a> {
    db_path: &'a Path,
    settings: &'a storage::AppSettings,
    protocol: Protocol,
    channel_id: &'a str,
    attempt: usize,
    total: usize,
    channels_cache: Option<watch::Sender<Arc<Vec<Channel>>>>,
}

impl AttemptCtx<'_> {
    fn fail(&self, err: &ProxyError, msg: &'static str) {
        maybe_record_failure(
            self.db_path,
            self.settings,
            self.channel_id,
            self.channels_cache.clone(),
        );
        tracing::warn!(
            protocol = self.protocol.as_str(),
            channel_id = %self.channel_id,
            attempt = self.attempt,
            total = self.total,
            err = %err,
            "{msg}"
        );
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ProxyError {
    #[error("未配置可用渠道：{0}")]
    NoEnabledChannel(Protocol),
    #[error("无可用渠道（可能被自动禁用）：{0}")]
    NoAvailableChannel(Protocol),
    #[error("渠道 base_url 无效：{0}")]
    InvalidBaseUrl(String),
    #[error("读取请求体失败：{0}")]
    ReadBody(String),
    #[error("发送上游请求失败：{0}")]
    Upstream(String),
    #[error(transparent)]
    Storage(#[from] anyhow::Error),
}

pub async fn forward(
    client: &reqwest::Client,
    db_path: std::path::PathBuf,
    protocol: Protocol,
    protocol_root: &'static str,
    req: Request<Body>,
) -> Result<Response<Body>, ProxyError> {
    let settings = Arc::new(storage::get_app_settings(db_path.clone()).await?);
    let channels = Arc::new(storage::list_channels(db_path.clone()).await?);

    forward_with_config(
        client,
        db_path,
        protocol,
        protocol_root,
        req,
        ProxyConfigSnapshot {
            settings,
            channels,
            channels_cache: None,
        },
    )
    .await
}

pub async fn forward_with_config(
    client: &reqwest::Client,
    db_path: std::path::PathBuf,
    protocol: Protocol,
    protocol_root: &'static str,
    req: Request<Body>,
    cfg: ProxyConfigSnapshot,
) -> Result<Response<Body>, ProxyError> {
    let db_path_ref = db_path.as_path();
    let request_id: Arc<str> = Arc::from(Uuid::new_v4().to_string());

    let ProxyConfigSnapshot {
        settings,
        channels: all_channels,
        channels_cache,
    } = cfg;

    let (parts, body) = req.into_parts();
    let is_count_tokens = protocol == Protocol::Anthropic
        && parts.uri.path().trim_end_matches('/') == "/v1/messages/count_tokens";

    if is_count_tokens && settings.anthropic_count_tokens_mock_enabled {
        // Some clients (e.g. Claude Code) use this endpoint heavily. When upstream providers
        // don't support it (403/404) or are unreachable, they may fall back to other models.
        // This is a best-effort local simulation; it is NOT an official Anthropic tokenizer.
        let body_bytes = to_bytes(body, limits::MAX_INBOUND_BODY_BYTES)
            .await
            .map_err(|e| ProxyError::ReadBody(e.to_string()))?;
        let input_tokens = mock_anthropic_count_tokens_input_tokens(&body_bytes);
        return Ok(build_anthropic_count_tokens_response(input_tokens));
    }

    let now_ms = storage::now_ms();
    let channels =
        list_available_channels(all_channels.as_ref(), protocol, now_ms, settings.as_ref())?;
    let total_channels = channels.len();
    let body_bytes = to_bytes(body, limits::MAX_INBOUND_BODY_BYTES)
        .await
        .map_err(|e| ProxyError::ReadBody(e.to_string()))?;

    let model = extract_model(protocol, &parts.headers, &parts.uri, &body_bytes);

    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
        .map_err(|e| ProxyError::Upstream(format!("invalid method: {e}")))?;

    let mut last_err: Option<ProxyError> = None;

    for (idx, channel) in channels.into_iter().enumerate() {
        let is_last = idx + 1 >= total_channels;
        let started = Instant::now();

        let attempt_ctx = AttemptCtx {
            db_path: db_path_ref,
            settings: settings.as_ref(),
            protocol,
            channel_id: channel.id.as_str(),
            attempt: idx + 1,
            total: total_channels,
            channels_cache: channels_cache.clone(),
        };

        if !is_count_tokens {
            tracing::debug!(
                protocol = protocol.as_str(),
                request_id = %request_id,
                channel_id = %channel.id,
                attempt = idx + 1,
                total = total_channels,
                "proxy attempt start"
            );
        }

        let mut url = match build_upstream_url(&channel.base_url, &parts.uri, protocol_root) {
            Ok(v) => v,
            Err(e) => {
                if !is_count_tokens {
                    attempt_ctx.fail(&e, "proxy attempt failed (build url)");
                }
                last_err = Some(e);
                if is_last {
                    break;
                }
                continue;
            }
        };

        let mut out_headers = filtered_headers(&parts.headers);
        // Prefer identity encoding to reduce decode errors on streaming responses when upstream
        // closes the connection abruptly.
        out_headers.insert(
            axum::http::header::ACCEPT_ENCODING,
            HeaderValue::from_static("identity"),
        );
        if let Err(e) = apply_auth(&channel, protocol, &mut url, &mut out_headers) {
            if !is_count_tokens {
                attempt_ctx.fail(&e, "proxy attempt failed (apply auth)");
            }
            last_err = Some(e);
            if is_last {
                break;
            }
            continue;
        }

        let upstream = match client
            .request(method.clone(), url)
            .headers(out_headers)
            .body(body_bytes.clone())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                if !is_count_tokens {
                    maybe_record_failure(
                        db_path_ref,
                        settings.as_ref(),
                        &channel.id,
                        channels_cache.clone(),
                    );
                    tracing::warn!(
                        protocol = protocol.as_str(),
                        channel_id = %channel.id,
                        attempt = idx + 1,
                        total = total_channels,
                        err = %e,
                        "proxy attempt failed (request error)"
                    );
                    spawn_usage_event(
                        build_usage_event(UsageEventParams {
                            request_id: Some(request_id.clone()),
                            protocol,
                            channel_id: channel.id.clone(),
                            model: model.clone(),
                            success: false,
                            http_status: None,
                            error_kind: Some(format!(
                                "upstream_error:{}",
                                truncate(&e.to_string(), 240)
                            )),
                            error_detail: Some(truncate(&e.to_string(), 2000)),
                            latency_ms: started.elapsed().as_millis() as i64,
                            ttft_ms: None,
                            tokens: (None, None, None, None, None),
                        }),
                        db_path.clone(),
                    );
                }

                let err = ProxyError::Upstream(e.to_string());
                last_err = Some(err);
                if is_last {
                    break;
                }
                continue;
            }
        };

        let status = upstream.status();
        if is_count_tokens && !status.is_success() && !is_last {
            // Claude Code relies heavily on this endpoint; some 3P providers don't support it
            // (404/403). Retry on next channel but don't auto-disable or emit usage events.
            continue;
        }
        if !is_count_tokens && !status.is_success() {
            maybe_record_failure(
                db_path_ref,
                settings.as_ref(),
                &channel.id,
                channels_cache.clone(),
            );
        }
        if !is_count_tokens && !status.is_success() && !is_last {
            tracing::warn!(
                protocol = protocol.as_str(),
                channel_id = %channel.id,
                attempt = idx + 1,
                total = total_channels,
                http_status = status.as_u16(),
                "proxy attempt got non-2xx, retry next channel"
            );
            let error_detail = read_error_detail(protocol, upstream).await;
            spawn_usage_event(
                build_usage_event(UsageEventParams {
                    request_id: Some(request_id.clone()),
                    protocol,
                    channel_id: channel.id.clone(),
                    model: model.clone(),
                    success: false,
                    http_status: Some(status.as_u16() as i64),
                    error_kind: Some(format!("upstream_http:{}", status.as_u16())),
                    error_detail,
                    latency_ms: started.elapsed().as_millis() as i64,
                    ttft_ms: None,
                    tokens: (None, None, None, None, None),
                }),
                db_path.clone(),
            );
            continue;
        }

        if !is_count_tokens && status.is_success() && settings.auto_disable_enabled {
            spawn_clear_channel_failures(db_path.clone(), channel.id.clone());
        }

        return proxy_upstream_response(
            upstream,
            StreamRecordContext {
                db_path: db_path.clone(),
                protocol,
                channel_id: channel.id.clone(),
                model: model.clone(),
                request_id: request_id.clone(),
                http_status: 0,
                status_is_success: false,
                started,
                parse_sse: false, // 将在内部按 Content-Type 决定
                record_usage: !is_count_tokens,
                span: tracing::Span::current(),
            },
        )
        .await;
    }

    Err(last_err.unwrap_or_else(|| ProxyError::Upstream("all channels failed".to_string())))
}

fn build_anthropic_count_tokens_response(input_tokens: i64) -> Response<Body> {
    let payload = serde_json::json!({ "input_tokens": input_tokens });
    let mut resp = Response::new(Body::from(payload.to_string()));
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    resp
}

fn mock_anthropic_count_tokens_input_tokens(body: &[u8]) -> i64 {
    // Make the estimate stable across different JSON formatting by normalizing it.
    // serde_json::Map is sorted by key by default (no preserve_order), so serialization is stable.
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(body)
        && let Ok(s) = serde_json::to_string(&v)
    {
        return estimate_tokens_from_string(&s);
    }

    // Best-effort fallback when request isn't valid JSON.
    match std::str::from_utf8(body) {
        Ok(s) => estimate_tokens_from_string(s),
        Err(_) => body.len().div_ceil(4) as i64,
    }
}

fn estimate_tokens_from_string(s: &str) -> i64 {
    let ascii_bytes = s.as_bytes().iter().filter(|b| b.is_ascii()).count();
    let non_ascii_chars = s.chars().filter(|c| !c.is_ascii()).count();

    // Heuristic:
    // - ASCII: ~4 bytes per token on average
    // - Non-ASCII: count each Unicode scalar value as 1 token (conservative for CJK)
    (ascii_bytes.div_ceil(4) + non_ascii_chars) as i64
}

fn list_available_channels(
    all_channels: &[Channel],
    protocol: Protocol,
    now_ms: i64,
    settings: &storage::AppSettings,
) -> Result<Vec<Channel>, ProxyError> {
    let enabled: Vec<Channel> = all_channels
        .iter()
        .filter(|c| c.enabled && c.protocol == protocol)
        .cloned()
        .collect();
    if enabled.is_empty() {
        return Err(ProxyError::NoEnabledChannel(protocol));
    }

    if !settings.auto_disable_enabled {
        return Ok(enabled);
    }

    let out: Vec<Channel> = enabled
        .into_iter()
        .filter(|c| !storage::channel_is_auto_disabled(c, now_ms))
        .collect();
    if out.is_empty() {
        return Err(ProxyError::NoAvailableChannel(protocol));
    }
    Ok(out)
}

fn maybe_record_failure(
    db_path: &Path,
    settings: &storage::AppSettings,
    channel_id: &str,
    channels_cache: Option<watch::Sender<Arc<Vec<Channel>>>>,
) {
    if !settings.auto_disable_enabled {
        return;
    }
    // Best-effort: never block proxy latency on auto-disable bookkeeping.
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    let db_path = db_path.to_path_buf();
    let channel_id = channel_id.to_string();
    let window_minutes = settings.auto_disable_window_minutes;
    let failure_times = settings.auto_disable_failure_times;
    let disable_minutes = settings.auto_disable_disable_minutes;
    tokio::spawn(async move {
        let now_ms = storage::now_ms();
        match storage::record_channel_failure_and_maybe_disable(
            db_path,
            channel_id.clone(),
            now_ms,
            window_minutes,
            failure_times,
            disable_minutes,
        )
        .await
        {
            Ok(Some(until_ms)) => {
                tracing::warn!(
                    channel_id = channel_id,
                    disabled_until_ms = until_ms,
                    "channel auto disabled"
                );
                if let Some(channels_cache) = channels_cache.as_ref() {
                    mark_channel_auto_disabled(channels_cache, &channel_id, until_ms, now_ms);
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(
                    channel_id = channel_id,
                    err = %e,
                    "record channel failure failed"
                );
            }
        }
    });
}

fn mark_channel_auto_disabled(
    channels_cache: &watch::Sender<Arc<Vec<Channel>>>,
    channel_id: &str,
    disabled_until_ms: i64,
    updated_at_ms: i64,
) {
    channels_cache.send_modify(|cur| {
        let mut next = (**cur).clone();
        let Some(channel) = next.iter_mut().find(|c| c.id == channel_id) else {
            return;
        };
        channel.auto_disabled_until_ms = disabled_until_ms;
        channel.updated_at_ms = updated_at_ms;
        *cur = Arc::new(next);
    });
}

fn spawn_clear_channel_failures(db_path: std::path::PathBuf, channel_id: String) {
    // Best-effort: don't block proxy latency; skip if runtime is shutting down.
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::spawn(async move {
        if let Err(e) = storage::clear_channel_failures(db_path, channel_id.clone()).await {
            tracing::debug!(channel_id = channel_id, err = %e, "clear channel failures failed");
        }
    });
}

async fn proxy_upstream_response(
    upstream: reqwest::Response,
    mut ctx: StreamRecordContext,
) -> Result<Response<Body>, ProxyError> {
    let status = upstream.status();
    ctx.http_status = status.as_u16() as i64;
    ctx.status_is_success = status.is_success();

    let headers = filtered_headers(upstream.headers());
    let content_type = upstream
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_sse = content_type.starts_with("text/event-stream");
    let is_json = content_type.contains("application/json") || content_type.contains("+json");

    ctx.parse_sse = is_sse;

    let mut resp = Response::builder().status(status);
    if let Some(h) = resp.headers_mut() {
        for (k, v) in headers.iter() {
            h.append(k, v.clone());
        }
    }

    if !ctx.record_usage {
        let stream = upstream
            .bytes_stream()
            .map_err(std::io::Error::other)
            .boxed();
        return resp
            .body(Body::from_stream(stream))
            .map_err(|e| ProxyError::Upstream(e.to_string()));
    }

    let can_capture_json = match upstream.content_length() {
        Some(n) => (n as usize) <= limits::MAX_JSON_CAPTURE_BYTES,
        None => true,
    };
    if !is_sse && is_json && can_capture_json {
        let (captured, remainder) = read_stream_prefix_or_all(
            upstream.bytes_stream().boxed(),
            limits::MAX_JSON_CAPTURE_BYTES,
        )
        .await
        .map_err(|e| ProxyError::Upstream(e.to_string()))?;

        let Some(remainder) = remainder else {
            let bytes = captured;

            // For small JSON responses we capture fully, parse once and reuse the Value for:
            // - pretty/normalized response preview logging
            // - token usage extraction
            // - error message extraction
            let parsed_json = serde_json::from_slice::<serde_json::Value>(&bytes).ok();

            let response_text = match &parsed_json {
                Some(v) => serde_json::to_string(v)
                    .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).to_string()),
                None => String::from_utf8_lossy(&bytes).to_string(),
            };
            let response_one_line = to_single_line(&response_text);
            let response_preview = truncate(&response_one_line, 4096);

            let duration_ms = ctx.started.elapsed().as_millis() as i64;
            let usage = parsed_json
                .as_ref()
                .map(|v| parse_usage_from_value(ctx.protocol, v))
                .unwrap_or_default();
            let (
                prompt_tokens,
                completion_tokens,
                total_tokens,
                cache_read_tokens,
                cache_write_tokens,
            ) = usage.as_event_fields();

            let http_status = Some(status.as_u16() as i64);
            let success = status.is_success();
            let error_kind = (!success).then(|| format!("upstream_http:{}", status.as_u16()));
            let error_detail = (!success).then(|| {
                let msg = parsed_json
                    .as_ref()
                    .and_then(|v| parse_error_message_from_value(ctx.protocol, v))
                    .unwrap_or_else(|| String::from_utf8_lossy(&bytes).to_string());
                truncate(&msg, 2000)
            });

            tracing::debug!(
                protocol = ctx.protocol.as_str(),
                request_id = %ctx.request_id,
                channel_id = %ctx.channel_id,
                model = ctx.model.as_deref().unwrap_or("-"),
                http_status = status.as_u16(),
                duration_ms,
                prompt_tokens = prompt_tokens.unwrap_or(-1),
                completion_tokens = completion_tokens.unwrap_or(-1),
                total_tokens = total_tokens.unwrap_or(-1),
                success,
                error_kind = error_kind.as_deref().unwrap_or("-"),
                response_preview = %response_preview,
                "proxy request result"
            );

            tracing::debug!(
                target: "proxy_body",
                protocol = ctx.protocol.as_str(),
                request_id = %ctx.request_id,
                channel_id = %ctx.channel_id,
                model = ctx.model.as_deref().unwrap_or("-"),
                http_status = status.as_u16(),
                duration_ms,
                response = %response_one_line,
                body = true,
                "proxy request result"
            );

            spawn_usage_event(
                build_usage_event(UsageEventParams {
                    request_id: Some(ctx.request_id.clone()),
                    protocol: ctx.protocol,
                    channel_id: ctx.channel_id.clone(),
                    model: ctx.model.clone(),
                    success,
                    http_status,
                    error_kind,
                    error_detail,
                    latency_ms: duration_ms,
                    ttft_ms: None,
                    tokens: (
                        prompt_tokens,
                        completion_tokens,
                        total_tokens,
                        cache_read_tokens,
                        cache_write_tokens,
                    ),
                }),
                ctx.db_path.clone(),
            );

            return resp
                .body(Body::from(bytes))
                .map_err(|e| ProxyError::Upstream(e.to_string()));
        };

        let prefix = captured;
        let combined =
            futures_util::stream::once(async move { Ok::<Bytes, reqwest::Error>(prefix) })
                .chain(remainder)
                .boxed();
        let stream = InstrumentedStream::new(combined, ctx);
        return resp
            .body(Body::from_stream(stream))
            .map_err(|e| ProxyError::Upstream(e.to_string()));
    }

    let stream = InstrumentedStream::new(upstream.bytes_stream().boxed(), ctx);

    resp.body(Body::from_stream(stream))
        .map_err(|e| ProxyError::Upstream(e.to_string()))
}

fn build_upstream_url(
    base_url: &str,
    inbound_uri: &axum::http::Uri,
    protocol_root: &'static str,
) -> Result<Url, ProxyError> {
    let mut url = Url::parse(base_url)
        .map_err(|e| ProxyError::InvalidBaseUrl(format!("{base_url} ({e})")))?;

    let base_path = url.path().trim_end_matches('/'); // "/" -> ""
    let inbound_path = inbound_uri.path();

    let merged_path = if base_path.is_empty() {
        inbound_path.to_string()
    } else if base_path.ends_with(protocol_root) && inbound_path.starts_with(protocol_root) {
        let suffix = &inbound_path[protocol_root.len()..];
        if suffix.is_empty() {
            base_path.to_string()
        } else {
            format!("{base_path}{suffix}")
        }
    } else {
        format!("{base_path}{inbound_path}")
    };

    url.set_path(&merged_path);
    url.set_query(inbound_uri.query());

    Ok(url)
}

fn filtered_headers(src: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();

    let mut connection_tokens = Vec::<String>::new();
    if let Some(v) = src.get(axum::http::header::CONNECTION)
        && let Ok(s) = v.to_str()
    {
        for token in s.split(',') {
            let t = token.trim().to_ascii_lowercase();
            if !t.is_empty() {
                connection_tokens.push(t);
            }
        }
    }

    for (name, value) in src.iter() {
        if is_hop_by_hop(name) {
            continue;
        }
        if name == axum::http::header::HOST || name == axum::http::header::CONTENT_LENGTH {
            continue;
        }
        if name == axum::http::header::ACCEPT_ENCODING {
            continue;
        }

        let lname = name.as_str().to_ascii_lowercase();
        if connection_tokens.iter().any(|t| t == &lname) {
            continue;
        }

        out.append(name.clone(), value.clone());
    }

    out
}

async fn read_error_detail(protocol: Protocol, upstream: reqwest::Response) -> Option<String> {
    let bytes = read_response_prefix_bytes(upstream, limits::MAX_ERROR_DETAIL_BYTES).await?;
    let msg = parse_error_message(protocol, &bytes)
        .unwrap_or_else(|| String::from_utf8_lossy(&bytes).to_string());
    Some(truncate(&msg, 2000))
}

async fn read_response_prefix_bytes(
    upstream: reqwest::Response,
    max_bytes: usize,
) -> Option<Bytes> {
    let mut stream = upstream.bytes_stream();
    let mut buf = BytesMut::new();
    while let Some(item) = stream.next().await {
        let chunk = item.ok()?;
        if buf.len() >= max_bytes {
            break;
        }
        let remain = max_bytes - buf.len();
        buf.extend_from_slice(&chunk[..chunk.len().min(remain)]);
        if chunk.len() > remain {
            break;
        }
    }
    Some(buf.freeze())
}

async fn read_stream_prefix_or_all(
    mut stream: futures_util::stream::BoxStream<'static, Result<Bytes, reqwest::Error>>,
    max_bytes: usize,
) -> Result<
    (
        Bytes,
        Option<futures_util::stream::BoxStream<'static, Result<Bytes, reqwest::Error>>>,
    ),
    reqwest::Error,
> {
    let mut buf = BytesMut::new();
    while let Some(item) = stream.next().await {
        let chunk = item?;
        if buf.len() + chunk.len() > max_bytes {
            buf.extend_from_slice(&chunk);
            return Ok((buf.freeze(), Some(stream)));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok((buf.freeze(), None))
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

pub(crate) fn apply_auth(
    channel: &Channel,
    protocol: Protocol,
    url: &mut Url,
    headers: &mut HeaderMap,
) -> Result<(), ProxyError> {
    let token = channel.auth_ref.trim();

    let detected = detect_request_auth_kind(protocol, headers, url);
    let auth_kind = resolve_auth_kind(protocol, detected);

    clear_auth(protocol, headers, url);
    apply_auth_kind(auth_kind, token, headers, url)?;

    if protocol == Protocol::Anthropic
        && !headers.contains_key(HeaderName::from_static("anthropic-version"))
    {
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            "2023-06-01"
                .parse()
                .map_err(|e| ProxyError::Upstream(format!("bad anthropic-version: {e}")))?,
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthKind {
    Bearer,
    XApiKey,
    XGoogApiKey,
    QueryKey,
}

fn detect_request_auth_kind(
    protocol: Protocol,
    headers: &HeaderMap,
    url: &Url,
) -> Option<AuthKind> {
    match protocol {
        Protocol::Openai => headers
            .contains_key(axum::http::header::AUTHORIZATION)
            .then_some(AuthKind::Bearer),
        Protocol::Anthropic => headers
            .contains_key(HeaderName::from_static("x-api-key"))
            .then_some(AuthKind::XApiKey),
        Protocol::Gemini => {
            if headers.contains_key(HeaderName::from_static("x-goog-api-key")) {
                Some(AuthKind::XGoogApiKey)
            } else if url.query_pairs().any(|(k, _)| k == "key") {
                Some(AuthKind::QueryKey)
            } else {
                None
            }
        }
    }
}

fn resolve_auth_kind(protocol: Protocol, detected: Option<AuthKind>) -> AuthKind {
    if let Some(kind) = detected
        && auth_kind_allowed_for_protocol(protocol, kind)
    {
        return kind;
    }
    default_auth_kind_for_protocol(protocol)
}

fn default_auth_kind_for_protocol(protocol: Protocol) -> AuthKind {
    match protocol {
        Protocol::Openai => AuthKind::Bearer,
        Protocol::Anthropic => AuthKind::XApiKey,
        Protocol::Gemini => AuthKind::QueryKey,
    }
}

fn auth_kind_allowed_for_protocol(protocol: Protocol, kind: AuthKind) -> bool {
    match protocol {
        Protocol::Openai => kind == AuthKind::Bearer,
        Protocol::Anthropic => kind == AuthKind::XApiKey,
        Protocol::Gemini => matches!(kind, AuthKind::QueryKey | AuthKind::XGoogApiKey),
    }
}

fn clear_auth(protocol: Protocol, headers: &mut HeaderMap, url: &mut Url) {
    headers.remove(axum::http::header::AUTHORIZATION);
    headers.remove(HeaderName::from_static("x-api-key"));
    headers.remove(HeaderName::from_static("x-goog-api-key"));
    if protocol == Protocol::Gemini {
        remove_query_param(url, "key");
    }
}

fn apply_auth_kind(
    kind: AuthKind,
    token: &str,
    headers: &mut HeaderMap,
    url: &mut Url,
) -> Result<(), ProxyError> {
    match kind {
        AuthKind::Bearer => {
            let v = format!("Bearer {token}");
            headers.insert(
                axum::http::header::AUTHORIZATION,
                v.parse()
                    .map_err(|e| ProxyError::Upstream(format!("bad bearer: {e}")))?,
            );
        }
        AuthKind::XApiKey => {
            headers.insert(
                HeaderName::from_static("x-api-key"),
                token
                    .parse()
                    .map_err(|e| ProxyError::Upstream(format!("bad x-api-key: {e}")))?,
            );
        }
        AuthKind::XGoogApiKey => {
            headers.insert(
                HeaderName::from_static("x-goog-api-key"),
                token
                    .parse()
                    .map_err(|e| ProxyError::Upstream(format!("bad x-goog-api-key: {e}")))?,
            );
        }
        AuthKind::QueryKey => {
            set_query_param(url, "key", token);
        }
    }
    Ok(())
}

fn set_query_param(url: &mut Url, name: &str, value: &str) {
    let existing: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    url.set_query(None);
    {
        let mut qp = url.query_pairs_mut();
        for (k, v) in existing {
            if k != name {
                qp.append_pair(&k, &v);
            }
        }
        qp.append_pair(name, value);
    }
}

fn remove_query_param(url: &mut Url, name: &str) {
    let existing: Vec<(String, String)> = url
        .query_pairs()
        .filter_map(|(k, v)| (k != name).then(|| (k.to_string(), v.to_string())))
        .collect();

    url.set_query(None);
    {
        let mut qp = url.query_pairs_mut();
        for (k, v) in existing {
            qp.append_pair(&k, &v);
        }
    }
}

fn extract_model_from_body(headers: &HeaderMap, body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }

    let is_json = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase())
        .is_some_and(|v| v.starts_with("application/json"));

    if !is_json {
        return None;
    }

    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    v.get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
}

fn extract_model(
    protocol: Protocol,
    headers: &HeaderMap,
    uri: &axum::http::Uri,
    body: &[u8],
) -> Option<String> {
    match protocol {
        // Gemini model is part of the URL in common requests, so prefer URI to avoid parsing body JSON.
        Protocol::Gemini => {
            extract_gemini_model_from_uri(uri).or_else(|| extract_model_from_body(headers, body))
        }
        _ => extract_model_from_body(headers, body),
    }
}

fn extract_gemini_model_from_uri(uri: &axum::http::Uri) -> Option<String> {
    let path = uri.path();
    for marker in ["/models/", "/tunedModels/"] {
        if let Some(pos) = path.rfind(marker) {
            let rest = &path[(pos + marker.len())..];
            let segment = rest.split('/').next().unwrap_or("");
            let model = segment.split(':').next().unwrap_or("").trim();
            if !model.is_empty() {
                return Some(model.to_string());
            }
        }
    }
    None
}

#[derive(Debug, Default, Clone, Copy)]
struct TokenUsage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
}

type TokenUsageEventFields = (
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

impl TokenUsage {
    fn as_event_fields(self) -> TokenUsageEventFields {
        let total = match (
            self.total_tokens,
            self.prompt_tokens,
            self.completion_tokens,
        ) {
            (Some(t), _, _) => Some(t),
            (None, Some(p), Some(c)) => Some(p + c),
            _ => None,
        };
        (
            self.prompt_tokens,
            self.completion_tokens,
            total,
            self.cache_read_tokens,
            self.cache_write_tokens,
        )
    }

    fn merge(&mut self, other: TokenUsage) {
        if other.prompt_tokens.is_some() {
            self.prompt_tokens = other.prompt_tokens;
        }
        if other.completion_tokens.is_some() {
            self.completion_tokens = other.completion_tokens;
        }
        if other.total_tokens.is_some() {
            self.total_tokens = other.total_tokens;
        }
        if other.cache_read_tokens.is_some() {
            self.cache_read_tokens = other.cache_read_tokens;
        }
        if other.cache_write_tokens.is_some() {
            self.cache_write_tokens = other.cache_write_tokens;
        }
    }
}

fn parse_usage_from_value(protocol: Protocol, v: &serde_json::Value) -> TokenUsage {
    extract_usage_from_value(protocol, v)
}

fn parse_error_message(_protocol: Protocol, bytes: &[u8]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    parse_error_message_from_value(_protocol, &v)
}

fn parse_error_message_from_value(_protocol: Protocol, v: &serde_json::Value) -> Option<String> {
    let message = get_error_message(v)?;

    let mut extras: Vec<String> = Vec::new();

    if let Some(t) = get_str_path(v, &["error", "type"]) {
        extras.push(format!("type={t}"));
    }
    if let Some(code) = get_any_path(v, &["error", "code"]) {
        extras.push(format!("code={code}"));
    }
    if let Some(status) = get_any_path(v, &["error", "status"]) {
        extras.push(format!("status={status}"));
    }

    if extras.is_empty() {
        Some(message)
    } else {
        Some(format!("{message} ({})", extras.join(", ")))
    }
}

fn get_error_message(v: &serde_json::Value) -> Option<String> {
    // 覆盖常见上游错误格式：
    // - OpenAI: { error: { message, type, code } }
    // - Anthropic: { error: { message, type } } 或 { type: "error", error: { message } }
    // - Gemini: { error: { message, status, code } }
    let msg = get_str_path(v, &["error", "message"])
        .or_else(|| get_str_path(v, &["error", "error", "message"]))
        .or_else(|| get_str_path(v, &["message"]))
        .or_else(|| get_str_path(v, &["detail"]))
        .or_else(|| get_str_path(v, &["error"]))
        .or_else(|| get_str_path(v, &["error", "details", "0", "message"]));
    msg.map(|s| s.to_string())
}

fn get_str_path<'a>(v: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for key in path {
        cur = match cur {
            serde_json::Value::Object(m) => m.get(*key)?,
            serde_json::Value::Array(a) => {
                let idx: usize = key.parse().ok()?;
                a.get(idx)?
            }
            _ => return None,
        };
    }
    cur.as_str()
}

fn get_any_path(v: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for key in path {
        cur = match cur {
            serde_json::Value::Object(m) => m.get(*key)?,
            serde_json::Value::Array(a) => {
                let idx: usize = key.parse().ok()?;
                a.get(idx)?
            }
            _ => return None,
        };
    }
    match cur {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn extract_usage_from_value(protocol: Protocol, v: &serde_json::Value) -> TokenUsage {
    match protocol {
        Protocol::Openai => extract_openai_usage(v),
        Protocol::Anthropic => extract_anthropic_usage(v),
        Protocol::Gemini => extract_gemini_usage(v),
    }
}

fn extract_openai_usage(v: &serde_json::Value) -> TokenUsage {
    // /v1/chat/completions /v1/completions: usage.prompt_tokens / usage.completion_tokens
    // /v1/responses: usage.input_tokens / usage.output_tokens
    let usage = v
        .get("usage")
        .or_else(|| v.get("response").and_then(|r| r.get("usage")));
    if let Some(u) = usage {
        let prompt_tokens = u
            .get("prompt_tokens")
            .or_else(|| u.get("input_tokens"))
            .and_then(|n| n.as_i64());
        let completion_tokens = u
            .get("completion_tokens")
            .or_else(|| u.get("output_tokens"))
            .and_then(|n| n.as_i64());
        let total_tokens = u.get("total_tokens").and_then(|n| n.as_i64());
        let cache_read_tokens = u
            .get("prompt_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .or_else(|| {
                u.get("input_tokens_details")
                    .and_then(|d| d.get("cached_tokens"))
            })
            .and_then(|n| n.as_i64());
        let cache_write_tokens = u
            .get("prompt_tokens_details")
            .and_then(|d| d.get("cache_creation_tokens"))
            .or_else(|| {
                u.get("input_tokens_details")
                    .and_then(|d| d.get("cache_creation_tokens"))
            })
            .and_then(|n| n.as_i64());
        return TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_read_tokens,
            cache_write_tokens,
        };
    }
    TokenUsage::default()
}

fn extract_anthropic_usage(v: &serde_json::Value) -> TokenUsage {
    let usage = v
        .get("usage")
        .or_else(|| v.get("message").and_then(|m| m.get("usage")));
    if let Some(u) = usage {
        let prompt_tokens = u.get("input_tokens").and_then(|n| n.as_i64());
        let completion_tokens = u.get("output_tokens").and_then(|n| n.as_i64());
        let cache_read_tokens = u
            .get("cache_read_input_tokens")
            .or_else(|| u.get("cache_read_tokens"))
            .and_then(|n| n.as_i64());
        let cache_write_tokens = u
            .get("cache_creation_input_tokens")
            .or_else(|| u.get("cache_write_input_tokens"))
            .or_else(|| u.get("cache_creation_tokens"))
            .or_else(|| u.get("cache_write_tokens"))
            .and_then(|n| n.as_i64());
        return TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: None,
            cache_read_tokens,
            cache_write_tokens,
        };
    }
    TokenUsage::default()
}

fn extract_gemini_usage(v: &serde_json::Value) -> TokenUsage {
    // Gemini returns usageMetadata.promptTokenCount / candidatesTokenCount / totalTokenCount
    let usage = v.get("usageMetadata");
    if let Some(u) = usage {
        let prompt_tokens = u.get("promptTokenCount").and_then(|n| n.as_i64());
        let completion_tokens = u.get("candidatesTokenCount").and_then(|n| n.as_i64());
        let total_tokens = u.get("totalTokenCount").and_then(|n| n.as_i64());
        return TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_read_tokens: None,
            cache_write_tokens: None,
        };
    }
    TokenUsage::default()
}

pub(super) fn spawn_usage_event(input: storage::CreateUsageEvent, db_path: std::path::PathBuf) {
    // Best-effort: `tokio::spawn` will panic if called when no runtime is available.
    // This can happen if a streaming body is dropped during shutdown.
    if tokio::runtime::Handle::try_current().is_err() {
        tracing::warn!("skip insert usage event: tokio runtime not available");
        return;
    }
    if !usage_writer::try_enqueue(db_path, input) {
        tracing::warn!("drop usage event: usage writer queue is full");
    }
}

pub(super) struct UsageEventParams {
    pub(super) request_id: Option<Arc<str>>,
    pub(super) protocol: Protocol,
    pub(super) channel_id: String,
    pub(super) model: Option<String>,
    pub(super) success: bool,
    pub(super) http_status: Option<i64>,
    pub(super) error_kind: Option<String>,
    pub(super) error_detail: Option<String>,
    pub(super) latency_ms: i64,
    pub(super) ttft_ms: Option<i64>,
    pub(super) tokens: TokenUsageEventFields,
}

pub(super) fn build_usage_event(params: UsageEventParams) -> storage::CreateUsageEvent {
    let (prompt_tokens, completion_tokens, total_tokens, cache_read_tokens, cache_write_tokens) =
        params.tokens;
    storage::CreateUsageEvent {
        request_id: params.request_id,
        ts_ms: storage::now_ms(),
        protocol: params.protocol,
        route_id: None,
        channel_id: params.channel_id,
        model: params.model,
        success: params.success,
        http_status: params.http_status,
        error_kind: params.error_kind,
        error_detail: params.error_detail,
        latency_ms: params.latency_ms,
        ttft_ms: params.ttft_ms,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_read_tokens,
        cache_write_tokens,
        estimated_cost_usd: None,
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let mut out = String::with_capacity(max_len + 1);
    let keep = max_len.saturating_sub(1);
    for ch in s.chars() {
        if out.len() + ch.len_utf8() > keep {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn to_single_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}
