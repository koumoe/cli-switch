use anyhow::Context as _;
use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

use crate::logging::LogLevel;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Openai,
    Anthropic,
    Gemini,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Protocol::Openai => "openai",
            Protocol::Anthropic => "anthropic",
            Protocol::Gemini => "gemini",
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Protocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "openai" => Ok(Protocol::Openai),
            "anthropic" => Ok(Protocol::Anthropic),
            "gemini" => Ok(Protocol::Gemini),
            other => Err(anyhow::anyhow!("未知 protocol：{other}")),
        }
    }
}

fn protocol_root(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Openai | Protocol::Anthropic => "/v1",
        Protocol::Gemini => "/v1beta",
    }
}

fn normalize_base_url(protocol: Protocol, base_url: &str) -> String {
    let base_url = base_url.trim();
    let (without_fragment, fragment) = match base_url.split_once('#') {
        Some((a, b)) => (a, Some(b)),
        None => (base_url, None),
    };
    let (without_query, query) = match without_fragment.split_once('?') {
        Some((a, b)) => (a, Some(b)),
        None => (without_fragment, None),
    };

    let root = protocol_root(protocol);
    let without_query = without_query.trim_end_matches('/');
    let normalized = if without_query.ends_with(root) {
        without_query[..without_query.len().saturating_sub(root.len())]
            .trim_end_matches('/')
            .to_string()
    } else {
        without_query.to_string()
    };

    let mut out = normalized;
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub auth_type: String,
    pub auth_ref: String,
    pub priority: i64,
    pub enabled: bool,
    pub auto_disabled_until_ms: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub match_model: Option<String>,
    pub enabled: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub fn init_db(db_path: &Path) -> anyhow::Result<()> {
    let conn = Connection::open(db_path).with_context(|| "打开 SQLite 文件失败")?;

    let migration = include_str!("../migrations/001_init.sql");
    conn.execute_batch(migration)
        .with_context(|| "执行 migrations/001_init.sql 失败")?;

    ensure_channels_schema(&conn)?;
    ensure_channel_failures_schema(&conn)?;
    ensure_app_settings_schema(&conn)?;
    ensure_pricing_models_schema(&conn)?;
    ensure_usage_events_schema(&conn)?;

    Ok(())
}

fn ensure_channels_schema(conn: &Connection) -> anyhow::Result<()> {
    ensure_column(conn, "channels", "priority", "INTEGER NOT NULL DEFAULT 0")?;
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

pub(crate) fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

const KEY_PRICING_AUTO_UPDATE_ENABLED: &str = "pricing_auto_update_enabled";
const KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS: &str = "pricing_auto_update_interval_hours";
const KEY_CLOSE_BEHAVIOR: &str = "close_behavior";
const KEY_AUTO_START_ENABLED: &str = "auto_start_enabled";
const KEY_APP_AUTO_UPDATE_ENABLED: &str = "app_auto_update_enabled";
const KEY_AUTO_DISABLE_ENABLED: &str = "auto_disable_enabled";
const KEY_AUTO_DISABLE_WINDOW_MINUTES: &str = "auto_disable_window_minutes";
const KEY_AUTO_DISABLE_FAILURE_TIMES: &str = "auto_disable_failure_times";
const KEY_AUTO_DISABLE_DISABLE_MINUTES: &str = "auto_disable_disable_minutes";
const KEY_LOG_LEVEL: &str = "log_level";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    Ask,
    MinimizeToTray,
    Quit,
}

impl CloseBehavior {
    fn as_str(self) -> &'static str {
        match self {
            CloseBehavior::Ask => "ask",
            CloseBehavior::MinimizeToTray => "minimize_to_tray",
            CloseBehavior::Quit => "quit",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub pricing_auto_update_enabled: bool,
    pub pricing_auto_update_interval_hours: i64,
    pub close_behavior: CloseBehavior,
    pub auto_start_enabled: bool,
    pub app_auto_update_enabled: bool,
    pub auto_disable_enabled: bool,
    pub auto_disable_window_minutes: i64,
    pub auto_disable_failure_times: i64,
    pub auto_disable_disable_minutes: i64,
    pub log_level: LogLevel,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            pricing_auto_update_enabled: false,
            pricing_auto_update_interval_hours: 24,
            close_behavior: CloseBehavior::Ask,
            auto_start_enabled: false,
            app_auto_update_enabled: false,
            auto_disable_enabled: false,
            auto_disable_window_minutes: 3,
            auto_disable_failure_times: 5,
            auto_disable_disable_minutes: 30,
            log_level: LogLevel::Warning,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppSettingsPatch {
    pub pricing_auto_update_enabled: Option<bool>,
    pub pricing_auto_update_interval_hours: Option<i64>,
    pub close_behavior: Option<CloseBehavior>,
    pub auto_start_enabled: Option<bool>,
    pub app_auto_update_enabled: Option<bool>,
    pub auto_disable_enabled: Option<bool>,
    pub auto_disable_window_minutes: Option<i64>,
    pub auto_disable_failure_times: Option<i64>,
    pub auto_disable_disable_minutes: Option<i64>,
    pub log_level: Option<LogLevel>,
}

fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

fn set_setting(
    conn: &Connection,
    key: &str,
    value: &str,
    updated_at_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"
        INSERT INTO app_settings (key, value, updated_at_ms)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET
          value = excluded.value,
          updated_at_ms = excluded.updated_at_ms
        "#,
        params![key, value, updated_at_ms],
    )?;
    Ok(())
}

pub async fn get_app_settings(db_path: PathBuf) -> anyhow::Result<AppSettings> {
    with_conn(db_path, move |conn| {
        let mut out = AppSettings::default();

        if let Some(v) = get_setting(conn, KEY_PRICING_AUTO_UPDATE_ENABLED)? {
            out.pricing_auto_update_enabled = matches!(v.trim(), "1" | "true" | "TRUE" | "True");
        }
        if let Some(v) = get_setting(conn, KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS)?
            && let Ok(n) = v.trim().parse::<i64>()
        {
            out.pricing_auto_update_interval_hours = n;
        }
        if let Some(v) = get_setting(conn, KEY_CLOSE_BEHAVIOR)? {
            match v.trim() {
                "ask" => out.close_behavior = CloseBehavior::Ask,
                "minimize_to_tray" => out.close_behavior = CloseBehavior::MinimizeToTray,
                "quit" => out.close_behavior = CloseBehavior::Quit,
                _ => {}
            }
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_START_ENABLED)? {
            out.auto_start_enabled = matches!(v.trim(), "1" | "true" | "TRUE" | "True");
        }
        if let Some(v) = get_setting(conn, KEY_APP_AUTO_UPDATE_ENABLED)? {
            out.app_auto_update_enabled = matches!(v.trim(), "1" | "true" | "TRUE" | "True");
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_ENABLED)? {
            out.auto_disable_enabled = matches!(v.trim(), "1" | "true" | "TRUE" | "True");
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_WINDOW_MINUTES)?
            && let Ok(n) = v.trim().parse::<i64>()
        {
            out.auto_disable_window_minutes = n;
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_FAILURE_TIMES)?
            && let Ok(n) = v.trim().parse::<i64>()
        {
            out.auto_disable_failure_times = n;
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_DISABLE_MINUTES)?
            && let Ok(n) = v.trim().parse::<i64>()
        {
            out.auto_disable_disable_minutes = n;
        }
        if let Some(v) = get_setting(conn, KEY_LOG_LEVEL)? {
            match v.trim() {
                "none" | "off" => out.log_level = LogLevel::None,
                "debug" => out.log_level = LogLevel::Debug,
                "info" => out.log_level = LogLevel::Info,
                "warn" | "warning" => out.log_level = LogLevel::Warning,
                "error" => out.log_level = LogLevel::Error,
                _ => {}
            }
        }

        Ok(out)
    })
    .await
}

pub async fn update_app_settings(
    db_path: PathBuf,
    patch: AppSettingsPatch,
) -> anyhow::Result<AppSettings> {
    let db_path2 = db_path.clone();
    with_conn(db_path2, move |conn| {
        let updated_at_ms = now_ms();
        if let Some(v) = patch.pricing_auto_update_enabled {
            set_setting(
                conn,
                KEY_PRICING_AUTO_UPDATE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.pricing_auto_update_interval_hours {
            set_setting(
                conn,
                KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.close_behavior {
            set_setting(conn, KEY_CLOSE_BEHAVIOR, v.as_str(), updated_at_ms)?;
        }
        if let Some(v) = patch.auto_start_enabled {
            set_setting(
                conn,
                KEY_AUTO_START_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.app_auto_update_enabled {
            set_setting(
                conn,
                KEY_APP_AUTO_UPDATE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.auto_disable_enabled {
            set_setting(
                conn,
                KEY_AUTO_DISABLE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.auto_disable_window_minutes {
            set_setting(
                conn,
                KEY_AUTO_DISABLE_WINDOW_MINUTES,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.auto_disable_failure_times {
            set_setting(
                conn,
                KEY_AUTO_DISABLE_FAILURE_TIMES,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.auto_disable_disable_minutes {
            set_setting(
                conn,
                KEY_AUTO_DISABLE_DISABLE_MINUTES,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.log_level {
            set_setting(conn, KEY_LOG_LEVEL, v.as_str(), updated_at_ms)?;
        }
        Ok(())
    })
    .await?;

    get_app_settings(db_path).await
}

pub fn channel_is_auto_disabled(channel: &Channel, now_ms: i64) -> bool {
    channel.auto_disabled_until_ms > now_ms
}

pub async fn record_channel_failure_and_maybe_disable(
    db_path: PathBuf,
    channel_id: String,
    now_ms: i64,
    window_minutes: i64,
    failure_times: i64,
    disable_minutes: i64,
) -> anyhow::Result<Option<i64>> {
    if window_minutes < 1 || disable_minutes < 1 || failure_times < 1 {
        anyhow::bail!(
            "auto_disable 配置非法：window_minutes={window_minutes}, failure_times={failure_times}, disable_minutes={disable_minutes}"
        );
    }
    let window_ms = window_minutes.saturating_mul(60_000);
    let disable_ms = disable_minutes.saturating_mul(60_000);

    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let cutoff_ms = now_ms.saturating_sub(window_ms);

        tx.execute(
            r#"DELETE FROM channel_failures WHERE channel_id = ?1 AND at_ms < ?2"#,
            params![channel_id, cutoff_ms],
        )?;
        tx.execute(
            r#"INSERT INTO channel_failures (channel_id, at_ms) VALUES (?1, ?2)"#,
            params![channel_id, now_ms],
        )?;

        let cnt: i64 = tx.query_row(
            r#"SELECT COUNT(*) FROM channel_failures WHERE channel_id = ?1 AND at_ms >= ?2"#,
            params![channel_id, cutoff_ms],
            |row| row.get(0),
        )?;

        if cnt < failure_times {
            tx.commit()?;
            return Ok(None);
        }

        let disabled_until_ms = now_ms.saturating_add(disable_ms);
        tx.execute(
            r#"
            UPDATE channels
            SET auto_disabled_until_ms = ?2, updated_at_ms = ?3
            WHERE id = ?1
            "#,
            params![channel_id, disabled_until_ms, now_ms],
        )?;
        tx.execute(
            r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        tx.commit()?;
        Ok(Some(disabled_until_ms))
    })
    .await
}

pub async fn clear_channel_failures(db_path: PathBuf, channel_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        Ok(())
    })
    .await
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

pub async fn list_channels(db_path: PathBuf) -> anyhow::Result<Vec<Channel>> {
    with_conn(db_path, |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
            FROM channels
            ORDER BY CASE protocol
              WHEN 'openai' THEN 0
              WHEN 'anthropic' THEN 1
              WHEN 'gemini' THEN 2
              ELSE 9
            END, priority DESC, name ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let protocol: String = row.get(2)?;
            let protocol = protocol.parse::<Protocol>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    e.into_boxed_dyn_error(),
                )
            })?;
            let base_url: String = row.get(3)?;
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol,
                base_url: normalize_base_url(protocol, &base_url),
                auth_type: row.get(4)?,
                auth_ref: row.get(5)?,
                priority: row.get(6)?,
                enabled: row.get::<_, i64>(7)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                created_at_ms: row.get(9)?,
                updated_at_ms: row.get(10)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChannel {
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub auth_type: Option<String>,
    pub auth_ref: String,
    #[serde(default)]
    pub priority: i64,
    pub enabled: bool,
}

pub async fn create_channel(db_path: PathBuf, input: CreateChannel) -> anyhow::Result<Channel> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let id = Uuid::new_v4().to_string();
        let auth_type = input
            .auth_type
            .unwrap_or_else(|| "auto".to_string())
            .trim()
            .to_string();
        let base_url = normalize_base_url(input.protocol, &input.base_url);
        conn.execute(
            r#"
            INSERT INTO channels (id, name, protocol, base_url, auth_type, auth_ref, priority, enabled, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                id,
                input.name,
                input.protocol.as_str(),
                base_url,
                auth_type,
                input.auth_ref,
                input.priority,
                if input.enabled { 1 } else { 0 },
                ts,
                ts,
            ],
        )?;

        Ok(Channel {
            id,
            name: input.name,
            protocol: input.protocol,
            base_url,
            auth_type,
            auth_ref: input.auth_ref,
            priority: input.priority,
            enabled: input.enabled,
            auto_disabled_until_ms: 0,
            created_at_ms: ts,
            updated_at_ms: ts,
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateChannel {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub auth_type: Option<String>,
    pub auth_ref: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

pub async fn update_channel(
    db_path: PathBuf,
    channel_id: String,
    input: UpdateChannel,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let clear_failures = input.enabled == Some(true);

        let mut channel: Channel = {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
                FROM channels
                WHERE id = ?1
                "#,
            )?;
            let row = stmt.query_row([&channel_id], |row| {
                let protocol: String = row.get(2)?;
                let protocol = protocol.parse::<Protocol>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        e.into_boxed_dyn_error(),
                    )
                })?;
                let base_url: String = row.get(3)?;
                Ok(Channel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    protocol,
                    base_url: normalize_base_url(protocol, &base_url),
                    auth_type: row.get(4)?,
                    auth_ref: row.get(5)?,
                    priority: row.get(6)?,
                    enabled: row.get::<_, i64>(7)? != 0,
                    auto_disabled_until_ms: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                    created_at_ms: row.get(9)?,
                    updated_at_ms: row.get(10)?,
                })
            });

            match row {
                Ok(v) => v,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(anyhow::anyhow!("channel not found: {channel_id}"));
                }
                Err(e) => return Err(e.into()),
            }
        };

        if let Some(v) = input.name {
            channel.name = v;
        }
        if let Some(v) = input.base_url {
            channel.base_url = normalize_base_url(channel.protocol, &v);
        }
        if let Some(v) = input.auth_type {
            channel.auth_type = v;
        }
        if let Some(v) = input.auth_ref {
            channel.auth_ref = v;
        }
        if let Some(v) = input.priority {
            channel.priority = v;
        }
        if let Some(v) = input.enabled {
            channel.enabled = v;
            if v {
                channel.auto_disabled_until_ms = 0;
            }
        }
        channel.updated_at_ms = ts;

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"
            UPDATE channels
            SET name = ?2, base_url = ?3, auth_type = ?4, auth_ref = ?5, priority = ?6, enabled = ?7, auto_disabled_until_ms = ?8, updated_at_ms = ?9
            WHERE id = ?1
            "#,
            params![
                channel.id,
                channel.name,
                channel.base_url,
                channel.auth_type,
                channel.auth_ref,
                channel.priority,
                if channel.enabled { 1 } else { 0 },
                channel.auto_disabled_until_ms,
                channel.updated_at_ms,
            ],
        )?;
        if clear_failures {
            tx.execute(
                r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
                params![channel.id],
            )?;
        }
        tx.commit()?;

        Ok(())
    })
    .await
}

pub async fn set_channel_enabled(
    db_path: PathBuf,
    channel_id: String,
    enabled: bool,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let tx = conn.unchecked_transaction()?;
        let updated = if enabled {
            tx.execute(
                r#"
                UPDATE channels
                SET enabled = ?2, auto_disabled_until_ms = 0, updated_at_ms = ?3
                WHERE id = ?1
                "#,
                params![channel_id, 1i64, ts],
            )?
        } else {
            tx.execute(
                r#"
                UPDATE channels
                SET enabled = ?2, updated_at_ms = ?3
                WHERE id = ?1
                "#,
                params![channel_id, 0i64, ts],
            )?
        };
        if enabled {
            tx.execute(
                r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
                params![channel_id],
            )?;
        }
        tx.commit()?;

        if updated == 0 {
            return Err(anyhow::anyhow!("channel not found"));
        }
        Ok(())
    })
    .await
}

pub async fn get_channel(db_path: PathBuf, channel_id: String) -> anyhow::Result<Option<Channel>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
            FROM channels
            WHERE id = ?1
            "#,
        )?;

        stmt.query_row([channel_id], |row| {
            let protocol: String = row.get(2)?;
            let protocol = protocol.parse::<Protocol>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    e.into_boxed_dyn_error(),
                )
            })?;
            let base_url: String = row.get(3)?;
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol,
                base_url: normalize_base_url(protocol, &base_url),
                auth_type: row.get(4)?,
                auth_ref: row.get(5)?,
                priority: row.get(6)?,
                enabled: row.get::<_, i64>(7)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(8)?.unwrap_or(0),
                created_at_ms: row.get(9)?,
                updated_at_ms: row.get(10)?,
            })
        })
        .optional()
        .map_err(Into::into)
    })
    .await
}

