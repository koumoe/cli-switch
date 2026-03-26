use anyhow::Context as _;
use rusqlite::OptionalExtension as _;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::Time;
use uuid::Uuid;

use super::{StorageError, now_ms, with_conn};

const CHECKIN_TIME_FORMAT: &str = "[hour]:[minute]:[second]";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NewApiAccountCheckinMode {
    SystemApi,
    PageOpen,
}

impl NewApiAccountCheckinMode {
    fn as_str(self) -> &'static str {
        match self {
            NewApiAccountCheckinMode::SystemApi => "system_api",
            NewApiAccountCheckinMode::PageOpen => "page_open",
        }
    }
}

impl std::str::FromStr for NewApiAccountCheckinMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system_api" => Ok(Self::SystemApi),
            "page_open" => Ok(Self::PageOpen),
            other => Err(anyhow::anyhow!("未知 newapi checkin mode：{other}")),
        }
    }
}

impl rusqlite::types::FromSql for NewApiAccountCheckinMode {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse::<Self>()
            .map_err(|e| rusqlite::types::FromSqlError::Other(e.into_boxed_dyn_error()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewApiAccount {
    pub id: String,
    pub base_url: String,
    pub user_id: String,
    #[serde(skip_serializing)]
    pub user_token: Option<String>,
    pub user_token_configured: bool,
    pub page_checkin_url: Option<String>,
    pub checkin_mode: NewApiAccountCheckinMode,
    pub auto_checkin_enabled: bool,
    pub auto_checkin_time: String,
    pub low_balance_alert_threshold: f64,
    pub remote_role: Option<i64>,
    pub remote_username: Option<String>,
    pub remote_display_name: Option<String>,
    pub remote_group: Option<String>,
    pub quota_display_type: String,
    pub quota_per_unit: f64,
    pub usd_exchange_rate: f64,
    pub custom_currency_symbol: Option<String>,
    pub custom_currency_exchange_rate: f64,
    pub remote_checkin_enabled: bool,
    pub remote_turnstile_check_enabled: bool,
    pub last_quota: Option<i64>,
    pub last_used_quota: Option<i64>,
    pub last_balance_amount: Option<f64>,
    pub last_sync_error: Option<String>,
    pub last_synced_at_ms: Option<i64>,
    pub low_balance_alert_notified: bool,
    pub last_balance_alert_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateNewApiAccount {
    pub base_url: String,
    pub user_id: String,
    pub user_token: String,
    pub page_checkin_url: Option<String>,
    pub checkin_mode: Option<NewApiAccountCheckinMode>,
    pub auto_checkin_enabled: Option<bool>,
    pub auto_checkin_time: Option<String>,
    pub low_balance_alert_threshold: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateNewApiAccount {
    pub base_url: Option<String>,
    pub user_id: Option<String>,
    pub user_token: Option<String>,
    pub page_checkin_url: Option<String>,
    pub checkin_mode: Option<NewApiAccountCheckinMode>,
    pub auto_checkin_enabled: Option<bool>,
    pub auto_checkin_time: Option<String>,
    pub low_balance_alert_threshold: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct NewApiAccountRemoteSnapshot {
    pub replace_remote_state: bool,
    pub remote_role: Option<i64>,
    pub remote_username: Option<String>,
    pub remote_display_name: Option<String>,
    pub remote_group: Option<String>,
    pub quota_display_type: Option<String>,
    pub quota_per_unit: Option<f64>,
    pub usd_exchange_rate: Option<f64>,
    pub custom_currency_symbol: Option<String>,
    pub custom_currency_exchange_rate: Option<f64>,
    pub remote_checkin_enabled: Option<bool>,
    pub remote_turnstile_check_enabled: Option<bool>,
    pub last_quota: Option<i64>,
    pub last_used_quota: Option<i64>,
    pub last_balance_amount: Option<f64>,
    pub last_sync_error: Option<String>,
    pub last_synced_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewApiAccountCheckinsToday {
    pub date: String,
    pub completed_account_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteNewApiAccountResult {
    pub deleted_managed_channel_ids: Vec<String>,
    pub detached_channel_ids: Vec<String>,
}

fn normalize_newapi_base_url(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
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

fn account_from_row(
    row: &rusqlite::Row<'_>,
    include_secret: bool,
) -> rusqlite::Result<NewApiAccount> {
    let user_token_raw: String = row.get(3)?;
    let user_token = if include_secret {
        Some(user_token_raw.clone()).filter(|value| token_configured(value))
    } else {
        None
    };
    Ok(NewApiAccount {
        id: row.get(0)?,
        base_url: row.get(1)?,
        user_id: row.get(2)?,
        user_token,
        user_token_configured: token_configured(&user_token_raw),
        page_checkin_url: row.get(4)?,
        checkin_mode: row.get(5)?,
        auto_checkin_enabled: row.get::<_, i64>(6)? != 0,
        auto_checkin_time: row.get(7)?,
        low_balance_alert_threshold: row.get(8)?,
        remote_role: row.get(9)?,
        remote_username: row.get(10)?,
        remote_display_name: row.get(11)?,
        remote_group: row.get(12)?,
        quota_display_type: row.get(13)?,
        quota_per_unit: row.get(14)?,
        usd_exchange_rate: row.get(15)?,
        custom_currency_symbol: row.get(16)?,
        custom_currency_exchange_rate: row.get(17)?,
        remote_checkin_enabled: row.get::<_, i64>(18)? != 0,
        remote_turnstile_check_enabled: row.get::<_, i64>(19)? != 0,
        last_quota: row.get(20)?,
        last_used_quota: row.get(21)?,
        last_balance_amount: row.get(22)?,
        last_sync_error: row.get(23)?,
        last_synced_at_ms: row.get(24)?,
        low_balance_alert_notified: row.get::<_, i64>(25)? != 0,
        last_balance_alert_at_ms: row.get(26)?,
        created_at_ms: row.get(27)?,
        updated_at_ms: row.get(28)?,
    })
}

fn local_today_ymd() -> anyhow::Result<String> {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let today = time::OffsetDateTime::now_utc().to_offset(offset).date();
    let fmt = time::format_description::parse("[year]-[month]-[day]")?;
    Ok(today.format(&fmt)?)
}

pub fn ensure_newapi_schema(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS newapi_accounts (
          id TEXT PRIMARY KEY,
          base_url TEXT NOT NULL,
          user_id TEXT NOT NULL,
          user_token TEXT NOT NULL,
          page_checkin_url TEXT NULL,
          checkin_mode TEXT NOT NULL DEFAULT 'system_api' CHECK(checkin_mode IN ('system_api','page_open')),
          auto_checkin_enabled INTEGER NOT NULL DEFAULT 0,
          auto_checkin_time TEXT NOT NULL DEFAULT '00:05:00',
          low_balance_alert_threshold REAL NOT NULL DEFAULT 0,
          remote_role INTEGER NULL,
          remote_username TEXT NULL,
          remote_display_name TEXT NULL,
          remote_group TEXT NULL,
          quota_display_type TEXT NOT NULL DEFAULT 'USD',
          quota_per_unit REAL NOT NULL DEFAULT 500000,
          usd_exchange_rate REAL NOT NULL DEFAULT 1,
          custom_currency_symbol TEXT NULL,
          custom_currency_exchange_rate REAL NOT NULL DEFAULT 1,
          remote_checkin_enabled INTEGER NOT NULL DEFAULT 0,
          remote_turnstile_check_enabled INTEGER NOT NULL DEFAULT 0,
          last_quota INTEGER NULL,
          last_used_quota INTEGER NULL,
          last_balance_amount REAL NULL,
          last_sync_error TEXT NULL,
          last_synced_at_ms INTEGER NULL,
          low_balance_alert_notified INTEGER NOT NULL DEFAULT 0,
          last_balance_alert_at_ms INTEGER NULL,
          created_at_ms INTEGER NOT NULL,
          updated_at_ms INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS newapi_account_checkins (
          account_id TEXT NOT NULL,
          date TEXT NOT NULL,
          method TEXT NOT NULL DEFAULT 'manual_page' CHECK(method IN ('manual_page','system_api','remote_detected')),
          completed_at_ms INTEGER NOT NULL,
          PRIMARY KEY (account_id, date)
        );
        CREATE INDEX IF NOT EXISTS idx_newapi_account_checkins_date
        ON newapi_account_checkins(date, completed_at_ms);
        "#,
    )?;

    ensure_newapi_account_column(conn, "sort_order", "INTEGER NOT NULL DEFAULT 0")?;
    conn.execute(r#"DROP INDEX IF EXISTS idx_newapi_accounts_base_user"#, [])?;
    conn.execute(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_newapi_accounts_base_user
        ON newapi_accounts(base_url, user_id)
        WHERE user_id <> ''
        "#,
        [],
    )?;

    ensure_channel_column(conn, "managed_by_newapi", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_channel_column(conn, "newapi_account_id", "TEXT NULL")?;
    ensure_channel_column(conn, "newapi_channel_id", "INTEGER NULL")?;
    ensure_channel_column(conn, "newapi_token_id", "INTEGER NULL")?;
    ensure_channel_column(conn, "newapi_token_name", "TEXT NULL")?;
    ensure_channel_column(conn, "newapi_group", "TEXT NULL")?;
    conn.execute(
        r#"CREATE INDEX IF NOT EXISTS idx_channels_newapi_account_id ON channels(newapi_account_id)"#,
        [],
    )?;

    Ok(())
}

fn ensure_channel_column(
    conn: &rusqlite::Connection,
    column_name: &str,
    column_ddl: &str,
) -> anyhow::Result<()> {
    ensure_table_column(conn, "channels", column_name, column_ddl)
}

fn ensure_newapi_account_column(
    conn: &rusqlite::Connection,
    column_name: &str,
    column_ddl: &str,
) -> anyhow::Result<()> {
    ensure_table_column(conn, "newapi_accounts", column_name, column_ddl)
}

fn ensure_table_column(
    conn: &rusqlite::Connection,
    table_name: &str,
    column_name: &str,
    column_ddl: &str,
) -> anyhow::Result<()> {
    let exists = {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table_name})"))?;
        let mut rows = stmt.query([])?;
        let mut found = false;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == column_name {
                found = true;
                break;
            }
        }
        found
    };
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {column_ddl}"),
            [],
        )?;
    }
    Ok(())
}

pub async fn list_newapi_accounts(db_path: PathBuf) -> anyhow::Result<Vec<NewApiAccount>> {
    list_newapi_accounts_impl(db_path, false).await
}

pub async fn list_newapi_accounts_with_secret(
    db_path: PathBuf,
) -> anyhow::Result<Vec<NewApiAccount>> {
    list_newapi_accounts_impl(db_path, true).await
}

async fn list_newapi_accounts_impl(
    db_path: PathBuf,
    include_secret: bool,
) -> anyhow::Result<Vec<NewApiAccount>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, base_url, user_id, user_token, page_checkin_url, checkin_mode, auto_checkin_enabled, auto_checkin_time,
                   low_balance_alert_threshold, remote_role, remote_username, remote_display_name, remote_group,
                   quota_display_type, quota_per_unit, usd_exchange_rate, custom_currency_symbol, custom_currency_exchange_rate,
                   remote_checkin_enabled, remote_turnstile_check_enabled, last_quota, last_used_quota, last_balance_amount,
                   last_sync_error, last_synced_at_ms, low_balance_alert_notified, last_balance_alert_at_ms, created_at_ms, updated_at_ms
            FROM newapi_accounts
            ORDER BY sort_order ASC, created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| account_from_row(row, include_secret))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn get_newapi_account(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<NewApiAccount> {
    get_newapi_account_impl(db_path, account_id, false)
        .await?
        .ok_or_else(|| {
            StorageError::NewApiAccountNotFound {
                account_id: "<unknown>".to_string(),
            }
            .into()
        })
}

pub async fn get_newapi_account_without_secret(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<NewApiAccount> {
    get_newapi_account_impl(db_path, account_id.clone(), false)
        .await?
        .ok_or_else(|| StorageError::NewApiAccountNotFound { account_id }.into())
}

pub async fn get_newapi_account_without_secret_optional(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Option<NewApiAccount>> {
    get_newapi_account_impl(db_path, account_id, false).await
}

pub async fn get_newapi_account_with_secret(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<NewApiAccount> {
    get_newapi_account_impl(db_path, account_id.clone(), true)
        .await?
        .ok_or_else(|| StorageError::NewApiAccountNotFound { account_id }.into())
}

pub async fn get_newapi_account_with_secret_optional(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Option<NewApiAccount>> {
    get_newapi_account_impl(db_path, account_id, true).await
}

pub async fn get_newapi_account_for_secret_use(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<NewApiAccount> {
    get_newapi_account_with_secret(db_path, account_id).await
}

async fn get_newapi_account_impl(
    db_path: PathBuf,
    account_id: String,
    include_secret: bool,
) -> anyhow::Result<Option<NewApiAccount>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, base_url, user_id, user_token, page_checkin_url, checkin_mode, auto_checkin_enabled, auto_checkin_time,
                   low_balance_alert_threshold, remote_role, remote_username, remote_display_name, remote_group,
                   quota_display_type, quota_per_unit, usd_exchange_rate, custom_currency_symbol, custom_currency_exchange_rate,
                   remote_checkin_enabled, remote_turnstile_check_enabled, last_quota, last_used_quota, last_balance_amount,
                   last_sync_error, last_synced_at_ms, low_balance_alert_notified, last_balance_alert_at_ms, created_at_ms, updated_at_ms
            FROM newapi_accounts
            WHERE id = ?1
            "#,
        )?;
        stmt.query_row([account_id], |row| account_from_row(row, include_secret))
            .optional()
            .map_err(Into::into)
    })
    .await
}

pub async fn create_newapi_account(
    db_path: PathBuf,
    input: CreateNewApiAccount,
) -> anyhow::Result<NewApiAccount> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let id = Uuid::new_v4().to_string();
        let base_url = normalize_newapi_base_url(&input.base_url);
        let user_id = input.user_id.trim().to_string();
        let user_token = normalize_optional_text(Some(input.user_token));
        let page_checkin_url = normalize_optional_text(input.page_checkin_url);
        let checkin_mode = input.checkin_mode.unwrap_or(NewApiAccountCheckinMode::SystemApi);
        let auto_checkin_enabled = input.auto_checkin_enabled.unwrap_or(false)
            && matches!(checkin_mode, NewApiAccountCheckinMode::SystemApi);
        let auto_checkin_time = normalize_auto_checkin_time(input.auto_checkin_time)?;
        let low_balance_alert_threshold = input.low_balance_alert_threshold.unwrap_or(0.0);
        let sort_order = ts;

        ensure_unique_account(conn, &base_url, &user_id, None)?;

        conn.execute(
            r#"
            INSERT INTO newapi_accounts (
                id, base_url, user_id, user_token, page_checkin_url, checkin_mode, auto_checkin_enabled, auto_checkin_time,
                low_balance_alert_threshold, sort_order, created_at_ms, updated_at_ms
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                id,
                base_url,
                user_id,
                user_token.as_deref().unwrap_or(""),
                page_checkin_url,
                checkin_mode.as_str(),
                if auto_checkin_enabled { 1 } else { 0 },
                auto_checkin_time,
                low_balance_alert_threshold,
                sort_order,
                ts,
                ts,
            ],
        )?;

        Ok(NewApiAccount {
            id,
            base_url,
            user_id,
            user_token: user_token.clone(),
            user_token_configured: user_token.is_some(),
            page_checkin_url,
            checkin_mode,
            auto_checkin_enabled,
            auto_checkin_time,
            low_balance_alert_threshold,
            remote_role: None,
            remote_username: None,
            remote_display_name: None,
            remote_group: None,
            quota_display_type: "USD".to_string(),
            quota_per_unit: 500_000.0,
            usd_exchange_rate: 1.0,
            custom_currency_symbol: None,
            custom_currency_exchange_rate: 1.0,
            remote_checkin_enabled: false,
            remote_turnstile_check_enabled: false,
            last_quota: None,
            last_used_quota: None,
            last_balance_amount: None,
            last_sync_error: None,
            last_synced_at_ms: None,
            low_balance_alert_notified: false,
            last_balance_alert_at_ms: None,
            created_at_ms: ts,
            updated_at_ms: ts,
        })
    })
    .await
}

pub async fn update_newapi_account(
    db_path: PathBuf,
    account_id: String,
    input: UpdateNewApiAccount,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let mut account = get_account_row(conn, &account_id)?
            .ok_or_else(|| StorageError::NewApiAccountNotFound {
                account_id: account_id.clone(),
            })?;

        if let Some(value) = input.base_url {
            account.base_url = normalize_newapi_base_url(&value);
        }
        if let Some(value) = input.user_id {
            account.user_id = value.trim().to_string();
        }
        if let Some(value) = input.user_token {
            account.user_token = normalize_optional_text(Some(value));
        }
        if input.page_checkin_url.is_some() {
            account.page_checkin_url = normalize_optional_text(input.page_checkin_url);
        }
        if let Some(value) = input.checkin_mode {
            account.checkin_mode = value;
            if matches!(account.checkin_mode, NewApiAccountCheckinMode::PageOpen) {
                account.auto_checkin_enabled = false;
            }
        }
        if let Some(value) = input.auto_checkin_enabled {
            account.auto_checkin_enabled =
                value && matches!(account.checkin_mode, NewApiAccountCheckinMode::SystemApi);
        }
        if let Some(value) = input.auto_checkin_time {
            account.auto_checkin_time = normalize_auto_checkin_time(Some(value))?;
        }
        if let Some(value) = input.low_balance_alert_threshold {
            account.low_balance_alert_threshold = value;
        }
        if matches!(account.checkin_mode, NewApiAccountCheckinMode::PageOpen) {
            account.auto_checkin_enabled = false;
        }

        ensure_unique_account(conn, &account.base_url, &account.user_id, Some(&account.id))?;

        conn.execute(
            r#"
            UPDATE newapi_accounts
            SET base_url = ?2, user_id = ?3, user_token = ?4, page_checkin_url = ?5, checkin_mode = ?6,
                auto_checkin_enabled = ?7, auto_checkin_time = ?8, low_balance_alert_threshold = ?9, updated_at_ms = ?10
            WHERE id = ?1
            "#,
            params![
                account.id,
                account.base_url,
                account.user_id,
                account.user_token.as_deref().unwrap_or(""),
                account.page_checkin_url,
                account.checkin_mode.as_str(),
                if account.auto_checkin_enabled { 1 } else { 0 },
                account.auto_checkin_time,
                account.low_balance_alert_threshold,
                ts,
            ],
        )?;
        Ok(())
    })
    .await
}

fn ensure_unique_account(
    conn: &rusqlite::Connection,
    base_url: &str,
    user_id: &str,
    exclude_id: Option<&str>,
) -> anyhow::Result<()> {
    if user_id.trim().is_empty() {
        return Ok(());
    }
    let existing: Option<String> = if let Some(exclude_id) = exclude_id {
        conn.query_row(
            r#"SELECT id FROM newapi_accounts WHERE base_url = ?1 AND user_id = ?2 AND id <> ?3"#,
            params![base_url, user_id, exclude_id],
            |row| row.get(0),
        )
        .optional()?
    } else {
        conn.query_row(
            r#"SELECT id FROM newapi_accounts WHERE base_url = ?1 AND user_id = ?2"#,
            params![base_url, user_id],
            |row| row.get(0),
        )
        .optional()?
    };
    if existing.is_some() {
        return Err(StorageError::NewApiAccountAlreadyExists {
            base_url: base_url.to_string(),
            user_id: user_id.to_string(),
        }
        .into());
    }
    Ok(())
}

fn get_account_row(
    conn: &rusqlite::Connection,
    account_id: &str,
) -> anyhow::Result<Option<NewApiAccount>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, base_url, user_id, user_token, page_checkin_url, checkin_mode, auto_checkin_enabled, auto_checkin_time,
               low_balance_alert_threshold, remote_role, remote_username, remote_display_name, remote_group,
               quota_display_type, quota_per_unit, usd_exchange_rate, custom_currency_symbol, custom_currency_exchange_rate,
               remote_checkin_enabled, remote_turnstile_check_enabled, last_quota, last_used_quota, last_balance_amount,
               last_sync_error, last_synced_at_ms, low_balance_alert_notified, last_balance_alert_at_ms, created_at_ms, updated_at_ms
        FROM newapi_accounts
        WHERE id = ?1
        "#,
    )?;
    stmt.query_row([account_id], |row| account_from_row(row, true))
        .optional()
        .map_err(Into::into)
}

