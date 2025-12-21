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
pub(in crate::server) struct StatsQuery {
    range: Option<String>,
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
    let range = q
        .range
        .as_deref()
        .unwrap_or("today")
        .parse::<StatsRange>()
        .map_err(ApiError::BadRequest)?;
    let start_ms = start_ms_for_range(range);
    let summary = storage::stats_summary(state.db_path(), start_ms).await?;
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

pub(in crate::server) async fn stats_channels(
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
    let items = storage::stats_channels(state.db_path(), start_ms).await?;
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

pub(in crate::server) async fn stats_trend(
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
        StatsRange::Today => Err(ApiError::BadRequest("trend 仅支持 range=month".to_string())),
    }
}
