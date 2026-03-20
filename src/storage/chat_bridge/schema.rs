use rusqlite::Connection;

mod ddl;

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
  preferred_locale TEXT NOT NULL DEFAULT 'zh-CN',
  bound_at INTEGER NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 1,
  UNIQUE(platform, platform_user_id)
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
"#;

pub(in crate::storage) fn ensure_chat_bridge_schema(conn: &Connection) -> anyhow::Result<()> {
    ddl::ensure_chat_bridge_tables(conn)?;
    ddl::ensure_chat_bridge_indexes(conn)?;
    Ok(())
}
