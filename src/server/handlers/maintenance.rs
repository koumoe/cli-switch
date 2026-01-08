use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::{log_files, logging, storage};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RecordsClearMode {
    DateRange,
    Errors,
    All,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct RecordsClearInput {
    mode: RecordsClearMode,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
}

pub(in crate::server) async fn records_clear(
    State(state): State<AppState>,
    Json(input): Json<RecordsClearInput>,
) -> Result<impl IntoResponse, ApiError> {
    fn validate_range(
        start_ms: Option<i64>,
        end_ms: Option<i64>,
    ) -> Result<Option<(i64, i64)>, ApiError> {
        match (start_ms, end_ms) {
            (None, None) => Ok(None),
            (Some(_), None) | (None, Some(_)) => Err(ApiError::bad_request(
                "maintenance_records_start_end_ms_required",
                "start_ms and end_ms must be provided together",
            )),
            (Some(start_ms), Some(end_ms)) => {
                if start_ms > end_ms {
                    return Err(ApiError::bad_request(
                        "maintenance_records_start_ms_gt_end_ms",
                        "start_ms must be <= end_ms",
                    ));
                }
                Ok(Some((start_ms, end_ms)))
            }
        }
    }

    let kind = match input.mode {
        RecordsClearMode::DateRange => {
            let start_ms = input.start_ms.ok_or_else(|| {
                ApiError::bad_request(
                    "maintenance_records_start_ms_required",
                    "start_ms is required when mode=date_range",
                )
            })?;
            let end_ms = input.end_ms.ok_or_else(|| {
                ApiError::bad_request(
                    "maintenance_records_end_ms_required",
                    "end_ms is required when mode=date_range",
                )
            })?;
            if start_ms > end_ms {
                return Err(ApiError::bad_request(
                    "maintenance_records_start_ms_gt_end_ms",
                    "start_ms must be <= end_ms",
                ));
            }
            storage::RecordsClearKind::DateRange { start_ms, end_ms }
        }
        RecordsClearMode::Errors => match validate_range(input.start_ms, input.end_ms)? {
            Some((start_ms, end_ms)) => {
                storage::RecordsClearKind::ErrorsDateRange { start_ms, end_ms }
            }
            None => storage::RecordsClearKind::Errors,
        },
        RecordsClearMode::All => match validate_range(input.start_ms, input.end_ms)? {
            Some((start_ms, end_ms)) => storage::RecordsClearKind::DateRange { start_ms, end_ms },
            None => storage::RecordsClearKind::All,
        },
    };

    let res = storage::clear_records(state.db_path(), kind).await?;
    tracing::info!(
        mode = ?input.mode,
        usage_events_deleted = res.usage_events_deleted,
        vacuumed = res.vacuumed,
        "records cleared"
    );
    Ok(Json(res))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LogsClearMode {
    DateRange,
    All,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct LogsClearInput {
    mode: LogsClearMode,
    start_date: Option<String>,
    end_date: Option<String>,
}

fn parse_ymd_date(input: &str) -> Result<time::Date, ApiError> {
    use std::sync::OnceLock;
    use time::format_description::BorrowedFormatItem;

    static FMT_DASH: OnceLock<Vec<BorrowedFormatItem<'static>>> = OnceLock::new();
    static FMT_SLASH: OnceLock<Vec<BorrowedFormatItem<'static>>> = OnceLock::new();

    let fmt_dash = FMT_DASH.get_or_init(|| {
        time::format_description::parse("[year]-[month]-[day]").expect("valid format")
    });
    let fmt_slash = FMT_SLASH.get_or_init(|| {
        time::format_description::parse("[year]/[month]/[day]").expect("valid format")
    });

    let s = input.trim();
    if let Ok(d) = time::Date::parse(s, fmt_dash) {
        return Ok(d);
    }
    if let Ok(d) = time::Date::parse(s, fmt_slash) {
        return Ok(d);
    }

    Err(ApiError::bad_request(
        "maintenance_logs_date_format_invalid",
        "Invalid date format, expected YYYY-MM-DD or YYYY/MM/DD",
    ))
}

pub(in crate::server) async fn logs_clear(
    State(state): State<AppState>,
    Json(input): Json<LogsClearInput>,
) -> Result<impl IntoResponse, ApiError> {
    let data_dir = state.data_dir();
    let log_dir = crate::app::logs_dir(&data_dir);
    let log_dir_display = log_dir.display().to_string();

    let kind = match input.mode {
        LogsClearMode::DateRange => {
            let start_opt = input
                .start_date
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let end_opt = input
                .end_date
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());

            let start_raw = start_opt.or(end_opt).ok_or_else(|| {
                ApiError::bad_request(
                    "maintenance_logs_date_range_requires_one",
                    "start_date or end_date is required when mode=date_range",
                )
            })?;
            let end_raw = end_opt.or(start_opt).unwrap_or(start_raw);

            let start = parse_ymd_date(start_raw)?;
            let end = parse_ymd_date(end_raw)?;
            if start > end {
                return Err(ApiError::bad_request(
                    "maintenance_logs_start_date_gt_end_date",
                    "start_date must be <= end_date",
                ));
            }
            log_files::LogsClearKind::DateRange { start, end }
        }
        LogsClearMode::All => log_files::LogsClearKind::All,
    };

    let res = tokio::task::spawn_blocking(move || log_files::clear_logs(&log_dir, kind))
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))??;
    tracing::info!(
        mode = ?input.mode,
        log_dir = %log_dir_display,
        deleted_files = res.deleted_files,
        truncated_files = res.truncated_files,
        "logs cleared"
    );
    Ok(Json(res))
}

