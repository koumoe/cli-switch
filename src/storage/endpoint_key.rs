use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{Protocol, with_conn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamAttempt {
    pub channel_id: String,
    pub endpoint_id: String,
    pub key_id: String,
    pub base_url: String,
    pub auth_ref: String,
}

pub async fn list_available_upstream_attempts(
    db_path: PathBuf,
    protocol: Protocol,
    now_ms: i64,
    auto_disable_enabled: bool,
) -> anyhow::Result<(i64, Vec<UpstreamAttempt>)> {
    with_conn(db_path, move |conn| {
        let enabled_channels: i64 = conn.query_row(
            r#"SELECT COUNT(*) FROM channels WHERE protocol = ?1 AND enabled = 1"#,
            params![protocol.as_str()],
            |row| row.get(0),
        )?;

        let attempts = if auto_disable_enabled {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                  c.id,
                  e.id,
                  k.id,
                  e.base_url,
                  k.auth_ref
                FROM channels c
                JOIN channel_endpoints e ON e.channel_id = c.id
                JOIN channel_keys k ON k.channel_id = c.id
                LEFT JOIN endpoint_key_states s
                  ON s.endpoint_id = e.id
                 AND s.key_id = k.id
                WHERE c.protocol = ?1
                  AND c.enabled = 1
                  AND e.enabled = 1
                  AND k.enabled = 1
                  AND COALESCE(e.auto_disabled_until_ms, 0) <= ?2
                  AND COALESCE(k.auto_disabled_until_ms, 0) <= ?2
                  AND COALESCE(s.auto_disabled_until_ms, 0) <= ?2
                ORDER BY
                  c.priority DESC,
                  c.name ASC,
                  e.priority DESC,
                  e.base_url ASC,
                  k.priority DESC,
                  k.id ASC
                "#,
            )?;

            let rows = stmt.query_map(params![protocol.as_str(), now_ms], |row| {
                Ok(UpstreamAttempt {
                    channel_id: row.get(0)?,
                    endpoint_id: row.get(1)?,
                    key_id: row.get(2)?,
                    base_url: row.get(3)?,
                    auth_ref: row.get(4)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(anyhow::Error::from)?
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT
                  c.id,
                  e.id,
                  k.id,
                  e.base_url,
                  k.auth_ref
                FROM channels c
                JOIN channel_endpoints e ON e.channel_id = c.id
                JOIN channel_keys k ON k.channel_id = c.id
                WHERE c.protocol = ?1
                  AND c.enabled = 1
                  AND e.enabled = 1
                  AND k.enabled = 1
                ORDER BY
                  c.priority DESC,
                  c.name ASC,
                  e.priority DESC,
                  e.base_url ASC,
                  k.priority DESC,
                  k.id ASC
                "#,
            )?;

            let rows = stmt.query_map(params![protocol.as_str()], |row| {
                Ok(UpstreamAttempt {
                    channel_id: row.get(0)?,
                    endpoint_id: row.get(1)?,
                    key_id: row.get(2)?,
                    base_url: row.get(3)?,
                    auth_ref: row.get(4)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(anyhow::Error::from)?
        };

        Ok((enabled_channels, attempts))
    })
    .await
}

pub async fn record_endpoint_key_failure_and_maybe_disable(
    db_path: PathBuf,
    endpoint_id: String,
    key_id: String,
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
            r#"
            DELETE FROM endpoint_key_failures
            WHERE endpoint_id = ?1 AND key_id = ?2 AND at_ms < ?3
            "#,
            params![endpoint_id, key_id, cutoff_ms],
        )?;
        tx.execute(
            r#"
            INSERT INTO endpoint_key_failures (endpoint_id, key_id, at_ms)
            VALUES (?1, ?2, ?3)
            "#,
            params![endpoint_id, key_id, now_ms],
        )?;

        let cnt: i64 = tx.query_row(
            r#"
            SELECT COUNT(*)
            FROM endpoint_key_failures
            WHERE endpoint_id = ?1 AND key_id = ?2 AND at_ms >= ?3
            "#,
            params![endpoint_id, key_id, cutoff_ms],
            |row| row.get(0),
        )?;

        if cnt < failure_times {
            tx.commit()?;
            return Ok(None);
        }

        let disabled_until_ms = now_ms.saturating_add(disable_ms);
        tx.execute(
            r#"
            INSERT INTO endpoint_key_states (endpoint_id, key_id, auto_disabled_until_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(endpoint_id, key_id) DO UPDATE SET
              auto_disabled_until_ms = excluded.auto_disabled_until_ms,
              updated_at_ms = excluded.updated_at_ms
            "#,
            params![endpoint_id, key_id, disabled_until_ms, now_ms],
        )?;
        tx.execute(
            r#"
            DELETE FROM endpoint_key_failures
            WHERE endpoint_id = ?1 AND key_id = ?2
            "#,
            params![endpoint_id, key_id],
        )?;

        tx.commit()?;
        Ok(Some(disabled_until_ms))
    })
    .await
}

pub async fn clear_endpoint_key_failures(
    db_path: PathBuf,
    endpoint_id: String,
    key_id: String,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"
            DELETE FROM endpoint_key_failures
            WHERE endpoint_id = ?1 AND key_id = ?2
            "#,
            params![endpoint_id, key_id],
        )?;
        Ok(())
    })
    .await
}
