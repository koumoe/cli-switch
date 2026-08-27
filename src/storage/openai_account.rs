use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::{StorageError, now_ms, with_conn};

pub const OPENAI_ACCOUNT_BASE_URL: &str = "https://chatgpt.com";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiQuotaWindow {
    pub used_percent: f64,
    pub window_minutes: i64,
    pub resets_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct OpenAiQuotaSnapshot {
    pub primary: Option<OpenAiQuotaWindow>,
    pub secondary: Option<OpenAiQuotaWindow>,
    #[serde(default)]
    pub additional: Vec<OpenAiQuotaWindow>,
    pub synced_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiAccount {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(skip_serializing)]
    pub access_token: Option<String>,
    #[serde(skip_serializing)]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing)]
    pub id_token: Option<String>,
    pub access_token_configured: bool,
    pub refresh_token_configured: bool,
    pub remote_user_id: String,
    pub remote_username: Option<String>,
    pub remote_display_name: Option<String>,
    pub plan_type: Option<String>,
    pub token_expires_at_ms: Option<i64>,
    pub last_refresh_at_ms: Option<i64>,
    pub quota: OpenAiQuotaSnapshot,
    pub last_sync_error: Option<String>,
    pub reauth_required: bool,
    pub last_synced_at_ms: Option<i64>,
    pub sort_order: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct OpenAiAccountTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub token_expires_at_ms: Option<i64>,
    pub account_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
}

const SELECT_COLUMNS: &str = r#"
    id, name, base_url, access_token, refresh_token, id_token, remote_user_id,
    remote_username, remote_display_name, remote_role, token_expires_at_ms,
    last_refresh_at_ms, primary_quota_used_percent, primary_quota_window_minutes,
    primary_quota_resets_at_ms, secondary_quota_used_percent,
    secondary_quota_window_minutes, secondary_quota_resets_at_ms,
    quota_windows_json, last_sync_error, reauth_required, last_synced_at_ms, sort_order,
    created_at_ms, updated_at_ms
"#;

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_required_text(value: String, field: &str) -> anyhow::Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(anyhow::anyhow!("{field} is required"));
    }
    Ok(value)
}

fn configured(value: &str) -> bool {
    !value.trim().is_empty()
}

fn from_row(row: &rusqlite::Row<'_>, include_secret: bool) -> rusqlite::Result<OpenAiAccount> {
    let access_token_raw: String = row.get(3)?;
    let refresh_token_raw: String = row.get(4)?;
    let id_token_raw: String = row.get(5)?;
    let primary_used: Option<f64> = row.get(12)?;
    let primary_window: Option<i64> = row.get(13)?;
    let secondary_used: Option<f64> = row.get(15)?;
    let secondary_window: Option<i64> = row.get(16)?;
    let additional: Vec<OpenAiQuotaWindow> = row
        .get::<_, Option<String>>(18)?
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    Ok(OpenAiAccount {
        id: row.get(0)?,
        name: row.get(1)?,
        base_url: row.get(2)?,
        access_token: include_secret
            .then_some(access_token_raw.clone())
            .filter(|value| configured(value)),
        refresh_token: include_secret
            .then_some(refresh_token_raw.clone())
            .filter(|value| configured(value)),
        id_token: include_secret
            .then_some(id_token_raw)
            .filter(|value| configured(value)),
        access_token_configured: configured(&access_token_raw),
        refresh_token_configured: configured(&refresh_token_raw),
        remote_user_id: row.get(6)?,
        remote_username: row.get(7)?,
        remote_display_name: row.get(8)?,
        plan_type: row.get(9)?,
        token_expires_at_ms: row.get(10)?,
        last_refresh_at_ms: row.get(11)?,
        quota: OpenAiQuotaSnapshot {
            primary: primary_used
                .zip(primary_window)
                .map(|(used_percent, window_minutes)| OpenAiQuotaWindow {
                    used_percent,
                    window_minutes,
                    resets_at_ms: row.get(14).ok().flatten(),
                }),
            secondary: secondary_used.zip(secondary_window).map(
                |(used_percent, window_minutes)| OpenAiQuotaWindow {
                    used_percent,
                    window_minutes,
                    resets_at_ms: row.get(17).ok().flatten(),
                },
            ),
            additional,
            synced_at_ms: row.get(21)?,
        },
        last_sync_error: row.get(19)?,
        reauth_required: row.get::<_, i64>(20)? != 0,
        last_synced_at_ms: row.get(21)?,
        sort_order: row.get(22)?,
        created_at_ms: row.get(23)?,
        updated_at_ms: row.get(24)?,
    })
}