pub async fn reorder_channels(
    db_path: PathBuf,
    protocol: Option<Protocol>,
    channel_ids_in_priority_order: Vec<String>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let mut all_ids = Vec::<String>::new();
        if let Some(p) = protocol {
            let mut stmt = conn.prepare(r#"SELECT id FROM channels WHERE protocol = ?1"#)?;
            let mut rows = stmt.query([p.as_str()])?;
            while let Some(row) = rows.next()? {
                all_ids.push(row.get::<_, String>(0)?);
            }
        } else {
            let mut stmt = conn.prepare(r#"SELECT id FROM channels"#)?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                all_ids.push(row.get::<_, String>(0)?);
            }
        }

        if all_ids.len() != channel_ids_in_priority_order.len() {
            return Err(anyhow::anyhow!("channel reorder mismatch: length"));
        }

        let all_set = all_ids
            .into_iter()
            .collect::<std::collections::HashSet<String>>();
        let incoming_set = channel_ids_in_priority_order
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<String>>();
        if incoming_set != all_set {
            if incoming_set.is_subset(&all_set) {
                return Err(anyhow::anyhow!("channel reorder mismatch: coverage"));
            }
            return Err(anyhow::anyhow!("channel not found"));
        }

        let ts = now_ms();
        let tx = conn.unchecked_transaction()?;
        let n = channel_ids_in_priority_order.len() as i64;
        for (idx, channel_id) in channel_ids_in_priority_order.into_iter().enumerate() {
            let priority = n - (idx as i64);
            tx.execute(
                r#"
                UPDATE channels
                SET priority = ?2, updated_at_ms = ?3
                WHERE id = ?1
                "#,
                params![channel_id, priority, ts],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn delete_channel(db_path: PathBuf, channel_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"DELETE FROM route_channels WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        let deleted = tx.execute(r#"DELETE FROM channels WHERE id = ?1"#, params![channel_id])?;
        tx.commit()?;

        if deleted == 0 {
            return Err(anyhow::anyhow!("channel not found"));
        }
        Ok(())
    })
    .await
}

pub async fn list_routes(db_path: PathBuf) -> anyhow::Result<Vec<Route>> {
    with_conn(db_path, |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, match_model, enabled, created_at_ms, updated_at_ms
            FROM routes
            ORDER BY name ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let protocol: String = row.get(2)?;
            Ok(Route {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: protocol.parse::<Protocol>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        e.into_boxed_dyn_error(),
                    )
                })?,
                match_model: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
                created_at_ms: row.get(5)?,
                updated_at_ms: row.get(6)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRoute {
    pub name: String,
    pub protocol: Protocol,
    pub match_model: Option<String>,
    pub enabled: bool,
}

pub async fn create_route(db_path: PathBuf, input: CreateRoute) -> anyhow::Result<Route> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let id = Uuid::new_v4().to_string();
        conn.execute(
            r#"
            INSERT INTO routes (id, name, protocol, match_model, enabled, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                id,
                input.name,
                input.protocol.as_str(),
                input.match_model,
                if input.enabled { 1 } else { 0 },
                ts,
                ts,
            ],
        )?;

        Ok(Route {
            id,
            name: input.name,
            protocol: input.protocol,
            match_model: input.match_model,
            enabled: input.enabled,
            created_at_ms: ts,
            updated_at_ms: ts,
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRoute {
    pub name: Option<String>,
    pub match_model: Option<Option<String>>,
    pub enabled: Option<bool>,
}

pub async fn update_route(
    db_path: PathBuf,
    route_id: String,
    input: UpdateRoute,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();

        let mut route: Route = {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, name, protocol, match_model, enabled, created_at_ms, updated_at_ms
                FROM routes
                WHERE id = ?1
                "#,
            )?;
            let row = stmt.query_row([&route_id], |row| {
                let protocol: String = row.get(2)?;
                Ok(Route {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    protocol: protocol.parse::<Protocol>().map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            e.into_boxed_dyn_error(),
                        )
                    })?,
                    match_model: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    created_at_ms: row.get(5)?,
                    updated_at_ms: row.get(6)?,
                })
            });

            match row {
                Ok(v) => v,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(anyhow::anyhow!("route not found: {route_id}"));
                }
                Err(e) => return Err(e.into()),
            }
        };

        if let Some(v) = input.name {
            route.name = v;
        }
        if let Some(v) = input.match_model {
            route.match_model = v;
        }
        if let Some(v) = input.enabled {
            route.enabled = v;
        }
        route.updated_at_ms = ts;

        conn.execute(
            r#"
            UPDATE routes
            SET name = ?2, match_model = ?3, enabled = ?4, updated_at_ms = ?5
            WHERE id = ?1
            "#,
            params![
                route.id,
                route.name,
                route.match_model,
                if route.enabled { 1 } else { 0 },
                route.updated_at_ms
            ],
        )?;

        Ok(())
    })
    .await
}

