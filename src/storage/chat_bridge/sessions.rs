use anyhow::Context as _;
use rusqlite::{OptionalExtension as _, params};
use std::path::PathBuf;

use super::*;

const BRIDGE_SESSION_COLUMNS: &str = r#"
  id,
  alias,
  platform,
  cli_type,
  cli_session_ref,
  project_id,
  project_name,
  working_dir,
  permission_mode,
  status,
  is_default,
  created_at,
  last_active
"#;

fn parse_cli_type(raw: &str) -> anyhow::Result<CliToolId> {
    match raw.trim() {
        "gemini" => Ok(CliToolId::Gemini),
        "claude" => Ok(CliToolId::Claude),
        "codex" => Ok(CliToolId::Codex),
        other => anyhow::bail!("unknown cli type: {other}"),
    }
}

fn from_sql_text_conversion_error(column: usize, err: anyhow::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            err.to_string(),
        )),
    )
}

fn row_to_bridge_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<BridgeSession> {
    let platform_raw: String = row.get(2)?;
    let cli_type_raw: String = row.get(3)?;
    let permission_raw: String = row.get(8)?;
    let status_raw: String = row.get(9)?;
    Ok(BridgeSession {
        id: row.get(0)?,
        alias: row.get(1)?,
        platform: ChatPlatform::parse(&platform_raw)
            .map_err(|err| from_sql_text_conversion_error(2, err))?,
        cli_type: parse_cli_type(&cli_type_raw)
            .map_err(|err| from_sql_text_conversion_error(3, err))?,
        cli_session_ref: row.get(4)?,
        project_id: row.get(5)?,
        project_name: row.get(6)?,
        working_dir: row.get(7)?,
        permission_mode: BridgePermissionMode::parse(&permission_raw)
            .map_err(|err| from_sql_text_conversion_error(8, err))?,
        status: BridgeSessionStatus::parse(&status_raw)
            .map_err(|err| from_sql_text_conversion_error(9, err))?,
        is_default: row.get::<_, i64>(10)? != 0,
        created_at_ms: row.get(11)?,
        last_active_ms: row.get::<_, Option<i64>>(12)?.unwrap_or(0),
    })
}

pub async fn create_bridge_session(
    db_path: PathBuf,
    input: CreateBridgeSessionInput,
) -> anyhow::Result<BridgeSession> {
    with_conn(db_path, move |conn| {
        let tx = conn
            .unchecked_transaction()
            .context("begin create bridge session transaction failed")?;
        let alias = normalize_optional_text(input.alias);
        if let Some(alias) = alias.as_deref() {
            let exists = tx
                .query_row(
                    "SELECT 1 FROM bridge_sessions WHERE platform = ?1 AND alias = ?2",
                    params![input.platform.as_str(), alias],
                    |_| Ok(()),
                )
                .optional()?;
            if exists.is_some() {
                return Err(StorageError::ChatSessionAliasExists {
                    platform: input.platform.as_str().to_string(),
                    alias: alias.to_string(),
                }
                .into());
            }
        }

        let now = now_ms();
        tx.execute(
            "UPDATE bridge_sessions SET is_default = 0 WHERE platform = ?1",
            params![input.platform.as_str()],
        )?;
        tx.execute(
            r#"
            INSERT INTO bridge_sessions (
              alias,
              platform,
              cli_type,
              cli_session_ref,
              project_id,
              project_name,
              working_dir,
              permission_mode,
              status,
              is_default,
              created_at,
              last_active
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'idle', 1, ?9, ?9)
            "#,
            params![
                alias.as_deref(),
                input.platform.as_str(),
                input.cli_type.as_str(),
                input.cli_session_ref.as_deref(),
                input.project_id.as_deref(),
                input.project_name,
                input.working_dir,
                input.permission_mode.as_str(),
                now,
            ],
        )?;
        let id = tx.last_insert_rowid();

        let sql = format!("SELECT {BRIDGE_SESSION_COLUMNS} FROM bridge_sessions WHERE id = ?1");
        let session = tx.query_row(&sql, params![id], row_to_bridge_session)?;
        tx.commit()?;
        Ok(session)
    })
    .await
}

