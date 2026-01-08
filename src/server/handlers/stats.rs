use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

#[derive(Debug, Clone, Copy)]
enum StatsRange {
    Today,
    Yesterday,
    Week,
    Month,
}

impl StatsRange {
    fn as_str(self) -> &'static str {
        match self {
            StatsRange::Today => "today",
            StatsRange::Yesterday => "yesterday",
            StatsRange::Week => "week",
            StatsRange::Month => "month",
        }
    }
}

impl std::str::FromStr for StatsRange {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "today" => Ok(StatsRange::Today),
            "yesterday" => Ok(StatsRange::Yesterday),
            "week" => Ok(StatsRange::Week),
            "month" => Ok(StatsRange::Month),
            other => Err(format!("Invalid range: {other}")),
        }
    }
}

fn window_ms_for_range(range: StatsRange) -> (i64, Option<i64>) {
    let now = time::OffsetDateTime::now_utc();
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let local = now.to_offset(offset);

    let start_local = match range {
        StatsRange::Today => local.replace_time(time::Time::MIDNIGHT),
        StatsRange::Yesterday => {
            (local - time::Duration::days(1)).replace_time(time::Time::MIDNIGHT)
        }
        StatsRange::Week => {
            let d = local.date();
            let days_since_monday = i64::from(d.weekday().number_from_monday().saturating_sub(1));
            let first = d - time::Duration::days(days_since_monday);
            local.replace_date(first).replace_time(time::Time::MIDNIGHT)
        }
        StatsRange::Month => {
            let d = local.date();
            let first = time::Date::from_calendar_date(d.year(), d.month(), 1).unwrap_or(d);
            local.replace_date(first).replace_time(time::Time::MIDNIGHT)
        }
    };

    let start_ms = (start_local
        .to_offset(time::UtcOffset::UTC)
        .unix_timestamp_nanos()
        / 1_000_000) as i64;

    let end_ms = match range {
        StatsRange::Yesterday => {
            let start_today_local = local.replace_time(time::Time::MIDNIGHT);
            let start_today_ms = (start_today_local
                .to_offset(time::UtcOffset::UTC)
                .unix_timestamp_nanos()
                / 1_000_000) as i64;
            Some(start_today_ms.saturating_sub(1))
        }
        StatsRange::Today | StatsRange::Week | StatsRange::Month => None,
    };

    (start_ms, end_ms)
}

fn current_local_offset_ms() -> i64 {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    (offset.whole_seconds() as i64) * 1000
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct StatsQuery {
    range: Option<String>,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
}

fn resolve_stats_window(q: &StatsQuery) -> Result<(String, i64, Option<i64>), ApiError> {
    if q.start_ms.is_some() || q.end_ms.is_some() {
        let start_ms = q
            .start_ms
            .ok_or_else(|| ApiError::bad_request("stats_missing_start_ms", "Missing start_ms"))?;
        let end_ms = q.end_ms;
        if let Some(end_ms) = end_ms {
            if end_ms < start_ms {
                return Err(ApiError::bad_request(
                    "stats_invalid_ms_range",
                    "end_ms must be >= start_ms",
                ));
            }
        }
        return Ok(("custom".to_string(), start_ms, end_ms));
    }

    let range = q
        .range
        .as_deref()
        .unwrap_or("today")
        .parse::<StatsRange>()
        .map_err(|e| ApiError::bad_request("stats_range_invalid", e))?;

    let (start_ms, end_ms) = window_ms_for_range(range);
    Ok((range.as_str().to_string(), start_ms, end_ms))
}

#[derive(Serialize)]
struct StatsSummaryResponse {
    range: String,
    #[serde(flatten)]
    summary: storage::StatsSummary,
}

pub(in crate::server) async fn stats_summary(
    State(state): State<AppState>,
    Query(q): Query<StatsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let (range, start_ms, end_ms) = resolve_stats_window(&q)?;
    let summary = storage::stats_summary(state.db_path(), start_ms, end_ms).await?;
    Ok(Json(StatsSummaryResponse {
        range,
        summary,
    }))
}

#[derive(Serialize)]
struct StatsChannelsResponse {
    range: String,
    start_ms: i64,
    items: Vec<storage::ChannelStats>,
}

pub(in crate::server) async fn stats_channels(
    State(state): State<AppState>,
    Query(q): Query<StatsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let (range, start_ms, end_ms) = resolve_stats_window(&q)?;
    let items = storage::stats_channels(state.db_path(), start_ms, end_ms).await?;
    Ok(Json(StatsChannelsResponse {
        range,
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

pub(in crate::server) async fn stats_trend(
    State(state): State<AppState>,
    Query(q): Query<StatsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    if q.start_ms.is_some() || q.end_ms.is_some() {
        return Err(ApiError::bad_request(
            "stats_trend_custom_unsupported",
            "trend does not support start_ms/end_ms",
        ));
    }

    let range = q
        .range
        .as_deref()
        .unwrap_or("today")
        .parse::<StatsRange>()
        .map_err(|e| ApiError::bad_request("stats_range_invalid", e))?;

    match range {
        StatsRange::Month => {
            let (start_ms, _end_ms) = window_ms_for_range(range);
            let items = storage::stats_trend_by_day_channel(
                state.db_path(),
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
        StatsRange::Today | StatsRange::Yesterday | StatsRange::Week => Err(ApiError::bad_request(
            "stats_trend_range_unsupported",
            "trend only supports range=month",
        )),
    }
}
