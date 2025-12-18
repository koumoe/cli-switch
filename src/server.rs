#[cfg(not(feature = "embed-ui"))]
use axum::response::Html;
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{OriginalUri, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{any, get, post, put},
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::watch;
use tokio::time::Duration;
#[cfg(not(feature = "embed-ui"))]
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::storage;
use crate::{proxy, proxy::ProxyError};

#[derive(Clone)]
pub struct AppState {
    pub listen_addr: SocketAddr,
    pub db_path: Arc<PathBuf>,
    pub http_client: reqwest::Client,
    pub settings_notify: watch::Sender<u64>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    listen_addr: String,
    data_dir: String,
    db_path: String,
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let data_dir = state
        .db_path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        listen_addr: state.listen_addr.to_string(),
        data_dir,
        db_path: state.db_path.display().to_string(),
    })
}

#[cfg(not(feature = "embed-ui"))]
async fn ui_placeholder() -> impl IntoResponse {
    Html(
        r#"<!doctype html>
<html>
  <head><meta charset="utf-8"><title>CliSwitch</title></head>
  <body>
    <h1>CliSwitch</h1>
    <p>UI 尚未构建或未启用内嵌（feature <code>embed-ui</code>）。</p>
    <p>开发：先构建 <code>ui</code>，再启动后端，或直接用 Vite dev server。</p>
    <p>健康检查：<a href="/api/health">/api/health</a></p>
  </body>
</html>"#,
    )
}

#[cfg(not(feature = "embed-ui"))]
async fn ui_fs_fallback(uri: OriginalUri) -> impl IntoResponse {
    let dist = std::path::PathBuf::from("ui/dist");
    if !dist.is_dir() {
        return ui_placeholder().await.into_response();
    }

    let mut path = uri.0.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    if path.contains("..") || path.contains('\\') {
        return StatusCode::NOT_FOUND.into_response();
    }

    let candidate = dist.join(&path);
    if candidate.is_file() {
        match tokio::fs::read(&candidate).await {
            Ok(bytes) => {
                let mime = mime_guess::from_path(&path).first_or_octet_stream();
                return (
                    [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                    Bytes::from(bytes),
                )
                    .into_response();
            }
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        }
    }

    let is_asset_like = path.starts_with("assets/") || path.contains('.');
    if is_asset_like {
        return StatusCode::NOT_FOUND.into_response();
    }

    let index = dist.join("index.html");
    match tokio::fs::read(&index).await {
        Ok(bytes) => (
            [(axum::http::header::CONTENT_TYPE, "text/html")],
            Bytes::from(bytes),
        )
            .into_response(),
        Err(_) => ui_placeholder().await.into_response(),
    }
}

async fn proxy_openai(
    State(state): State<AppState>,
    req: axum::http::Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    proxy::forward(
        &state.http_client,
        (*state.db_path).clone(),
        storage::Protocol::Openai,
        "/v1",
        req,
    )
    .await
    .map_err(map_proxy_error)
}

async fn proxy_anthropic(
    State(state): State<AppState>,
    req: axum::http::Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    proxy::forward(
        &state.http_client,
        (*state.db_path).clone(),
        storage::Protocol::Anthropic,
        "/v1",
        req,
    )
    .await
    .map_err(map_proxy_error)
}

async fn proxy_gemini(
    State(state): State<AppState>,
    req: axum::http::Request<Body>,
) -> Result<axum::response::Response, ApiError> {
    proxy::forward(
        &state.http_client,
        (*state.db_path).clone(),
        storage::Protocol::Gemini,
        "/v1beta",
        req,
    )
    .await
    .map_err(map_proxy_error)
}

#[cfg(feature = "embed-ui")]
#[derive(rust_embed::RustEmbed)]
#[folder = "ui/dist"]
struct UiDist;

#[cfg(feature = "embed-ui")]
async fn ui_fallback(uri: OriginalUri) -> impl IntoResponse {
    let mut path = uri.0.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    if let Some(asset) = UiDist::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        let body = Bytes::from(asset.data.into_owned());
        return ([(axum::http::header::CONTENT_TYPE, mime.as_ref())], body).into_response();
    }

    let is_asset_like = path.starts_with("assets/") || path.contains('.');
    if is_asset_like {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Some(index) = UiDist::get("index.html") {
        let body = Bytes::from(index.data.into_owned());
        return ([(axum::http::header::CONTENT_TYPE, "text/html")], body).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(thiserror::Error, Debug)]
enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadGateway(String),
    #[error("{0}")]
    Unavailable(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ApiError::Unavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            ApiError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            ),
        };
        (status, Json(ErrorBody { error: msg })).into_response()
    }
}

