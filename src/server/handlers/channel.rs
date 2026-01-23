use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::proxy;
use crate::server::AppState;
use crate::server::error::{ApiError, map_storage_unit_no_content_err};
use crate::storage;

fn real_multiplier_is_valid(v: f64) -> bool {
    if !v.is_finite() || v < 0.0 {
        return false;
    }
    let scaled = v * 100.0;
    (scaled - scaled.round()).abs() < 1e-9
}

fn protocol_rank(p: storage::Protocol) -> i32 {
    match p {
        storage::Protocol::Openai => 0,
        storage::Protocol::Anthropic => 1,
        storage::Protocol::Gemini => 2,
    }
}

fn sort_channels(channels: &mut [storage::Channel]) {
    channels.sort_by(|a, b| {
        protocol_rank(a.protocol)
            .cmp(&protocol_rank(b.protocol))
            .then_with(|| b.priority.cmp(&a.priority))
            .then_with(|| a.name.cmp(&b.name))
    });
}

fn update_channels_cache(state: &AppState, f: impl FnOnce(&mut Vec<storage::Channel>) -> bool) {
    state.channels_cache.send_modify(|cur| {
        let mut next = (**cur).clone();
        if !f(&mut next) {
            return;
        }
        sort_channels(&mut next);
        *cur = Arc::new(next);
    });
}

pub(in crate::server) async fn list_channels(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let channels = storage::list_channels(state.db_path()).await?;
    let _ = state.channels_cache.send(Arc::new(channels.clone()));
    Ok(Json(channels))
}

pub(in crate::server) async fn channel_checkins_today(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::get_channel_checkins_today(state.db_path()).await?;
    Ok(Json(res))
}

pub(in crate::server) async fn complete_channel_checkin_today(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::complete_channel_checkin_today(state.db_path(), channel_id).await;
    map_storage_unit_no_content_err(res, |e| {
        matches!(
            e.downcast_ref::<storage::StorageError>(),
            Some(storage::StorageError::ChannelNotFound { .. })
        )
        .then(|| ApiError::not_found("channel_not_found", "Channel not found"))
    })
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
            return Err(ApiError::bad_request(
                "channel_ids_duplicate",
                "channel_ids contains duplicates",
            ));
        }
    }

    let channel_ids = input.channel_ids.clone();
    let res = storage::reorder_channels(state.db_path(), input.protocol, input.channel_ids).await;
    if res.is_ok() {
        let ts = storage::now_ms();
        let mut priorities = std::collections::HashMap::<String, i64>::new();
        let n = channel_ids.len() as i64;
        for (idx, channel_id) in channel_ids.iter().enumerate() {
            priorities.insert(channel_id.clone(), n - (idx as i64));
        }
        update_channels_cache(&state, move |channels| {
            let mut changed = false;
            for c in channels.iter_mut() {
                let Some(p) = priorities.get(&c.id) else {
                    continue;
                };
                c.priority = *p;
                c.updated_at_ms = ts;
                changed = true;
            }
            changed
        });
    }
    map_storage_unit_no_content_err(res, |e| match e.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::ChannelNotFound { .. }) => Some(ApiError::not_found(
            "channel_not_found",
            "Channel not found",
        )),
        Some(storage::StorageError::ChannelReorderMismatch { .. }) => Some(ApiError::bad_request(
            "channel_ids_mismatch",
            "channel_ids must cover all channels under the given protocol",
        )),
        _ => None,
    })
}

pub(in crate::server) async fn create_channel(
    State(state): State<AppState>,
    Json(input): Json<storage::CreateChannel>,
) -> Result<impl IntoResponse, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "channel_name_required",
            "Channel name is required",
        ));
    }
    if input.base_url.trim().is_empty() {
        return Err(ApiError::bad_request(
            "channel_base_url_required",
            "Channel base_url is required",
        ));
    }
    if let Some(v) = input.real_multiplier
        && !real_multiplier_is_valid(v)
    {
        return Err(ApiError::bad_request(
            "channel_real_multiplier_invalid",
            "real_multiplier must be a finite number >= 0, with at most 2 decimal places",
        ));
    }

    let channel = storage::create_channel(state.db_path(), input).await?;
    let channel2 = channel.clone();
    update_channels_cache(&state, move |channels| {
        channels.push(channel2);
        true
    });
    Ok((StatusCode::CREATED, Json(channel)))
}

