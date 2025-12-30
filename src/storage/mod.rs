use anyhow::Context as _;
use rusqlite::{Connection, params};
use serde::Serialize;
use std::path::{Path, PathBuf};

mod channel;
mod pricing;
mod protocol;
mod route;
mod settings;
mod stats;
mod usage;

pub use channel::{
    Channel, ChannelEndpoint, ChannelKey, CreateChannel, CreateChannelEndpoint, CreateChannelKey,
    RechargeCurrency, UpdateChannel, UpdateChannelEndpoint, UpdateChannelKey,
    channel_is_auto_disabled, clear_channel_failures, clear_endpoint_failures, clear_key_failures,
    create_channel, create_channel_endpoint, create_channel_key, delete_channel,
    delete_channel_endpoint, delete_channel_key, endpoint_is_auto_disabled, get_channel,
    key_is_auto_disabled, list_channel_endpoints, list_channel_keys, list_channels,
    record_channel_failure_and_maybe_disable, record_endpoint_failure_and_maybe_disable,
    record_key_failure_and_maybe_disable, reorder_channels, set_channel_enabled,
    set_channel_endpoint_enabled, set_channel_key_enabled, update_channel, update_channel_endpoint,
    update_channel_key,
};
pub use pricing::{
    PricingModel, PricingStatus, UpsertPricingModel, pricing_status, search_pricing_models,
    upsert_pricing_models,
};
pub use protocol::Protocol;
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
pub use usage::{
    CreateUsageEvent, UsageEvent, UsageListQuery, UsageListResult, backfill_usage_event_costs,
    insert_usage_event, list_usage_events, list_usage_events_recent,
};

pub fn init_db(db_path: &Path) -> anyhow::Result<()> {
    let conn = Connection::open(db_path).with_context(|| "打开 SQLite 文件失败")?;

    let migration = include_str!("../../migrations/001_init.sql");
    conn.execute_batch(migration)
        .with_context(|| "执行 migrations/001_init.sql 失败")?;

    ensure_channels_schema(&conn)?;
    ensure_channel_failures_schema(&conn)?;
    ensure_app_settings_schema(&conn)?;
    ensure_pricing_models_schema(&conn)?;
    ensure_usage_events_schema(&conn)?;
    ensure_multi_key_endpoint_schema(&conn)?;

    Ok(())
}

fn ensure_channels_schema(conn: &Connection) -> anyhow::Result<()> {
    ensure_column(conn, "channels", "priority", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(
        conn,
        "channels",
        "recharge_currency",
        "TEXT NOT NULL DEFAULT 'CNY'",
    )?;
    ensure_column(
        conn,
        "channels",
        "real_multiplier",
        "REAL NOT NULL DEFAULT 1.0",
    )?;
    ensure_column(
        conn,
        "channels",
        "auto_disabled_until_ms",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    Ok(())
}

fn ensure_channel_failures_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS channel_failures (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          channel_id TEXT NOT NULL,
          at_ms INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"CREATE INDEX IF NOT EXISTS idx_channel_failures_channel_ts ON channel_failures(channel_id, at_ms)"#,
        [],
    )?;
    Ok(())
}

fn ensure_app_settings_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS app_settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at_ms INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    Ok(())
}

fn ensure_pricing_models_schema(conn: &Connection) -> anyhow::Result<()> {
    ensure_column(conn, "pricing_models", "cache_read_price", "TEXT NULL")?;
    ensure_column(conn, "pricing_models", "cache_write_price", "TEXT NULL")?;
    Ok(())
}

fn ensure_usage_events_schema(conn: &Connection) -> anyhow::Result<()> {
    ensure_column(conn, "usage_events", "ttft_ms", "INTEGER NULL")?;
    ensure_column(conn, "usage_events", "request_id", "TEXT NULL")?;
    ensure_column(conn, "usage_events", "error_detail", "TEXT NULL")?;
    ensure_column(conn, "usage_events", "cache_read_tokens", "INTEGER NULL")?;
    ensure_column(conn, "usage_events", "cache_write_tokens", "INTEGER NULL")?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_usage_request_ts ON usage_events(request_id, ts_ms)",
        [],
    )?;
    Ok(())
}

