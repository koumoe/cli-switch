use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, HeaderName, Request, Response};
use bytes::Bytes;
use futures_util::StreamExt as _;
use reqwest::Url;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use crate::storage::{self, Channel, Protocol};

const MAX_INBOUND_BODY_BYTES: usize = 64 * 1024 * 1024;

#[derive(thiserror::Error, Debug)]
pub enum ProxyError {
    #[error("未配置可用渠道：{0}")]
    NoEnabledChannel(Protocol),
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
    let started = Instant::now();

    let Some(channel) = pick_enabled_channel(db_path.clone(), protocol).await? else {
        return Err(ProxyError::NoEnabledChannel(protocol));
    };

    let (parts, body) = req.into_parts();
    let body_bytes = to_bytes(body, MAX_INBOUND_BODY_BYTES)
        .await
        .map_err(|e| ProxyError::ReadBody(e.to_string()))?;

    let model = extract_model_from_body(&parts.headers, &body_bytes);

    let mut url = build_upstream_url(&channel.base_url, &parts.uri, protocol_root)?;

    let mut out_headers = filtered_headers(&parts.headers);
    apply_auth(&channel, protocol, &mut url, &mut out_headers)?;

    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes())
        .map_err(|e| ProxyError::Upstream(format!("invalid method: {e}")))?;

    let upstream = client
        .request(method, url)
        .headers(out_headers)
        .body(body_bytes)
        .send()
        .await
        .map_err(|e| {
            spawn_usage_event(
                storage::CreateUsageEvent {
                    ts_ms: storage::now_ms(),
                    protocol,
                    route_id: None,
                    channel_id: channel.id.clone(),
                    model: model.clone(),
                    success: false,
                    http_status: None,
                    error_kind: Some(format!("upstream_error:{}", truncate(&e.to_string(), 240))),
                    latency_ms: started.elapsed().as_millis() as i64,
                    ttft_ms: None,
                    prompt_tokens: None,
                    completion_tokens: None,
                    total_tokens: None,
                    estimated_cost_usd: None,
                },
                db_path.clone(),
            );
            ProxyError::Upstream(e.to_string())
        })?;

    let status = upstream.status();
    let headers = filtered_headers(upstream.headers());
    let content_type = upstream
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_sse = content_type.starts_with("text/event-stream");
    let is_json = content_type.contains("application/json") || content_type.contains("+json");

    let mut resp = Response::builder().status(status);
    if let Some(h) = resp.headers_mut() {
        for (k, v) in headers.iter() {
            h.append(k, v.clone());
        }
    }

    if !is_sse && is_json && upstream.content_length().unwrap_or(0) <= 8 * 1024 * 1024 {
        let bytes = upstream
            .bytes()
            .await
            .map_err(|e| ProxyError::Upstream(e.to_string()))?;

        let duration_ms = started.elapsed().as_millis() as i64;
        let usage = parse_usage_from_json(protocol, &bytes);
        let (prompt_tokens, completion_tokens, total_tokens) = usage.as_tuple();

        let http_status = Some(status.as_u16() as i64);
        let success = status.is_success();
        let error_kind = (!success).then(|| format!("upstream_http:{}", status.as_u16()));

        spawn_usage_event(
            storage::CreateUsageEvent {
                ts_ms: storage::now_ms(),
                protocol,
                route_id: None,
                channel_id: channel.id.clone(),
                model,
                success,
                http_status,
                error_kind,
                latency_ms: duration_ms,
                ttft_ms: None,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                estimated_cost_usd: None,
            },
            db_path,
        );

        return resp
            .body(Body::from(bytes))
            .map_err(|e| ProxyError::Upstream(e.to_string()));
    }

    let stream = InstrumentedStream::new(
        upstream.bytes_stream().boxed(),
        StreamRecordContext {
            db_path,
            protocol,
            channel_id: channel.id.clone(),
            model,
            http_status: status.as_u16() as i64,
            status_is_success: status.is_success(),
            started,
            parse_sse: is_sse,
        },
    );

    resp.body(Body::from_stream(stream))
        .map_err(|e| ProxyError::Upstream(e.to_string()))
}

async fn pick_enabled_channel(
    db_path: std::path::PathBuf,
    protocol: Protocol,
) -> Result<Option<Channel>, anyhow::Error> {
    let channels = storage::list_channels(db_path).await?;
    Ok(channels
        .into_iter()
        .find(|c| c.enabled && c.protocol == protocol))
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

        let lname = name.as_str().to_ascii_lowercase();
        if connection_tokens.iter().any(|t| t == &lname) {
            continue;
        }

        out.append(name.clone(), value.clone());
    }

    out
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

#[derive(Debug, Default, Clone, Copy)]
struct TokenUsage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

