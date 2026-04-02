use anyhow::Context as _;
use rusqlite::OptionalExtension as _;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::Time;
use uuid::Uuid;

use crate::bearer_token::normalize_optional_bearer_token;

use super::{RechargeCurrency, StorageError, now_ms, with_conn};

const CHECKIN_TIME_FORMAT: &str = "[hour]:[minute]:[second]";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemoteAccountProvider {
    #[serde(rename = "sub2api")]
    Sub2Api,
}

impl RemoteAccountProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            RemoteAccountProvider::Sub2Api => "sub2api",
        }
    }
}

impl std::str::FromStr for RemoteAccountProvider {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sub2api" => Ok(Self::Sub2Api),
            other => Err(anyhow::anyhow!("未知 remote provider：{other}")),
        }
    }
}

impl rusqlite::types::FromSql for RemoteAccountProvider {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse::<Self>()
            .map_err(|e| rusqlite::types::FromSqlError::Other(e.into_boxed_dyn_error()))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RemoteAccountCheckinMode {
    Disabled,
    PageOpen,
}

impl RemoteAccountCheckinMode {
    fn as_str(self) -> &'static str {
        match self {
            RemoteAccountCheckinMode::Disabled => "disabled",
            RemoteAccountCheckinMode::PageOpen => "page_open",
        }
    }
}

impl std::str::FromStr for RemoteAccountCheckinMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "disabled" => Ok(Self::Disabled),
            "page_open" => Ok(Self::PageOpen),
            other => Err(anyhow::anyhow!("未知 remote checkin mode：{other}")),
        }
    }
}

