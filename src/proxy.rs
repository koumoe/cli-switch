use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, HeaderName, Request, Response};
use futures_util::StreamExt as _;
use reqwest::Url;
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
            record_usage_event(
                db_path.clone(),
                protocol,
                &channel,
                model.clone(),
                None,
                started,
                e.to_string(),
            );
            ProxyError::Upstream(e.to_string())
        })?;

    let status = upstream.status();
    let headers = filtered_headers(upstream.headers());
    let stream = upstream
        .bytes_stream()
        .map(|chunk| chunk.map_err(std::io::Error::other));

    let mut resp = Response::builder().status(status);
    if let Some(h) = resp.headers_mut() {
        for (k, v) in headers.iter() {
            h.append(k, v.clone());
        }
    }

    tracing::event!(
        tracing::Level::DEBUG,
        protocol = protocol.as_str(),
        channel_id = %channel.id,
        latency_ms = started.elapsed().as_millis() as u64,
        "proxy request finished"
    );

    record_usage_event(
        db_path,
        protocol,
        &channel,
        model,
        Some(status.as_u16() as i64),
        started,
        String::new(),
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

fn record_usage_event(
    db_path: std::path::PathBuf,
    protocol: Protocol,
    channel: &Channel,
    model: Option<String>,
    http_status: Option<i64>,
    started: Instant,
    error: String,
) {
    let ts_ms = storage::now_ms();
    let latency_ms = started.elapsed().as_millis() as i64;
    let success = http_status.is_some_and(|s| (200..300).contains(&s));

    let error_kind = if success {
        None
    } else if let Some(status) = http_status {
        Some(format!("upstream_http:{status}"))
    } else if error.is_empty() {
        Some("upstream_error".to_string())
    } else {
        Some(format!("upstream_error:{}", truncate(&error, 240)))
    };

    let channel_id = channel.id.clone();
    tokio::spawn(async move {
        let _ = storage::insert_usage_event(
            db_path,
            storage::CreateUsageEvent {
                ts_ms,
                protocol,
                route_id: None,
                channel_id,
                model,
                success,
                http_status,
                error_kind,
                latency_ms,
                prompt_tokens: None,
                completion_tokens: None,
                total_tokens: None,
                estimated_cost_usd: None,
            },
        )
        .await;
    });
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
