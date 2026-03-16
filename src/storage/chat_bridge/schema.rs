use anyhow::Context as _;
use rusqlite::Connection;

mod ddl;
mod inspect;
mod legacy;
mod migrate;

const DEFAULT_LEGACY_PLATFORM: &str = "telegram";
const CHAT_BRIDGE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS pairing_tokens (
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

CREATE TABLE IF NOT EXISTS chat_bindings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  platform TEXT NOT NULL,
  platform_user_id TEXT NOT NULL,
  display_name TEXT,
  bound_at INTEGER NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 1,
  UNIQUE(platform)
);

CREATE TABLE IF NOT EXISTS bridge_known_projects (
  path TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS bridge_sessions (
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

CREATE TABLE IF NOT EXISTS chat_audit_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  platform TEXT NOT NULL,
  sender_id TEXT NOT NULL,
  chat_id TEXT NOT NULL,
  message_type TEXT NOT NULL,
  content TEXT NOT NULL,
  session_id INTEGER,
  created_at INTEGER NOT NULL
);
"#;

pub(in crate::storage) fn ensure_chat_bridge_schema(conn: &Connection) -> anyhow::Result<()> {
    let has_any_table = inspect::has_any_chat_bridge_table(conn)?;
    if !has_any_table {
        ddl::ensure_chat_bridge_tables(conn)?;
        ddl::ensure_chat_bridge_indexes(conn)?;
        return Ok(());
    }

    if inspect::table_exists(conn, "pairing_tokens")?
        && !inspect::table_has_column(conn, "pairing_tokens", "token_hint")?
    {
        conn.execute_batch("ALTER TABLE pairing_tokens ADD COLUMN token_hint TEXT;")
            .with_context(|| "升级 pairing_tokens.token_hint 失败")?;
    }

    let legacy_pairing_tokens = inspect::table_exists(conn, "pairing_tokens")?
        && !inspect::table_has_column(conn, "pairing_tokens", "platform")?;
    let legacy_bindings = inspect::table_exists(conn, "chat_bindings")?
        && inspect::table_has_column(conn, "chat_bindings", "user_id")?;
    let legacy_projects = inspect::table_exists(conn, "bridge_known_projects")?
        && inspect::table_has_column(conn, "bridge_known_projects", "added_by_user_id")?;
    let legacy_sessions = inspect::table_exists(conn, "bridge_sessions")?
        && inspect::table_has_column(conn, "bridge_sessions", "owner")?;
    let legacy_audit = inspect::table_exists(conn, "chat_audit_log")?
        && inspect::table_has_column(conn, "chat_audit_log", "user_id")?;

    if legacy_pairing_tokens
        || legacy_bindings
        || legacy_projects
        || legacy_sessions
        || legacy_audit
    {
        migrate::migrate_chat_bridge_scope_schema(conn)?;
    } else {
        ddl::ensure_chat_bridge_tables(conn)?;
    }

    if inspect::table_exists(conn, "chat_users")? {
        conn.execute_batch("DROP TABLE IF EXISTS chat_users;")
            .with_context(|| "清理旧版 chat_users 表失败")?;
    }

    ddl::ensure_chat_bridge_indexes(conn)?;
    Ok(())
}