pub async fn reorder_newapi_accounts(
    db_path: PathBuf,
    account_ids: Vec<String>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id
            FROM newapi_accounts
            ORDER BY sort_order ASC, created_at_ms ASC
            "#,
        )?;
        let existing_ids = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        if existing_ids.len() != account_ids.len() {
            return Err(StorageError::NewApiAccountReorderMismatch {
                reason: "account_ids must cover all accounts",
            }
            .into());
        }

        let existing_set = existing_ids
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let input_set = account_ids
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        if existing_set != input_set {
            return Err(StorageError::NewApiAccountReorderMismatch {
                reason: "account_ids must match all accounts",
            }
            .into());
        }

        let tx = conn.unchecked_transaction()?;
        for (index, account_id) in account_ids.iter().enumerate() {
            tx.execute(
                r#"UPDATE newapi_accounts SET sort_order = ?2, updated_at_ms = ?3 WHERE id = ?1"#,
                params![account_id, index as i64, now_ms()],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn update_newapi_account_remote_snapshot(
    db_path: PathBuf,
    account_id: String,
    snapshot: NewApiAccountRemoteSnapshot,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let current = get_account_row(conn, &account_id)?.ok_or_else(|| {
            StorageError::NewApiAccountNotFound {
                account_id: account_id.clone(),
            }
        })?;
        let remote_role = if snapshot.replace_remote_state {
            snapshot.remote_role
        } else {
            snapshot.remote_role.or(current.remote_role)
        };
        let remote_username = if snapshot.replace_remote_state {
            snapshot.remote_username
        } else {
            snapshot.remote_username.or(current.remote_username)
        };
        let remote_display_name = if snapshot.replace_remote_state {
            snapshot.remote_display_name
        } else {
            snapshot.remote_display_name.or(current.remote_display_name)
        };
        let remote_group = if snapshot.replace_remote_state {
            snapshot.remote_group
        } else {
            snapshot.remote_group.or(current.remote_group)
        };
        let quota_display_type = if snapshot.replace_remote_state {
            snapshot
                .quota_display_type
                .unwrap_or_else(|| "USD".to_string())
        } else {
            snapshot
                .quota_display_type
                .unwrap_or(current.quota_display_type)
        };
        let quota_per_unit = if snapshot.replace_remote_state {
            snapshot.quota_per_unit.unwrap_or(500_000.0)
        } else {
            snapshot.quota_per_unit.unwrap_or(current.quota_per_unit)
        };
        let usd_exchange_rate = if snapshot.replace_remote_state {
            snapshot.usd_exchange_rate.unwrap_or(1.0)
        } else {
            snapshot
                .usd_exchange_rate
                .unwrap_or(current.usd_exchange_rate)
        };
        let custom_currency_symbol = if snapshot.replace_remote_state {
            snapshot.custom_currency_symbol
        } else {
            snapshot
                .custom_currency_symbol
                .or(current.custom_currency_symbol)
        };
        let custom_currency_exchange_rate = if snapshot.replace_remote_state {
            snapshot.custom_currency_exchange_rate.unwrap_or(1.0)
        } else {
            snapshot
                .custom_currency_exchange_rate
                .unwrap_or(current.custom_currency_exchange_rate)
        };
        let remote_checkin_enabled = if snapshot.replace_remote_state {
            snapshot.remote_checkin_enabled.unwrap_or(false)
        } else {
            snapshot
                .remote_checkin_enabled
                .unwrap_or(current.remote_checkin_enabled)
        };
        let remote_turnstile_check_enabled = if snapshot.replace_remote_state {
            snapshot.remote_turnstile_check_enabled.unwrap_or(false)
        } else {
            snapshot
                .remote_turnstile_check_enabled
                .unwrap_or(current.remote_turnstile_check_enabled)
        };
        let last_quota = if snapshot.replace_remote_state {
            snapshot.last_quota
        } else {
            snapshot.last_quota.or(current.last_quota)
        };
        let last_used_quota = if snapshot.replace_remote_state {
            snapshot.last_used_quota
        } else {
            snapshot.last_used_quota.or(current.last_used_quota)
        };
        let last_balance_amount = if snapshot.replace_remote_state {
            snapshot.last_balance_amount
        } else {
            snapshot.last_balance_amount.or(current.last_balance_amount)
        };
        let last_sync_error = if snapshot.replace_remote_state {
            snapshot.last_sync_error
        } else if snapshot.last_synced_at_ms.is_some() && snapshot.last_sync_error.is_none() {
            None
        } else {
            snapshot.last_sync_error.or(current.last_sync_error)
        };
        let last_synced_at_ms = if snapshot.replace_remote_state {
            snapshot.last_synced_at_ms
        } else {
            snapshot.last_synced_at_ms.or(current.last_synced_at_ms)
        };
        conn.execute(
            r#"
            UPDATE newapi_accounts
            SET remote_role = ?2,
                remote_username = ?3,
                remote_display_name = ?4,
                remote_group = ?5,
                quota_display_type = ?6,
                quota_per_unit = ?7,
                usd_exchange_rate = ?8,
                custom_currency_symbol = ?9,
                custom_currency_exchange_rate = ?10,
                remote_checkin_enabled = ?11,
                remote_turnstile_check_enabled = ?12,
                last_quota = ?13,
                last_used_quota = ?14,
                last_balance_amount = ?15,
                last_sync_error = ?16,
                last_synced_at_ms = ?17,
                updated_at_ms = ?18
            WHERE id = ?1
            "#,
            params![
                account_id,
                remote_role,
                remote_username,
                remote_display_name,
                remote_group,
                quota_display_type,
                quota_per_unit,
                usd_exchange_rate,
                custom_currency_symbol,
                custom_currency_exchange_rate,
                if remote_checkin_enabled { 1 } else { 0 },
                if remote_turnstile_check_enabled { 1 } else { 0 },
                last_quota,
                last_used_quota,
                last_balance_amount,
                last_sync_error,
                last_synced_at_ms,
                now_ms(),
            ],
        )?;
        Ok(())
    })
    .await
}

