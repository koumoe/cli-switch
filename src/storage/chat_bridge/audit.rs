use rusqlite::{Row, named_params, params};
use std::path::PathBuf;

use super::*;

const DEFAULT_AUDIT_LOG_LIMIT: i64 = 100;
const MAX_AUDIT_LOG_LIMIT: i64 = 200;

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

pub async fn list_chat_audit_logs(
    db_path: PathBuf,
    input: ListChatAuditLogsInput,
) -> anyhow::Result<ListChatAuditLogsResult> {
    with_conn(db_path, move |conn| {
        let limit = input
            .limit
            .unwrap_or(DEFAULT_AUDIT_LOG_LIMIT)
            .clamp(1, MAX_AUDIT_LOG_LIMIT);
        let offset = input.offset.unwrap_or(0).max(0);
        let query_limit = limit.saturating_add(1);
        let platform = input.platform.map(ChatPlatform::as_str);

        let mut stmt = conn.prepare(
            r#"
            SELECT
              id,
              platform,
              sender_id,
              chat_id,
              message_type,
              content,
              session_id,
              created_at
            FROM chat_audit_log
            WHERE (:platform IS NULL OR platform = :platform)
            ORDER BY created_at DESC, id DESC
            LIMIT :limit OFFSET :offset
            "#,
        )?;
        let rows = stmt.query_map(
            named_params! {
                ":platform": platform,
                ":limit": query_limit,
                ":offset": offset,
            },
            read_chat_audit_log_row,
        )?;

        let mut items = Vec::new();
        for row in rows {
            items.push(parse_chat_audit_log_row(row?)?);
        }

        let has_more = i64::try_from(items.len()).unwrap_or(i64::MAX) > limit;
        if has_more {
            items.truncate(limit as usize);
        }
        let next_offset = has_more.then_some(offset.saturating_add(limit));

        Ok(ListChatAuditLogsResult {
            items,
            limit,
            offset,
            has_more,
            next_offset,
        })
    })
    .await
}

struct RawChatAuditLogRow {
    id: i64,
    platform: String,
    sender_id: String,
    chat_id: String,
    message_type: String,
    content: String,
    session_id: Option<i64>,
    created_at_ms: i64,
}

fn read_chat_audit_log_row(row: &Row<'_>) -> rusqlite::Result<RawChatAuditLogRow> {
    Ok(RawChatAuditLogRow {
        id: row.get(0)?,
        platform: row.get(1)?,
        sender_id: row.get(2)?,
        chat_id: row.get(3)?,
        message_type: row.get(4)?,
        content: row.get(5)?,
        session_id: row.get(6)?,
        created_at_ms: row.get(7)?,
    })
}

fn parse_chat_audit_log_row(row: RawChatAuditLogRow) -> anyhow::Result<ChatAuditLogEntry> {
    Ok(ChatAuditLogEntry {
        id: row.id,
        platform: ChatPlatform::parse(&row.platform)?,
        sender_id: row.sender_id,
        chat_id: row.chat_id,
        message_type: row.message_type,
        content: row.content,
        session_id: row.session_id,
        created_at_ms: row.created_at_ms,
    })
}