pub async fn get_route(db_path: PathBuf, route_id: String) -> anyhow::Result<Option<Route>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, match_model, enabled, created_at_ms, updated_at_ms
            FROM routes
            WHERE id = ?1
            "#,
        )?;

        stmt.query_row([route_id], |row| {
            let protocol: String = row.get(2)?;
            Ok(Route {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: protocol.parse::<Protocol>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        e.into_boxed_dyn_error(),
                    )
                })?,
                match_model: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
                created_at_ms: row.get(5)?,
                updated_at_ms: row.get(6)?,
            })
        })
        .optional()
        .map_err(Into::into)
    })
    .await
}

pub async fn delete_route(db_path: PathBuf, route_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"DELETE FROM route_channels WHERE route_id = ?1"#,
            params![route_id],
        )?;
        let deleted = tx.execute(r#"DELETE FROM routes WHERE id = ?1"#, params![route_id])?;
        tx.commit()?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("route not found"));
        }
        Ok(())
    })
    .await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteChannel {
    pub route_id: String,
    pub channel_id: String,
    pub priority: i64,
    pub cooldown_until_ms: Option<i64>,
}

pub async fn list_route_channels(
    db_path: PathBuf,
    route_id: String,
) -> anyhow::Result<Vec<RouteChannel>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT route_id, channel_id, priority, cooldown_until_ms
            FROM route_channels
            WHERE route_id = ?1
            ORDER BY priority ASC
            "#,
        )?;
        let rows = stmt.query_map([route_id], |row| {
            Ok(RouteChannel {
                route_id: row.get(0)?,
                channel_id: row.get(1)?,
                priority: row.get(2)?,
                cooldown_until_ms: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

pub async fn set_route_channels(
    db_path: PathBuf,
    route_id: String,
    channel_ids_in_priority_order: Vec<String>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let route_protocol: String = conn
            .query_row(
                r#"SELECT protocol FROM routes WHERE id = ?1"#,
                [&route_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("route not found"))?;

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"DELETE FROM route_channels WHERE route_id = ?1"#,
            params![route_id],
        )?;

        for (idx, channel_id) in channel_ids_in_priority_order.into_iter().enumerate() {
            let channel_protocol: String = tx
                .query_row(
                    r#"SELECT protocol FROM channels WHERE id = ?1"#,
                    [&channel_id],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| anyhow::anyhow!("channel not found"))?;

            if channel_protocol != route_protocol {
                return Err(anyhow::anyhow!(
                    "channel protocol mismatch: route={route_protocol} channel={channel_protocol}"
                ));
            }

            tx.execute(
                r#"
                INSERT INTO route_channels (route_id, channel_id, priority, cooldown_until_ms)
                VALUES (?1, ?2, ?3, NULL)
                "#,
                params![route_id, channel_id, idx as i64],
            )?;
        }

        tx.commit()?;
        Ok(())
    })
    .await
}

#[derive(Debug, Clone)]
pub struct UpsertPricingModel {
    pub model_id: String,
    pub prompt_price: Option<String>,
    pub completion_price: Option<String>,
    pub request_price: Option<String>,
    pub cache_read_price: Option<String>,
    pub cache_write_price: Option<String>,
    pub raw_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingModel {
    pub model_id: String,
    pub prompt_price: Option<String>,
    pub completion_price: Option<String>,
    pub request_price: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PricingStatus {
    pub count: i64,
    pub last_sync_ms: Option<i64>,
}

pub async fn pricing_status(db_path: PathBuf) -> anyhow::Result<PricingStatus> {
    with_conn(db_path, |conn| {
        let count: i64 = conn.query_row(r#"SELECT COUNT(*) FROM pricing_models"#, [], |row| {
            row.get(0)
        })?;
        let last_sync_ms: Option<i64> = conn
            .query_row(
                r#"SELECT MAX(updated_at_ms) FROM pricing_models"#,
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(PricingStatus {
            count,
            last_sync_ms,
        })
    })
    .await
}

pub async fn upsert_pricing_models(
    db_path: PathBuf,
    models: Vec<UpsertPricingModel>,
    updated_at_ms: i64,
) -> anyhow::Result<usize> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO pricing_models (
                  model_id, prompt_price, completion_price, request_price,
                  cache_read_price, cache_write_price,
                  raw_json, updated_at_ms
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(model_id) DO UPDATE SET
                  prompt_price = excluded.prompt_price,
                  completion_price = excluded.completion_price,
                  request_price = excluded.request_price,
                  cache_read_price = excluded.cache_read_price,
                  cache_write_price = excluded.cache_write_price,
                  raw_json = excluded.raw_json,
                  updated_at_ms = excluded.updated_at_ms
                "#,
            )?;

            for m in &models {
                stmt.execute(params![
                    m.model_id,
                    m.prompt_price,
                    m.completion_price,
                    m.request_price,
                    m.cache_read_price,
                    m.cache_write_price,
                    m.raw_json,
                    updated_at_ms
                ])?;
            }
        }

        tx.commit()?;
        Ok(models.len())
    })
    .await
}