fn map_proxy_error(e: ProxyError) -> ApiError {
    match e {
        ProxyError::NoEnabledChannel(p) => {
            ApiError::Unavailable(format!("未配置启用的 {} 渠道", p.as_str()))
        }
        ProxyError::InvalidBaseUrl(msg) => ApiError::Internal(anyhow::anyhow!(msg)),
        ProxyError::ReadBody(msg) => ApiError::BadRequest(msg),
        ProxyError::Upstream(msg) => ApiError::BadGateway(msg),
        ProxyError::Storage(e) => ApiError::Internal(e),
    }
}

async fn list_channels(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let channels = storage::list_channels((*state.db_path).clone()).await?;
    Ok(Json(channels))
}

#[derive(Debug, Deserialize)]
struct ReorderChannelsInput {
    protocol: Option<storage::Protocol>,
    channel_ids: Vec<String>,
}

async fn reorder_channels(
    State(state): State<AppState>,
    Json(input): Json<ReorderChannelsInput>,
) -> Result<impl IntoResponse, ApiError> {
    let mut seen = std::collections::HashSet::<String>::new();
    for id in &input.channel_ids {
        if !seen.insert(id.clone()) {
            return Err(ApiError::BadRequest("channel_ids 存在重复项".to_string()));
        }
    }

    let res =
        storage::reorder_channels((*state.db_path).clone(), input.protocol, input.channel_ids)
            .await;
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.to_string().starts_with("channel not found") => {
            Err(ApiError::NotFound("channel not found".to_string()))
        }
        Err(e) if e.to_string().starts_with("channel reorder mismatch") => Err(
            ApiError::BadRequest("channel_ids 需要覆盖该终端下所有渠道".to_string()),
        ),
        Err(e) => Err(ApiError::Internal(e)),
    }
}

async fn create_channel(
    State(state): State<AppState>,
    Json(input): Json<storage::CreateChannel>,
) -> Result<impl IntoResponse, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name 不能为空".to_string()));
    }
    if input.base_url.trim().is_empty() {
        return Err(ApiError::BadRequest("base_url 不能为空".to_string()));
    }

    let channel = storage::create_channel((*state.db_path).clone(), input).await?;
    Ok((StatusCode::CREATED, Json(channel)))
}

async fn update_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
    Json(input): Json<storage::UpdateChannel>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::update_channel((*state.db_path).clone(), channel_id, input).await;
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.to_string().starts_with("channel not found") => {
            Err(ApiError::NotFound("channel not found".to_string()))
        }
        Err(e) => Err(ApiError::Internal(e)),
    }
}

async fn enable_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    storage::set_channel_enabled((*state.db_path).clone(), channel_id, true).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn disable_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    storage::set_channel_enabled((*state.db_path).clone(), channel_id, false).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::delete_channel((*state.db_path).clone(), channel_id).await;
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.to_string().starts_with("channel not found") => {
            Err(ApiError::NotFound("channel not found".to_string()))
        }
        Err(e) => Err(ApiError::Internal(e)),
    }
}

#[derive(Serialize)]
struct ChannelTestResponse {
    reachable: bool,
    ok: bool,
    status: Option<u16>,
    latency_ms: u64,
    error: Option<String>,
}

fn build_models_url(mut url: reqwest::Url, protocol: storage::Protocol) -> reqwest::Url {
    let base_path = url.path().trim_end_matches('/');
    let root = match protocol {
        storage::Protocol::Openai | storage::Protocol::Anthropic => "/v1",
        storage::Protocol::Gemini => "/v1beta",
    };

    let path = if base_path.is_empty() {
        format!("{root}/models")
    } else if base_path.ends_with(root) {
        format!("{base_path}/models")
    } else {
        format!("{base_path}{root}/models")
    };

    url.set_path(&path);
    url
}

