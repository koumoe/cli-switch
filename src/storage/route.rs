use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::{Protocol, now_ms, with_conn};

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
            Ok(Route {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: row.get(2)?,
                match_model: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
                created_at_ms: row.get(5)?,
                updated_at_ms: row.get(6)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
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
                Ok(Route {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    protocol: row.get(2)?,
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
            Ok(Route {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: row.get(2)?,
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

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
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
