use rusqlite::OptionalExtension as _;
use rusqlite::params;
use std::collections::HashSet;
use std::path::PathBuf;

use super::{ManagedRemoteProvider, now_ms, with_conn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteGroupSnapshotEntry {
    pub group_key: String,
    pub group_id: Option<i64>,
    pub group_name: String,
}

fn has_snapshot_state(
    conn: &rusqlite::Connection,
    provider: ManagedRemoteProvider,
    account_id: &str,
) -> anyhow::Result<bool> {
    let exists = conn
        .query_row(
            r#"
            SELECT 1
            FROM remote_account_group_snapshot_states
            WHERE provider = ?1 AND account_id = ?2
            "#,
            params![provider.as_str(), account_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

fn list_snapshot_entries(
    conn: &rusqlite::Connection,
    provider: ManagedRemoteProvider,
    account_id: &str,
) -> anyhow::Result<Vec<RemoteGroupSnapshotEntry>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT group_key, group_id, group_name
        FROM remote_account_group_snapshots
        WHERE provider = ?1 AND account_id = ?2
        ORDER BY group_name ASC, group_key ASC
        "#,
    )?;
    let rows = stmt.query_map(params![provider.as_str(), account_id], |row| {
        Ok(RemoteGroupSnapshotEntry {
            group_key: row.get(0)?,
            group_id: row.get(1)?,
            group_name: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub async fn sync_remote_group_snapshot(
    db_path: PathBuf,
    provider: ManagedRemoteProvider,
    account_id: String,
    entries: Vec<RemoteGroupSnapshotEntry>,
) -> anyhow::Result<Vec<RemoteGroupSnapshotEntry>> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let has_previous = has_snapshot_state(&tx, provider, &account_id)?;
        let previous_entries = if has_previous {
            list_snapshot_entries(&tx, provider, &account_id)?
        } else {
            Vec::new()
        };
        let previous_keys = previous_entries
            .iter()
            .map(|item| item.group_key.clone())
            .collect::<HashSet<_>>();
        let added_entries = if has_previous {
            entries
                .iter()
                .filter(|item| !previous_keys.contains(&item.group_key))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let ts = now_ms();

        tx.execute(
            r#"
            DELETE FROM remote_account_group_snapshots
            WHERE provider = ?1 AND account_id = ?2
            "#,
            params![provider.as_str(), account_id],
        )?;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO remote_account_group_snapshots (
                    provider, account_id, group_key, group_id, group_name, updated_at_ms
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
            )?;
            for entry in &entries {
                stmt.execute(params![
                    provider.as_str(),
                    account_id,
                    entry.group_key,
                    entry.group_id,
                    entry.group_name,
                    ts
                ])?;
            }
        }

        tx.execute(
            r#"
            INSERT INTO remote_account_group_snapshot_states (
                provider, account_id, last_synced_at_ms, updated_at_ms
            )
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(provider, account_id) DO UPDATE SET
              last_synced_at_ms = excluded.last_synced_at_ms,
              updated_at_ms = excluded.updated_at_ms
            "#,
            params![provider.as_str(), account_id, ts, ts],
        )?;

        tx.commit()?;
        Ok(added_entries)
    })
    .await
}

pub async fn clear_remote_group_snapshot(
    db_path: PathBuf,
    provider: ManagedRemoteProvider,
    account_id: String,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"
            DELETE FROM remote_account_group_snapshots
            WHERE provider = ?1 AND account_id = ?2
            "#,
            params![provider.as_str(), account_id],
        )?;
        tx.execute(
            r#"
            DELETE FROM remote_account_group_snapshot_states
            WHERE provider = ?1 AND account_id = ?2
            "#,
            params![provider.as_str(), account_id],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        RemoteGroupSnapshotEntry, clear_remote_group_snapshot, sync_remote_group_snapshot,
    };
    use crate::storage::{self, ManagedRemoteProvider};
    use std::path::{Path, PathBuf};

    fn remove_sqlite_artifacts(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    }

    fn temp_db_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-test-remote-group-snapshot-{}.sqlite",
            uuid::Uuid::new_v4()
        ))
    }

    fn entry(group_key: &str, group_id: Option<i64>, group_name: &str) -> RemoteGroupSnapshotEntry {
        RemoteGroupSnapshotEntry {
            group_key: group_key.to_string(),
            group_id,
            group_name: group_name.to_string(),
        }
    }

    #[tokio::test]
    async fn first_snapshot_sync_seeds_state_without_added_notifications() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).unwrap();

        let added = sync_remote_group_snapshot(
            db_path.clone(),
            ManagedRemoteProvider::Newapi,
            "account-1".to_string(),
            vec![entry("default", None, "default")],
        )
        .await
        .unwrap();

        assert!(added.is_empty());

        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn later_snapshot_sync_only_returns_new_groups() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).unwrap();

        sync_remote_group_snapshot(
            db_path.clone(),
            ManagedRemoteProvider::Sub2Api,
            "account-2".to_string(),
            vec![entry("1", Some(1), "default")],
        )
        .await
        .unwrap();

        let added = sync_remote_group_snapshot(
            db_path.clone(),
            ManagedRemoteProvider::Sub2Api,
            "account-2".to_string(),
            vec![
                entry("1", Some(1), "default"),
                entry("2", Some(2), "new-group"),
            ],
        )
        .await
        .unwrap();

        assert_eq!(added, vec![entry("2", Some(2), "new-group")]);

        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn clearing_snapshot_resets_initial_sync_state() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).unwrap();

        sync_remote_group_snapshot(
            db_path.clone(),
            ManagedRemoteProvider::Sub2Api,
            "account-3".to_string(),
            vec![entry("1", Some(1), "default")],
        )
        .await
        .unwrap();

        clear_remote_group_snapshot(
            db_path.clone(),
            ManagedRemoteProvider::Sub2Api,
            "account-3".to_string(),
        )
        .await
        .unwrap();

        let added = sync_remote_group_snapshot(
            db_path.clone(),
            ManagedRemoteProvider::Sub2Api,
            "account-3".to_string(),
            vec![entry("2", Some(2), "another")],
        )
        .await
        .unwrap();

        assert!(added.is_empty());

        remove_sqlite_artifacts(&db_path);
    }
}