async fn test_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(channel) = storage::get_channel((*state.db_path).clone(), channel_id).await? else {
        return Err(ApiError::NotFound("channel not found".to_string()));
    };

    let base_url = reqwest::Url::parse(&channel.base_url)
        .map_err(|e| ApiError::BadRequest(format!("base_url 无效：{e}")))?;

    let mut url = build_models_url(base_url, channel.protocol);
    let mut headers = axum::http::HeaderMap::new();
    proxy::apply_auth(&channel, channel.protocol, &mut url, &mut headers)
        .map_err(|e| ApiError::BadGateway(e.to_string()))?;

    let started = std::time::Instant::now();
    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        state.http_client.get(url).headers(headers).send(),
    )
    .await;

    let latency_ms = started.elapsed().as_millis() as u64;

    match resp {
        Ok(Ok(r)) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            Ok(Json(ChannelTestResponse {
                reachable: true,
                ok,
                status: Some(status),
                latency_ms,
                error: None,
            }))
        }
        Ok(Err(e)) => Ok(Json(ChannelTestResponse {
            reachable: false,
            ok: false,
            status: None,
            latency_ms,
            error: Some(e.to_string()),
        })),
        Err(_) => Ok(Json(ChannelTestResponse {
            reachable: false,
            ok: false,
            status: None,
            latency_ms,
            error: Some("timeout".to_string()),
        })),
    }
}

async fn list_routes(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let routes = storage::list_routes((*state.db_path).clone()).await?;
    Ok(Json(routes))
}

async fn create_route(
    State(state): State<AppState>,
    Json(input): Json<storage::CreateRoute>,
) -> Result<impl IntoResponse, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name 不能为空".to_string()));
    }
    let route = storage::create_route((*state.db_path).clone(), input).await?;
    Ok((StatusCode::CREATED, Json(route)))
}

async fn update_route(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
    Json(input): Json<storage::UpdateRoute>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::update_route((*state.db_path).clone(), route_id, input).await;
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.to_string().starts_with("route not found") => {
            Err(ApiError::NotFound("route not found".to_string()))
        }
        Err(e) => Err(ApiError::Internal(e)),
    }
}

async fn delete_route(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::delete_route((*state.db_path).clone(), route_id).await;
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.to_string().starts_with("route not found") => {
            Err(ApiError::NotFound("route not found".to_string()))
        }
        Err(e) => Err(ApiError::Internal(e)),
    }
}

async fn list_route_channels(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(_) = storage::get_route((*state.db_path).clone(), route_id.clone()).await? else {
        return Err(ApiError::NotFound("route not found".to_string()));
    };
    let items = storage::list_route_channels((*state.db_path).clone(), route_id).await?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
struct ReorderRouteChannelsInput {
    channel_ids: Vec<String>,
}

async fn reorder_route_channels(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
    Json(input): Json<ReorderRouteChannelsInput>,
) -> Result<impl IntoResponse, ApiError> {
    let mut seen = std::collections::HashSet::<String>::new();
    for id in &input.channel_ids {
        if !seen.insert(id.clone()) {
            return Err(ApiError::BadRequest("channel_ids 存在重复项".to_string()));
        }
    }

    let res =
        storage::set_route_channels((*state.db_path).clone(), route_id, input.channel_ids).await;
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.to_string().starts_with("route not found") => {
            Err(ApiError::NotFound("route not found".to_string()))
        }
        Err(e) if e.to_string().starts_with("channel not found") => {
            Err(ApiError::NotFound("channel not found".to_string()))
        }
        Err(e) if e.to_string().starts_with("channel protocol mismatch") => {
            Err(ApiError::BadRequest(e.to_string()))
        }
        Err(e) => Err(ApiError::Internal(e)),
    }
}

#[derive(Debug, Deserialize)]
struct PricingModelsQuery {
    query: Option<String>,
    limit: Option<i64>,
}

async fn pricing_status(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let status = storage::pricing_status((*state.db_path).clone()).await?;
    Ok(Json(status))
}

