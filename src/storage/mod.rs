use anyhow::Context as _;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension as _, params};
use serde::Serialize;
use std::path::{Path, PathBuf};

const SQLITE_BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

mod channel;
mod chat_bridge;
mod checkin;
mod error;
mod newapi;
mod pricing;
mod prompt;
mod protocol;
mod remote_account;
mod settings;
mod stats;
mod update_ignore;
mod usage;

pub use error::StorageError;

pub use channel::{
    Channel, CreateChannel, ManagedRemoteProvider, RechargeCurrency, UpdateChannel,
    channel_is_auto_disabled, clear_channel_failures, create_channel, delete_channel,
    detach_channels_from_managed_account, get_channel, list_channel_ids_by_managed_account,
    list_channels, record_channel_failure_and_maybe_disable, reorder_channels, set_channel_enabled,
    update_channel,
};
pub use chat_bridge::{
    BridgeKnownProject, BridgePermissionMode, BridgeSession, BridgeSessionStatus,
    ChatBridgeBinding, ChatBridgePairingToken, ChatPlatform, CreateBridgeSessionInput,
    CreatePairingTokenInput, DEFAULT_PAIRING_TOKEN_EXPIRES_MINUTES,
    MAX_PAIRING_TOKEN_EXPIRES_MINUTES, UpdateBridgeSessionInput, consume_pairing_token,
    count_active_bridge_sessions_for_platform, create_bridge_session, create_pairing_token,
    deactivate_chat_binding, get_bridge_session, get_default_bridge_session_for_platform,
    list_bridge_known_projects, list_bridge_sessions_for_platform, list_chat_bindings,
    resolve_chat_binding, set_default_bridge_session_for_platform,
    stop_all_bridge_sessions_for_platform, stop_bridge_session, update_bridge_session,
    upsert_bridge_known_project,
};
pub use checkin::{
    ChannelCheckinsToday, complete_channel_checkin_today, get_channel_checkins_today,
};
pub use newapi::{
    CreateNewApiAccount, DeleteNewApiAccountResult, NewApiAccount, NewApiAccountCheckinMode,
    NewApiAccountCheckinsToday, NewApiAccountRemoteSnapshot, UpdateNewApiAccount,
    assign_newapi_account_sort_orders, complete_newapi_account_checkin_today,
    create_newapi_account, delete_newapi_account, detach_channels_from_newapi_account,
    get_newapi_account, get_newapi_account_for_secret_use, get_newapi_account_with_secret,
    get_newapi_account_with_secret_optional, get_newapi_account_without_secret,
    get_newapi_account_without_secret_optional, get_newapi_accounts_checkins_today,
    list_channels_by_newapi_account, list_newapi_accounts, list_newapi_accounts_with_secret,
    reorder_newapi_accounts, set_newapi_account_balance_alert_notified, update_newapi_account,
    update_newapi_account_remote_snapshot,
};
pub use pricing::{
    PricingModel, PricingStatus, UpsertPricingModel, pricing_status, search_pricing_models,
    upsert_pricing_models,
};
pub use prompt::{
    DeletePromptDocument, PromptDocument, PromptProject, PromptScope, SavePromptDocument,
    delete_prompt_document, delete_prompt_project, get_prompt_document, list_prompt_projects,
    save_prompt_document,
};
pub use protocol::Protocol;
pub(crate) use protocol::normalize_base_url;
pub use remote_account::{
    CreateRemoteAccount, RemoteAccount, RemoteAccountCheckinMode, RemoteAccountCheckinsToday,
    RemoteAccountProvider, RemoteAccountRemoteSnapshot, UpdateRemoteAccount,
    apply_remote_account_sync_failure, apply_remote_account_sync_success,
    assign_remote_account_sort_orders, complete_remote_account_checkin_today,
    create_remote_account, delete_remote_account, get_remote_account_with_secret,
    get_remote_account_with_secret_optional, get_remote_account_without_secret,
    get_remote_account_without_secret_optional, get_remote_accounts_checkins_today,
    list_remote_accounts, list_remote_accounts_with_secret,
    set_remote_account_balance_alert_notified, update_remote_account,
    update_remote_account_auth_session,
};
pub use settings::{
    AppSettings, AppSettingsPatch, AutoStartLaunchMode, CloseBehavior, get_app_settings,
    update_app_settings,
};
pub use stats::{
    ChannelStats, StatsSummary, TrendPoint, stats_channels, stats_summary,
    stats_trend_by_day_channel,
};
pub use update_ignore::{ignore_update_version, is_update_version_ignored};
pub use usage::{
    CreateUsageEvent, UsageEvent, UsageListQuery, UsageListResult, backfill_usage_event_costs,
    insert_usage_event, list_usage_events, list_usage_events_recent,
};