impl TokenUsage {
    fn as_tuple(self) -> (Option<i64>, Option<i64>, Option<i64>) {
        let total = match (
            self.total_tokens,
            self.prompt_tokens,
            self.completion_tokens,
        ) {
            (Some(t), _, _) => Some(t),
            (None, Some(p), Some(c)) => Some(p + c),
            _ => None,
        };
        (self.prompt_tokens, self.completion_tokens, total)
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
    }
}

fn parse_usage_from_json(protocol: Protocol, bytes: &[u8]) -> TokenUsage {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return TokenUsage::default();
    };
    extract_usage_from_value(protocol, &v)
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
        return TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
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
        return TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: None,
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
        };
    }
    TokenUsage::default()
}

fn spawn_usage_event(input: storage::CreateUsageEvent, db_path: std::path::PathBuf) {
    tokio::spawn(async move {
        let _ = storage::insert_usage_event(db_path, input).await;
    });
}

#[derive(Clone)]
struct StreamRecordContext {
    db_path: std::path::PathBuf,
    protocol: Protocol,
    channel_id: String,
    model: Option<String>,
    http_status: i64,
    status_is_success: bool,
    started: Instant,
    parse_sse: bool,
}

struct InstrumentedStream {
    inner: futures_util::stream::BoxStream<'static, Result<Bytes, reqwest::Error>>,
    ctx: StreamRecordContext,
    finalized: bool,
    ttft_ms: Option<i64>,
    usage: TokenUsage,
    sse_buf: Vec<u8>,
    stream_error: Option<String>,
}

impl InstrumentedStream {
    fn new(
        inner: futures_util::stream::BoxStream<'static, Result<Bytes, reqwest::Error>>,
        ctx: StreamRecordContext,
    ) -> Self {
        Self {
            inner,
            ctx,
            finalized: false,
            ttft_ms: None,
            usage: TokenUsage::default(),
            sse_buf: Vec::new(),
            stream_error: None,
        }
    }

    fn on_chunk(&mut self, bytes: &Bytes) {
        if self.ttft_ms.is_none() {
            self.ttft_ms = Some(self.ctx.started.elapsed().as_millis() as i64);
        }
        if self.ctx.parse_sse {
            self.consume_sse(bytes);
        }
    }

    fn consume_sse(&mut self, bytes: &Bytes) {
        const MAX_SSE_BUF: usize = 256 * 1024;
        if self.sse_buf.len() < MAX_SSE_BUF {
            let remain = MAX_SSE_BUF - self.sse_buf.len();
            self.sse_buf
                .extend_from_slice(&bytes[..bytes.len().min(remain)]);
        }

        while let Some(nl) = self.sse_buf.iter().position(|b| *b == b'\n') {
            let line = self.sse_buf.drain(..=nl).collect::<Vec<u8>>();
            let Ok(mut s) = std::str::from_utf8(&line) else {
                continue;
            };
            s = s.trim();
            if !s.starts_with("data:") {
                continue;
            }
            let data = s["data:".len()..].trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };
            self.usage
                .merge(extract_usage_from_value(self.ctx.protocol, &v));
        }
    }

    fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;

        let duration_ms = self.ctx.started.elapsed().as_millis() as i64;
        let (prompt_tokens, completion_tokens, total_tokens) = self.usage.as_tuple();

        let success = self.ctx.status_is_success && self.stream_error.is_none();
        let error_kind = if success {
            None
        } else if !self.ctx.status_is_success {
            Some(format!("upstream_http:{}", self.ctx.http_status))
        } else if let Some(err) = self.stream_error.as_deref() {
            Some(format!("stream_error:{}", truncate(err, 240)))
        } else {
            Some("upstream_error".to_string())
        };

        tracing::event!(
            tracing::Level::DEBUG,
            protocol = self.ctx.protocol.as_str(),
            channel_id = %self.ctx.channel_id,
            http_status = self.ctx.http_status,
            ttft_ms = self.ttft_ms.unwrap_or(-1),
            duration_ms,
            "proxy request finished"
        );

        spawn_usage_event(
            storage::CreateUsageEvent {
                ts_ms: storage::now_ms(),
                protocol: self.ctx.protocol,
                route_id: None,
                channel_id: self.ctx.channel_id.clone(),
                model: self.ctx.model.clone(),
                success,
                http_status: Some(self.ctx.http_status),
                error_kind,
                latency_ms: duration_ms,
                ttft_ms: self.ttft_ms,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                estimated_cost_usd: None,
            },
            self.ctx.db_path.clone(),
        );
    }
}

impl futures_util::Stream for InstrumentedStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let polled = self.inner.as_mut().poll_next(cx);
        match polled {
            Poll::Ready(Some(Ok(bytes))) => {
                self.on_chunk(&bytes);
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.stream_error = Some(e.to_string());
                Poll::Ready(Some(Err(std::io::Error::other(e))))
            }
            Poll::Ready(None) => {
                self.finalize();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
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
