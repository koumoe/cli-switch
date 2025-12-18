use anyhow::Context as _;
use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

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
    ensure_usage_events_schema(&conn)?;

    Ok(())
}

fn ensure_channels_schema(conn: &Connection) -> anyhow::Result<()> {
    ensure_column(conn, "channels", "priority", "INTEGER NOT NULL DEFAULT 0")?;
    Ok(())
}

fn ensure_usage_events_schema(conn: &Connection) -> anyhow::Result<()> {
    ensure_column(conn, "usage_events", "ttft_ms", "INTEGER NULL")?;
    ensure_column(conn, "usage_events", "request_id", "TEXT NULL")?;
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
            SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, enabled, created_at_ms, updated_at_ms
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
                created_at_ms: row.get(8)?,
                updated_at_ms: row.get(9)?,
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

        let mut channel: Channel = {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, enabled, created_at_ms, updated_at_ms
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
                    created_at_ms: row.get(8)?,
                    updated_at_ms: row.get(9)?,
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
        }
        channel.updated_at_ms = ts;

        conn.execute(
            r#"
            UPDATE channels
            SET name = ?2, base_url = ?3, auth_type = ?4, auth_ref = ?5, priority = ?6, enabled = ?7, updated_at_ms = ?8
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
                channel.updated_at_ms,
            ],
        )?;

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
        let updated = conn.execute(
            r#"
            UPDATE channels
            SET enabled = ?2, updated_at_ms = ?3
            WHERE id = ?1
            "#,
            params![channel_id, if enabled { 1 } else { 0 }, ts],
        )?;

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
            SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, enabled, created_at_ms, updated_at_ms
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
                created_at_ms: row.get(8)?,
                updated_at_ms: row.get(9)?,
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
                INSERT INTO pricing_models (model_id, prompt_price, completion_price, request_price, raw_json, updated_at_ms)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(model_id) DO UPDATE SET
                  prompt_price = excluded.prompt_price,
                  completion_price = excluded.completion_price,
                  request_price = excluded.request_price,
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
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
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
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
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
            latency_ms,
            ttft_ms,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            estimated_cost_usd,
        } = input;
        conn.execute(
            r#"
            INSERT INTO usage_events (
              id, request_id, ts_ms, protocol, route_id, channel_id, model,
              success, http_status, error_kind, latency_ms,
              ttft_ms, prompt_tokens, completion_tokens, total_tokens, estimated_cost_usd
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
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
                latency_ms,
                ttft_ms,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                estimated_cost_usd,
            ],
        )?;
        Ok(())
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
                   success, http_status, error_kind, latency_ms,
                   ttft_ms, prompt_tokens, completion_tokens, total_tokens, estimated_cost_usd
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
                latency_ms: row.get(10)?,
                ttft_ms: row.get(11)?,
                prompt_tokens: row.get(12)?,
                completion_tokens: row.get(13)?,
                total_tokens: row.get(14)?,
                estimated_cost_usd: row.get(15)?,
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
pub struct StatsSummary {
    pub start_ms: i64,
    pub requests: i64,
    pub success: i64,
    pub failed: i64,
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
            let prompt_tokens: Option<i64> = row.get(3)?;
            let completion_tokens: Option<i64> = row.get(4)?;
            let total_tokens: Option<i64> = row.get(5)?;
            let estimated_cost: Option<f64> = row.get(6)?;

            Ok(StatsSummary {
                start_ms,
                requests,
                success: success.unwrap_or(0),
                failed: failed.unwrap_or(0),
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