async fn list_impl(db_path: PathBuf, include_secret: bool) -> anyhow::Result<Vec<OpenAiAccount>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM remote_accounts WHERE provider = 'openai' ORDER BY sort_order ASC, created_at_ms ASC"
        ))?;
        stmt.query_map([], |row| from_row(row, include_secret))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn list_openai_accounts(db_path: PathBuf) -> anyhow::Result<Vec<OpenAiAccount>> {
    list_impl(db_path, false).await
}

pub async fn list_openai_accounts_with_secret(
    db_path: PathBuf,
) -> anyhow::Result<Vec<OpenAiAccount>> {
    list_impl(db_path, true).await
}

async fn get_optional_impl(
    db_path: PathBuf,
    account_id: String,
    include_secret: bool,
) -> anyhow::Result<Option<OpenAiAccount>> {
    with_conn(db_path, move |conn| {
        conn.query_row(
            &format!(
                "SELECT {SELECT_COLUMNS} FROM remote_accounts WHERE provider = 'openai' AND id = ?1"
            ),
            [account_id],
            |row| from_row(row, include_secret),
        )
        .optional()
        .map_err(Into::into)
    })
    .await
}

pub async fn get_openai_account_without_secret_optional(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Option<OpenAiAccount>> {
    get_optional_impl(db_path, account_id, false).await
}

pub async fn get_openai_account_with_secret_optional(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Option<OpenAiAccount>> {
    get_optional_impl(db_path, account_id, true).await
}

pub async fn get_openai_account_without_secret(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<OpenAiAccount> {
    get_openai_account_without_secret_optional(db_path, account_id.clone())
        .await?
        .ok_or_else(|| StorageError::RemoteAccountNotFound { account_id }.into())
}

pub async fn get_openai_account_with_secret(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<OpenAiAccount> {
    get_openai_account_with_secret_optional(db_path, account_id.clone())
        .await?
        .ok_or_else(|| StorageError::RemoteAccountNotFound { account_id }.into())
}

pub async fn upsert_openai_account_tokens(
    db_path: PathBuf,
    name: Option<String>,
    tokens: OpenAiAccountTokens,
) -> anyhow::Result<OpenAiAccount> {
    let id = with_conn(db_path.clone(), move |conn| {
        let now = now_ms();
        let access_token = normalize_required_text(tokens.access_token, "access_token")?;
        let account_id = normalize_required_text(tokens.account_id, "account_id")?;
        let refresh_token = normalize_optional_text(tokens.refresh_token);
        let id_token = normalize_optional_text(tokens.id_token);
        let email = normalize_optional_text(tokens.email);
        let display_name = normalize_optional_text(tokens.display_name);
        let plan_type = normalize_optional_text(tokens.plan_type);
        let requested_name = normalize_optional_text(name);
        let existing = conn
            .query_row(
                "SELECT id, name, refresh_token, id_token, created_at_ms, sort_order FROM remote_accounts WHERE provider = 'openai' AND remote_user_id = ?1",
                [&account_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                },
            )
            .optional()?;
        let (id, account_name, persisted_refresh, persisted_id, created_at, sort_order) =
            match existing {
                Some((id, old_name, old_refresh, old_id, created_at, sort_order)) => (
                    id,
                    requested_name.unwrap_or(old_name),
                    refresh_token.unwrap_or(old_refresh),
                    id_token.unwrap_or(old_id),
                    created_at,
                    sort_order,
                ),
                None => {
                    let fallback_name = email
                        .clone()
                        .unwrap_or_else(|| "OpenAI".to_string());
                    (
                        Uuid::new_v4().to_string(),
                        requested_name.unwrap_or(fallback_name),
                        refresh_token.unwrap_or_default(),
                        id_token.unwrap_or_default(),
                        now,
                        now,
                    )
                }
            };
        conn.execute(
            r#"
            INSERT INTO remote_accounts (
              id, name, provider, base_url, access_token, refresh_token, id_token,
              token_expires_at_ms, last_refresh_at_ms, remote_user_id, remote_role,
              remote_username, remote_display_name, reauth_required, last_sync_error,
              last_synced_at_ms, sort_order, created_at_ms, updated_at_ms
            ) VALUES (?1, ?2, 'openai', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, NULL, ?8, ?13, ?14, ?8)
            ON CONFLICT(remote_user_id) WHERE provider = 'openai' AND remote_user_id IS NOT NULL AND remote_user_id <> ''
            DO UPDATE SET
              name = excluded.name,
              access_token = excluded.access_token,
              refresh_token = excluded.refresh_token,
              id_token = excluded.id_token,
              token_expires_at_ms = excluded.token_expires_at_ms,
              last_refresh_at_ms = excluded.last_refresh_at_ms,
              remote_role = excluded.remote_role,
              remote_username = excluded.remote_username,
              remote_display_name = excluded.remote_display_name,
              reauth_required = 0,
              last_sync_error = NULL,
              last_synced_at_ms = excluded.last_synced_at_ms,
              updated_at_ms = excluded.updated_at_ms
            "#,
            params![
                id,
                account_name,
                OPENAI_ACCOUNT_BASE_URL,
                access_token,
                persisted_refresh,
                persisted_id,
                tokens.token_expires_at_ms,
                now,
                account_id,
                plan_type,
                email,
                display_name,
                sort_order,
                created_at,
            ],
        )?;
        Ok(id)
    })
    .await?;
    get_openai_account_without_secret(db_path, id).await
}

pub async fn update_openai_account_name(
    db_path: PathBuf,
    account_id: String,
    name: String,
) -> anyhow::Result<OpenAiAccount> {
    let name = normalize_required_text(name, "name")?;
    with_conn(db_path.clone(), {
        let account_id = account_id.clone();
        move |conn| {
            let changed = conn.execute(
                "UPDATE remote_accounts SET name = ?2, updated_at_ms = ?3 WHERE provider = 'openai' AND id = ?1",
                params![account_id, name, now_ms()],
            )?;
            if changed == 0 {
                return Err(StorageError::RemoteAccountNotFound { account_id }.into());
            }
            Ok(())
        }
    })
    .await?;
    get_openai_account_without_secret(db_path, account_id).await
}

pub async fn update_openai_account_quota(
    db_path: PathBuf,
    account_id: String,
    quota: OpenAiQuotaSnapshot,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let synced_at = quota.synced_at_ms.unwrap_or_else(now_ms);
        let changed = conn.execute(
            r#"
            UPDATE remote_accounts SET
              primary_quota_used_percent = ?2, primary_quota_window_minutes = ?3,
              primary_quota_resets_at_ms = ?4, secondary_quota_used_percent = ?5,
              secondary_quota_window_minutes = ?6, secondary_quota_resets_at_ms = ?7,
              quota_windows_json = ?8, last_synced_at_ms = ?9,
              last_sync_error = NULL, reauth_required = 0, updated_at_ms = ?10
            WHERE provider = 'openai' AND id = ?1
            "#,
            params![
                account_id,
                quota.primary.as_ref().map(|value| value.used_percent),
                quota.primary.as_ref().map(|value| value.window_minutes),
                quota.primary.as_ref().and_then(|value| value.resets_at_ms),
                quota.secondary.as_ref().map(|value| value.used_percent),
                quota.secondary.as_ref().map(|value| value.window_minutes),
                quota
                    .secondary
                    .as_ref()
                    .and_then(|value| value.resets_at_ms),
                serde_json::to_string(&quota.additional).unwrap_or_else(|_| "[]".to_string()),
                synced_at,
                now_ms(),
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::RemoteAccountNotFound { account_id }.into());
        }
        Ok(())
    })
    .await
}

pub async fn mark_openai_account_auth_failure(
    db_path: PathBuf,
    account_id: String,
    message: String,
    reauth_required: Option<bool>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let now = now_ms();
        let changed = conn.execute(
            "UPDATE remote_accounts SET last_sync_error = ?2, reauth_required = COALESCE(?3, reauth_required), last_synced_at_ms = ?4, updated_at_ms = ?4 WHERE provider = 'openai' AND id = ?1",
            params![account_id, message, reauth_required.map(i64::from), now],
        )?;
        if changed == 0 {
            return Err(StorageError::RemoteAccountNotFound { account_id }.into());
        }
        Ok(())
    })
    .await
}