impl rusqlite::types::FromSql for RemoteAccountCheckinMode {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse::<Self>()
            .map_err(|e| rusqlite::types::FromSqlError::Other(e.into_boxed_dyn_error()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAccount {
    pub id: String,
    pub provider: RemoteAccountProvider,
    pub base_url: String,
    pub api_url: Option<String>,
    #[serde(skip_serializing)]
    pub access_token: Option<String>,
    #[serde(skip_serializing)]
    pub refresh_token: Option<String>,
    pub access_token_configured: bool,
    pub page_checkin_url: Option<String>,
    pub checkin_mode: RemoteAccountCheckinMode,
    pub auto_checkin_time: String,
    pub low_balance_alert_threshold: f64,
    pub recharge_currency: RechargeCurrency,
    pub remote_user_id: Option<String>,
    pub remote_role: Option<String>,
    pub remote_username: Option<String>,
    pub remote_display_name: Option<String>,
    pub last_balance_amount: Option<f64>,
    pub last_sync_error: Option<String>,
    pub reauth_required: bool,
    pub last_synced_at_ms: Option<i64>,
    pub low_balance_alert_notified: bool,
    pub last_balance_alert_at_ms: Option<i64>,
    pub sort_order: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateRemoteAccount {
    pub provider: RemoteAccountProvider,
    pub base_url: String,
    pub api_url: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub page_checkin_url: Option<String>,
    pub checkin_mode: Option<RemoteAccountCheckinMode>,
    pub auto_checkin_time: Option<String>,
    pub low_balance_alert_threshold: Option<f64>,
    pub recharge_currency: Option<RechargeCurrency>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateRemoteAccount {
    pub base_url: Option<String>,
    pub api_url: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub page_checkin_url: Option<String>,
    pub checkin_mode: Option<RemoteAccountCheckinMode>,
    pub auto_checkin_time: Option<String>,
    pub low_balance_alert_threshold: Option<f64>,
    pub recharge_currency: Option<RechargeCurrency>,
}

#[derive(Debug, Clone, Default)]
pub struct RemoteAccountRemoteSnapshot {
    pub remote_user_id: Option<String>,
    pub remote_role: Option<String>,
    pub remote_username: Option<String>,
    pub remote_display_name: Option<String>,
    pub last_balance_amount: Option<f64>,
    pub last_synced_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteAccountCheckinsToday {
    pub date: String,
    pub completed_account_ids: Vec<String>,
}

fn normalize_base_url(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

fn normalize_optional_base_url(raw: Option<String>) -> Option<String> {
    raw.map(|value| normalize_base_url(&value))
        .filter(|value| !value.is_empty())
}

fn normalize_optional_text(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn token_configured(raw: &str) -> bool {
    !raw.trim().is_empty()
}

fn normalize_auto_checkin_time(raw: Option<String>) -> anyhow::Result<String> {
    let fmt = time::format_description::parse(CHECKIN_TIME_FORMAT)?;
    let input = raw.unwrap_or_else(|| "00:05:00".to_string());
    let parsed = Time::parse(input.trim(), &fmt)
        .with_context(|| format!("非法自动签到时间：{}", input.trim()))?;
    Ok(parsed.format(&fmt)?)
}

fn local_today_ymd() -> anyhow::Result<String> {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    let fmt = time::format_description::parse("[year]-[month]-[day]")?;
    Ok(now.format(&fmt)?)
}

fn remote_account_from_row(
    row: &rusqlite::Row<'_>,
    include_secret: bool,
) -> rusqlite::Result<RemoteAccount> {
    let access_token_raw: String = row.get(4)?;
    let refresh_token_raw: String = row.get(5)?;
    let access_token = if include_secret {
        Some(access_token_raw.clone()).filter(|value| token_configured(value))
    } else {
        None
    };
    let refresh_token = if include_secret {
        Some(refresh_token_raw.clone()).filter(|value| token_configured(value))
    } else {
        None
    };
    Ok(RemoteAccount {
        id: row.get(0)?,
        provider: row.get(1)?,
        base_url: row.get(2)?,
        api_url: row.get(3)?,
        access_token,
        refresh_token,
        access_token_configured: token_configured(&access_token_raw),
        page_checkin_url: row.get(6)?,
        checkin_mode: row.get(7)?,
        auto_checkin_time: row.get(8)?,
        low_balance_alert_threshold: row.get(9)?,
        recharge_currency: row.get(10)?,
        remote_user_id: row.get(11)?,
        remote_role: row.get(12)?,
        remote_username: row.get(13)?,
        remote_display_name: row.get(14)?,
        last_balance_amount: row.get(15)?,
        last_sync_error: row.get(16)?,
        reauth_required: row.get::<_, i64>(17)? != 0,
        last_synced_at_ms: row.get(18)?,
        low_balance_alert_notified: row.get::<_, i64>(19)? != 0,
        last_balance_alert_at_ms: row.get(20)?,
        sort_order: row.get(21)?,
        created_at_ms: row.get(22)?,
        updated_at_ms: row.get(23)?,
    })
}

pub async fn list_remote_accounts(db_path: PathBuf) -> anyhow::Result<Vec<RemoteAccount>> {
    list_remote_accounts_impl(db_path, false).await
}

pub async fn list_remote_accounts_with_secret(
    db_path: PathBuf,
) -> anyhow::Result<Vec<RemoteAccount>> {
    list_remote_accounts_impl(db_path, true).await
}

async fn list_remote_accounts_impl(
    db_path: PathBuf,
    include_secret: bool,
) -> anyhow::Result<Vec<RemoteAccount>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, provider, base_url, api_url, access_token, refresh_token, page_checkin_url, checkin_mode,
                   auto_checkin_time, low_balance_alert_threshold, recharge_currency, remote_user_id,
                   remote_role, remote_username, remote_display_name, last_balance_amount,
                   last_sync_error, reauth_required, last_synced_at_ms, low_balance_alert_notified,
                   last_balance_alert_at_ms, sort_order, created_at_ms, updated_at_ms
            FROM remote_accounts
            WHERE provider = 'sub2api'
            ORDER BY sort_order ASC, created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| remote_account_from_row(row, include_secret))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn get_remote_account_without_secret(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<RemoteAccount> {
    get_remote_account_impl(db_path, account_id.clone(), false)
        .await?
        .ok_or_else(|| StorageError::RemoteAccountNotFound { account_id }.into())
}

pub async fn get_remote_account_with_secret(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<RemoteAccount> {
    get_remote_account_impl(db_path, account_id.clone(), true)
        .await?
        .ok_or_else(|| StorageError::RemoteAccountNotFound { account_id }.into())
}

pub async fn get_remote_account_without_secret_optional(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Option<RemoteAccount>> {
    get_remote_account_impl(db_path, account_id, false).await
}

pub async fn get_remote_account_with_secret_optional(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Option<RemoteAccount>> {
    get_remote_account_impl(db_path, account_id, true).await
}

async fn get_remote_account_impl(
    db_path: PathBuf,
    account_id: String,
    include_secret: bool,
) -> anyhow::Result<Option<RemoteAccount>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, provider, base_url, api_url, access_token, refresh_token, page_checkin_url, checkin_mode,
                   auto_checkin_time, low_balance_alert_threshold, recharge_currency, remote_user_id,
                   remote_role, remote_username, remote_display_name, last_balance_amount,
                   last_sync_error, reauth_required, last_synced_at_ms, low_balance_alert_notified,
                   last_balance_alert_at_ms, sort_order, created_at_ms, updated_at_ms
            FROM remote_accounts
            WHERE provider = 'sub2api' AND id = ?1
            "#,
        )?;
        stmt.query_row([account_id], |row| remote_account_from_row(row, include_secret))
            .optional()
            .map_err(Into::into)
    })
    .await
}

fn ensure_unique_account(
    conn: &rusqlite::Connection,
    provider: RemoteAccountProvider,
    base_url: &str,
    exclude_id: Option<&str>,
) -> anyhow::Result<()> {
    let exists = match exclude_id {
        Some(account_id) => conn.query_row(
            r#"SELECT id FROM remote_accounts WHERE provider = ?1 AND base_url = ?2 AND id <> ?3"#,
            params![provider.as_str(), base_url, account_id],
            |_| Ok(()),
        ),
        None => conn.query_row(
            r#"SELECT id FROM remote_accounts WHERE provider = ?1 AND base_url = ?2"#,
            params![provider.as_str(), base_url],
            |_| Ok(()),
        ),
    }
    .optional()?;
    if exists.is_some() {
        return Err(StorageError::RemoteAccountAlreadyExists {
            provider: provider.as_str().to_string(),
            base_url: base_url.to_string(),
        }
        .into());
    }
    Ok(())
}

pub async fn create_remote_account(
    db_path: PathBuf,
    input: CreateRemoteAccount,
) -> anyhow::Result<RemoteAccount> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let id = Uuid::new_v4().to_string();
        let provider = input.provider;
        let base_url = normalize_base_url(&input.base_url);
        let api_url = normalize_optional_base_url(input.api_url);
        let access_token = normalize_optional_bearer_token(Some(input.access_token));
        let refresh_token = normalize_optional_bearer_token(input.refresh_token);
        let page_checkin_url = normalize_optional_text(input.page_checkin_url);
        let checkin_mode = input
            .checkin_mode
            .unwrap_or(RemoteAccountCheckinMode::Disabled);
        let auto_checkin_time = normalize_auto_checkin_time(input.auto_checkin_time)?;
        let low_balance_alert_threshold = input.low_balance_alert_threshold.unwrap_or(0.0);
        let recharge_currency = input.recharge_currency.unwrap_or(RechargeCurrency::Cny);
        let sort_order = ts;

        ensure_unique_account(conn, provider, &base_url, None)?;

        conn.execute(
            r#"
            INSERT INTO remote_accounts (
                id, provider, base_url, api_url, access_token, refresh_token, page_checkin_url, checkin_mode,
                auto_checkin_time, low_balance_alert_threshold, recharge_currency, sort_order,
                created_at_ms, updated_at_ms
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                id,
                provider.as_str(),
                base_url,
                api_url,
                access_token.as_deref().unwrap_or(""),
                refresh_token.as_deref().unwrap_or(""),
                page_checkin_url,
                checkin_mode.as_str(),
                auto_checkin_time,
                low_balance_alert_threshold,
                recharge_currency.as_str(),
                sort_order,
                ts,
                ts,
            ],
        )?;

        Ok(RemoteAccount {
            id,
            provider,
            base_url,
            api_url,
            access_token: access_token.clone(),
            refresh_token: refresh_token.clone(),
            access_token_configured: access_token.is_some(),
            page_checkin_url,
            checkin_mode,
            auto_checkin_time,
            low_balance_alert_threshold,
            recharge_currency,
            remote_user_id: None,
            remote_role: None,
            remote_username: None,
            remote_display_name: None,
            last_balance_amount: None,
            last_sync_error: None,
            reauth_required: false,
            last_synced_at_ms: None,
            low_balance_alert_notified: false,
            last_balance_alert_at_ms: None,
            sort_order,
            created_at_ms: ts,
            updated_at_ms: ts,
        })
    })
    .await
}

pub async fn update_remote_account(
    db_path: PathBuf,
    account_id: String,
    input: UpdateRemoteAccount,
) -> anyhow::Result<RemoteAccount> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let mut account = get_remote_account_impl_sync(conn, account_id.clone(), true)?
            .ok_or_else(|| StorageError::RemoteAccountNotFound {
                account_id: account_id.clone(),
            })?;

        if let Some(value) = input.base_url.as_deref() {
            account.base_url = normalize_base_url(value);
        }
        if input.api_url.is_some() {
            account.api_url = normalize_optional_base_url(input.api_url);
        }
        if let Some(value) = input.access_token {
            account.access_token = normalize_optional_bearer_token(Some(value));
            account.access_token_configured = account.access_token.is_some();
        }
        if let Some(value) = input.refresh_token {
            account.refresh_token = normalize_optional_bearer_token(Some(value));
        }
        if input.page_checkin_url.is_some() {
            account.page_checkin_url = normalize_optional_text(input.page_checkin_url);
        }
        if let Some(value) = input.checkin_mode {
            account.checkin_mode = value;
        }
        if let Some(value) = input.auto_checkin_time {
            account.auto_checkin_time = normalize_auto_checkin_time(Some(value))?;
        }
        if let Some(value) = input.low_balance_alert_threshold {
            account.low_balance_alert_threshold = value;
        }
        if let Some(value) = input.recharge_currency {
            account.recharge_currency = value;
        }

        ensure_unique_account(conn, account.provider, &account.base_url, Some(&account.id))?;

        conn.execute(
            r#"
            UPDATE remote_accounts
            SET base_url = ?2, api_url = ?3, access_token = ?4, refresh_token = ?5,
                page_checkin_url = ?6, checkin_mode = ?7, auto_checkin_time = ?8,
                low_balance_alert_threshold = ?9, recharge_currency = ?10, updated_at_ms = ?11
            WHERE id = ?1
            "#,
            params![
                account.id,
                account.base_url,
                account.api_url,
                account.access_token.as_deref().unwrap_or(""),
                account.refresh_token.as_deref().unwrap_or(""),
                account.page_checkin_url,
                account.checkin_mode.as_str(),
                account.auto_checkin_time,
                account.low_balance_alert_threshold,
                account.recharge_currency.as_str(),
                ts,
            ],
        )?;
        account.updated_at_ms = ts;
        Ok(account)
    })
    .await
}

fn get_remote_account_impl_sync(
    conn: &rusqlite::Connection,
    account_id: String,
    include_secret: bool,
) -> anyhow::Result<Option<RemoteAccount>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, provider, base_url, api_url, access_token, refresh_token, page_checkin_url, checkin_mode,
               auto_checkin_time, low_balance_alert_threshold, recharge_currency, remote_user_id,
               remote_role, remote_username, remote_display_name, last_balance_amount,
               last_sync_error, reauth_required, last_synced_at_ms, low_balance_alert_notified,
               last_balance_alert_at_ms, sort_order, created_at_ms, updated_at_ms
        FROM remote_accounts
        WHERE provider = 'sub2api' AND id = ?1
        "#,
    )?;
    stmt.query_row([account_id], |row| {
        remote_account_from_row(row, include_secret)
    })
    .optional()
    .map_err(Into::into)
}

