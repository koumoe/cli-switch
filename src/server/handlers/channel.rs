use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::proxy;
use crate::server::AppState;
use crate::server::error::{ApiError, map_storage_unit_no_content};
use crate::storage;

pub(in crate::server) async fn list_channels(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let channels = storage::list_channels(state.db_path()).await?;
    Ok(Json(channels))
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct ReorderChannelsInput {
    protocol: Option<storage::Protocol>,
    channel_ids: Vec<String>,
}

pub(in crate::server) async fn reorder_channels(
    State(state): State<AppState>,
    Json(input): Json<ReorderChannelsInput>,
) -> Result<impl IntoResponse, ApiError> {
    let mut seen = std::collections::HashSet::<String>::new();
    for id in &input.channel_ids {
        if !seen.insert(id.clone()) {
            return Err(ApiError::BadRequest("channel_ids 存在重复项".to_string()));
        }
    }

    let res = storage::reorder_channels(state.db_path(), input.protocol, input.channel_ids).await;
    map_storage_unit_no_content(res, |msg| {
        if msg.starts_with("channel not found") {
            Some(ApiError::NotFound("channel not found".to_string()))
        } else if msg.starts_with("channel reorder mismatch") {
            Some(ApiError::BadRequest(
                "channel_ids 需要覆盖该终端下所有渠道".to_string(),
            ))
        } else {
            None
        }
    })
}

pub(in crate::server) async fn create_channel(
    State(state): State<AppState>,
    Json(input): Json<storage::CreateChannel>,
) -> Result<impl IntoResponse, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name 不能为空".to_string()));
    }
    if input.base_url.trim().is_empty() {
        return Err(ApiError::BadRequest("base_url 不能为空".to_string()));
    }

    let channel = storage::create_channel(state.db_path(), input).await?;
    Ok((StatusCode::CREATED, Json(channel)))
}

pub(in crate::server) async fn update_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
    Json(input): Json<storage::UpdateChannel>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::update_channel(state.db_path(), channel_id, input).await;
    map_storage_unit_no_content(res, |msg| {
        msg.starts_with("channel not found")
            .then(|| ApiError::NotFound("channel not found".to_string()))
    })
}

pub(in crate::server) async fn enable_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    storage::set_channel_enabled(state.db_path(), channel_id, true).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(in crate::server) async fn disable_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    storage::set_channel_enabled(state.db_path(), channel_id, false).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(in crate::server) async fn delete_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::delete_channel(state.db_path(), channel_id).await;
    map_storage_unit_no_content(res, |msg| {
        msg.starts_with("channel not found")
            .then(|| ApiError::NotFound("channel not found".to_string()))
    })
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

pub(in crate::server) async fn test_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(channel) = storage::get_channel(state.db_path(), channel_id).await? else {
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