async fn pricing_models(
    State(state): State<AppState>,
    Query(q): Query<PricingModelsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = q.limit.unwrap_or(200).clamp(1, 2000);
    let models = storage::search_pricing_models((*state.db_path).clone(), q.query, limit).await?;
    Ok(Json(models))
}

#[derive(Serialize)]
struct PricingSyncResponse {
    updated: usize,
    updated_at_ms: i64,
}

async fn pricing_sync(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let (updated, updated_at_ms) =
        run_pricing_sync(&state.http_client, (*state.db_path).clone()).await?;
    Ok(Json(PricingSyncResponse {
        updated,
        updated_at_ms,
    }))
}

async fn run_pricing_sync(
    http_client: &reqwest::Client,
    db_path: PathBuf,
) -> Result<(usize, i64), ApiError> {
    const PRICING_METADATA_URL: &str = "https://basellm.github.io/llm-metadata/api/all.json";
    const USD_PER_MILLION_DIVISOR: f64 = 1_000_000.0;

    fn json_value_to_f64(v: &serde_json::Value) -> Option<f64> {
        match v {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    fn format_unit_price_usd_per_token(v: f64) -> Option<String> {
        if !v.is_finite() || v <= 0.0 {
            return None;
        }
        let mut s = format!("{v:.18}");
        while s.contains('.') && s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        if s == "0" || s == "0.0" {
            None
        } else {
            Some(s)
        }
    }

    fn provider_priority(provider_id: &str) -> i32 {
        match provider_id {
            "openai" | "anthropic" | "google" | "deepseek" | "xai" => 0,
            _ => 10,
        }
    }

    let resp = http_client
        .get(PRICING_METADATA_URL)
        .send()
        .await
        .map_err(|e| ApiError::BadGateway(format!("请求 llm-metadata 失败：{e}")))?;

    let status = resp.status();
    let body = resp
        .bytes()
        .await
        .map_err(|e| ApiError::BadGateway(format!("读取 llm-metadata 响应失败：{e}")))?;

    if !status.is_success() {
        let snippet = String::from_utf8_lossy(&body);
        return Err(ApiError::BadGateway(format!(
            "llm-metadata 返回非成功状态：{status} body={snippet}"
        )));
    }

    let root: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| ApiError::BadGateway(format!("解析 llm-metadata JSON 失败：{e}")))?;
    let providers = root.as_object().ok_or_else(|| {
        ApiError::BadGateway("llm-metadata JSON 顶层不是对象 (object)".to_string())
    })?;

    let updated_at_ms = storage::now_ms();
    let mut selected: std::collections::HashMap<String, (i32, storage::UpsertPricingModel)> =
        std::collections::HashMap::new();

    for (provider_id, provider) in providers {
        let Some(models_obj) = provider.get("models").and_then(|v| v.as_object()) else {
            continue;
        };
        let pri = provider_priority(provider_id);

        for (model_key, model) in models_obj {
            let model_id = model
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(model_key)
                .trim();
            if model_id.is_empty() {
                continue;
            }

            let cost = model.get("cost").unwrap_or(&serde_json::Value::Null);
            let prompt_per_million = cost.get("input").and_then(json_value_to_f64);
            let completion_per_million = cost.get("output").and_then(json_value_to_f64);
            let cache_read_per_million = cost.get("cache_read").and_then(json_value_to_f64);
            let cache_write_per_million = cost.get("cache_write").and_then(json_value_to_f64);

            let prompt_price = prompt_per_million
                .map(|v| v / USD_PER_MILLION_DIVISOR)
                .and_then(format_unit_price_usd_per_token);
            let completion_price = completion_per_million
                .map(|v| v / USD_PER_MILLION_DIVISOR)
                .and_then(format_unit_price_usd_per_token);
            let cache_read_price = cache_read_per_million
                .map(|v| v / USD_PER_MILLION_DIVISOR)
                .and_then(format_unit_price_usd_per_token);
            let cache_write_price = cache_write_per_million
                .map(|v| v / USD_PER_MILLION_DIVISOR)
                .and_then(format_unit_price_usd_per_token);

            if prompt_price.is_none()
                && completion_price.is_none()
                && cache_read_price.is_none()
                && cache_write_price.is_none()
            {
                continue;
            }

            let raw_json = serde_json::to_string(&serde_json::json!({
                "provider": provider_id,
                "model_key": model_key,
                "model": model,
            }))
            .ok();

            let candidate = storage::UpsertPricingModel {
                model_id: model_id.to_string(),
                prompt_price,
                completion_price,
                request_price: None,
                cache_read_price,
                cache_write_price,
                raw_json,
            };

            match selected.get(model_id) {
                None => {
                    selected.insert(model_id.to_string(), (pri, candidate));
                }
                Some((existing_pri, existing)) => {
                    let candidate_has_more = (candidate.prompt_price.is_some()
                        && existing.prompt_price.is_none())
                        || (candidate.completion_price.is_some()
                            && existing.completion_price.is_none());
                    let candidate_has_more = candidate_has_more
                        || (candidate.cache_read_price.is_some()
                            && existing.cache_read_price.is_none())
                        || (candidate.cache_write_price.is_some()
                            && existing.cache_write_price.is_none());
                    if pri < *existing_pri || (pri == *existing_pri && candidate_has_more) {
                        selected.insert(model_id.to_string(), (pri, candidate));
                    }
                }
            }
        }
    }

    let models: Vec<storage::UpsertPricingModel> = selected.into_values().map(|(_, m)| m).collect();
    let updated = storage::upsert_pricing_models(db_path.clone(), models, updated_at_ms)
        .await
        .map_err(ApiError::Internal)?;

    if let Err(e) = storage::backfill_usage_event_costs(db_path).await {
        tracing::warn!(err = %e, "backfill usage event costs failed");
    }

    Ok((updated, updated_at_ms))
}

async fn pricing_auto_update_loop(
    db_path: PathBuf,
    http_client: reqwest::Client,
    mut notify: watch::Receiver<u64>,
) {
    loop {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed");
                storage::AppSettings::default()
            }
        };

        if !settings.pricing_auto_update_enabled {
            if notify.changed().await.is_err() {
                break;
            }
            continue;
        }

        let hours = settings.pricing_auto_update_interval_hours.clamp(1, 8760);
        if let Err(e) = run_pricing_sync(&http_client, db_path.clone()).await {
            tracing::warn!(err = %e, "pricing auto sync failed");
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs((hours as u64) * 3600)) => {}
            changed = notify.changed() => {
                if changed.is_err() { break; }
                continue;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StatsRange {
    Today,
    Month,
}

impl StatsRange {
    fn as_str(self) -> &'static str {
        match self {
            StatsRange::Today => "today",
            StatsRange::Month => "month",
        }
    }
}

impl std::str::FromStr for StatsRange {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "today" => Ok(StatsRange::Today),
            "month" => Ok(StatsRange::Month),
            other => Err(format!("未知 range：{other}")),
        }
    }
}