pub async fn search_pricing_models(
    db_path: PathBuf,
    query: Option<String>,
    limit: i64,
) -> anyhow::Result<Vec<PricingModel>> {
    with_conn(db_path, move |conn| {
        let mut out = Vec::new();
        if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
            let like = format!("%{}%", q.trim());
            let mut stmt = conn.prepare(
                r#"
                SELECT model_id, prompt_price, completion_price, request_price, updated_at_ms
                FROM pricing_models
                WHERE model_id LIKE ?1
                ORDER BY model_id ASC
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![like, limit], |row| {
                Ok(PricingModel {
                    model_id: row.get(0)?,
                    prompt_price: row.get(1)?,
                    completion_price: row.get(2)?,
                    request_price: row.get(3)?,
                    updated_at_ms: row.get(4)?,
                })
            })?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT model_id, prompt_price, completion_price, request_price, updated_at_ms
                FROM pricing_models
                ORDER BY model_id ASC
                LIMIT ?1
                "#,
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(PricingModel {
                    model_id: row.get(0)?,
                    prompt_price: row.get(1)?,
                    completion_price: row.get(2)?,
                    request_price: row.get(3)?,
                    updated_at_ms: row.get(4)?,
                })
            })?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    })
    .await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub id: String,
    pub request_id: Option<String>,
    pub ts_ms: i64,
    pub protocol: Protocol,
    pub route_id: Option<String>,
    pub channel_id: String,
    pub model: Option<String>,
    pub success: bool,
    pub http_status: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub estimated_cost_usd: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateUsageEvent {
    pub request_id: Option<Arc<str>>,
    pub ts_ms: i64,
    pub protocol: Protocol,
    pub route_id: Option<String>,
    pub channel_id: String,
    pub model: Option<String>,
    pub success: bool,
    pub http_status: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub estimated_cost_usd: Option<String>,
}

