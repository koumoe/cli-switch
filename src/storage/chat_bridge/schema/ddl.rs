use anyhow::Context as _;
use rusqlite::Connection;

use super::CHAT_BRIDGE_SCHEMA_SQL;

pub(super) fn ensure_chat_bridge_tables(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(CHAT_BRIDGE_SCHEMA_SQL)
        .with_context(|| "创建消息互联基础表失败")?;
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