pub async fn list_bridge_sessions_for_platform(
    db_path: PathBuf,
    platform: ChatPlatform,
    active_only: bool,
) -> anyhow::Result<Vec<BridgeSession>> {
    with_conn(db_path, move |conn| {
        let status_filter = if active_only {
            " AND status != 'stopped'"
        } else {
            ""
        };
        let sql = format!(
            "SELECT {BRIDGE_SESSION_COLUMNS} FROM bridge_sessions WHERE platform = ?1{status_filter} ORDER BY is_default DESC, last_active DESC, id ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![platform.as_str()], row_to_bridge_session)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn get_bridge_session(
    db_path: PathBuf,
    session_id: i64,
) -> anyhow::Result<BridgeSession> {
    with_conn(db_path, move |conn| {
        let sql = format!("SELECT {BRIDGE_SESSION_COLUMNS} FROM bridge_sessions WHERE id = ?1");
        conn.query_row(&sql, params![session_id], row_to_bridge_session)
            .optional()?
            .ok_or(StorageError::ChatSessionNotFound { session_id }.into())
    })
    .await
}

pub async fn get_default_bridge_session_for_platform(
    db_path: PathBuf,
    platform: ChatPlatform,
) -> anyhow::Result<Option<BridgeSession>> {
    with_conn(db_path, move |conn| {
        let sql = format!(
            "SELECT {BRIDGE_SESSION_COLUMNS} FROM bridge_sessions WHERE platform = ?1 AND is_default = 1 AND status != 'stopped' ORDER BY last_active DESC, id DESC LIMIT 1"
        );
        conn.query_row(&sql, params![platform.as_str()], row_to_bridge_session)
            .optional()
            .map_err(Into::into)
    })
    .await
}

pub async fn count_active_bridge_sessions_for_platform(
    db_path: PathBuf,
    platform: ChatPlatform,
) -> anyhow::Result<i64> {
    with_conn(db_path, move |conn| {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM bridge_sessions WHERE platform = ?1 AND status != 'stopped'",
            params![platform.as_str()],
            |row| row.get(0),
        )?;
        Ok(count)
    })
    .await
}

pub async fn set_default_bridge_session_for_platform(
    db_path: PathBuf,
    platform: ChatPlatform,
    session_id: i64,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn
            .unchecked_transaction()
            .context("begin set default bridge session transaction failed")?;
        let status = tx
            .query_row(
                "SELECT status FROM bridge_sessions WHERE id = ?1 AND platform = ?2",
                params![session_id, platform.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(StorageError::ChatSessionNotFound { session_id })?;
        if status == "stopped" {
            return Err(anyhow::anyhow!("cannot set a stopped session as default"));
        }

        tx.execute(
            "UPDATE bridge_sessions SET is_default = 0 WHERE platform = ?1",
            params![platform.as_str()],
        )?;
        tx.execute(
            "UPDATE bridge_sessions SET is_default = 1 WHERE id = ?1",
            params![session_id],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn update_bridge_session(
    db_path: PathBuf,
    session_id: i64,
    input: UpdateBridgeSessionInput,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn
            .unchecked_transaction()
            .context("begin update bridge session transaction failed")?;
        let exists = tx
            .query_row(
                "SELECT 1 FROM bridge_sessions WHERE id = ?1",
                params![session_id],
                |_| Ok(()),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StorageError::ChatSessionNotFound { session_id }.into());
        }

        if let Some(ref maybe_ref) = input.cli_session_ref {
            tx.execute(
                "UPDATE bridge_sessions SET cli_session_ref = ?2 WHERE id = ?1",
                params![session_id, maybe_ref.as_deref()],
            )?;
        }
        if let Some(status) = input.status {
            tx.execute(
                "UPDATE bridge_sessions SET status = ?2 WHERE id = ?1",
                params![session_id, status.as_str()],
            )?;
        }
        if let Some(is_default) = input.is_default {
            let platform: String = tx.query_row(
                "SELECT platform FROM bridge_sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )?;
            if is_default {
                tx.execute(
                    "UPDATE bridge_sessions SET is_default = 0 WHERE platform = ?1",
                    params![platform],
                )?;
            }
            tx.execute(
                "UPDATE bridge_sessions SET is_default = ?2 WHERE id = ?1",
                params![session_id, if is_default { 1 } else { 0 }],
            )?;
        }
        if let Some(last_active_ms) = input.last_active_ms {
            tx.execute(
                "UPDATE bridge_sessions SET last_active = ?2 WHERE id = ?1",
                params![session_id, last_active_ms],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn stop_bridge_session(
    db_path: PathBuf,
    session_id: i64,
) -> anyhow::Result<BridgeSession> {
    with_conn(db_path, move |conn| {
        let tx = conn
            .unchecked_transaction()
            .context("begin stop bridge session transaction failed")?;
        let sql = format!("SELECT {BRIDGE_SESSION_COLUMNS} FROM bridge_sessions WHERE id = ?1");
        let session = tx
            .query_row(&sql, params![session_id], row_to_bridge_session)
            .optional()?
            .ok_or(StorageError::ChatSessionNotFound { session_id })?;

        let now = now_ms();
        tx.execute(
            "UPDATE bridge_sessions SET status = 'stopped', is_default = 0, last_active = ?2 WHERE id = ?1",
            params![session_id, now],
        )?;

        if session.is_default {
            let next_default = tx
                .query_row(
                    r#"
                    SELECT id
                    FROM bridge_sessions
                    WHERE platform = ?1 AND status != 'stopped' AND id != ?2
                    ORDER BY last_active DESC, id DESC
                    LIMIT 1
                    "#,
                    params![session.platform.as_str(), session_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if let Some(next_id) = next_default {
                tx.execute(
                    "UPDATE bridge_sessions SET is_default = 1 WHERE id = ?1",
                    params![next_id],
                )?;
            }
        }

        tx.commit()?;
        Ok(BridgeSession {
            status: BridgeSessionStatus::Stopped,
            is_default: false,
            last_active_ms: now,
            ..session
        })
    })
    .await
}

pub async fn stop_all_bridge_sessions_for_platform(
    db_path: PathBuf,
    platform: ChatPlatform,
) -> anyhow::Result<i64> {
    with_conn(db_path, move |conn| {
        let updated = conn.execute(
            "UPDATE bridge_sessions SET status = 'stopped', is_default = 0, last_active = ?2 WHERE platform = ?1 AND status != 'stopped'",
            params![platform.as_str(), now_ms()],
        )?;
        Ok(updated as i64)
    })
    .await
}