pub async fn apply_remote_account_sync_success(
    db_path: PathBuf,
    account_id: String,
    snapshot: RemoteAccountRemoteSnapshot,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let synced_at_ms = snapshot.last_synced_at_ms.unwrap_or_else(now_ms);
        let updated_at_ms = now_ms();
        let changed = conn.execute(
            r#"
            UPDATE remote_accounts
            SET remote_user_id = ?2, remote_role = ?3, remote_username = ?4, remote_display_name = ?5,
                last_balance_amount = ?6, last_sync_error = ?7, reauth_required = 0,
                last_synced_at_ms = ?8, updated_at_ms = ?9
            WHERE provider = 'sub2api' AND id = ?1
            "#,
            params![
                account_id,
                snapshot.remote_user_id,
                snapshot.remote_role,
                snapshot.remote_username,
                snapshot.remote_display_name,
                snapshot.last_balance_amount,
                Option::<String>::None,
                synced_at_ms,
                updated_at_ms,
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::RemoteAccountNotFound { account_id }.into());
        }
        Ok(())
    })
    .await
}

pub async fn apply_remote_account_sync_failure(
    db_path: PathBuf,
    account_id: String,
    last_sync_error: String,
    last_synced_at_ms: Option<i64>,
    reauth_required: bool,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let synced_at_ms = last_synced_at_ms.unwrap_or_else(now_ms);
        let updated_at_ms = now_ms();
        let changed = conn.execute(
            r#"
            UPDATE remote_accounts
            SET last_sync_error = ?2, reauth_required = ?3, last_synced_at_ms = ?4, updated_at_ms = ?5
            WHERE provider = 'sub2api' AND id = ?1
            "#,
            params![
                account_id,
                last_sync_error,
                if reauth_required { 1 } else { 0 },
                synced_at_ms,
                updated_at_ms
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::RemoteAccountNotFound { account_id }.into());
        }
        Ok(())
    })
    .await
}

