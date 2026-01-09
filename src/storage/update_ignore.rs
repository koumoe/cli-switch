use anyhow::Context as _;
use rusqlite::{Connection, OptionalExtension as _, params};
use std::path::PathBuf;

use super::{now_ms, with_conn};

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}

pub(super) fn ensure_ignored_updates_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS ignored_updates (
          version TEXT PRIMARY KEY,
          ignored_at_ms INTEGER NOT NULL
        )
        "#,
        [],
    )
    .with_context(|| "创建表 ignored_updates 失败")?;
    Ok(())
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

pub async fn upsert_ignored_update_versions(
    db_path: PathBuf,
    versions: Vec<String>,
) -> anyhow::Result<usize> {
    let versions: Vec<String> = versions
        .into_iter()
        .map(|v| normalize_version(&v))
        .filter(|v| !v.is_empty())
        .collect();
    if versions.is_empty() {
        return Ok(0);
    }

    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let now = now_ms();
        let mut inserted = 0usize;
        for v in versions {
            let changed = tx.execute(
                r#"
                INSERT INTO ignored_updates (version, ignored_at_ms)
                VALUES (?1, ?2)
                ON CONFLICT(version) DO UPDATE SET ignored_at_ms = excluded.ignored_at_ms
                "#,
                params![v, now],
            )?;
            if changed > 0 {
                inserted += 1;
            }
        }
        tx.commit()?;
        Ok(inserted)
    })
    .await
}

