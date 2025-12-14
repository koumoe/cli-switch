use anyhow::Context as _;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub auth_type: String,
    pub auth_ref: String,
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

    Ok(())
}

fn now_ms() -> i64 {
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
            SELECT id, name, protocol, base_url, auth_type, auth_ref, enabled, created_at_ms, updated_at_ms
            FROM channels
            ORDER BY name ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let protocol: String = row.get(2)?;
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: protocol.parse::<Protocol>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        e.into_boxed_dyn_error(),
                    )
                })?,
                base_url: row.get(3)?,
                auth_type: row.get(4)?,
                auth_ref: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
                created_at_ms: row.get(7)?,
                updated_at_ms: row.get(8)?,
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
    pub auth_type: String,
    pub auth_ref: String,
    pub enabled: bool,
}

pub async fn create_channel(db_path: PathBuf, input: CreateChannel) -> anyhow::Result<Channel> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let id = Uuid::new_v4().to_string();
        conn.execute(
            r#"
            INSERT INTO channels (id, name, protocol, base_url, auth_type, auth_ref, enabled, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                id,
                input.name,
                input.protocol.as_str(),
                input.base_url,
                input.auth_type,
                input.auth_ref,
                if input.enabled { 1 } else { 0 },
                ts,
                ts,
            ],
        )?;

        Ok(Channel {
            id,
            name: input.name,
            protocol: input.protocol,
            base_url: input.base_url,
            auth_type: input.auth_type,
            auth_ref: input.auth_ref,
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
                SELECT id, name, protocol, base_url, auth_type, auth_ref, enabled, created_at_ms, updated_at_ms
                FROM channels
                WHERE id = ?1
                "#,
            )?;
            let row = stmt.query_row([&channel_id], |row| {
                let protocol: String = row.get(2)?;
                Ok(Channel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    protocol: protocol.parse::<Protocol>().map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            e.into_boxed_dyn_error(),
                        )
                    })?,
                    base_url: row.get(3)?,
                    auth_type: row.get(4)?,
                    auth_ref: row.get(5)?,
                    enabled: row.get::<_, i64>(6)? != 0,
                    created_at_ms: row.get(7)?,
                    updated_at_ms: row.get(8)?,
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
            channel.base_url = v;
        }
        if let Some(v) = input.auth_type {
            channel.auth_type = v;
        }
        if let Some(v) = input.auth_ref {
            channel.auth_ref = v;
        }
        if let Some(v) = input.enabled {
            channel.enabled = v;
        }
        channel.updated_at_ms = ts;

        conn.execute(
            r#"
            UPDATE channels
            SET name = ?2, base_url = ?3, auth_type = ?4, auth_ref = ?5, enabled = ?6, updated_at_ms = ?7
            WHERE id = ?1
            "#,
            params![
                channel.id,
                channel.name,
                channel.base_url,
                channel.auth_type,
                channel.auth_ref,
                if channel.enabled { 1 } else { 0 },
                channel.updated_at_ms
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
