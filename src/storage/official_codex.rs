use super::{now_ms, with_conn};
use rusqlite::{OptionalExtension as _, params};
use serde::Serialize;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct OfficialCodexAccount {
    pub id: String,
    pub account_id: String,
    pub email: Option<String>,
    #[serde(skip_serializing)]
    pub access_token: String,
    #[serde(skip_serializing)]
    pub refresh_token: String,
    #[serde(skip_serializing)]
    pub id_token: String,
    pub token_configured: bool,
    pub expires_at_ms: i64,
    pub enabled: bool,
    pub last_error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

fn from_row(row: &rusqlite::Row<'_>, secrets: bool) -> rusqlite::Result<OfficialCodexAccount> {
    let access: String = row.get(3)?;
    let refresh: String = row.get(4)?;
    let id_token: String = row.get(5)?;
    Ok(OfficialCodexAccount {
        id: row.get(0)?,
        account_id: row.get(1)?,
        email: row.get(2)?,
        token_configured: !access.is_empty() && !refresh.is_empty(),
        access_token: if secrets { access } else { String::new() },
        refresh_token: if secrets { refresh } else { String::new() },
        id_token: if secrets { id_token } else { String::new() },
        expires_at_ms: row.get(6)?,
        enabled: row.get::<_, i64>(7)? != 0,
        last_error: row.get(8)?,
        created_at_ms: row.get(9)?,
        updated_at_ms: row.get(10)?,
    })
}

pub async fn list_official_codex_accounts(
    db: PathBuf,
) -> anyhow::Result<Vec<OfficialCodexAccount>> {
    with_conn(db, |c| {
    let mut s=c.prepare("SELECT id,account_id,email,access_token,refresh_token,id_token,expires_at_ms,enabled,last_error,created_at_ms,updated_at_ms FROM official_codex_accounts ORDER BY created_at_ms")?;
    Ok(s.query_map([], |r| from_row(r,false))?.collect::<Result<Vec<_>,_>>()?) }).await
}

pub async fn get_official_codex_account_secret(
    db: PathBuf,
    id: String,
) -> anyhow::Result<Option<OfficialCodexAccount>> {
    with_conn(db,move|c|Ok(c.query_row("SELECT id,account_id,email,access_token,refresh_token,id_token,expires_at_ms,enabled,last_error,created_at_ms,updated_at_ms FROM official_codex_accounts WHERE id=?1",[id],|r|from_row(r,true)).optional()?)).await
}

pub async fn upsert_official_codex_account(
    db: PathBuf,
    account_id: String,
    email: Option<String>,
    access: String,
    refresh: String,
    id_token: String,
    expires_at_ms: i64,
) -> anyhow::Result<OfficialCodexAccount> {
    with_conn(db,move|c|{
    let now=now_ms(); let existing:Option<String>=c.query_row("SELECT id FROM official_codex_accounts WHERE account_id=?1",[&account_id],|r|r.get(0)).optional()?; let id=existing.unwrap_or_else(||Uuid::new_v4().to_string());
    c.execute("INSERT INTO official_codex_accounts(id,account_id,email,access_token,refresh_token,id_token,expires_at_ms,enabled,last_error,created_at_ms,updated_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,1,NULL,?8,?8) ON CONFLICT(account_id) DO UPDATE SET email=excluded.email,access_token=excluded.access_token,refresh_token=excluded.refresh_token,id_token=excluded.id_token,expires_at_ms=excluded.expires_at_ms,enabled=1,last_error=NULL,updated_at_ms=excluded.updated_at_ms",params![id,account_id,email,access,refresh,id_token,expires_at_ms,now])?;
    c.query_row("SELECT id,account_id,email,access_token,refresh_token,id_token,expires_at_ms,enabled,last_error,created_at_ms,updated_at_ms FROM official_codex_accounts WHERE account_id=?1",[account_id],|r|from_row(r,false)).map_err(Into::into)}).await
}

pub async fn delete_official_codex_account(db: PathBuf, id: String) -> anyhow::Result<()> {
    with_conn(db, move |c| {
        c.execute("DELETE FROM official_codex_accounts WHERE id=?1", [id])?;
        Ok(())
    })
    .await
}

pub async fn update_official_codex_tokens(
    db: PathBuf,
    id: String,
    access: String,
    refresh: String,
    id_token: String,
    expires_at_ms: i64,
) -> anyhow::Result<()> {
    with_conn(db,move|c|{c.execute("UPDATE official_codex_accounts SET access_token=?2,refresh_token=?3,id_token=?4,expires_at_ms=?5,last_error=NULL,updated_at_ms=?6 WHERE id=?1",params![id,access,refresh,id_token,expires_at_ms,now_ms()])?;Ok(())}).await
}
