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

    let Some(channel) = pick_enabled_channel(db_path, protocol).await? else {
        return Err(ProxyError::NoEnabledChannel(protocol));
    };

    let (parts, body) = req.into_parts();
    let body_bytes = to_bytes(body, MAX_INBOUND_BODY_BYTES)
        .await
        .map_err(|e| ProxyError::ReadBody(e.to_string()))?;

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
        .map_err(|e| ProxyError::Upstream(e.to_string()))?;

    let status = upstream.status();
    let headers = filtered_headers(upstream.headers());
    let stream = upstream
        .bytes_stream()
        .map(|chunk| chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));

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
    if let Some(v) = src.get(axum::http::header::CONNECTION) {
        if let Ok(s) = v.to_str() {
            for token in s.split(',') {
                let t = token.trim().to_ascii_lowercase();
                if !t.is_empty() {
                    connection_tokens.push(t);
                }
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

fn apply_auth(
    channel: &Channel,
    protocol: Protocol,
    url: &mut Url,
    headers: &mut HeaderMap,
) -> Result<(), ProxyError> {
    let auth_type = channel.auth_type.trim().to_ascii_lowercase();
    let token = channel.auth_ref.trim();

    match auth_type.as_str() {
        "bearer" => {
            let v = format!("Bearer {token}");
            headers.insert(
                axum::http::header::AUTHORIZATION,
                v.parse()
                    .map_err(|e| ProxyError::Upstream(format!("bad bearer: {e}")))?,
            );
        }
        "x-api-key" => {
            headers.insert(
                HeaderName::from_static("x-api-key"),
                token
                    .parse()
                    .map_err(|e| ProxyError::Upstream(format!("bad x-api-key: {e}")))?,
            );
        }
        "x-goog-api-key" => {
            headers.insert(
                HeaderName::from_static("x-goog-api-key"),
                token
                    .parse()
                    .map_err(|e| ProxyError::Upstream(format!("bad x-goog-api-key: {e}")))?,
            );
        }
        "query" => {
            set_query_param(url, "key", token);
        }
        other => {
            return Err(ProxyError::Upstream(format!(
                "unsupported auth_type: {other}"
            )));
        }
    }

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