fn start_ms_for_range(range: StatsRange) -> i64 {
    let now = time::OffsetDateTime::now_utc();
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let local = now.to_offset(offset);

    let start_local = match range {
        StatsRange::Today => local.replace_time(time::Time::MIDNIGHT),
        StatsRange::Month => {
            let d = local.date();
            let first = time::Date::from_calendar_date(d.year(), d.month(), 1).unwrap_or(d);
            local.replace_date(first).replace_time(time::Time::MIDNIGHT)
        }
    };

    (start_local
        .to_offset(time::UtcOffset::UTC)
        .unix_timestamp_nanos()
        / 1_000_000) as i64
}

fn current_local_offset_ms() -> i64 {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    (offset.whole_seconds() as i64) * 1000
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    range: Option<String>,
}

#[derive(Serialize)]
struct StatsSummaryResponse {
    range: String,
    #[serde(flatten)]
    summary: storage::StatsSummary,
}

async fn stats_summary(
    State(state): State<AppState>,
    Query(q): Query<StatsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let range = q
        .range
        .as_deref()
        .unwrap_or("today")
        .parse::<StatsRange>()
        .map_err(ApiError::BadRequest)?;
    let start_ms = start_ms_for_range(range);
    let summary = storage::stats_summary((*state.db_path).clone(), start_ms).await?;
    Ok(Json(StatsSummaryResponse {
        range: range.as_str().to_string(),
        summary,
    }))
}

