use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::protocol::normalize_base_url;
use super::{Protocol, with_conn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEndpoint {
    pub id: String,
    pub channel_id: String,
    pub base_url: String,
    pub priority: i64,
    pub enabled: bool,
    pub auto_disabled_until_ms: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub async fn list_channel_endpoints(
    db_path: PathBuf,
    channel_id: String,
) -> anyhow::Result<Vec<ChannelEndpoint>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, channel_id, base_url, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
            FROM channel_endpoints
            WHERE channel_id = ?1
            ORDER BY priority DESC, created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([channel_id], |row| {
            Ok(ChannelEndpoint {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                base_url: row.get(2)?,
                priority: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                created_at_ms: row.get(6)?,
                updated_at_ms: row.get(7)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub(crate) fn replace_channel_endpoints_tx(
    tx: &rusqlite::Transaction<'_>,
    channel_id: &str,
    protocol: Protocol,
    base_urls_in_priority_order: Vec<String>,
    ts: i64,
) -> anyhow::Result<Vec<ChannelEndpoint>> {
    let mut old_ids = Vec::<String>::new();
    {
        let mut stmt = tx.prepare(r#"SELECT id FROM channel_endpoints WHERE channel_id = ?1"#)?;
        let mut rows = stmt.query([channel_id])?;
        while let Some(row) = rows.next()? {
            old_ids.push(row.get::<_, String>(0)?);
        }
    }

    for id in &old_ids {
        tx.execute(
            r#"DELETE FROM endpoint_key_failures WHERE endpoint_id = ?1"#,
            params![id],
        )?;
        tx.execute(
            r#"DELETE FROM endpoint_key_states WHERE endpoint_id = ?1"#,
            params![id],
        )?;
        tx.execute(
            r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1"#,
            params![id],
        )?;
    }
    tx.execute(
        r#"DELETE FROM channel_endpoints WHERE channel_id = ?1"#,
        params![channel_id],
    )?;

    let mut endpoints = Vec::<ChannelEndpoint>::new();
    let n = base_urls_in_priority_order.len() as i64;
    for (idx, raw) in base_urls_in_priority_order.into_iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        let base_url = normalize_base_url(protocol, &raw);
        let priority = n - (idx as i64);
        tx.execute(
            r#"
            INSERT INTO channel_endpoints (id, channel_id, base_url, priority, enabled, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)
            "#,
            params![id, channel_id, base_url, priority, ts],
        )?;
        endpoints.push(ChannelEndpoint {
            id,
            channel_id: channel_id.to_string(),
            base_url,
            priority,
            enabled: true,
            auto_disabled_until_ms: 0,
            created_at_ms: ts,
            updated_at_ms: ts,
        });
    }

    Ok(endpoints)
}

pub(crate) fn get_primary_endpoint_base_url_tx(
    tx: &rusqlite::Transaction<'_>,
    channel_id: &str,
) -> anyhow::Result<Option<String>> {
    tx.query_row(
        r#"
        SELECT base_url
        FROM channel_endpoints
        WHERE channel_id = ?1 AND enabled = 1
        ORDER BY priority DESC, created_at_ms ASC
        LIMIT 1
        "#,
        params![channel_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub async fn record_endpoint_failure_and_maybe_disable(
    db_path: PathBuf,
    endpoint_id: String,
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
            r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1 AND at_ms < ?2"#,
            params![endpoint_id, cutoff_ms],
        )?;
        tx.execute(
            r#"INSERT INTO endpoint_failures (endpoint_id, at_ms) VALUES (?1, ?2)"#,
            params![endpoint_id, now_ms],
        )?;

        let cnt: i64 = tx.query_row(
            r#"SELECT COUNT(*) FROM endpoint_failures WHERE endpoint_id = ?1 AND at_ms >= ?2"#,
            params![endpoint_id, cutoff_ms],
            |row| row.get(0),
        )?;

        if cnt < failure_times {
            tx.commit()?;
            return Ok(None);
        }

        let disabled_until_ms = now_ms.saturating_add(disable_ms);
        tx.execute(
            r#"
            UPDATE channel_endpoints
            SET auto_disabled_until_ms = ?2, updated_at_ms = ?3
            WHERE id = ?1
            "#,
            params![endpoint_id, disabled_until_ms, now_ms],
        )?;
        tx.execute(
            r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1"#,
            params![endpoint_id],
        )?;
        tx.commit()?;
        Ok(Some(disabled_until_ms))
    })
    .await
}

pub async fn clear_endpoint_failures(db_path: PathBuf, endpoint_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1"#,
            params![endpoint_id],
        )?;
        Ok(())
    })
    .await
}