pub async fn insert_usage_event(db_path: PathBuf, input: CreateUsageEvent) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let id = Uuid::new_v4().to_string();
        let CreateUsageEvent {
            request_id,
            ts_ms,
            protocol,
            route_id,
            channel_id,
            model,
            success,
            http_status,
            error_kind,
            error_detail,
            latency_ms,
            ttft_ms,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_read_tokens,
            cache_write_tokens,
            estimated_cost_usd,
        } = input;

        let estimated_cost_usd = estimated_cost_usd.or_else(|| {
            model.as_deref().and_then(|m| {
                estimate_cost_usd(
                    conn,
                    m,
                    success,
                    prompt_tokens,
                    completion_tokens,
                    cache_read_tokens,
                    cache_write_tokens,
                )
            })
        });

        conn.execute(
            r#"
            INSERT INTO usage_events (
              id, request_id, ts_ms, protocol, route_id, channel_id, model,
              success, http_status, error_kind, error_detail, latency_ms,
              ttft_ms, prompt_tokens, completion_tokens, total_tokens,
              cache_read_tokens, cache_write_tokens,
              estimated_cost_usd
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            "#,
            params![
                id,
                request_id.as_deref(),
                ts_ms,
                protocol.as_str(),
                route_id,
                channel_id,
                model,
                if success { 1 } else { 0 },
                http_status,
                error_kind,
                error_detail,
                latency_ms,
                ttft_ms,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                cache_read_tokens,
                cache_write_tokens,
                estimated_cost_usd,
            ],
        )?;
        Ok(())
    })
    .await
}