#[derive(Serialize)]
struct StatsChannelsResponse {
    range: String,
    start_ms: i64,
    items: Vec<storage::ChannelStats>,
}

async fn stats_channels(
    State(state): State<AppState>,
    Query(q): Query<StatsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let range = q
        .range
        .as_deref()
        .unwrap_or("today")
        .parse::<StatsRange>()
        .map_err(ApiError::BadRequest)?;
    let start_ms = start_ms_for_range(range);
    let items = storage::stats_channels((*state.db_path).clone(), start_ms).await?;
    Ok(Json(StatsChannelsResponse {
        range: range.as_str().to_string(),
        start_ms,
        items,
    }))
}

#[derive(Serialize)]
struct StatsTrendResponse {
    range: String,
    start_ms: i64,
    unit: String,
    items: Vec<storage::TrendPoint>,
}

async fn stats_trend(
    State(state): State<AppState>,
    Query(q): Query<StatsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let range = q
        .range
        .as_deref()
        .unwrap_or("today")
        .parse::<StatsRange>()
        .map_err(ApiError::BadRequest)?;

    match range {
        StatsRange::Month => {
            let start_ms = start_ms_for_range(range);
            let items = storage::stats_trend_by_day_channel(
                (*state.db_path).clone(),
                start_ms,
                current_local_offset_ms(),
            )
            .await?;
            Ok(Json(StatsTrendResponse {
                range: range.as_str().to_string(),
                start_ms,
                unit: "day".to_string(),
                items,
            }))
        }
        StatsRange::Today => Err(ApiError::BadRequest("trend 仅支持 range=month".to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct UsageListQueryParams {
    start_ms: Option<String>,
    end_ms: Option<String>,
    protocol: Option<String>,
    channel_id: Option<String>,
    model: Option<String>,
    request_id: Option<String>,
    success: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

async fn usage_list(
    State(state): State<AppState>,
    Query(q): Query<UsageListQueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    fn parse_i64(name: &str, v: Option<String>) -> Result<Option<i64>, ApiError> {
        let Some(v) = v else { return Ok(None) };
        let s = v.trim();
        if s.is_empty() {
            return Ok(None);
        }
        s.parse::<i64>()
            .map(Some)
            .map_err(|e| ApiError::BadRequest(format!("{name} 无效：{e}")))
    }

    fn parse_bool(name: &str, v: Option<String>) -> Result<Option<bool>, ApiError> {
        let Some(v) = v else { return Ok(None) };
        let s = v.trim().to_ascii_lowercase();
        if s.is_empty() {
            return Ok(None);
        }
        match s.as_str() {
            "true" | "1" => Ok(Some(true)),
            "false" | "0" => Ok(Some(false)),
            _ => Err(ApiError::BadRequest(format!("{name} 无效：{v}"))),
        }
    }

    let start_ms = parse_i64("start_ms", q.start_ms)?;
    let end_ms = parse_i64("end_ms", q.end_ms)?;
    let limit = parse_i64("limit", q.limit)?.unwrap_or(50).clamp(1, 500);
    let offset = parse_i64("offset", q.offset)?
        .unwrap_or(0)
        .clamp(0, 10_000_000);
    let success = parse_bool("success", q.success)?;

    let protocol = match q.protocol.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(
            s.parse::<storage::Protocol>()
                .map_err(|e| ApiError::BadRequest(e.to_string()))?,
        ),
        _ => None,
    };

    let res = storage::list_usage_events(
        (*state.db_path).clone(),
        storage::UsageListQuery {
            start_ms,
            end_ms,
            protocol,
            channel_id: q.channel_id,
            model: q.model,
            request_id: q.request_id,
            success,
            limit,
            offset,
        },
    )
    .await?;

    Ok(Json(res))
}

async fn get_settings(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let settings = storage::get_app_settings((*state.db_path).clone()).await?;
    Ok(Json(settings))
}

#[derive(Debug, Deserialize)]
struct UpdateSettingsInput {
    pricing_auto_update_enabled: Option<bool>,
    pricing_auto_update_interval_hours: Option<i64>,
    close_behavior: Option<storage::CloseBehavior>,
}

async fn update_settings(
    State(state): State<AppState>,
    Json(input): Json<UpdateSettingsInput>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(h) = input.pricing_auto_update_interval_hours {
        if !(1..=8760).contains(&h) {
            return Err(ApiError::BadRequest(
                "pricing_auto_update_interval_hours 必须在 1..=8760 之间".to_string(),
            ));
        }
    }

    let settings = storage::update_app_settings(
        (*state.db_path).clone(),
        storage::AppSettingsPatch {
            pricing_auto_update_enabled: input.pricing_auto_update_enabled,
            pricing_auto_update_interval_hours: input.pricing_auto_update_interval_hours,
            close_behavior: input.close_behavior,
        },
    )
    .await?;

    let next = *state.settings_notify.borrow() + 1;
    let _ = state.settings_notify.send(next);

    Ok(Json(settings))
}

fn build_app(state: AppState) -> Router {
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/settings", get(get_settings).put(update_settings))
        .route("/api/channels", get(list_channels).post(create_channel))
        .route("/api/channels/reorder", post(reorder_channels))
        .route(
            "/api/channels/{id}",
            put(update_channel).delete(delete_channel),
        )
        .route("/api/channels/{id}/enable", post(enable_channel))
        .route("/api/channels/{id}/disable", post(disable_channel))
        .route("/api/channels/{id}/test", post(test_channel))
        .route("/api/routes", get(list_routes).post(create_route))
        .route("/api/routes/{id}", put(update_route).delete(delete_route))
        .route("/api/routes/{id}/channels", get(list_route_channels))
        .route(
            "/api/routes/{id}/channels/reorder",
            post(reorder_route_channels),
        )
        .route("/api/pricing/status", get(pricing_status))
        .route("/api/pricing/models", get(pricing_models))
        .route("/api/pricing/sync", post(pricing_sync))
        .route("/api/stats/summary", get(stats_summary))
        .route("/api/stats/channels", get(stats_channels))
        .route("/api/stats/trend", get(stats_trend))
        .route("/api/usage/list", get(usage_list))
        .route("/v1/messages", any(proxy_anthropic))
        .route("/v1/messages/{*path}", any(proxy_anthropic))
        .route("/v1beta/{*path}", any(proxy_gemini))
        .route("/v1/{*path}", any(proxy_openai))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    #[cfg(feature = "embed-ui")]
    let app = app.fallback(any(ui_fallback));

    #[cfg(not(feature = "embed-ui"))]
    let app = {
        let dist = std::path::PathBuf::from("ui/dist");
        if dist.is_dir() {
            app.fallback(any(ui_fs_fallback)).nest_service(
                "/assets",
                ServeDir::new(dist.join("assets")).append_index_html_on_directories(false),
            )
        } else {
            app.route("/", get(ui_placeholder))
                .fallback(any(ui_placeholder))
        }
    };

    app
}

pub async fn serve_with_listener(
    listener: tokio::net::TcpListener,
    db_path: PathBuf,
    open_browser: bool,
) -> anyhow::Result<()> {
    let addr = listener.local_addr()?;
    let (settings_notify, settings_rx) = watch::channel(0u64);
    let http_client = reqwest::Client::builder().build()?;
    let db_path = Arc::new(db_path);
    let state = AppState {
        listen_addr: addr,
        db_path: db_path.clone(),
        http_client: http_client.clone(),
        settings_notify,
    };

    let app = build_app(state);

    tokio::spawn(pricing_auto_update_loop(
        (*db_path).clone(),
        http_client,
        settings_rx,
    ));

    if open_browser {
        let url = format!("http://{addr}");
        if let Err(e) = open_in_browser(&url) {
            tracing::warn!(url = %url, err = %e, "open browser failed");
        }
    }

    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn serve(addr: SocketAddr, db_path: PathBuf, open_browser: bool) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    serve_with_listener(listener, db_path, open_browser).await
}

fn open_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = url;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unsupported platform",
        ))
    }
}