fn open_conn(db_path: &Path) -> anyhow::Result<Connection> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("打开 SQLite 文件失败：{}", db_path.display()))?;
    conn.busy_timeout(SQLITE_BUSY_TIMEOUT)
        .with_context(|| "设置 SQLite busy_timeout 失败")?;
    Ok(conn)
}

pub fn init_db(db_path: &Path) -> anyhow::Result<()> {
    let conn = open_conn(db_path)?;

    let migration = include_str!("../../migrations/001_init.sql");
    conn.execute_batch(migration)
        .with_context(|| "执行 migrations/001_init.sql 失败")?;

    Ok(())
}

#[derive(Debug, Clone)]
pub enum UnifiedRemoteAccount {
    Newapi(NewApiAccount),
    Sub2Api(RemoteAccount),
}

impl UnifiedRemoteAccount {
    pub fn id(&self) -> &str {
        match self {
            Self::Newapi(account) => &account.id,
            Self::Sub2Api(account) => &account.id,
        }
    }

    pub fn managed_provider(&self) -> ManagedRemoteProvider {
        match self {
            Self::Newapi(_) => ManagedRemoteProvider::Newapi,
            Self::Sub2Api(_) => ManagedRemoteProvider::Sub2Api,
        }
    }
}

const UNIFIED_REMOTE_ACCOUNT_SELECT_COLUMNS: &str = r#"
    id, provider, base_url, api_url, user_id, user_token, access_token, refresh_token,
    page_checkin_url, checkin_mode, auto_checkin_enabled, auto_checkin_time,
    low_balance_alert_threshold, recharge_currency, remote_user_id, remote_role,
    remote_username, remote_display_name, remote_group, quota_display_type, quota_per_unit,
    usd_exchange_rate, custom_currency_symbol, custom_currency_exchange_rate,
    remote_checkin_enabled, remote_turnstile_check_enabled, last_quota, last_used_quota,
    last_balance_amount, last_sync_error, reauth_required, last_synced_at_ms,
    low_balance_alert_notified, last_balance_alert_at_ms, sort_order, created_at_ms, updated_at_ms
"#;