pub async fn update_remote_account_auth_session(
    db_path: PathBuf,
    account_id: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let updated_at_ms = now_ms();
        let access_token = normalize_optional_bearer_token(access_token);
        let refresh_token = normalize_optional_bearer_token(refresh_token);
        let changed = conn.execute(
            r#"
            UPDATE remote_accounts
            SET access_token = ?2, refresh_token = ?3, last_sync_error = NULL,
                reauth_required = 0, updated_at_ms = ?4
            WHERE provider = 'sub2api' AND id = ?1
            "#,
            params![
                account_id,
                access_token.as_deref().unwrap_or(""),
                refresh_token.as_deref().unwrap_or(""),
                updated_at_ms,
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::RemoteAccountNotFound { account_id }.into());
        }
        Ok(())
    })
    .await
}

pub async fn set_remote_account_balance_alert_notified(
    db_path: PathBuf,
    account_id: String,
    notified: bool,
    last_balance_alert_at_ms: Option<i64>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let changed: usize = conn.execute(
            r#"
            UPDATE remote_accounts
            SET low_balance_alert_notified = ?2, last_balance_alert_at_ms = ?3, updated_at_ms = ?4
            WHERE provider = 'sub2api' AND id = ?1
            "#,
            params![
                account_id,
                if notified { 1 } else { 0 },
                last_balance_alert_at_ms,
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

pub async fn delete_remote_account(db_path: PathBuf, account_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let deleted = tx.execute(
            r#"DELETE FROM remote_accounts WHERE provider = 'sub2api' AND id = ?1"#,
            [&account_id],
        )?;
        if deleted == 0 {
            return Err(StorageError::RemoteAccountNotFound { account_id }.into());
        }
        tx.execute(
            r#"DELETE FROM remote_account_checkins WHERE account_id = ?1"#,
            [&account_id],
        )?;
        tx.execute(
            r#"
            DELETE FROM remote_account_group_snapshots
            WHERE provider = 'sub2api' AND account_id = ?1
            "#,
            [&account_id],
        )?;
        tx.execute(
            r#"
            DELETE FROM remote_account_group_snapshot_states
            WHERE provider = 'sub2api' AND account_id = ?1
            "#,
            [&account_id],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn assign_remote_account_sort_orders(
    db_path: PathBuf,
    account_orders: Vec<(String, i64)>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let ts = now_ms();
        for (account_id, sort_order) in account_orders {
            tx.execute(
                r#"UPDATE remote_accounts SET sort_order = ?2, updated_at_ms = ?3 WHERE id = ?1"#,
                params![account_id, sort_order, ts],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn get_remote_accounts_checkins_today(
    db_path: PathBuf,
) -> anyhow::Result<RemoteAccountCheckinsToday> {
    with_conn(db_path, move |conn| {
        let date = local_today_ymd()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT account_id
            FROM remote_account_checkins c
            INNER JOIN remote_accounts a ON a.id = c.account_id
            WHERE c.date = ?1
              AND a.provider = 'sub2api'
            ORDER BY completed_at_ms ASC
            "#,
        )?;
        let completed_account_ids = stmt
            .query_map([date.clone()], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(RemoteAccountCheckinsToday {
            date,
            completed_account_ids,
        })
    })
    .await
}

pub async fn complete_remote_account_checkin_today(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let exists = conn
            .query_row(
                r#"SELECT id FROM remote_accounts WHERE provider = 'sub2api' AND id = ?1"#,
                [&account_id],
                |_| Ok(()),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StorageError::RemoteAccountNotFound { account_id }.into());
        }
        let date = local_today_ymd()?;
        conn.execute(
            r#"
            INSERT INTO remote_account_checkins (account_id, date, method, completed_at_ms)
            VALUES (?1, ?2, 'manual_page', ?3)
            ON CONFLICT(account_id, date) DO UPDATE SET completed_at_ms = excluded.completed_at_ms
            "#,
            params![account_id, date, now_ms()],
        )?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::RemoteAccountProvider;

    #[test]
    fn remote_account_provider_serializes_sub2api_without_extra_underscore() {
        let json = serde_json::to_string(&RemoteAccountProvider::Sub2Api).expect("serialize");
        assert_eq!(json, "\"sub2api\"");
    }
}
