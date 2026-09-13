use rusqlite::params;
use serde::Serialize;
use std::path::PathBuf;

use super::{now_ms, with_conn};

#[derive(Debug, Clone, Serialize)]
pub struct EndpointFailure {
    pub endpoint: String,
    pub transport: String,
    pub reason: String,
    pub count: i64,
    pub first_seen_at_ms: i64,
    pub last_seen_at_ms: i64,
}

pub async fn record_endpoint_failure(
    db_path: PathBuf,
    endpoint: String,
    transport: String,
    reason: String,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let now = now_ms();
        conn.execute(
            r#"
            INSERT INTO endpoint_failures
              (endpoint, transport, reason, count, first_seen_at_ms, last_seen_at_ms)
            VALUES (?1, ?2, ?3, 1, ?4, ?4)
            ON CONFLICT(endpoint, transport, reason) DO UPDATE SET
              count = endpoint_failures.count + 1,
              last_seen_at_ms = excluded.last_seen_at_ms
            "#,
            params![endpoint, transport, reason, now],
        )?;
        Ok(())
    })
    .await
}

pub async fn list_endpoint_failures(
    db_path: PathBuf,
    limit: usize,
) -> anyhow::Result<Vec<EndpointFailure>> {
    with_conn(db_path, move |conn| {
        let limit = i64::try_from(limit.clamp(1, 100)).unwrap_or(100);
        let mut stmt = conn.prepare(
            "SELECT endpoint, transport, reason, count, first_seen_at_ms, last_seen_at_ms
             FROM endpoint_failures ORDER BY count DESC, last_seen_at_ms DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(EndpointFailure {
                endpoint: row.get(0)?,
                transport: row.get(1)?,
                reason: row.get(2)?,
                count: row.get(3)?,
                first_seen_at_ms: row.get(4)?,
                last_seen_at_ms: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn clear_endpoint_failures(db_path: PathBuf) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute("DELETE FROM endpoint_failures", [])?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_and_clears_endpoint_failures() {
        let dir = std::env::temp_dir().join(format!(
            "cliswitch-endpoint-failures-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("test.sqlite");
        crate::storage::init_db(&db).unwrap();

        record_endpoint_failure(
            db.clone(),
            "ws://*/v1/responses".into(),
            "ws".into(),
            "local_endpoint_not_supported".into(),
        )
        .await
        .unwrap();
        record_endpoint_failure(
            db.clone(),
            "ws://*/v1/responses".into(),
            "ws".into(),
            "local_endpoint_not_supported".into(),
        )
        .await
        .unwrap();
        let rows = list_endpoint_failures(db.clone(), 5).await.unwrap();
        assert_eq!(rows[0].count, 2);
        assert_eq!(rows[0].endpoint, "ws://*/v1/responses");

        clear_endpoint_failures(db.clone()).await.unwrap();
        assert!(list_endpoint_failures(db, 5).await.unwrap().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
