use rusqlite::params;
use std::path::{Path, PathBuf};

use super::*;

fn normalize_existing_dir(path: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(path).ok()?;
    canonical.is_dir().then_some(canonical)
}

fn default_project_display_name(path: &Path) -> String {
    if let Some(name) = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return name.to_string();
    }

    path.to_string_lossy().to_string()
}

pub async fn upsert_bridge_known_project(
    db_path: PathBuf,
    path: String,
    display_name: Option<String>,
) -> anyhow::Result<BridgeKnownProject> {
    with_conn(db_path, move |conn| {
        let canonical = normalize_existing_dir(Path::new(path.trim())).ok_or_else(|| {
            anyhow::anyhow!("bridge known project path must be an existing directory")
        })?;
        let canonical_str = canonical.to_string_lossy().to_string();
        let display_name = display_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default_project_display_name(&canonical));
        let now = now_ms();

        conn.execute(
            r#"
            INSERT INTO bridge_known_projects (
              path, display_name, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(path) DO UPDATE SET
              display_name = excluded.display_name,
              updated_at = excluded.updated_at
            "#,
            params![canonical_str, display_name, now, now],
        )?;

        let project = conn.query_row(
            r#"
            SELECT path, display_name, created_at, updated_at
            FROM bridge_known_projects
            WHERE path = ?1
            "#,
            params![canonical_str],
            |row| {
                Ok(BridgeKnownProject {
                    path: row.get(0)?,
                    display_name: row.get(1)?,
                    created_at_ms: row.get(2)?,
                    updated_at_ms: row.get(3)?,
                })
            },
        )?;
        Ok(project)
    })
    .await
}

pub async fn list_bridge_known_projects(
    db_path: PathBuf,
) -> anyhow::Result<Vec<BridgeKnownProject>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT path, display_name, created_at, updated_at
            FROM bridge_known_projects
            ORDER BY updated_at DESC, path ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BridgeKnownProject {
                path: row.get(0)?,
                display_name: row.get(1)?,
                created_at_ms: row.get(2)?,
                updated_at_ms: row.get(3)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}