pub(in crate::server) async fn update_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
    Json(input): Json<storage::UpdateChannel>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(v) = input.real_multiplier
        && !real_multiplier_is_valid(v)
    {
        return Err(ApiError::bad_request(
            "channel_real_multiplier_invalid",
            "real_multiplier must be a finite number >= 0, with at most 2 decimal places",
        ));
    }
    let patch = input.clone();
    let channel_id2 = channel_id.clone();
    let res = storage::update_channel(state.db_path(), channel_id, input).await;
    if res.is_ok() {
        let ts = storage::now_ms();
        update_channels_cache(&state, move |channels| {
            let Some(channel) = channels.iter_mut().find(|c| c.id == channel_id2) else {
                return false;
            };

            if let Some(v) = patch.name {
                channel.name = v;
            }
            if let Some(v) = patch.base_url {
                channel.base_url = storage::normalize_base_url(channel.protocol, &v);
            }
            if let Some(v) = patch.auth_type {
                channel.auth_type = v;
            }
            if let Some(v) = patch.auth_ref {
                channel.auth_ref = v;
            }
            if let Some(v) = patch.checkin_url {
                channel.checkin_url = Some(v.trim().to_string()).filter(|s| !s.is_empty());
            }
            if let Some(v) = patch.priority {
                channel.priority = v;
            }
            if let Some(v) = patch.recharge_currency {
                channel.recharge_currency = v;
            }
            if let Some(v) = patch.real_multiplier {
                channel.real_multiplier = v;
            }
            if let Some(v) = patch.enabled {
                channel.enabled = v;
                if v {
                    channel.auto_disabled_until_ms = 0;
                }
            }
            channel.updated_at_ms = ts;

            true
        });
    }
    map_storage_unit_no_content_err(res, |e| {
        matches!(
            e.downcast_ref::<storage::StorageError>(),
            Some(storage::StorageError::ChannelNotFound { .. })
        )
        .then(|| ApiError::not_found("channel_not_found", "Channel not found"))
    })
}

pub(in crate::server) async fn enable_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let channel_id2 = channel_id.clone();
    let res = storage::set_channel_enabled(state.db_path(), channel_id, true).await;
    if res.is_ok() {
        let ts = storage::now_ms();
        update_channels_cache(&state, move |channels| {
            let Some(channel) = channels.iter_mut().find(|c| c.id == channel_id2) else {
                return false;
            };
            channel.enabled = true;
            channel.auto_disabled_until_ms = 0;
            channel.updated_at_ms = ts;
            true
        });
    }
    map_storage_unit_no_content_err(res, |e| {
        matches!(
            e.downcast_ref::<storage::StorageError>(),
            Some(storage::StorageError::ChannelNotFound { .. })
        )
        .then(|| ApiError::not_found("channel_not_found", "Channel not found"))
    })
}

pub(in crate::server) async fn disable_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let channel_id2 = channel_id.clone();
    let res = storage::set_channel_enabled(state.db_path(), channel_id, false).await;
    if res.is_ok() {
        let ts = storage::now_ms();
        update_channels_cache(&state, move |channels| {
            let Some(channel) = channels.iter_mut().find(|c| c.id == channel_id2) else {
                return false;
            };
            channel.enabled = false;
            channel.updated_at_ms = ts;
            true
        });
    }
    map_storage_unit_no_content_err(res, |e| {
        matches!(
            e.downcast_ref::<storage::StorageError>(),
            Some(storage::StorageError::ChannelNotFound { .. })
        )
        .then(|| ApiError::not_found("channel_not_found", "Channel not found"))
    })
}

pub(in crate::server) async fn delete_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let channel_id2 = channel_id.clone();
    let res = storage::delete_channel(state.db_path(), channel_id).await;
    if res.is_ok() {
        update_channels_cache(&state, move |channels| {
            let before = channels.len();
            channels.retain(|c| c.id != channel_id2);
            before != channels.len()
        });
    }
    map_storage_unit_no_content_err(res, |e| {
        matches!(
            e.downcast_ref::<storage::StorageError>(),
            Some(storage::StorageError::ChannelNotFound { .. })
        )
        .then(|| ApiError::not_found("channel_not_found", "Channel not found"))
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
        return Err(ApiError::not_found(
            "channel_not_found",
            "Channel not found",
        ));
    };

    let base_url = reqwest::Url::parse(&channel.base_url).map_err(|e| {
        ApiError::bad_request("channel_base_url_invalid", format!("Invalid base_url: {e}"))
    })?;

    let mut url = build_models_url(base_url, channel.protocol);
    let mut headers = axum::http::HeaderMap::new();
    proxy::apply_auth(&channel, channel.protocol, &mut url, &mut headers).map_err(|_| {
        ApiError::bad_request("channel_auth_apply_failed", "Failed to apply channel auth")
    })?;

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
