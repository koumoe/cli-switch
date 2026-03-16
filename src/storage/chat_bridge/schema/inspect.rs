use rusqlite::{Connection, OptionalExtension as _, params};

pub(super) fn table_exists(conn: &Connection, table: &str) -> anyhow::Result<bool> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            params![table],
            |_| Ok(()),
        )
        .optional()?;
    Ok(exists.is_some())
}

pub(super) fn table_has_column(
    conn: &Connection,
    table: &str,
    column: &str,
) -> anyhow::Result<bool> {
    if !table_exists(conn, table)? {
        return Ok(false);
    }
    let pragma = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&pragma)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row?.trim() == column {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn has_any_chat_bridge_table(conn: &Connection) -> anyhow::Result<bool> {
    Ok(table_exists(conn, "pairing_tokens")?
        || table_exists(conn, "chat_bindings")?
        || table_exists(conn, "bridge_known_projects")?
        || table_exists(conn, "bridge_sessions")?
        || table_exists(conn, "chat_audit_log")?
        || table_exists(conn, "chat_users")?)
}
