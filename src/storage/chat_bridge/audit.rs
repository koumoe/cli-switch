use rusqlite::params;
use std::path::PathBuf;

use super::*;

pub async fn create_chat_audit_log(
    db_path: PathBuf,
    input: CreateChatAuditLogInput,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"
            INSERT INTO chat_audit_log (
              platform, sender_id, chat_id, message_type, content, session_id, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                input.platform.as_str(),
                input.sender_id,
                input.chat_id,
                input.message_type,
                input.content,
                input.session_id,
                now_ms(),
            ],
        )?;
        Ok(())
    })
    .await
}