pub async fn set_newapi_account_balance_alert_notified(
    db_path: PathBuf,
    account_id: String,
    notified: bool,
    last_balance_alert_at_ms: Option<i64>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let updated = conn.execute(
            r#"
            UPDATE newapi_accounts
            SET low_balance_alert_notified = ?2, last_balance_alert_at_ms = ?3, updated_at_ms = ?4
            WHERE id = ?1
            "#,
            params![
                account_id,
                if notified { 1 } else { 0 },
                last_balance_alert_at_ms,
                now_ms(),
            ],
        )?;
        if updated == 0 {
            return Err(StorageError::NewApiAccountNotFound {
                account_id: account_id.clone(),
            }
            .into());
        }
        Ok(())
    })
    .await
}

pub async fn get_newapi_accounts_checkins_today(
    db_path: PathBuf,
) -> anyhow::Result<NewApiAccountCheckinsToday> {
    let date = local_today_ymd()?;
    let completed_account_ids = with_conn(db_path, {
        let date = date.clone();
        move |conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT account_id
                FROM newapi_account_checkins
                WHERE date = ?1
                "#,
            )?;
            let mut rows = stmt.query([date])?;
            let mut ids = Vec::new();
            while let Some(row) = rows.next()? {
                ids.push(row.get::<_, String>(0)?);
            }
            Ok(ids)
        }
    })
    .await?;

    Ok(NewApiAccountCheckinsToday {
        date,
        completed_account_ids,
    })
}

