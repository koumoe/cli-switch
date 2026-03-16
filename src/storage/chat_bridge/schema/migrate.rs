use anyhow::Context as _;
use rusqlite::{Connection, params};

use super::legacy::{
    build_owner_platforms, choose_latest_binding_per_platform, load_legacy_audit_logs,
    load_legacy_bindings, load_legacy_known_projects, load_legacy_pairing_tokens,
    load_legacy_sessions, migrate_legacy_sessions,
};

pub(super) fn migrate_chat_bridge_scope_schema(conn: &Connection) -> anyhow::Result<()> {
    let tx = conn
        .unchecked_transaction()
        .context("begin chat bridge scope migration transaction failed")?;

    let pairing_tokens = load_legacy_pairing_tokens(&tx)?;
    let bindings = choose_latest_binding_per_platform(load_legacy_bindings(&tx)?);
    let known_projects = load_legacy_known_projects(&tx)?;
    let owner_platforms = build_owner_platforms(&bindings);
    let sessions = migrate_legacy_sessions(load_legacy_sessions(&tx)?, &owner_platforms);
    let audit_logs = load_legacy_audit_logs(&tx)?;

    tx.execute_batch(
        r#"
        CREATE TABLE pairing_tokens_new (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          platform TEXT NOT NULL,
          token_hash TEXT NOT NULL UNIQUE,
          token_hint TEXT,
          created_at INTEGER NOT NULL,
          expires_at INTEGER NOT NULL,
          used_at INTEGER,
          used_by_platform TEXT,
          used_by_sender_id TEXT
        );

        CREATE TABLE chat_bindings_new (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          platform TEXT NOT NULL,
          platform_user_id TEXT NOT NULL,
          display_name TEXT,
          bound_at INTEGER NOT NULL,
          is_active INTEGER NOT NULL DEFAULT 1,
          UNIQUE(platform)
        );

        CREATE TABLE bridge_known_projects_new (
          path TEXT PRIMARY KEY,
          display_name TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE bridge_sessions_new (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          alias TEXT,
          platform TEXT NOT NULL,
          cli_type TEXT NOT NULL,
          cli_session_ref TEXT,
          project_id TEXT,
          project_name TEXT NOT NULL,
          working_dir TEXT NOT NULL,
          permission_mode TEXT NOT NULL DEFAULT 'safe',
          status TEXT NOT NULL DEFAULT 'idle',
          is_default INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          last_active INTEGER,
          UNIQUE(platform, alias)
        );

        CREATE TABLE chat_audit_log_new (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          platform TEXT NOT NULL,
          sender_id TEXT NOT NULL,
          chat_id TEXT NOT NULL,
          message_type TEXT NOT NULL,
          content TEXT NOT NULL,
          session_id INTEGER,
          created_at INTEGER NOT NULL
        );
        "#,
    )?;

    {
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO pairing_tokens_new (
              id,
              platform,
              token_hash,
              token_hint,
              created_at,
              expires_at,
              used_at,
              used_by_platform,
              used_by_sender_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )?;
        for item in pairing_tokens {
            stmt.execute(params![
                item.id,
                item.platform,
                item.token_hash,
                item.token_hint,
                item.created_at_ms,
                item.expires_at_ms,
                item.used_at_ms,
                item.used_by_platform,
                item.used_by_sender_id,
            ])?;
        }
    }

    {
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO chat_bindings_new (
              id,
              platform,
              platform_user_id,
              display_name,
              bound_at,
              is_active
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )?;
        for item in bindings {
            stmt.execute(params![
                item.id,
                item.platform,
                item.platform_user_id,
                item.display_name,
                item.bound_at_ms,
                if item.is_active { 1 } else { 0 },
            ])?;
        }
    }

    {
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO bridge_known_projects_new (
              path,
              display_name,
              created_at,
              updated_at
            ) VALUES (?1, ?2, ?3, ?4)
            "#,
        )?;
        for item in known_projects {
            stmt.execute(params![
                item.path,
                item.display_name,
                item.created_at_ms,
                item.updated_at_ms,
            ])?;
        }
    }

    {
        let mut stmt_with_id = tx.prepare(
            r#"
            INSERT INTO bridge_sessions_new (
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )?;
        let mut stmt_without_id = tx.prepare(
            r#"
            INSERT INTO bridge_sessions_new (
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )?;
        for item in sessions {
            if let Some(id) = item.id {
                stmt_with_id.execute(params![
                    id,
                    item.alias,
                    item.platform,
                    item.cli_type,
                    item.cli_session_ref,
                    item.project_id,
                    item.project_name,
                    item.working_dir,
                    item.permission_mode,
                    item.status,
                    if item.is_default { 1 } else { 0 },
                    item.created_at_ms,
                    item.last_active_ms,
                ])?;
            } else {
                stmt_without_id.execute(params![
                    item.alias,
                    item.platform,
                    item.cli_type,
                    item.cli_session_ref,
                    item.project_id,
                    item.project_name,
                    item.working_dir,
                    item.permission_mode,
                    item.status,
                    if item.is_default { 1 } else { 0 },
                    item.created_at_ms,
                    item.last_active_ms,
                ])?;
            }
        }
    }

    {
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO chat_audit_log_new (
              id,
              platform,
              sender_id,
              chat_id,
              message_type,
              content,
              session_id,
              created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )?;
        for item in audit_logs {
            stmt.execute(params![
                item.id,
                item.platform,
                item.sender_id,
                item.chat_id,
                item.message_type,
                item.content,
                item.session_id,
                item.created_at_ms,
            ])?;
        }
    }

    tx.execute_batch(
        r#"
        DROP TABLE IF EXISTS pairing_tokens;
        ALTER TABLE pairing_tokens_new RENAME TO pairing_tokens;

        DROP TABLE IF EXISTS chat_bindings;
        ALTER TABLE chat_bindings_new RENAME TO chat_bindings;

        DROP TABLE IF EXISTS bridge_known_projects;
        ALTER TABLE bridge_known_projects_new RENAME TO bridge_known_projects;

        DROP TABLE IF EXISTS bridge_sessions;
        ALTER TABLE bridge_sessions_new RENAME TO bridge_sessions;

        DROP TABLE IF EXISTS chat_audit_log;
        ALTER TABLE chat_audit_log_new RENAME TO chat_audit_log;

        DROP TABLE IF EXISTS chat_users;
        "#,
    )?;

    tx.commit()?;
    Ok(())
}