#[derive(Debug, Clone)]
struct UnifiedRemoteAccountRow {
    id: String,
    provider: ManagedRemoteProvider,
    base_url: String,
    api_url: Option<String>,
    user_id: String,
    user_token_raw: String,
    access_token_raw: String,
    refresh_token_raw: String,
    page_checkin_url: Option<String>,
    checkin_mode: String,
    auto_checkin_enabled: bool,
    auto_checkin_time: String,
    low_balance_alert_threshold: f64,
    recharge_currency: RechargeCurrency,
    remote_user_id: Option<String>,
    remote_role: Value,
    remote_username: Option<String>,
    remote_display_name: Option<String>,
    remote_group: Option<String>,
    quota_display_type: String,
    quota_per_unit: f64,
    usd_exchange_rate: f64,
    custom_currency_symbol: Option<String>,
    custom_currency_exchange_rate: f64,
    remote_checkin_enabled: bool,
    remote_turnstile_check_enabled: bool,
    last_quota: Option<i64>,
    last_used_quota: Option<i64>,
    last_balance_amount: Option<f64>,
    last_sync_error: Option<String>,
    reauth_required: bool,
    last_synced_at_ms: Option<i64>,
    low_balance_alert_notified: bool,
    last_balance_alert_at_ms: Option<i64>,
    sort_order: i64,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl UnifiedRemoteAccountRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            provider: row.get(1)?,
            base_url: row.get(2)?,
            api_url: row.get(3)?,
            user_id: row.get(4)?,
            user_token_raw: row.get(5)?,
            access_token_raw: row.get(6)?,
            refresh_token_raw: row.get(7)?,
            page_checkin_url: row.get(8)?,
            checkin_mode: row.get(9)?,
            auto_checkin_enabled: row.get::<_, i64>(10)? != 0,
            auto_checkin_time: row.get(11)?,
            low_balance_alert_threshold: row.get(12)?,
            recharge_currency: row.get(13)?,
            remote_user_id: row.get(14)?,
            remote_role: row.get(15)?,
            remote_username: row.get(16)?,
            remote_display_name: row.get(17)?,
            remote_group: row.get(18)?,
            quota_display_type: row.get(19)?,
            quota_per_unit: row.get(20)?,
            usd_exchange_rate: row.get(21)?,
            custom_currency_symbol: row.get(22)?,
            custom_currency_exchange_rate: row.get(23)?,
            remote_checkin_enabled: row.get::<_, i64>(24)? != 0,
            remote_turnstile_check_enabled: row.get::<_, i64>(25)? != 0,
            last_quota: row.get(26)?,
            last_used_quota: row.get(27)?,
            last_balance_amount: row.get(28)?,
            last_sync_error: row.get(29)?,
            reauth_required: row.get::<_, i64>(30)? != 0,
            last_synced_at_ms: row.get(31)?,
            low_balance_alert_notified: row.get::<_, i64>(32)? != 0,
            last_balance_alert_at_ms: row.get(33)?,
            sort_order: row.get(34)?,
            created_at_ms: row.get(35)?,
            updated_at_ms: row.get(36)?,
        })
    }

    fn into_account(self, include_secret: bool) -> anyhow::Result<UnifiedRemoteAccount> {
        match self.provider {
            ManagedRemoteProvider::Newapi => self
                .into_newapi_account(include_secret)
                .map(UnifiedRemoteAccount::Newapi),
            ManagedRemoteProvider::Sub2Api => self
                .into_sub2api_account(include_secret)
                .map(UnifiedRemoteAccount::Sub2Api),
        }
    }

    fn into_newapi_account(self, include_secret: bool) -> anyhow::Result<NewApiAccount> {
        let user_token_configured = token_configured(&self.user_token_raw);
        let user_token = include_secret
            .then_some(self.user_token_raw)
            .filter(|value| token_configured(value));
        Ok(NewApiAccount {
            id: self.id,
            base_url: self.base_url,
            api_url: self.api_url,
            user_id: self.user_id,
            user_token,
            user_token_configured,
            page_checkin_url: self.page_checkin_url,
            checkin_mode: self.checkin_mode.parse()?,
            auto_checkin_enabled: self.auto_checkin_enabled,
            auto_checkin_time: self.auto_checkin_time,
            low_balance_alert_threshold: self.low_balance_alert_threshold,
            recharge_currency: self.recharge_currency,
            remote_role: decode_newapi_remote_role(self.remote_role)?,
            remote_username: self.remote_username,
            remote_display_name: self.remote_display_name,
            remote_group: self.remote_group,
            quota_display_type: self.quota_display_type,
            quota_per_unit: self.quota_per_unit,
            usd_exchange_rate: self.usd_exchange_rate,
            custom_currency_symbol: self.custom_currency_symbol,
            custom_currency_exchange_rate: self.custom_currency_exchange_rate,
            remote_checkin_enabled: self.remote_checkin_enabled,
            remote_turnstile_check_enabled: self.remote_turnstile_check_enabled,
            last_quota: self.last_quota,
            last_used_quota: self.last_used_quota,
            last_balance_amount: self.last_balance_amount,
            last_sync_error: self.last_sync_error,
            last_synced_at_ms: self.last_synced_at_ms,
            low_balance_alert_notified: self.low_balance_alert_notified,
            last_balance_alert_at_ms: self.last_balance_alert_at_ms,
            sort_order: self.sort_order,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        })
    }

    fn into_sub2api_account(self, include_secret: bool) -> anyhow::Result<RemoteAccount> {
        let access_token_configured = token_configured(&self.access_token_raw);
        let refresh_token_configured = token_configured(&self.refresh_token_raw);
        let access_token = include_secret
            .then_some(self.access_token_raw)
            .filter(|value| token_configured(value));
        let refresh_token = include_secret
            .then_some(self.refresh_token_raw)
            .filter(|value| token_configured(value));
        Ok(RemoteAccount {
            id: self.id,
            provider: RemoteAccountProvider::Sub2Api,
            base_url: self.base_url,
            api_url: self.api_url,
            access_token,
            refresh_token: refresh_token.filter(|_| refresh_token_configured),
            access_token_configured,
            page_checkin_url: self.page_checkin_url,
            checkin_mode: self.checkin_mode.parse()?,
            auto_checkin_time: self.auto_checkin_time,
            low_balance_alert_threshold: self.low_balance_alert_threshold,
            recharge_currency: self.recharge_currency,
            remote_user_id: self.remote_user_id,
            remote_role: decode_sub2api_remote_role(self.remote_role)?,
            remote_username: self.remote_username,
            remote_display_name: self.remote_display_name,
            last_balance_amount: self.last_balance_amount,
            last_sync_error: self.last_sync_error,
            reauth_required: self.reauth_required,
            last_synced_at_ms: self.last_synced_at_ms,
            low_balance_alert_notified: self.low_balance_alert_notified,
            last_balance_alert_at_ms: self.last_balance_alert_at_ms,
            sort_order: self.sort_order,
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        })
    }
}