fn ensure_multi_key_endpoint_schema(conn: &Connection) -> anyhow::Result<()> {
    // channels 表新增开关字段
    ensure_column(conn, "channels", "use_key_pool", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(conn, "channels", "use_endpoint_pool", "INTEGER NOT NULL DEFAULT 0")?;

    // 密钥池表
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS channel_keys (
            id TEXT PRIMARY KEY,
            channel_id TEXT NOT NULL,
            auth_ref TEXT NOT NULL,
            label TEXT,
            priority INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1,
            auto_disabled_until_ms INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_channel_keys_channel ON channel_keys(channel_id, priority DESC)",
        [],
    )?;

    // 端点池表
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS channel_endpoints (
            id TEXT PRIMARY KEY,
            channel_id TEXT NOT NULL,
            base_url TEXT NOT NULL,
            label TEXT,
            priority INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 1,
            auto_disabled_until_ms INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_channel_endpoints_channel ON channel_endpoints(channel_id, priority DESC)",
        [],
    )?;

    // 密钥故障记录表
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS key_failures (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key_id TEXT NOT NULL,
            at_ms INTEGER NOT NULL,
            FOREIGN KEY (key_id) REFERENCES channel_keys(id) ON DELETE CASCADE
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_key_failures_key_ts ON key_failures(key_id, at_ms)",
        [],
    )?;

    // 端点故障记录表
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS endpoint_failures (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            endpoint_id TEXT NOT NULL,
            at_ms INTEGER NOT NULL,
            FOREIGN KEY (endpoint_id) REFERENCES channel_endpoints(id) ON DELETE CASCADE
        )
        "#,
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_endpoint_failures_endpoint_ts ON endpoint_failures(endpoint_id, at_ms)",
        [],
    )?;

    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    column_def: &str,
) -> anyhow::Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(());
        }
    }

    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {column_def}"),
        [],
    )
    .with_context(|| format!("为 {table} 添加字段 {column} 失败"))?;

    Ok(())
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy)]
pub enum RecordsClearKind {
    DateRange { start_ms: i64, end_ms: i64 },
    Errors,
    All,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearRecordsResult {
    pub usage_events_deleted: i64,
    pub channel_failures_deleted: i64,
    pub vacuumed: bool,
}

pub async fn clear_records(
    db_path: PathBuf,
    kind: RecordsClearKind,
) -> anyhow::Result<ClearRecordsResult> {
    with_conn(db_path, move |conn| {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        let usage_events_deleted: i64 = match kind {
            RecordsClearKind::DateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM usage_events WHERE ts_ms >= ?1 AND ts_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .unwrap_or(i64::MAX),
            RecordsClearKind::Errors => conn
                .execute(r#"DELETE FROM usage_events WHERE success = 0"#, [])?
                .try_into()
                .unwrap_or(i64::MAX),
            RecordsClearKind::All => conn
                .execute(r#"DELETE FROM usage_events"#, [])?
                .try_into()
                .unwrap_or(i64::MAX),
        };

        let channel_failures_deleted: i64 = match kind {
            RecordsClearKind::DateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM channel_failures WHERE at_ms >= ?1 AND at_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .unwrap_or(i64::MAX),
            RecordsClearKind::Errors | RecordsClearKind::All => conn
                .execute(r#"DELETE FROM channel_failures"#, [])?
                .try_into()
                .unwrap_or(i64::MAX),
        };

        let vacuumed = matches!(kind, RecordsClearKind::Errors | RecordsClearKind::All);
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        if vacuumed {
            conn.execute_batch("VACUUM;")?;
        }

        Ok(ClearRecordsResult {
            usage_events_deleted,
            channel_failures_deleted,
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
        let conn = Connection::open(&db_path)
            .with_context(|| format!("打开 SQLite 文件失败：{}", db_path.display()))?;
        f(&conn)
    })
    .await
    .context("等待 sqlite blocking 任务失败")?
}
