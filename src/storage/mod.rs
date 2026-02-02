use anyhow::Context as _;
use rusqlite::{Connection, params};
use serde::Serialize;
use std::path::{Path, PathBuf};

const SQLITE_BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

mod channel;
mod checkin;
mod error;
mod pricing;
mod protocol;
mod route;
mod settings;
mod stats;
mod update_ignore;
mod usage;

pub use error::StorageError;

pub use channel::{
    Channel, CreateChannel, RechargeCurrency, UpdateChannel, channel_is_auto_disabled,
    clear_channel_failures, create_channel, delete_channel, get_channel, list_channels,
    record_channel_failure_and_maybe_disable, reorder_channels, set_channel_enabled,
    update_channel,
};
pub use checkin::{
    ChannelCheckinsToday, complete_channel_checkin_today, get_channel_checkins_today,
};
pub use pricing::{
    PricingModel, PricingStatus, UpsertPricingModel, pricing_status, search_pricing_models,
    upsert_pricing_models,
};
pub use protocol::Protocol;
pub(crate) use protocol::normalize_base_url;
pub use route::{
    CreateRoute, Route, RouteChannel, UpdateRoute, create_route, delete_route, get_route,
    list_route_channels, list_routes, set_route_channels, update_route,
};
pub use settings::{
    AppSettings, AppSettingsPatch, AutoStartLaunchMode, CloseBehavior, get_app_settings,
    update_app_settings,
};
pub use stats::{
    ChannelStats, StatsSummary, TrendPoint, stats_channels, stats_summary,
    stats_trend_by_day_channel,
};
pub use update_ignore::{ignore_update_version, is_update_version_ignored};
pub use usage::{
    CreateUsageEvent, UsageEvent, UsageListQuery, UsageListResult, backfill_usage_event_costs,
    insert_usage_event, list_usage_events, list_usage_events_recent,
};

fn open_conn(db_path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("打开 SQLite 文件失败：{}", db_path.display()))?;
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)
        .with_context(|| "设置 SQLite busy_timeout 失败")?;
    Ok(conn)
}

pub fn init_db(db_path: &Path) -> anyhow::Result<()> {
    let conn = open_conn(db_path)?;

    let migration = include_str!("../../migrations/001_init.sql");
    conn.execute_batch(migration)
        .with_context(|| "执行 migrations/001_init.sql 失败")?;

    Ok(())
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or_else(|_| {
            tracing::error!("system time is too far in the future (ms overflow)");
            i64::MAX
        }),
        Err(e) => {
            tracing::error!(err = %e, "system time is before unix epoch");
            0
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RecordsClearKind {
    DateRange { start_ms: i64, end_ms: i64 },
    ErrorsDateRange { start_ms: i64, end_ms: i64 },
    Errors,
    All,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearRecordsResult {
    pub usage_events_deleted: i64,
    pub vacuumed: bool,
}

pub async fn clear_records(
    db_path: PathBuf,
    kind: RecordsClearKind,
) -> anyhow::Result<ClearRecordsResult> {
    with_conn(db_path, move |conn| {
        let usage_events_deleted: i64 = match kind {
            RecordsClearKind::DateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM usage_events WHERE ts_ms >= ?1 AND ts_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .context("转换删除行数失败")?,
            RecordsClearKind::ErrorsDateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM usage_events WHERE success = 0 AND ts_ms >= ?1 AND ts_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .context("转换删除行数失败")?,
            RecordsClearKind::Errors => conn
                .execute(r#"DELETE FROM usage_events WHERE success = 0"#, [])?
                .try_into()
                .context("转换删除行数失败")?,
            RecordsClearKind::All => conn
                .execute(r#"DELETE FROM usage_events"#, [])?
                .try_into()
                .context("转换删除行数失败")?,
        };

        let vacuumed = matches!(kind, RecordsClearKind::Errors | RecordsClearKind::All);
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        if vacuumed {
            conn.execute_batch("VACUUM;")?;
        }

        Ok(ClearRecordsResult {
            usage_events_deleted,
            vacuumed,
        })
    })
    .await
}

async fn with_conn<T, F>(db_path: PathBuf, f: F) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> anyhow::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let conn = open_conn(&db_path)?;
        f(&conn)
    })
    .await
    .context("等待 sqlite blocking 任务失败")?
}