fn parse_price_usd(s: &str) -> Option<f64> {
    let v = s.trim().parse::<f64>().ok()?;
    if v.is_finite() && v > 0.0 {
        Some(v)
    } else {
        None
    }
}

fn format_cost_usd(v: f64) -> Option<String> {
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    let mut s = format!("{v:.12}");
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

type PricingModelPrices = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn find_pricing_for_model(
    conn: &Connection,
    model: &str,
) -> rusqlite::Result<Option<PricingModelPrices>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT prompt_price, completion_price, request_price, cache_read_price, cache_write_price
        FROM pricing_models
        WHERE model_id = ?1
        "#,
    )?;
    if let Some(row) = stmt
        .query_row(params![model], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .optional()?
    {
        return Ok(Some(row));
    }

    let like = format!("%/{}", model.trim());
    let mut stmt = conn.prepare(
        r#"
        SELECT prompt_price, completion_price, request_price, cache_read_price, cache_write_price
        FROM pricing_models
        WHERE model_id LIKE ?1
        ORDER BY LENGTH(model_id) ASC
        LIMIT 1
        "#,
    )?;
    stmt.query_row(params![like], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        ))
    })
    .optional()
}

fn estimate_cost_usd(
    conn: &Connection,
    model: &str,
    success: bool,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
) -> Option<String> {
    let Ok(Some((
        prompt_price,
        completion_price,
        request_price,
        cache_read_price,
        cache_write_price,
    ))) = find_pricing_for_model(conn, model)
    else {
        return None;
    };

    let prompt_unit = prompt_price
        .as_deref()
        .and_then(parse_price_usd)
        .unwrap_or(0.0);
    let completion_unit = completion_price
        .as_deref()
        .and_then(parse_price_usd)
        .unwrap_or(0.0);
    let cache_read_unit = cache_read_price
        .as_deref()
        .and_then(parse_price_usd)
        .unwrap_or(0.0);
    let cache_write_unit = cache_write_price
        .as_deref()
        .and_then(parse_price_usd)
        .unwrap_or(0.0);
    let request_unit = if success {
        request_price
            .as_deref()
            .and_then(parse_price_usd)
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let c = completion_tokens.unwrap_or(0).max(0) as f64;
    let cr = cache_read_tokens.unwrap_or(0).max(0);
    let cw = cache_write_tokens.unwrap_or(0).max(0);

    let mut regular_prompt_tokens = prompt_tokens.unwrap_or(0).max(0);
    if cr <= regular_prompt_tokens {
        regular_prompt_tokens -= cr;
    }
    if cw <= regular_prompt_tokens {
        regular_prompt_tokens -= cw;
    }
    let p = regular_prompt_tokens as f64;
    let cr = cr as f64;
    let cw = cw as f64;

    let cost = p * prompt_unit
        + c * completion_unit
        + cr * cache_read_unit
        + cw * cache_write_unit
        + request_unit;
    format_cost_usd(cost)
}

pub async fn backfill_usage_event_costs(db_path: PathBuf) -> anyhow::Result<i64> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, model, success, prompt_tokens, completion_tokens, cache_read_tokens, cache_write_tokens
            FROM usage_events
            WHERE estimated_cost_usd IS NULL
              AND model IS NOT NULL
              AND (prompt_tokens IS NOT NULL OR completion_tokens IS NOT NULL OR success = 1)
            ORDER BY ts_ms DESC
            LIMIT 20000
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })?;

        let mut updated = 0i64;
        for row in rows {
            let (id, model, success, prompt_tokens, completion_tokens, cache_read_tokens, cache_write_tokens) =
                row?;
            let Some(cost) = estimate_cost_usd(
                conn,
                &model,
                success,
                prompt_tokens,
                completion_tokens,
                cache_read_tokens,
                cache_write_tokens,
            ) else {
                continue;
            };
            let n = conn.execute(
                "UPDATE usage_events SET estimated_cost_usd = ?1 WHERE id = ?2 AND estimated_cost_usd IS NULL",
                params![cost, id],
            )?;
            updated += n as i64;
        }

        Ok(updated)
    })
    .await
}

