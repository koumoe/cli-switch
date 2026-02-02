use rusqlite::{OptionalExtension as _, params};
use std::path::PathBuf;

use super::{now_ms, with_conn};

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

pub async fn ignore_update_version(db_path: PathBuf, version: &str) -> anyhow::Result<()> {
    let version = normalize_version(version);
    if version.is_empty() {
        anyhow::bail!("version is empty");
    }

    with_conn(db_path, move |conn| {
        let now = now_ms();
        conn.execute(
            r#"
            INSERT INTO ignored_updates (version, ignored_at_ms)
            VALUES (?1, ?2)
            ON CONFLICT(version) DO UPDATE SET ignored_at_ms = excluded.ignored_at_ms
            "#,
            params![version, now],
        )?;
        Ok(())
    })
    .await
}

pub async fn is_update_version_ignored(db_path: PathBuf, version: &str) -> anyhow::Result<bool> {
    let version = normalize_version(version);
    if version.is_empty() {
        return Ok(false);
    }

    with_conn(db_path, move |conn| {
        let exists: Option<i64> = conn
            .query_row(
                r#"SELECT 1 FROM ignored_updates WHERE version = ?1 LIMIT 1"#,
                params![version],
                |row| row.get(0),
            )
            .optional()?;
        Ok(exists.is_some())
    })
    .await
}