pub async fn delete_openai_account(db_path: PathBuf, account_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let changed = conn.execute(
            "DELETE FROM remote_accounts WHERE provider = 'openai' AND id = ?1",
            [&account_id],
        )?;
        if changed == 0 {
            return Err(StorageError::RemoteAccountNotFound { account_id }.into());
        }
        Ok(())
    })
    .await
}

pub async fn assign_openai_account_sort_orders(
    db_path: PathBuf,
    account_orders: Vec<(String, i64)>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let now = now_ms();
        for (account_id, sort_order) in account_orders {
            tx.execute(
                "UPDATE remote_accounts SET sort_order = ?2, updated_at_ms = ?3 WHERE provider = 'openai' AND id = ?1",
                params![account_id, sort_order, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> PathBuf {
        std::env::temp_dir().join(format!("cliswitch-openai-account-{}.db", Uuid::new_v4()))
    }

    fn tokens(account_id: &str, access: &str, refresh: Option<&str>) -> OpenAiAccountTokens {
        OpenAiAccountTokens {
            access_token: access.to_string(),
            refresh_token: refresh.map(str::to_string),
            id_token: Some("id-token".to_string()),
            token_expires_at_ms: Some(1_800_000_000_000),
            account_id: account_id.to_string(),
            email: Some(format!("{account_id}@example.com")),
            display_name: Some("OpenAI User".to_string()),
            plan_type: Some("plus".to_string()),
        }
    }

    #[tokio::test]
    async fn stores_secrets_but_hides_them_from_public_reads() {
        let db_path = temp_db();
        super::super::init_db(&db_path).unwrap();
        let created = upsert_openai_account_tokens(
            db_path.clone(),
            None,
            tokens("acct-1", "access-1", Some("refresh-1")),
        )
        .await
        .unwrap();
        assert!(created.access_token.is_none());
        assert!(created.refresh_token.is_none());
        assert!(created.id_token.is_none());
        assert!(created.access_token_configured);
        assert!(created.refresh_token_configured);

        let secret = get_openai_account_with_secret(db_path.clone(), created.id)
            .await
            .unwrap();
        assert_eq!(secret.access_token.as_deref(), Some("access-1"));
        assert_eq!(secret.refresh_token.as_deref(), Some("refresh-1"));
        assert_eq!(secret.id_token.as_deref(), Some("id-token"));
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn refresh_rotation_preserves_old_refresh_token_when_omitted() {
        let db_path = temp_db();
        super::super::init_db(&db_path).unwrap();
        let created = upsert_openai_account_tokens(
            db_path.clone(),
            None,
            tokens("acct-1", "access-1", Some("refresh-1")),
        )
        .await
        .unwrap();
        upsert_openai_account_tokens(
            db_path.clone(),
            Some("Custom name".to_string()),
            tokens("acct-1", "access-2", None),
        )
        .await
        .unwrap();
        let refreshed = get_openai_account_with_secret(db_path.clone(), created.id)
            .await
            .unwrap();
        assert_eq!(refreshed.name, "Custom name");
        assert_eq!(refreshed.access_token.as_deref(), Some("access-2"));
        assert_eq!(refreshed.refresh_token.as_deref(), Some("refresh-1"));
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn quota_success_clears_reauth_required() {
        let db_path = temp_db();
        super::super::init_db(&db_path).unwrap();
        let created = upsert_openai_account_tokens(
            db_path.clone(),
            None,
            tokens("acct-1", "access-1", Some("refresh-1")),
        )
        .await
        .unwrap();

        mark_openai_account_auth_failure(
            db_path.clone(),
            created.id.clone(),
            "expired access token".to_string(),
            Some(true),
        )
        .await
        .unwrap();
        let marked = get_openai_account_without_secret(db_path.clone(), created.id.clone())
            .await
            .unwrap();
        assert!(marked.reauth_required);

        mark_openai_account_auth_failure(
            db_path.clone(),
            created.id.clone(),
            "temporary usage failure".to_string(),
            None,
        )
        .await
        .unwrap();
        let preserved = get_openai_account_without_secret(db_path.clone(), created.id.clone())
            .await
            .unwrap();
        assert!(preserved.reauth_required);

        update_openai_account_quota(
            db_path.clone(),
            created.id.clone(),
            OpenAiQuotaSnapshot {
                primary: Some(OpenAiQuotaWindow {
                    used_percent: 12.0,
                    window_minutes: 300,
                    resets_at_ms: None,
                }),
                secondary: None,
                additional: vec![OpenAiQuotaWindow {
                    used_percent: 9.0,
                    window_minutes: 43_200,
                    resets_at_ms: Some(1_800_086_400_000),
                }],
                synced_at_ms: Some(1_800_000_000_000),
            },
        )
        .await
        .unwrap();

        let recovered = get_openai_account_without_secret(db_path.clone(), created.id)
            .await
            .unwrap();
        assert_eq!(recovered.quota.additional.len(), 1);
        assert_eq!(recovered.quota.additional[0].window_minutes, 43_200);
        assert!(!recovered.reauth_required);
        assert_eq!(recovered.last_sync_error, None);
        assert_eq!(recovered.quota.primary.unwrap().used_percent, 12.0);
        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn supports_multiple_openai_accounts() {
        let db_path = temp_db();
        super::super::init_db(&db_path).unwrap();
        upsert_openai_account_tokens(db_path.clone(), None, tokens("acct-1", "a", Some("r1")))
            .await
            .unwrap();
        upsert_openai_account_tokens(db_path.clone(), None, tokens("acct-2", "b", Some("r2")))
            .await
            .unwrap();
        assert_eq!(
            list_openai_accounts(db_path.clone()).await.unwrap().len(),
            2
        );
        let _ = std::fs::remove_file(db_path);
    }
}