fn token_configured(raw: &str) -> bool {
    !raw.trim().is_empty()
}

fn decode_newapi_remote_role(value: Value) -> anyhow::Result<Option<i64>> {
    match value {
        Value::Null => Ok(None),
        Value::Integer(value) => Ok(Some(value)),
        Value::Real(value) if value.is_finite() && value.fract() == 0.0 => Ok(Some(value as i64)),
        Value::Text(value) => value
            .parse::<i64>()
            .map(Some)
            .with_context(|| format!("invalid newapi remote_role value: {value}")),
        Value::Real(value) => Err(anyhow::anyhow!(
            "invalid newapi remote_role value: non-integer real {value}"
        )),
        Value::Blob(_) => Err(anyhow::anyhow!("invalid newapi remote_role value: blob")),
    }
}

fn decode_sub2api_remote_role(value: Value) -> anyhow::Result<Option<String>> {
    match value {
        Value::Null => Ok(None),
        Value::Integer(value) => Ok(Some(value.to_string())),
        Value::Real(value) => Ok(Some(value.to_string())),
        Value::Text(value) => Ok(Some(value)),
        Value::Blob(_) => Err(anyhow::anyhow!("invalid sub2api remote_role value: blob")),
    }
}

fn local_today_ymd() -> anyhow::Result<String> {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let today = time::OffsetDateTime::now_utc().to_offset(offset).date();
    let fmt = time::format_description::parse("[year]-[month]-[day]")?;
    Ok(today.format(&fmt)?)
}

async fn list_all_remote_accounts_impl(
    db_path: PathBuf,
    include_secret: bool,
) -> anyhow::Result<Vec<UnifiedRemoteAccount>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(&format!(
            r#"
            SELECT {UNIFIED_REMOTE_ACCOUNT_SELECT_COLUMNS}
            FROM remote_accounts
            ORDER BY sort_order ASC, created_at_ms ASC
            "#
        ))?;
        let rows = stmt.query_map([], UnifiedRemoteAccountRow::from_row)?;
        rows.map(|row| row.map_err(anyhow::Error::from))
            .map(|row| row.and_then(|row| row.into_account(include_secret)))
            .collect()
    })
    .await
}

async fn get_unified_remote_account_optional_impl(
    db_path: PathBuf,
    account_id: String,
    include_secret: bool,
) -> anyhow::Result<Option<UnifiedRemoteAccount>> {
    with_conn(db_path, move |conn| {
        let row = conn
            .query_row(
                &format!(
                    r#"
                    SELECT {UNIFIED_REMOTE_ACCOUNT_SELECT_COLUMNS}
                    FROM remote_accounts
                    WHERE id = ?1
                    "#
                ),
                [&account_id],
                UnifiedRemoteAccountRow::from_row,
            )
            .optional()?;
        row.map(|row| row.into_account(include_secret)).transpose()
    })
    .await
}

pub async fn list_all_remote_accounts(
    db_path: PathBuf,
) -> anyhow::Result<Vec<UnifiedRemoteAccount>> {
    list_all_remote_accounts_impl(db_path, false).await
}

pub async fn list_all_remote_accounts_with_secret(
    db_path: PathBuf,
) -> anyhow::Result<Vec<UnifiedRemoteAccount>> {
    list_all_remote_accounts_impl(db_path, true).await
}