pub async fn complete_newapi_account_checkin_today(
    db_path: PathBuf,
    account_id: String,
    method: &str,
) -> anyhow::Result<()> {
    let date = local_today_ymd()?;
    let ts = now_ms();
    let method = method.to_string();
    with_conn(db_path, move |conn| {
        let exists: Option<i64> = conn
            .query_row(
                r#"SELECT 1 FROM newapi_accounts WHERE id = ?1"#,
                params![account_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StorageError::NewApiAccountNotFound {
                account_id: account_id.clone(),
            }
            .into());
        }
        conn.execute(
            r#"
            INSERT INTO newapi_account_checkins (account_id, date, method, completed_at_ms)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(account_id, date) DO UPDATE SET
              method = excluded.method,
              completed_at_ms = excluded.completed_at_ms
            "#,
            params![account_id, date, method, ts],
        )?;
        Ok(())
    })
    .await
}

pub async fn list_channels_by_newapi_account(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Vec<String>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id
            FROM channels
            WHERE newapi_account_id = ?1
            ORDER BY created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([account_id], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn detach_channels_from_newapi_account(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Vec<String>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id
            FROM channels
            WHERE newapi_account_id = ?1
            ORDER BY created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([account_id.clone()], |row| row.get::<_, String>(0))?;
        let ids = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        conn.execute(
            r#"
            UPDATE channels
            SET managed_by_newapi = 0,
                newapi_account_id = NULL,
                newapi_channel_id = NULL,
                newapi_token_id = NULL,
                newapi_token_name = NULL,
                newapi_group = NULL,
                updated_at_ms = ?2
            WHERE newapi_account_id = ?1
            "#,
            params![account_id, now_ms()],
        )?;
        Ok(ids)
    })
    .await
}

