use rusqlite::OptionalExtension as _;
use rusqlite::params;
use std::path::PathBuf;

use super::{now_ms, with_conn};

#[derive(Debug, Clone)]
pub struct OpenAiCodexTicket {
    pub account_id: String,
    pub model: String,
    pub state: String,
    pub captured_at_ms: i64,
    pub expires_at_ms: i64,
    pub last_attempt_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenAiCodexTicketStatus {
    pub account_id: String,
    pub model: String,
    pub ready: bool,
    pub length: usize,
    pub remaining_seconds: i64,
    pub expires_at_ms: Option<i64>,
    pub last_attempt_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

fn row_to_ticket(row: &rusqlite::Row<'_>) -> rusqlite::Result<OpenAiCodexTicket> {
    Ok(OpenAiCodexTicket {
        account_id: row.get(0)?,
        model: row.get(1)?,
        state: row.get(2)?,
        captured_at_ms: row.get(3)?,
        expires_at_ms: row.get(4)?,
        last_attempt_at_ms: row.get(5)?,
        last_error: row.get(6)?,
    })
}

pub async fn get_openai_codex_ticket(
    db_path: PathBuf,
    account_id: String,
    model: String,
) -> anyhow::Result<Option<OpenAiCodexTicket>> {
    with_conn(db_path, move |conn| {
        conn.query_row(
            "SELECT account_id, model, state, captured_at_ms, expires_at_ms, last_attempt_at_ms, last_error FROM openai_codex_tickets WHERE account_id = ?1 AND model = ?2",
            params![account_id, model],
            row_to_ticket,
        )
        .optional()
        .map_err(Into::into)
    })
    .await
}

pub async fn list_openai_codex_tickets(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Vec<OpenAiCodexTicket>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            "SELECT account_id, model, state, captured_at_ms, expires_at_ms, last_attempt_at_ms, last_error FROM openai_codex_tickets WHERE account_id = ?1 ORDER BY model",
        )?;
        stmt.query_map([account_id], row_to_ticket)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn upsert_openai_codex_ticket(
    db_path: PathBuf,
    account_id: String,
    model: String,
    state: String,
    captured_at_ms: i64,
    expires_at_ms: i64,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"
            INSERT INTO openai_codex_tickets
                (account_id, model, state, captured_at_ms, expires_at_ms, last_attempt_at_ms, last_error)
            VALUES (?1, ?2, ?3, ?4, ?5, ?4, NULL)
            ON CONFLICT(account_id, model) DO UPDATE SET
                state = excluded.state,
                captured_at_ms = excluded.captured_at_ms,
                expires_at_ms = excluded.expires_at_ms,
                last_attempt_at_ms = excluded.last_attempt_at_ms,
                last_error = NULL
            "#,
            params![account_id, model, state, captured_at_ms, expires_at_ms],
        )?;
        Ok(())
    })
    .await
}

pub async fn record_openai_codex_ticket_attempt(
    db_path: PathBuf,
    account_id: String,
    model: String,
    error: Option<String>,
) -> anyhow::Result<()> {
    let attempted_at_ms = now_ms();
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"
            INSERT INTO openai_codex_tickets
                (account_id, model, state, captured_at_ms, expires_at_ms, last_attempt_at_ms, last_error)
            VALUES (?1, ?2, '', 0, 0, ?3, ?4)
            ON CONFLICT(account_id, model) DO UPDATE SET
                last_attempt_at_ms = excluded.last_attempt_at_ms,
                last_error = excluded.last_error
            "#,
            params![account_id, model, attempted_at_ms, error],
        )?;
        Ok(())
    })
    .await
}

pub async fn invalidate_openai_codex_ticket(
    db_path: PathBuf,
    account_id: String,
    model: String,
    error: String,
) -> anyhow::Result<()> {
    let invalidated_at_ms = now_ms();
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"
            UPDATE openai_codex_tickets
            SET state = '', captured_at_ms = 0, expires_at_ms = 0,
                last_attempt_at_ms = ?3, last_error = ?4
            WHERE account_id = ?1 AND model = ?2
            "#,
            params![account_id, model, invalidated_at_ms, error],
        )?;
        Ok(())
    })
    .await
}

pub async fn delete_openai_codex_tickets_for_account(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(
            "DELETE FROM openai_codex_tickets WHERE account_id = ?1",
            [account_id],
        )?;
        Ok(())
    })
    .await
}

pub fn ticket_is_valid(ticket: &OpenAiCodexTicket, now_ms: i64) -> bool {
    crate::openai_codex_ticket::is_valid_ticket_state(&ticket.state)
        && ticket.expires_at_ms > now_ms
}

pub fn ticket_status(ticket: OpenAiCodexTicket, now_ms: i64) -> OpenAiCodexTicketStatus {
    let ready = ticket_is_valid(&ticket, now_ms);
    OpenAiCodexTicketStatus {
        account_id: ticket.account_id,
        model: ticket.model,
        ready,
        length: ticket.state.len(),
        remaining_seconds: if ready {
            (ticket.expires_at_ms - now_ms).max(0) / 1000
        } else {
            0
        },
        expires_at_ms: (ticket.expires_at_ms > 0).then_some(ticket.expires_at_ms),
        last_attempt_at_ms: ticket.last_attempt_at_ms,
        last_error: ticket.last_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticket(expires_at_ms: i64, state: &str) -> OpenAiCodexTicket {
        OpenAiCodexTicket {
            account_id: "account".to_string(),
            model: "gpt-6-astra".to_string(),
            state: state.to_string(),
            captured_at_ms: 1,
            expires_at_ms,
            last_attempt_at_ms: None,
            last_error: None,
        }
    }

    #[test]
    fn validates_ticket_shape_and_expiration() {
        let now = 1_000;
        assert!(ticket_is_valid(
            &ticket(now + 1_000, &format!("gAAAAA{}", "x".repeat(286))),
            now
        ));
        assert!(!ticket_is_valid(
            &ticket(now - 1, &format!("gAAAAA{}", "x".repeat(286))),
            now
        ));
        assert!(!ticket_is_valid(
            &ticket(now + 1_000, &format!("bad!!!{}", "x".repeat(286))),
            now
        ));
        assert!(!ticket_is_valid(
            &ticket(now + 1_000, &format!("gAAAAA{}", "x".repeat(306))),
            now
        ));
    }
}