pub async fn get_unified_remote_account_without_secret_optional(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Option<UnifiedRemoteAccount>> {
    get_unified_remote_account_optional_impl(db_path, account_id, false).await
}

pub async fn get_unified_remote_account_with_secret_optional(
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<Option<UnifiedRemoteAccount>> {
    get_unified_remote_account_optional_impl(db_path, account_id, true).await
}

pub async fn get_all_remote_accounts_checkins_today(
    db_path: PathBuf,
) -> anyhow::Result<RemoteAccountCheckinsToday> {
    let date = local_today_ymd()?;
    let completed_account_ids = with_conn(db_path, {
        let date = date.clone();
        move |conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT c.account_id
                FROM remote_account_checkins c
                INNER JOIN remote_accounts a ON a.id = c.account_id
                WHERE c.date = ?1
                ORDER BY c.completed_at_ms ASC
                "#,
            )?;
            stmt.query_map([date], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into)
        }
    })
    .await?;
    Ok(RemoteAccountCheckinsToday {
        date,
        completed_account_ids,
    })
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or_else(|_| {
            tracing::error!("system time is too far in the future (ms overflow)");
            i64::MAX
        }),
        Err(e) => {
            tracing::error!(err = %e, "system time is before unix epoch");
            0
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RecordsClearKind {
    DateRange { start_ms: i64, end_ms: i64 },
    ErrorsDateRange { start_ms: i64, end_ms: i64 },
    Errors,
    All,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClearRecordsResult {
    pub usage_events_deleted: i64,
    pub vacuumed: bool,
}

pub async fn clear_records(
    db_path: PathBuf,
    kind: RecordsClearKind,
) -> anyhow::Result<ClearRecordsResult> {
    with_conn(db_path, move |conn| {
        let usage_events_deleted: i64 = match kind {
            RecordsClearKind::DateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM usage_events WHERE ts_ms >= ?1 AND ts_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .context("转换删除行数失败")?,
            RecordsClearKind::ErrorsDateRange { start_ms, end_ms } => conn
                .execute(
                    r#"DELETE FROM usage_events WHERE success = 0 AND ts_ms >= ?1 AND ts_ms <= ?2"#,
                    params![start_ms, end_ms],
                )?
                .try_into()
                .context("转换删除行数失败")?,
            RecordsClearKind::Errors => conn
                .execute(r#"DELETE FROM usage_events WHERE success = 0"#, [])?
                .try_into()
                .context("转换删除行数失败")?,
            RecordsClearKind::All => conn
                .execute(r#"DELETE FROM usage_events"#, [])?
                .try_into()
                .context("转换删除行数失败")?,
        };

        let vacuumed = matches!(kind, RecordsClearKind::Errors | RecordsClearKind::All);
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        if vacuumed {
            conn.execute_batch("VACUUM;")?;
        }

        Ok(ClearRecordsResult {
            usage_events_deleted,
            vacuumed,
        })
    })
    .await
}

async fn with_conn<T, F>(db_path: PathBuf, f: F) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> anyhow::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let conn = open_conn(&db_path)?;
        f(&conn)
    })
    .await
    .context("等待 sqlite blocking 任务失败")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn remove_sqlite_artifacts(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    }

    fn temp_db_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-test-unified-remote-accounts-{}.sqlite",
            uuid::Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn unified_remote_queries_decode_both_provider_shapes_from_single_table() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        init_db(&db_path).unwrap();

        let newapi = create_newapi_account(
            db_path.clone(),
            CreateNewApiAccount {
                base_url: "https://newapi.example.com".to_string(),
                api_url: None,
                user_id: "demo-user".to_string(),
                user_token: "demo-user-token".to_string(),
                page_checkin_url: Some("https://newapi.example.com/user/checkin".to_string()),
                checkin_mode: Some(NewApiAccountCheckinMode::SystemApi),
                auto_checkin_enabled: Some(true),
                auto_checkin_time: Some("06:30:00".to_string()),
                low_balance_alert_threshold: Some(5.0),
                recharge_currency: Some(RechargeCurrency::Cny),
            },
        )
        .await
        .unwrap();
        update_newapi_account_remote_snapshot(
            db_path.clone(),
            newapi.id.clone(),
            NewApiAccountRemoteSnapshot {
                remote_role: Some(100),
                remote_username: Some("newapi-user".to_string()),
                remote_display_name: Some("NewAPI User".to_string()),
                remote_group: Some("default".to_string()),
                last_balance_amount: Some(12.5),
                last_synced_at_ms: Some(111),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let remote = create_remote_account(
            db_path.clone(),
            CreateRemoteAccount {
                provider: RemoteAccountProvider::Sub2Api,
                base_url: "https://sub2api.example.com".to_string(),
                api_url: Some("https://sub2api.example.com/v1".to_string()),
                access_token: "Bearer remote-token".to_string(),
                refresh_token: Some("Bearer remote-refresh".to_string()),
                page_checkin_url: Some("https://sub2api.example.com/dashboard".to_string()),
                checkin_mode: Some(RemoteAccountCheckinMode::PageOpen),
                auto_checkin_time: Some("07:45:00".to_string()),
                low_balance_alert_threshold: Some(3.0),
                recharge_currency: Some(RechargeCurrency::Usd),
            },
        )
        .await
        .unwrap();
        apply_remote_account_sync_success(
            db_path.clone(),
            remote.id.clone(),
            RemoteAccountRemoteSnapshot {
                remote_user_id: Some("42".to_string()),
                remote_role: Some("admin".to_string()),
                remote_username: Some("sub2api-user".to_string()),
                remote_display_name: Some("Sub2API User".to_string()),
                last_balance_amount: Some(6.5),
                last_synced_at_ms: Some(222),
            },
        )
        .await
        .unwrap();

        complete_newapi_account_checkin_today(db_path.clone(), newapi.id.clone(), "system_api")
            .await
            .unwrap();
        complete_remote_account_checkin_today(db_path.clone(), remote.id.clone())
            .await
            .unwrap();

        let public_accounts = list_all_remote_accounts(db_path.clone()).await.unwrap();
        assert_eq!(public_accounts.len(), 2);
        assert!(
            public_accounts
                .iter()
                .any(|account| account.id() == newapi.id)
        );
        assert!(
            public_accounts
                .iter()
                .any(|account| account.id() == remote.id)
        );

        let secret_accounts = list_all_remote_accounts_with_secret(db_path.clone())
            .await
            .unwrap();
        assert_eq!(secret_accounts.len(), 2);

        let public_newapi = public_accounts
            .iter()
            .find_map(|account| match account {
                UnifiedRemoteAccount::Newapi(account) if account.id == newapi.id => Some(account),
                _ => None,
            })
            .unwrap();
        assert!(public_newapi.user_token.is_none());
        assert_eq!(public_newapi.remote_role, Some(100));
        assert_eq!(public_newapi.remote_group.as_deref(), Some("default"));

        let public_remote = public_accounts
            .iter()
            .find_map(|account| match account {
                UnifiedRemoteAccount::Sub2Api(account) if account.id == remote.id => Some(account),
                _ => None,
            })
            .unwrap();
        assert!(public_remote.access_token.is_none());
        assert_eq!(public_remote.provider, RemoteAccountProvider::Sub2Api);
        assert_eq!(public_remote.remote_role.as_deref(), Some("admin"));
        assert_eq!(public_remote.remote_user_id.as_deref(), Some("42"));

        let secret_newapi = secret_accounts
            .iter()
            .find_map(|account| match account {
                UnifiedRemoteAccount::Newapi(account) if account.id == newapi.id => Some(account),
                _ => None,
            })
            .unwrap();
        assert_eq!(secret_newapi.user_token.as_deref(), Some("demo-user-token"));

        let secret_remote =
            get_unified_remote_account_with_secret_optional(db_path.clone(), remote.id.clone())
                .await
                .unwrap()
                .unwrap();
        match secret_remote {
            UnifiedRemoteAccount::Sub2Api(account) => {
                assert_eq!(account.access_token.as_deref(), Some("remote-token"));
                assert_eq!(account.refresh_token.as_deref(), Some("remote-refresh"));
                assert_eq!(account.remote_role.as_deref(), Some("admin"));
            }
            UnifiedRemoteAccount::Newapi(_) => panic!("expected sub2api account"),
        }

        let secret_newapi_fetched =
            get_unified_remote_account_with_secret_optional(db_path.clone(), newapi.id.clone())
                .await
                .unwrap()
                .unwrap();
        match secret_newapi_fetched {
            UnifiedRemoteAccount::Newapi(account) => {
                assert_eq!(account.user_token.as_deref(), Some("demo-user-token"));
                assert_eq!(account.remote_role, Some(100));
            }
            UnifiedRemoteAccount::Sub2Api(_) => panic!("expected newapi account"),
        }

        let checkins = get_all_remote_accounts_checkins_today(db_path.clone())
            .await
            .unwrap();
        assert_eq!(checkins.completed_account_ids.len(), 2);
        assert!(checkins.completed_account_ids.contains(&newapi.id));
        assert!(checkins.completed_account_ids.contains(&remote.id));

        remove_sqlite_artifacts(&db_path);
    }
}