pub async fn delete_newapi_account(
    db_path: PathBuf,
    account_id: String,
    delete_managed_channels: bool,
) -> anyhow::Result<DeleteNewApiAccountResult> {
    with_conn(db_path, move |conn| {
        let exists: Option<i64> = conn
            .query_row(
                r#"SELECT 1 FROM newapi_accounts WHERE id = ?1"#,
                params![account_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StorageError::NewApiAccountNotFound {
                account_id: account_id.clone(),
            }
            .into());
        }

        let mut stmt = conn.prepare(
            r#"
            SELECT id
            FROM channels
            WHERE newapi_account_id = ?1
            ORDER BY created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([account_id.clone()], |row| row.get::<_, String>(0))?;
        let linked_channel_ids = rows.collect::<rusqlite::Result<Vec<_>>>()?;

        let tx = conn.unchecked_transaction()?;
        let result = if delete_managed_channels {
            for channel_id in &linked_channel_ids {
                tx.execute(
                    r#"DELETE FROM route_channels WHERE channel_id = ?1"#,
                    params![channel_id],
                )?;
                tx.execute(r#"DELETE FROM channels WHERE id = ?1"#, params![channel_id])?;
            }
            DeleteNewApiAccountResult {
                deleted_managed_channel_ids: linked_channel_ids,
                detached_channel_ids: Vec::new(),
            }
        } else {
            tx.execute(
                r#"
                UPDATE channels
                SET managed_by_newapi = 0,
                    newapi_account_id = NULL,
                    newapi_channel_id = NULL,
                    newapi_token_id = NULL,
                    newapi_token_name = NULL,
                    newapi_group = NULL,
                    updated_at_ms = ?2
                WHERE newapi_account_id = ?1
                "#,
                params![account_id.clone(), now_ms()],
            )?;
            DeleteNewApiAccountResult {
                deleted_managed_channel_ids: Vec::new(),
                detached_channel_ids: linked_channel_ids,
            }
        };

        tx.execute(
            r#"DELETE FROM newapi_account_checkins WHERE account_id = ?1"#,
            params![account_id.clone()],
        )?;
        tx.execute(
            r#"DELETE FROM newapi_accounts WHERE id = ?1"#,
            params![account_id],
        )?;
        tx.commit()?;
        Ok(result)
    })
    .await
}