pub async fn list_usage_events_recent(
    db_path: PathBuf,
    limit: i64,
) -> anyhow::Result<Vec<UsageEvent>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, request_id, ts_ms, protocol, route_id, channel_id, model,
                   success, http_status, error_kind, error_detail, latency_ms,
                   ttft_ms, prompt_tokens, completion_tokens, total_tokens,
                   cache_read_tokens, cache_write_tokens,
                   estimated_cost_usd
            FROM usage_events
            ORDER BY ts_ms DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            let protocol: String = row.get(3)?;
            Ok(UsageEvent {
                id: row.get(0)?,
                request_id: row.get(1)?,
                ts_ms: row.get(2)?,
                protocol: protocol.parse::<Protocol>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        e.into_boxed_dyn_error(),
                    )
                })?,
                route_id: row.get(4)?,
                channel_id: row.get(5)?,
                model: row.get(6)?,
                success: row.get::<_, i64>(7)? != 0,
                http_status: row.get(8)?,
                error_kind: row.get(9)?,
                error_detail: row.get(10)?,
                latency_ms: row.get(11)?,
                ttft_ms: row.get(12)?,
                prompt_tokens: row.get(13)?,
                completion_tokens: row.get(14)?,
                total_tokens: row.get(15)?,
                cache_read_tokens: row.get(16)?,
                cache_write_tokens: row.get(17)?,
                estimated_cost_usd: row.get(18)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

#[derive(Debug, Clone, Default)]
pub struct UsageListQuery {
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub protocol: Option<Protocol>,
    pub channel_id: Option<String>,
    pub model: Option<String>,
    pub request_id: Option<String>,
    pub success: Option<bool>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageListResult {
    pub total: i64,
    pub items: Vec<UsageEvent>,
}

pub async fn list_usage_events(
    db_path: PathBuf,
    q: UsageListQuery,
) -> anyhow::Result<UsageListResult> {
    with_conn(db_path, move |conn| {
        let mut where_sql = Vec::<String>::new();
        let mut params = Vec::<rusqlite::types::Value>::new();

        if let Some(start_ms) = q.start_ms {
            where_sql.push("ts_ms >= ?".to_string());
            params.push(start_ms.into());
        }
        if let Some(end_ms) = q.end_ms {
            where_sql.push("ts_ms <= ?".to_string());
            params.push(end_ms.into());
        }
        if let Some(protocol) = q.protocol {
            where_sql.push("protocol = ?".to_string());
            params.push(protocol.as_str().to_string().into());
        }
        if let Some(channel_id) = q.channel_id.filter(|s| !s.trim().is_empty()) {
            where_sql.push("channel_id = ?".to_string());
            params.push(channel_id.into());
        }
        if let Some(model) = q.model.filter(|s| !s.trim().is_empty()) {
            where_sql.push("model LIKE ?".to_string());
            params.push(format!("%{}%", model.trim()).into());
        }
        if let Some(request_id) = q.request_id.filter(|s| !s.trim().is_empty()) {
            where_sql.push("COALESCE(request_id, id) LIKE ?".to_string());
            params.push(format!("%{}%", request_id.trim()).into());
        }
        if let Some(success) = q.success {
            where_sql.push("success = ?".to_string());
            params.push((if success { 1i64 } else { 0i64 }).into());
        }

        let where_clause = if where_sql.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", where_sql.join(" AND "))
        };

        let total: i64 = {
            let sql = format!("SELECT COUNT(*) FROM usage_events {where_clause}");
            let mut stmt = conn.prepare(&sql)?;
            stmt.query_row(rusqlite::params_from_iter(params.iter()), |row| row.get(0))?
        };

        let mut params_items = params;
        params_items.push(q.limit.into());
        params_items.push(q.offset.into());

        let sql = format!(
            r#"
            SELECT id, request_id, ts_ms, protocol, route_id, channel_id, model,
                   success, http_status, error_kind, error_detail, latency_ms,
                   ttft_ms, prompt_tokens, completion_tokens, total_tokens,
                   cache_read_tokens, cache_write_tokens,
                   estimated_cost_usd
            FROM usage_events
            {where_clause}
            ORDER BY ts_ms DESC
            LIMIT ? OFFSET ?
            "#
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_items.iter()), |row| {
            let protocol: String = row.get(3)?;
            Ok(UsageEvent {
                id: row.get(0)?,
                request_id: row.get(1)?,
                ts_ms: row.get(2)?,
                protocol: protocol.parse::<Protocol>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        e.into_boxed_dyn_error(),
                    )
                })?,
                route_id: row.get(4)?,
                channel_id: row.get(5)?,
                model: row.get(6)?,
                success: row.get::<_, i64>(7)? != 0,
                http_status: row.get(8)?,
                error_kind: row.get(9)?,
                error_detail: row.get(10)?,
                latency_ms: row.get(11)?,
                ttft_ms: row.get(12)?,
                prompt_tokens: row.get(13)?,
                completion_tokens: row.get(14)?,
                total_tokens: row.get(15)?,
                cache_read_tokens: row.get(16)?,
                cache_write_tokens: row.get(17)?,
                estimated_cost_usd: row.get(18)?,
            })
        })?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }

        Ok(UsageListResult { total, items })
    })
    .await
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsSummary {
    pub start_ms: i64,
    pub requests: i64,
    pub success: i64,
    pub failed: i64,
    pub avg_latency_ms: Option<f64>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_usd: Option<String>,
}

