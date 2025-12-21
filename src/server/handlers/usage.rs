use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

#[derive(Debug, Deserialize)]
pub(in crate::server) struct UsageListQueryParams {
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

pub(in crate::server) async fn usage_list(
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
        state.db_path(),
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