#[derive(Serialize)]
struct LogsSizeResponse {
    path: String,
    total_bytes: u64,
    file_count: u64,
}

pub(in crate::server) async fn logs_size(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let data_dir = state.data_dir();
    let log_dir = crate::app::logs_dir(&data_dir);
    let log_dir_display = log_dir.display().to_string();

    let res = tokio::task::spawn_blocking(move || log_files::logs_size(&log_dir))
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))??;

    Ok(Json(LogsSizeResponse {
        path: log_dir_display,
        total_bytes: res.total_bytes,
        file_count: res.file_count,
    }))
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct FrontendLogIngestInput {
    level: logging::LogLevel,
    message: String,
    event: Option<String>,
    fields: Option<serde_json::Value>,
    ts_ms: Option<i64>,
}

pub(in crate::server) async fn frontend_log_ingest(
    headers: axum::http::HeaderMap,
    Json(input): Json<FrontendLogIngestInput>,
) -> impl IntoResponse {
    let ua = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    let ts_ms = input.ts_ms.unwrap_or_else(storage::now_ms);
    let event = input.event.unwrap_or_else(|| "frontend_log".to_string());

    match input.level {
        logging::LogLevel::None => {}
        logging::LogLevel::Debug => tracing::debug!(
            target: "frontend",
            ts_ms,
            ua = %ua,
            event = %event,
            message = %input.message,
            fields = ?input.fields
        ),
        logging::LogLevel::Info => tracing::info!(
            target: "frontend",
            ts_ms,
            ua = %ua,
            event = %event,
            message = %input.message,
            fields = ?input.fields
        ),
        logging::LogLevel::Warning => tracing::warn!(
            target: "frontend",
            ts_ms,
            ua = %ua,
            event = %event,
            message = %input.message,
            fields = ?input.fields
        ),
        logging::LogLevel::Error => tracing::error!(
            target: "frontend",
            ts_ms,
            ua = %ua,
            event = %event,
            message = %input.message,
            fields = ?input.fields
        ),
    }

    StatusCode::NO_CONTENT
}

#[derive(Serialize)]
struct DbSizeResponse {
    path: String,
    db_bytes: u64,
    wal_bytes: u64,
    shm_bytes: u64,
    total_bytes: u64,
}

pub(in crate::server) async fn db_size(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    fn file_len(p: &Path) -> u64 {
        std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
    }

    let db_path = state.db_path.as_path();
    let wal_path = PathBuf::from(format!("{}-wal", db_path.display()));
    let shm_path = PathBuf::from(format!("{}-shm", db_path.display()));

    let db_bytes = file_len(db_path);
    let wal_bytes = file_len(&wal_path);
    let shm_bytes = file_len(&shm_path);
    let total_bytes = db_bytes.saturating_add(wal_bytes).saturating_add(shm_bytes);

    Ok(Json(DbSizeResponse {
        path: db_path.display().to_string(),
        db_bytes,
        wal_bytes,
        shm_bytes,
        total_bytes,
    }))
}