pub async fn stats_summary(db_path: PathBuf, start_ms: i64) -> anyhow::Result<StatsSummary> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            WITH per_req AS (
              SELECT
                COALESCE(request_id, id) AS rid,
                MAX(success) AS any_success
              FROM usage_events
              WHERE ts_ms >= ?1
              GROUP BY rid
            ),
            req_agg AS (
              SELECT
                COUNT(*) AS requests,
                SUM(any_success) AS success
              FROM per_req
            )
            SELECT
              (SELECT requests FROM req_agg) AS requests,
              (SELECT success FROM req_agg) AS success,
              (SELECT requests - success FROM req_agg) AS failed,
              AVG(CASE WHEN latency_ms > 0 THEN latency_ms ELSE NULL END) AS avg_latency_ms,
              SUM(COALESCE(prompt_tokens, 0)) AS prompt_tokens,
              SUM(COALESCE(completion_tokens, 0)) AS completion_tokens,
              SUM(COALESCE(total_tokens, 0)) AS total_tokens,
              SUM(COALESCE(CAST(estimated_cost_usd AS REAL), 0.0)) AS estimated_cost
            FROM usage_events
            WHERE ts_ms >= ?1
            "#,
        )?;
        let row = stmt.query_row(params![start_ms], |row| {
            let requests: i64 = row.get(0)?;
            let success: Option<i64> = row.get(1)?;
            let failed: Option<i64> = row.get(2)?;
            let avg_latency_ms: Option<f64> = row.get(3)?;
            let prompt_tokens: Option<i64> = row.get(4)?;
            let completion_tokens: Option<i64> = row.get(5)?;
            let total_tokens: Option<i64> = row.get(6)?;
            let estimated_cost: Option<f64> = row.get(7)?;

            Ok(StatsSummary {
                start_ms,
                requests,
                success: success.unwrap_or(0),
                failed: failed.unwrap_or(0),
                avg_latency_ms,
                prompt_tokens: prompt_tokens.unwrap_or(0),
                completion_tokens: completion_tokens.unwrap_or(0),
                total_tokens: total_tokens.unwrap_or(0),
                estimated_cost_usd: estimated_cost
                    .filter(|v| *v > 0.0)
                    .map(|v| format!("{v:.6}")),
            })
        })?;
        Ok(row)
    })
    .await
}

#[derive(Debug, Clone, Serialize)]
pub struct TrendPoint {
    pub bucket_start_ms: i64,
    pub channel_id: String,
    pub name: String,
    pub success: i64,
}

pub async fn stats_trend_by_day_channel(
    db_path: PathBuf,
    start_ms: i64,
    offset_ms: i64,
) -> anyhow::Result<Vec<TrendPoint>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT
              ((u.ts_ms + ?2) / 86400000) AS bucket_day,
              c.id,
              c.name,
              SUM(CASE WHEN u.success = 1 THEN 1 ELSE 0 END) AS success
            FROM usage_events u
            JOIN channels c ON c.id = u.channel_id
            WHERE u.ts_ms >= ?1
            GROUP BY bucket_day, c.id, c.name
            HAVING SUM(CASE WHEN u.success = 1 THEN 1 ELSE 0 END) > 0
            ORDER BY bucket_day ASC, c.name ASC
            "#,
        )?;

        let rows = stmt.query_map(params![start_ms, offset_ms], |row| {
            let bucket_day: i64 = row.get(0)?;
            let bucket_start_ms = bucket_day
                .saturating_mul(86_400_000)
                .saturating_sub(offset_ms);
            Ok(TrendPoint {
                bucket_start_ms,
                channel_id: row.get(1)?,
                name: row.get(2)?,
                success: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelStats {
    pub channel_id: String,
    pub name: String,
    pub protocol: Protocol,
    pub requests: i64,
    pub success: i64,
    pub failed: i64,
    pub avg_latency_ms: Option<f64>,
    pub total_tokens: i64,
    pub estimated_cost_usd: Option<String>,
}

pub async fn stats_channels(db_path: PathBuf, start_ms: i64) -> anyhow::Result<Vec<ChannelStats>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT
              c.id,
              c.name,
              c.protocol,
              COUNT(u.id) AS requests,
              SUM(CASE WHEN u.success = 1 THEN 1 ELSE 0 END) AS success,
              SUM(CASE WHEN u.success = 0 THEN 1 ELSE 0 END) AS failed,
              AVG(u.latency_ms) AS avg_latency_ms,
              SUM(COALESCE(u.total_tokens, 0)) AS total_tokens,
              SUM(COALESCE(CAST(u.estimated_cost_usd AS REAL), 0.0)) AS estimated_cost
            FROM channels c
            LEFT JOIN usage_events u
              ON u.channel_id = c.id
             AND u.ts_ms >= ?1
            GROUP BY c.id, c.name, c.protocol
            ORDER BY c.name ASC
            "#,
        )?;
        let rows = stmt.query_map(params![start_ms], |row| {
            let protocol: String = row.get(2)?;
            Ok(ChannelStats {
                channel_id: row.get(0)?,
                name: row.get(1)?,
                protocol: protocol.parse::<Protocol>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        e.into_boxed_dyn_error(),
                    )
                })?,
                requests: row.get(3)?,
                success: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                failed: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                avg_latency_ms: row.get(6)?,
                total_tokens: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                estimated_cost_usd: row
                    .get::<_, Option<f64>>(8)?
                    .filter(|v| *v > 0.0)
                    .map(|v| format!("{v:.6}")),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}
