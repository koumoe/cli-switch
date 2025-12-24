use anyhow::Context as _;
use rusqlite::{Connection, params};
use serde::Serialize;
use std::path::{Path, PathBuf};

mod channel;
mod channel_endpoint;
mod channel_key;
mod endpoint_key;
mod pricing;
mod protocol;
mod route;
mod settings;
mod stats;
mod usage;

pub use channel::{
    Channel, CreateChannel, RechargeCurrency, UpdateChannel, channel_is_auto_disabled,
    clear_channel_failures, create_channel, delete_channel, get_channel, list_channels,
    record_channel_failure_and_maybe_disable, reorder_channels, set_channel_enabled,
    update_channel,
};
pub use channel_endpoint::{
    ChannelEndpoint, clear_endpoint_failures, list_channel_endpoints,
    record_endpoint_failure_and_maybe_disable,
};
pub use channel_key::{
    ChannelKey, clear_key_failures, list_channel_keys, record_key_failure_and_maybe_disable,
};
pub use endpoint_key::{
    UpstreamAttempt, clear_endpoint_key_failures, list_available_upstream_attempts,
    record_endpoint_key_failure_and_maybe_disable,
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
    ensure_channel_endpoints_schema(&conn)?;
    ensure_channel_keys_schema(&conn)?;
    ensure_endpoint_failures_schema(&conn)?;
    ensure_key_failures_schema(&conn)?;
    ensure_endpoint_key_states_schema(&conn)?;
    ensure_endpoint_key_failures_schema(&conn)?;
    ensure_app_settings_schema(&conn)?;
    ensure_pricing_models_schema(&conn)?;
    ensure_usage_events_schema(&conn)?;
    backfill_channels_to_endpoints_and_keys(&conn)?;

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

fn ensure_channel_endpoints_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS channel_endpoints (
          id TEXT PRIMARY KEY,
          channel_id TEXT NOT NULL,
          base_url TEXT NOT NULL,
          priority INTEGER NOT NULL DEFAULT 0,
          enabled INTEGER NOT NULL,
          created_at_ms INTEGER NOT NULL,
          updated_at_ms INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_channel_endpoints_channel_priority
          ON channel_endpoints(channel_id, priority)
        "#,
        [],
    )?;
    ensure_column(
        conn,
        "channel_endpoints",
        "auto_disabled_until_ms",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    Ok(())
}

fn ensure_channel_keys_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS channel_keys (
          id TEXT PRIMARY KEY,
          channel_id TEXT NOT NULL,
          auth_ref TEXT NOT NULL,
          priority INTEGER NOT NULL DEFAULT 0,
          enabled INTEGER NOT NULL,
          created_at_ms INTEGER NOT NULL,
          updated_at_ms INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_channel_keys_channel_priority
          ON channel_keys(channel_id, priority)
        "#,
        [],
    )?;
    ensure_column(
        conn,
        "channel_keys",
        "auto_disabled_until_ms",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    Ok(())
}

fn ensure_endpoint_failures_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS endpoint_failures (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          endpoint_id TEXT NOT NULL,
          at_ms INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_endpoint_failures_endpoint_ts
          ON endpoint_failures(endpoint_id, at_ms)
        "#,
        [],
    )?;
    Ok(())
}

fn ensure_key_failures_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS key_failures (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          key_id TEXT NOT NULL,
          at_ms INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_key_failures_key_ts
          ON key_failures(key_id, at_ms)
        "#,
        [],
    )?;
    Ok(())
}

fn ensure_endpoint_key_states_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS endpoint_key_states (
          endpoint_id TEXT NOT NULL,
          key_id TEXT NOT NULL,
          auto_disabled_until_ms INTEGER NOT NULL DEFAULT 0,
          updated_at_ms INTEGER NOT NULL,
          PRIMARY KEY (endpoint_id, key_id)
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"CREATE INDEX IF NOT EXISTS idx_endpoint_key_states_until ON endpoint_key_states(auto_disabled_until_ms)"#,
        [],
    )?;
    Ok(())
}

fn ensure_endpoint_key_failures_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS endpoint_key_failures (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          endpoint_id TEXT NOT NULL,
          key_id TEXT NOT NULL,
          at_ms INTEGER NOT NULL
        )
        "#,
        [],
    )?;
    conn.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_endpoint_key_failures_pair_ts
          ON endpoint_key_failures(endpoint_id, key_id, at_ms)
        "#,
        [],
    )?;
    Ok(())
}

fn backfill_channels_to_endpoints_and_keys(conn: &Connection) -> anyhow::Result<()> {
    let ts = now_ms();

    let mut stmt = conn.prepare(r#"SELECT id, protocol, base_url, auth_ref FROM channels"#)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let channel_id: String = row.get(0)?;
        let protocol: Protocol = row.get(1)?;
        let base_url: String = row.get(2)?;
        let auth_ref: String = row.get(3)?;

        let endpoint_cnt: i64 = conn.query_row(
            r#"SELECT COUNT(*) FROM channel_endpoints WHERE channel_id = ?1"#,
            params![channel_id],
            |r| r.get(0),
        )?;
        if endpoint_cnt == 0 {
            let endpoint_id = uuid::Uuid::new_v4().to_string();
            let normalized = protocol::normalize_base_url(protocol, &base_url);
            conn.execute(
                r#"
                INSERT INTO channel_endpoints (id, channel_id, base_url, priority, enabled, created_at_ms, updated_at_ms)
                VALUES (?1, ?2, ?3, 0, 1, ?4, ?4)
                "#,
                params![endpoint_id, channel_id, normalized, ts],
            )?;
        }

        let key_cnt: i64 = conn.query_row(
            r#"SELECT COUNT(*) FROM channel_keys WHERE channel_id = ?1"#,
            params![channel_id],
            |r| r.get(0),
        )?;
        if key_cnt == 0 {
            let key_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                r#"
                INSERT INTO channel_keys (id, channel_id, auth_ref, priority, enabled, created_at_ms, updated_at_ms)
                VALUES (?1, ?2, ?3, 0, 1, ?4, ?4)
                "#,
                params![key_id, channel_id, auth_ref, ts],
            )?;
        }
    }

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

        let endpoint_failures_deleted: i64 = match kind {
            RecordsClearKind::DateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM endpoint_failures WHERE at_ms >= ?1 AND at_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .unwrap_or(i64::MAX),
            RecordsClearKind::Errors | RecordsClearKind::All => conn
                .execute(r#"DELETE FROM endpoint_failures"#, [])?
                .try_into()
                .unwrap_or(i64::MAX),
        };

        let key_failures_deleted: i64 = match kind {
            RecordsClearKind::DateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM key_failures WHERE at_ms >= ?1 AND at_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .unwrap_or(i64::MAX),
            RecordsClearKind::Errors | RecordsClearKind::All => conn
                .execute(r#"DELETE FROM key_failures"#, [])?
                .try_into()
                .unwrap_or(i64::MAX),
        };

        let endpoint_key_failures_deleted: i64 = match kind {
            RecordsClearKind::DateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM endpoint_key_failures WHERE at_ms >= ?1 AND at_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .unwrap_or(i64::MAX),
            RecordsClearKind::Errors | RecordsClearKind::All => conn
                .execute(r#"DELETE FROM endpoint_key_failures"#, [])?
                .try_into()
                .unwrap_or(i64::MAX),
        };

        let channel_failures_deleted = channel_failures_deleted
            .saturating_add(endpoint_failures_deleted)
            .saturating_add(key_failures_deleted)
            .saturating_add(endpoint_key_failures_deleted);

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
