use anyhow::Context as _;
use rusqlite::Connection;

use super::CHAT_BRIDGE_SCHEMA_SQL;

pub(super) fn ensure_chat_bridge_tables(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(CHAT_BRIDGE_SCHEMA_SQL)
        .with_context(|| "创建消息互联基础表失败")?;
    migrate_chat_bindings_unique_key(conn)?;
    Ok(())
}

pub(super) fn ensure_chat_bridge_indexes(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_pairing_tokens_lookup_hint ON pairing_tokens(token_hint, expires_at, used_at);
        CREATE INDEX IF NOT EXISTS idx_chat_bindings_bound_at ON chat_bindings(bound_at DESC);
        CREATE INDEX IF NOT EXISTS idx_bridge_sessions_platform_last_active ON bridge_sessions(platform, status, last_active DESC);
        "#,
    )
    .with_context(|| "创建消息互联索引失败")?;
    Ok(())
}

fn migrate_chat_bindings_unique_key(conn: &Connection) -> anyhow::Result<()> {
    if has_chat_bindings_composite_unique(conn)? {
        return Ok(());
    }

    let tx = conn
        .unchecked_transaction()
        .with_context(|| "begin chat_bindings unique key migration transaction failed")?;
    tx.execute_batch(
        r#"
        CREATE TABLE chat_bindings_v2 (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          platform TEXT NOT NULL,
          platform_user_id TEXT NOT NULL,
          display_name TEXT,
          bound_at INTEGER NOT NULL,
          is_active INTEGER NOT NULL DEFAULT 1,
          UNIQUE(platform, platform_user_id)
        );

        INSERT INTO chat_bindings_v2 (
          id,
          platform,
          platform_user_id,
          display_name,
          bound_at,
          is_active
        )
        SELECT
          id,
          platform,
          platform_user_id,
          display_name,
          bound_at,
          is_active
        FROM (
          SELECT
            id,
            platform,
            platform_user_id,
            display_name,
            bound_at,
            is_active,
            ROW_NUMBER() OVER (
              PARTITION BY platform, platform_user_id
              ORDER BY is_active DESC, bound_at DESC, id DESC
            ) AS rn
          FROM chat_bindings
        ) AS ranked
        WHERE rn = 1;

        DROP TABLE chat_bindings;
        ALTER TABLE chat_bindings_v2 RENAME TO chat_bindings;
        "#,
    )
    .with_context(|| "migrate chat_bindings unique key failed")?;
    tx.commit()
        .with_context(|| "commit chat_bindings unique key migration failed")?;
    Ok(())
}

fn has_chat_bindings_composite_unique(conn: &Connection) -> anyhow::Result<bool> {
    let mut stmt = conn.prepare("PRAGMA index_list('chat_bindings')")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?,   // name
            row.get::<_, i64>(2)? != 0, // unique
        ))
    })?;

    for row in rows {
        let (index_name, is_unique) = row?;
        if !is_unique {
            continue;
        }

        let pragma = format!("PRAGMA index_info('{}')", index_name.replace('\'', "''"));
        let mut col_stmt = conn.prepare(&pragma)?;
        let cols = col_stmt.query_map([], |col_row| col_row.get::<_, String>(2))?;
        let col_names = cols.collect::<rusqlite::Result<Vec<_>>>()?;
        if col_names == ["platform".to_string(), "platform_user_id".to_string()] {
            return Ok(true);
        }
    }

    Ok(false)
}
