use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::watch;
use tokio::time::Duration;

use crate::cli_tools::CLI_TOOLS;
use crate::events::{
    self, AppEvent, RemoteLowBalanceAlert, RemoteManagedChannelMissingPrompt,
    RemoteManagedChannelMultiplierPrompt,
};
use crate::{autostart, log_files, newapi, nodejs, storage, sub2api, update};

use super::handlers::pricing::run_pricing_sync;
use super::scheduler::{self, DailyLocalTimeTrigger, IntervalTrigger};
use super::state::data_dir_from_db_path;

const RETRY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LowBalanceAlertAction {
    None,
    MarkNotifiedSilently,
    PublishAndMarkNotified,
    ClearNotified,
}

fn decide_low_balance_alert_action(
    threshold: f64,
    last_balance_amount: Option<f64>,
    notified: bool,
    notifications_enabled: bool,
) -> LowBalanceAlertAction {
    if threshold <= 0.0 {
        return if notified {
            LowBalanceAlertAction::ClearNotified
        } else {
            LowBalanceAlertAction::None
        };
    }

    let Some(balance_amount) = last_balance_amount else {
        return LowBalanceAlertAction::None;
    };

    if balance_amount <= threshold {
        if notified {
            LowBalanceAlertAction::None
        } else if notifications_enabled {
            LowBalanceAlertAction::PublishAndMarkNotified
        } else {
            LowBalanceAlertAction::MarkNotifiedSilently
        }
    } else if notified {
        LowBalanceAlertAction::ClearNotified
    } else {
        LowBalanceAlertAction::None
    }
}

async fn wait_for_retry_or_shutdown(notify: &mut watch::Receiver<u64>) -> bool {
    !scheduler::wait_for_trigger(&IntervalTrigger::new(RETRY_INTERVAL), notify)
        .await
        .closed()
}

async fn wait_for_change_or_shutdown(notify: &mut watch::Receiver<u64>) -> bool {
    !scheduler::wait_for_change(notify).await.closed()
}

async fn wait_for_interval_or_shutdown(
    interval: Duration,
    notify: &mut watch::Receiver<u64>,
) -> bool {
    !scheduler::wait_for_trigger(&IntervalTrigger::new(interval), notify)
        .await
        .closed()
}

async fn wait_until_or_shutdown(
    deadline: time::OffsetDateTime,
    notify: &mut watch::Receiver<u64>,
) -> bool {
    !scheduler::wait_until(deadline, notify).await.closed()
}

pub(crate) async fn pricing_auto_update_loop(
    db_path: PathBuf,
    http_client: reqwest::Client,
    mut notify: watch::Receiver<u64>,
) {
    loop {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed");
                if !wait_for_retry_or_shutdown(&mut notify).await {
                    break;
                }
                continue;
            }
        };

        if !settings.pricing_auto_update_enabled {
            if !wait_for_change_or_shutdown(&mut notify).await {
                break;
            }
            continue;
        }

        let hours = settings.pricing_auto_update_interval_hours.clamp(1, 8760);
        if let Err(e) = run_pricing_sync(&http_client, db_path.clone()).await {
            tracing::warn!(err = %e, "pricing auto sync failed");
        }

        if !wait_for_interval_or_shutdown(Duration::from_secs((hours as u64) * 3600), &mut notify)
            .await
        {
            break;
        }
    }
}

pub(crate) async fn app_update_auto_loop(
    db_path: PathBuf,
    http_client: reqwest::Client,
    mut notify: watch::Receiver<u64>,
    update_runtime: Arc<tokio::sync::Mutex<update::UpdateRuntime>>,
) {
    let interval = Duration::from_secs(6 * 3600);

    loop {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed");
                if !wait_for_retry_or_shutdown(&mut notify).await {
                    break;
                }
                continue;
            }
        };

        if !settings.app_auto_update_enabled {
            if !wait_for_change_or_shutdown(&mut notify).await {
                break;
            }
            continue;
        }

        let data_dir = data_dir_from_db_path(db_path.as_path());
        let _ = update::check_latest(
            &http_client,
            update_runtime.clone(),
            db_path.clone(),
            &data_dir,
            settings.ui_locale,
        )
        .await;

        if !wait_for_interval_or_shutdown(interval, &mut notify).await {
            break;
        }
    }
}

pub(crate) async fn cli_tools_auto_update_loop(db_path: PathBuf, mut notify: watch::Receiver<u64>) {
    let interval = Duration::from_secs(24 * 3600);
    let http_client = reqwest::Client::builder()
        .user_agent(format!("CliSwitch/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .ok();

    loop {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed");
                if !wait_for_retry_or_shutdown(&mut notify).await {
                    break;
                }
                continue;
            }
        };

        let enabled = settings.gemini_cli_auto_update_enabled
            || settings.claude_code_auto_update_enabled
            || settings.codex_auto_update_enabled;

        if !enabled {
            if !wait_for_change_or_shutdown(&mut notify).await {
                break;
            }
            continue;
        }

        let to_update: Vec<_> = CLI_TOOLS
            .iter()
            .filter(|d| match d.id {
                crate::cli_tools::CliToolId::Gemini => settings.gemini_cli_auto_update_enabled,
                crate::cli_tools::CliToolId::Claude => settings.claude_code_auto_update_enabled,
                crate::cli_tools::CliToolId::Codex => settings.codex_auto_update_enabled,
            })
            .copied()
            .collect();

        let mut npm_path = settings.cli_tools_npm_path.clone();
        let mut node_path = settings.cli_tools_node_path.clone();
        let npm_registry = if let Some(client) = http_client.as_ref() {
            crate::cli_tools::pick_cli_tools_npm_registry(client).await
        } else {
            crate::cli_tools::NPM_REGISTRY_OFFICIAL.to_string()
        };
        let data_dir = data_dir_from_db_path(db_path.as_path());
        let tools_prefix_dir = crate::cli_tools::cli_tools_npm_prefix_dir(&data_dir);

        // Keep it fully automatic: if we need npm for enabled tools but it's not available,
        // install our bundled npm env and persist it internally.
        let (needs_npm, npm_available) = tokio::task::spawn_blocking({
            let npm_path = npm_path.clone();
            let node_path = node_path.clone();
            let data_dir = data_dir.clone();
            let to_update = to_update.clone();
            move || {
                let env =
                    crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
                let needs_npm = to_update.iter().any(|d| {
                    let detected = crate::cli_tools::detect_cli_tool(&env, &data_dir, d);
                    detected.install_method != crate::cli_tools::CliToolInstallMethod::Brew
                });
                (needs_npm, env.npm_available())
            }
        })
        .await
        .unwrap_or((false, false));

        if needs_npm
            && !npm_available
            && let Some(client) = http_client.as_ref()
        {
            match nodejs::ensure_npm_env_installed(client, &data_dir).await {
                Ok(paths) => {
                    let npm_path1 = paths.npm_path.to_string_lossy().to_string();
                    let node_path1 = paths.node_path.to_string_lossy().to_string();

                    if let Err(e) = storage::update_app_settings(
                        db_path.clone(),
                        storage::AppSettingsPatch {
                            cli_tools_npm_path: Some(npm_path1.clone()),
                            cli_tools_node_path: Some(node_path1.clone()),
                            ..Default::default()
                        },
                    )
                    .await
                    {
                        tracing::warn!(err = %e, "persist npm env install paths failed");
                    } else {
                        npm_path = Some(npm_path1);
                        node_path = Some(node_path1);
                    }
                }
                Err(e) => {
                    tracing::warn!(err = %e, "ensure npm env installed for cli tool auto update failed");
                }
            }
        }

        let res = tokio::task::spawn_blocking(move || {
            let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
            for d in to_update {
                let detected = crate::cli_tools::detect_cli_tool(&env, &data_dir, &d);
                match detected.install_method {
                    crate::cli_tools::CliToolInstallMethod::Brew => {
                        let Some(brew) = detected.installer_path.as_ref() else {
                            continue;
                        };
                        match crate::cli_tools::brew_upgrade_cli_tool(brew, d.id) {
                            Ok(out) => {
                                if !out.status.success() {
                                    let code = out.status.code();
                                    tracing::warn!(
                                        tool = d.name,
                                        exit_code = ?code,
                                        stderr = %out.stderr.trim(),
                                        "cli tool auto update (brew) failed"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(tool = d.name, err = %e, "cli tool auto update (brew) failed");
                            }
                        }
                    }
                    crate::cli_tools::CliToolInstallMethod::Npm => {
                        if !env.npm_available() {
                            continue;
                        }
                        match env.npm_install_global_with_registry(
                            d.npm_package,
                            Some(npm_registry.as_str()),
                        ) {
                            Ok(out) => {
                                if !out.status.success() {
                                    let code = out.status.code();
                                    tracing::warn!(
                                        tool = d.name,
                                        pkg = d.npm_package,
                                        exit_code = ?code,
                                        stderr = %out.stderr.trim(),
                                        "cli tool auto update (npm) failed"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(tool = d.name, pkg = d.npm_package, err = %e, "cli tool auto update (npm) failed");
                            }
                        }
                    }
                    crate::cli_tools::CliToolInstallMethod::ManagedNpmPrefix
                    | crate::cli_tools::CliToolInstallMethod::Other => {
                        if !env.npm_available() {
                            continue;
                        }
                        match env.npm_install_global_to_prefix(
                            d.npm_package,
                            &tools_prefix_dir,
                            Some(npm_registry.as_str()),
                        ) {
                            Ok(out) => {
                                if !out.status.success() {
                                    let code = out.status.code();
                                    tracing::warn!(
                                        tool = d.name,
                                        pkg = d.npm_package,
                                        exit_code = ?code,
                                        stderr = %out.stderr.trim(),
                                        "cli tool auto update failed"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(tool = d.name, pkg = d.npm_package, err = %e, "cli tool auto update failed");
                            }
                        }
                    }
                };
            }
        })
        .await;

        if let Err(e) = res {
            tracing::warn!(err = %e, "cli tool auto update task join failed");
        }

        if !wait_for_interval_or_shutdown(interval, &mut notify).await {
            break;
        }
    }
}

pub(crate) async fn apply_autostart_setting(db_path: PathBuf) {
    let settings = match storage::get_app_settings(db_path).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(err = %e, "load app settings failed");
            return;
        }
    };

    let desired = settings.auto_start_enabled;
    let _ = tokio::task::spawn_blocking(move || {
        let actual = match autostart::is_enabled() {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(err = %e, "read autostart state failed");
                return;
            }
        };
        if actual != desired
            && let Err(e) = autostart::set_enabled(desired)
        {
            tracing::warn!(err = %e, desired, "apply autostart setting failed");
        }
    })
    .await;
}

pub(crate) async fn logs_retention_cleanup_loop(
    db_path: PathBuf,
    mut notify: watch::Receiver<u64>,
) {
    let interval = Duration::from_secs(24 * 3600);

    loop {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed");
                if !wait_for_retry_or_shutdown(&mut notify).await {
                    break;
                }
                continue;
            }
        };

        let retention_days = settings.log_retention_days.clamp(1, 3650);
        let data_dir = data_dir_from_db_path(db_path.as_path());
        let log_dir = crate::app::logs_dir(&data_dir);
        let log_dir_display = log_dir.display().to_string();

        let res = tokio::task::spawn_blocking(move || {
            log_files::clear_logs_by_retention_days(&log_dir, retention_days)
        })
        .await;

        match res {
            Ok(Ok(r)) => {
                if r.deleted_files > 0 || r.truncated_files > 0 {
                    tracing::info!(
                        log_dir = %log_dir_display,
                        retention_days,
                        deleted_files = r.deleted_files,
                        truncated_files = r.truncated_files,
                        "logs retention cleanup done"
                    );
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(log_dir = %log_dir_display, retention_days, err = %e, "logs retention cleanup failed")
            }
            Err(e) => {
                tracing::warn!(log_dir = %log_dir_display, retention_days, err = %e, "logs retention cleanup task join failed")
            }
        }

        if !wait_for_interval_or_shutdown(interval, &mut notify).await {
            break;
        }
    }
}

async fn persist_newapi_overview(
    db_path: PathBuf,
    account_id: &str,
    overview: &newapi::NewApiAccountOverview,
    completed_ids: &mut HashSet<String>,
) -> anyhow::Result<()> {
    storage::update_newapi_account_remote_snapshot(
        db_path.clone(),
        account_id.to_string(),
        newapi::build_remote_snapshot(overview),
    )
    .await?;
    if overview.checked_in_today {
        storage::complete_newapi_account_checkin_today(
            db_path,
            account_id.to_string(),
            "remote_detected",
        )
        .await?;
        completed_ids.insert(account_id.to_string());
    }
    Ok(())
}

async fn persist_newapi_sync_failure(
    db_path: PathBuf,
    account_id: &str,
    err: &anyhow::Error,
) -> anyhow::Result<()> {
    storage::update_newapi_account_remote_snapshot(
        db_path,
        account_id.to_string(),
        storage::NewApiAccountRemoteSnapshot {
            last_sync_error: Some(err.to_string()),
            last_synced_at_ms: Some(storage::now_ms()),
            ..Default::default()
        },
    )
    .await
}

fn parse_auto_checkin_trigger(account: &storage::NewApiAccount) -> Option<DailyLocalTimeTrigger> {
    match DailyLocalTimeTrigger::parse(&account.auto_checkin_time) {
        Ok(trigger) => Some(trigger),
        Err(err) => {
            tracing::warn!(
                account_id = %account.id,
                auto_checkin_time = %account.auto_checkin_time,
                err = %err,
                "parse newapi auto checkin time failed"
            );
            None
        }
    }
}

fn earlier_deadline(
    current: Option<time::OffsetDateTime>,
    next: time::OffsetDateTime,
) -> Option<time::OffsetDateTime> {
    match current {
        Some(existing) if existing <= next => Some(existing),
        _ => Some(next),
    }
}

async fn run_due_newapi_auto_checkin(
    db_path: PathBuf,
    http_client: &reqwest::Client,
    account: &storage::NewApiAccount,
    completed_ids: &mut HashSet<String>,
) {
    let latest_overview = match newapi::fetch_account_overview(http_client, account).await {
        Ok(overview) => {
            if let Err(err) =
                persist_newapi_overview(db_path.clone(), &account.id, &overview, completed_ids)
                    .await
            {
                tracing::warn!(account_id = %account.id, err = %err, "persist newapi overview failed");
            }
            Some(overview)
        }
        Err(err) => {
            tracing::warn!(account_id = %account.id, err = %err, "sync newapi overview failed");
            if let Err(update_err) =
                persist_newapi_sync_failure(db_path.clone(), &account.id, &err).await
            {
                tracing::warn!(account_id = %account.id, err = %update_err, "persist newapi sync failure failed");
            }
            None
        }
    };

    match newapi::perform_system_checkin_with_overview(
        http_client,
        account,
        latest_overview.as_ref(),
    )
    .await
    {
        Ok(result) => {
            let method = if result.already_checked_in {
                "remote_detected"
            } else {
                "system_api"
            };
            if let Err(err) = storage::complete_newapi_account_checkin_today(
                db_path.clone(),
                account.id.clone(),
                method,
            )
            .await
            {
                tracing::warn!(account_id = %account.id, err = %err, "record newapi system checkin failed");
            } else {
                completed_ids.insert(account.id.clone());
            }

            if result.already_checked_in && latest_overview.is_some() {
                if let Some(overview) = latest_overview.as_ref()
                    && let Err(err) = persist_newapi_overview(
                        db_path.clone(),
                        &account.id,
                        overview,
                        completed_ids,
                    )
                    .await
                {
                    tracing::warn!(account_id = %account.id, err = %err, "persist cached newapi overview after already checked in");
                }
            } else {
                match newapi::fetch_account_overview(http_client, account).await {
                    Ok(overview) => {
                        if let Err(err) = persist_newapi_overview(
                            db_path.clone(),
                            &account.id,
                            &overview,
                            completed_ids,
                        )
                        .await
                        {
                            tracing::warn!(account_id = %account.id, err = %err, "persist newapi overview after system checkin failed");
                        }
                    }
                    Err(err) => {
                        tracing::warn!(account_id = %account.id, err = %err, "refresh newapi overview after system checkin failed");
                        if let Err(update_err) =
                            persist_newapi_sync_failure(db_path.clone(), &account.id, &err).await
                        {
                            tracing::warn!(account_id = %account.id, err = %update_err, "persist newapi sync failure after system checkin failed");
                        }
                    }
                }
            }
        }
        Err(err) => {
            tracing::warn!(account_id = %account.id, err = %err, "auto newapi system checkin failed");
        }
    }
}

pub(crate) async fn newapi_auto_checkin_loop(
    db_path: PathBuf,
    http_client: reqwest::Client,
    mut notify: watch::Receiver<u64>,
) {
    loop {
        let now = scheduler::local_now();
        let accounts = match storage::list_newapi_accounts_with_secret(db_path.clone()).await {
            Ok(accounts) => accounts,
            Err(err) => {
                tracing::warn!(err = %err, "load newapi accounts failed");
                if !wait_for_retry_or_shutdown(&mut notify).await {
                    break;
                }
                continue;
            }
        };

        let mut completed_ids =
            match storage::get_newapi_accounts_checkins_today(db_path.clone()).await {
                Ok(checkins) => checkins
                    .completed_account_ids
                    .into_iter()
                    .collect::<HashSet<_>>(),
                Err(err) => {
                    tracing::warn!(err = %err, "load newapi account checkins failed");
                    if !wait_for_retry_or_shutdown(&mut notify).await {
                        break;
                    }
                    continue;
                }
            };

        let mut due_accounts = Vec::new();
        let mut next_deadline = None;

        for account in accounts {
            if !account.auto_checkin_enabled
                || !matches!(
                    account.checkin_mode,
                    storage::NewApiAccountCheckinMode::SystemApi
                )
                || !newapi::account_has_user_api_credentials(&account)
            {
                continue;
            }

            let Some(trigger) = parse_auto_checkin_trigger(&account) else {
                continue;
            };

            if completed_ids.contains(&account.id) {
                next_deadline = earlier_deadline(next_deadline, trigger.tomorrow_deadline(now));
                continue;
            }

            if trigger.is_due(now) {
                due_accounts.push(account);
                continue;
            }

            next_deadline = earlier_deadline(next_deadline, trigger.today_deadline(now));
        }

        if !due_accounts.is_empty() {
            for account in due_accounts {
                run_due_newapi_auto_checkin(
                    db_path.clone(),
                    &http_client,
                    &account,
                    &mut completed_ids,
                )
                .await;
            }
            continue;
        }

        match next_deadline {
            Some(deadline) => {
                if !wait_until_or_shutdown(deadline, &mut notify).await {
                    break;
                }
            }
            None => {
                if !wait_for_change_or_shutdown(&mut notify).await {
                    break;
                }
            }
        }
    }
}

fn normalize_remote_multiplier(raw: f64) -> Option<f64> {
    if !raw.is_finite() || raw < 0.0 {
        return None;
    }
    Some((raw * 100.0).round() / 100.0)
}

fn format_currency_balance(amount: f64, currency: storage::RechargeCurrency) -> String {
    let symbol = match currency {
        storage::RechargeCurrency::Cny => "¥",
        storage::RechargeCurrency::Usd => "$",
    };
    format!("{symbol}{amount:.2}")
}

fn group_managed_channels_by_account(
    channels: Vec<storage::Channel>,
) -> HashMap<(storage::ManagedRemoteProvider, String), Vec<storage::Channel>> {
    channels
        .into_iter()
        .filter_map(|channel| {
            let provider = channel.managed_provider()?;
            let account_id = channel.managed_account_id()?.to_string();
            Some(((provider, account_id), channel))
        })
        .fold(
            HashMap::<(storage::ManagedRemoteProvider, String), Vec<storage::Channel>>::new(),
            |mut acc, (key, channel)| {
                acc.entry(key).or_default().push(channel);
                acc
            },
        )
}

async fn persist_sub2api_overview(
    db_path: PathBuf,
    account_id: &str,
    overview: &sub2api::Sub2ApiAccountOverview,
) -> anyhow::Result<()> {
    storage::apply_remote_account_sync_success(
        db_path,
        account_id.to_string(),
        storage::RemoteAccountRemoteSnapshot {
            remote_user_id: Some(overview.remote_user_id.to_string()),
            remote_role: overview.remote_role.clone(),
            remote_username: overview.remote_username.clone(),
            remote_display_name: overview.remote_display_name.clone(),
            last_balance_amount: overview.balance,
            last_synced_at_ms: Some(storage::now_ms()),
        },
    )
    .await
}

async fn persist_sub2api_sync_failure(
    db_path: PathBuf,
    account_id: &str,
    err: &anyhow::Error,
) -> anyhow::Result<()> {
    storage::apply_remote_account_sync_failure(
        db_path,
        account_id.to_string(),
        err.to_string(),
        Some(storage::now_ms()),
    )
    .await
}

async fn run_newapi_account_maintenance(
    db_path: PathBuf,
    http_client: &reqwest::Client,
    settings: &storage::AppSettings,
    account: &mut storage::NewApiAccount,
    managed_channels_by_account: &HashMap<
        (storage::ManagedRemoteProvider, String),
        Vec<storage::Channel>,
    >,
    completed_ids: &mut HashSet<String>,
) {
    if !newapi::account_has_user_api_credentials(account) {
        if account.low_balance_alert_notified {
            let _ = storage::set_newapi_account_balance_alert_notified(
                db_path,
                account.id.clone(),
                false,
                None,
            )
            .await;
        }
        return;
    }

    let latest_overview = match newapi::fetch_account_overview(http_client, account).await {
        Ok(overview) => {
            if let Err(err) =
                persist_newapi_overview(db_path.clone(), &account.id, &overview, completed_ids)
                    .await
            {
                tracing::warn!(account_id = %account.id, err = %err, "persist newapi overview failed");
            }
            account.remote_display_name = overview.remote_display_name.clone();
            account.remote_username = overview.remote_username.clone();
            account.custom_currency_symbol = overview.custom_currency_symbol.clone();
            account.quota_display_type = overview.quota_display_type.clone();
            Some(overview)
        }
        Err(err) => {
            tracing::warn!(account_id = %account.id, err = %err, "sync newapi overview failed");
            if let Err(update_err) =
                persist_newapi_sync_failure(db_path.clone(), &account.id, &err).await
            {
                tracing::warn!(account_id = %account.id, err = %update_err, "persist newapi sync failure failed");
            }
            None
        }
    };

    if let Some(overview) = latest_overview.as_ref() {
        match decide_low_balance_alert_action(
            account.low_balance_alert_threshold,
            overview.last_balance_amount,
            account.low_balance_alert_notified,
            settings.newapi_low_balance_system_notification_enabled(),
        ) {
            LowBalanceAlertAction::None => {}
            LowBalanceAlertAction::MarkNotifiedSilently => {
                if let Err(err) = storage::set_newapi_account_balance_alert_notified(
                    db_path.clone(),
                    account.id.clone(),
                    true,
                    None,
                )
                .await
                {
                    tracing::warn!(account_id = %account.id, err = %err, "mark newapi low balance alert notified silently failed");
                } else {
                    account.low_balance_alert_notified = true;
                }
            }
            LowBalanceAlertAction::PublishAndMarkNotified => {
                if let Some(balance_amount) = overview.last_balance_amount {
                    let balance_text = newapi::format_balance_amount(
                        balance_amount,
                        &overview.quota_display_type,
                        overview.custom_currency_symbol.as_deref(),
                    );
                    events::publish(AppEvent::RemoteLowBalanceAlert(RemoteLowBalanceAlert {
                        account_id: account.id.clone(),
                        provider: storage::ManagedRemoteProvider::Newapi,
                        account_base_url: account.base_url.clone(),
                        balance_text,
                    }));
                    if let Err(err) = storage::set_newapi_account_balance_alert_notified(
                        db_path.clone(),
                        account.id.clone(),
                        true,
                        Some(storage::now_ms()),
                    )
                    .await
                    {
                        tracing::warn!(account_id = %account.id, err = %err, "mark newapi low balance alert notified failed");
                    } else {
                        account.low_balance_alert_notified = true;
                    }
                }
            }
            LowBalanceAlertAction::ClearNotified => {
                if let Err(err) = storage::set_newapi_account_balance_alert_notified(
                    db_path.clone(),
                    account.id.clone(),
                    false,
                    None,
                )
                .await
                {
                    tracing::warn!(account_id = %account.id, err = %err, "clear newapi low balance alert notified failed");
                } else {
                    account.low_balance_alert_notified = false;
                }
            }
        }
    }

    let managed_channels = managed_channels_by_account
        .get(&(storage::ManagedRemoteProvider::Newapi, account.id.clone()))
        .cloned()
        .unwrap_or_default();
    if managed_channels.is_empty()
        || (!settings.newapi_managed_channel_missing_prompt_enabled
            && !settings.newapi_managed_channel_sync_multiplier_enabled)
    {
        return;
    }

    let (remote_group_names, remote_groups) = match newapi::list_groups(http_client, account).await
    {
        Ok(groups) => {
            let mut remote_group_names = HashSet::new();
            let mut remote_groups = HashMap::new();
            for group in groups {
                remote_group_names.insert(group.name.clone());
                if let Some(ratio) = group.ratio.and_then(normalize_remote_multiplier) {
                    remote_groups.insert(group.name, ratio);
                }
            }
            (Some(remote_group_names), Some(remote_groups))
        }
        Err(err) => {
            tracing::warn!(account_id = %account.id, err = %err, "load newapi groups failed");
            (None, None)
        }
    };
    let remote_token_ids = if settings.newapi_managed_channel_missing_prompt_enabled {
        match newapi::list_tokens(http_client, account).await {
            Ok(tokens) => Some(
                tokens
                    .into_iter()
                    .map(|token| token.id)
                    .collect::<HashSet<i64>>(),
            ),
            Err(err) => {
                tracing::warn!(account_id = %account.id, err = %err, "load newapi tokens failed");
                None
            }
        }
    } else {
        None
    };

    for channel in managed_channels {
        let managed_resource_id = channel
            .managed_resource_id()
            .and_then(|value| value.parse::<i64>().ok())
            .or(channel.newapi_token_id);
        let missing_resource = settings.newapi_managed_channel_missing_prompt_enabled
            && channel.enabled
            && match (managed_resource_id, remote_token_ids.as_ref()) {
                (Some(token_id), Some(remote_token_ids)) => !remote_token_ids.contains(&token_id),
                _ => false,
            };
        let missing_group = settings.newapi_managed_channel_missing_prompt_enabled
            && channel.enabled
            && match (channel.managed_group_name(), remote_group_names.as_ref()) {
                (Some(group_name), Some(remote_group_names)) => {
                    !remote_group_names.contains(group_name)
                }
                _ => false,
            };
        if missing_resource || missing_group {
            events::publish(AppEvent::RemoteManagedChannelMissingPrompt(
                RemoteManagedChannelMissingPrompt {
                    channel_id: channel.id.clone(),
                    channel_name: channel.name.clone(),
                    account_id: account.id.clone(),
                    account_base_url: account.base_url.clone(),
                    provider: storage::ManagedRemoteProvider::Newapi,
                    group_name: channel.managed_group_name().map(ToOwned::to_owned),
                    resource_name: channel.managed_resource_name().map(ToOwned::to_owned),
                    missing_group,
                    missing_resource,
                },
            ));
        }

        if !settings.newapi_managed_channel_sync_multiplier_enabled {
            continue;
        }
        let Some(group_name) = channel.managed_group_name() else {
            continue;
        };
        let Some(remote_groups) = remote_groups.as_ref() else {
            continue;
        };
        let Some(remote_multiplier) = remote_groups.get(group_name).copied() else {
            continue;
        };
        if settings.newapi_managed_channel_sync_free_multiplier_enabled
            && channel.real_multiplier.abs() < 1e-9
        {
            continue;
        }
        if (channel.real_multiplier - remote_multiplier).abs() < 1e-9 {
            continue;
        }

        events::publish(AppEvent::RemoteManagedChannelMultiplierPrompt(
            RemoteManagedChannelMultiplierPrompt {
                channel_id: channel.id.clone(),
                channel_name: channel.name.clone(),
                account_id: account.id.clone(),
                account_base_url: account.base_url.clone(),
                provider: storage::ManagedRemoteProvider::Newapi,
                group_name: channel.managed_group_name().map(ToOwned::to_owned),
                current_multiplier: channel.real_multiplier,
                remote_multiplier,
            },
        ));
    }
}

async fn run_sub2api_account_maintenance(
    db_path: PathBuf,
    http_client: &reqwest::Client,
    settings: &storage::AppSettings,
    account: &mut storage::RemoteAccount,
    managed_channels_by_account: &HashMap<
        (storage::ManagedRemoteProvider, String),
        Vec<storage::Channel>,
    >,
) {
    let Some(token) = account
        .access_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        if account.low_balance_alert_notified {
            let _ = storage::set_remote_account_balance_alert_notified(
                db_path,
                account.id.clone(),
                false,
                None,
            )
            .await;
        }
        return;
    };

    let latest_overview = match sub2api::fetch_account_overview(
        http_client,
        &account.base_url,
        token,
    )
    .await
    {
        Ok(overview) => {
            if let Err(err) =
                persist_sub2api_overview(db_path.clone(), &account.id, &overview).await
            {
                tracing::warn!(account_id = %account.id, err = %err, "persist sub2api overview failed");
            }
            account.remote_display_name = overview.remote_display_name.clone();
            account.remote_username = overview.remote_username.clone();
            account.remote_role = overview.remote_role.clone();
            Some(overview)
        }
        Err(err) => {
            tracing::warn!(account_id = %account.id, err = %err, "sync sub2api overview failed");
            if let Err(update_err) =
                persist_sub2api_sync_failure(db_path.clone(), &account.id, &err).await
            {
                tracing::warn!(account_id = %account.id, err = %update_err, "persist sub2api sync failure failed");
            }
            None
        }
    };

    if let Some(overview) = latest_overview.as_ref() {
        match decide_low_balance_alert_action(
            account.low_balance_alert_threshold,
            overview.balance,
            account.low_balance_alert_notified,
            settings.newapi_low_balance_system_notification_enabled(),
        ) {
            LowBalanceAlertAction::None => {}
            LowBalanceAlertAction::MarkNotifiedSilently => {
                if let Err(err) = storage::set_remote_account_balance_alert_notified(
                    db_path.clone(),
                    account.id.clone(),
                    true,
                    None,
                )
                .await
                {
                    tracing::warn!(account_id = %account.id, err = %err, "mark sub2api low balance alert notified silently failed");
                } else {
                    account.low_balance_alert_notified = true;
                }
            }
            LowBalanceAlertAction::PublishAndMarkNotified => {
                if let Some(balance_amount) = overview.balance {
                    events::publish(AppEvent::RemoteLowBalanceAlert(RemoteLowBalanceAlert {
                        account_id: account.id.clone(),
                        provider: storage::ManagedRemoteProvider::Sub2Api,
                        account_base_url: account.base_url.clone(),
                        balance_text: format_currency_balance(
                            balance_amount,
                            account.recharge_currency,
                        ),
                    }));
                    if let Err(err) = storage::set_remote_account_balance_alert_notified(
                        db_path.clone(),
                        account.id.clone(),
                        true,
                        Some(storage::now_ms()),
                    )
                    .await
                    {
                        tracing::warn!(account_id = %account.id, err = %err, "mark sub2api low balance alert notified failed");
                    } else {
                        account.low_balance_alert_notified = true;
                    }
                }
            }
            LowBalanceAlertAction::ClearNotified => {
                if let Err(err) = storage::set_remote_account_balance_alert_notified(
                    db_path.clone(),
                    account.id.clone(),
                    false,
                    None,
                )
                .await
                {
                    tracing::warn!(account_id = %account.id, err = %err, "clear sub2api low balance alert notified failed");
                } else {
                    account.low_balance_alert_notified = false;
                }
            }
        }
    }

    let managed_channels = managed_channels_by_account
        .get(&(storage::ManagedRemoteProvider::Sub2Api, account.id.clone()))
        .cloned()
        .unwrap_or_default();
    if managed_channels.is_empty()
        || (!settings.newapi_managed_channel_missing_prompt_enabled
            && !settings.newapi_managed_channel_sync_multiplier_enabled)
    {
        return;
    }

    let (
        remote_group_names,
        remote_group_ids,
        remote_group_ratios_by_name,
        remote_group_ratios_by_id,
    ) = match sub2api::list_groups(http_client, &account.base_url, token).await {
        Ok(groups) => {
            let mut remote_group_names = HashSet::new();
            let mut remote_group_ids = HashSet::new();
            let mut remote_group_ratios_by_name = HashMap::new();
            let mut remote_group_ratios_by_id = HashMap::new();
            for group in groups {
                remote_group_names.insert(group.name.clone());
                remote_group_ids.insert(group.id);
                if let Some(ratio) = group.rate_multiplier.and_then(normalize_remote_multiplier) {
                    remote_group_ratios_by_name.insert(group.name.clone(), ratio);
                    remote_group_ratios_by_id.insert(group.id, ratio);
                }
            }
            (
                Some(remote_group_names),
                Some(remote_group_ids),
                Some(remote_group_ratios_by_name),
                Some(remote_group_ratios_by_id),
            )
        }
        Err(err) => {
            tracing::warn!(account_id = %account.id, err = %err, "load sub2api groups failed");
            (None, None, None, None)
        }
    };
    let (remote_key_ids, remote_key_names) = if settings
        .newapi_managed_channel_missing_prompt_enabled
    {
        match sub2api::list_keys(http_client, &account.base_url, token).await {
            Ok(keys) => (
                Some(keys.iter().map(|item| item.id).collect::<HashSet<i64>>()),
                Some(
                    keys.into_iter()
                        .map(|item| item.name)
                        .collect::<HashSet<String>>(),
                ),
            ),
            Err(err) => {
                tracing::warn!(account_id = %account.id, err = %err, "load sub2api keys failed");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    for channel in managed_channels {
        let managed_resource_id = channel
            .managed_resource_id()
            .and_then(|value| value.parse::<i64>().ok());
        let missing_resource = settings.newapi_managed_channel_missing_prompt_enabled
            && channel.enabled
            && match (
                managed_resource_id,
                remote_key_ids.as_ref(),
                channel.managed_resource_name(),
                remote_key_names.as_ref(),
            ) {
                (Some(key_id), Some(remote_key_ids), _, _) => !remote_key_ids.contains(&key_id),
                (None, _, Some(resource_name), Some(remote_key_names)) => {
                    !remote_key_names.contains(resource_name)
                }
                _ => false,
            };
        let missing_group = settings.newapi_managed_channel_missing_prompt_enabled
            && channel.enabled
            && match (
                channel.managed_group_id(),
                remote_group_ids.as_ref(),
                channel.managed_group_name(),
                remote_group_names.as_ref(),
            ) {
                (Some(group_id), Some(remote_group_ids), _, _) => {
                    !remote_group_ids.contains(&group_id)
                }
                (None, _, Some(group_name), Some(remote_group_names)) => {
                    !remote_group_names.contains(group_name)
                }
                _ => false,
            };
        if missing_resource || missing_group {
            events::publish(AppEvent::RemoteManagedChannelMissingPrompt(
                RemoteManagedChannelMissingPrompt {
                    channel_id: channel.id.clone(),
                    channel_name: channel.name.clone(),
                    account_id: account.id.clone(),
                    account_base_url: account.base_url.clone(),
                    provider: storage::ManagedRemoteProvider::Sub2Api,
                    group_name: channel.managed_group_name().map(ToOwned::to_owned),
                    resource_name: channel.managed_resource_name().map(ToOwned::to_owned),
                    missing_group,
                    missing_resource,
                },
            ));
        }

        if !settings.newapi_managed_channel_sync_multiplier_enabled {
            continue;
        }
        let remote_multiplier = channel
            .managed_group_id()
            .and_then(|group_id| {
                remote_group_ratios_by_id
                    .as_ref()
                    .and_then(|items| items.get(&group_id).copied())
            })
            .or_else(|| {
                channel.managed_group_name().and_then(|group_name| {
                    remote_group_ratios_by_name
                        .as_ref()
                        .and_then(|items| items.get(group_name).copied())
                })
            });
        let Some(remote_multiplier) = remote_multiplier else {
            continue;
        };
        if settings.newapi_managed_channel_sync_free_multiplier_enabled
            && channel.real_multiplier.abs() < 1e-9
        {
            continue;
        }
        if (channel.real_multiplier - remote_multiplier).abs() < 1e-9 {
            continue;
        }

        events::publish(AppEvent::RemoteManagedChannelMultiplierPrompt(
            RemoteManagedChannelMultiplierPrompt {
                channel_id: channel.id.clone(),
                channel_name: channel.name.clone(),
                account_id: account.id.clone(),
                account_base_url: account.base_url.clone(),
                provider: storage::ManagedRemoteProvider::Sub2Api,
                group_name: channel.managed_group_name().map(ToOwned::to_owned),
                current_multiplier: channel.real_multiplier,
                remote_multiplier,
            },
        ));
    }
}

pub(crate) async fn remote_accounts_maintenance_loop(
    db_path: PathBuf,
    http_client: reqwest::Client,
    mut notify: watch::Receiver<u64>,
) {
    let interval = Duration::from_secs(30 * 60);

    loop {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(settings) => settings,
            Err(err) => {
                tracing::warn!(err = %err, "load app settings failed");
                if !wait_for_retry_or_shutdown(&mut notify).await {
                    break;
                }
                continue;
            }
        };

        let newapi_accounts = match storage::list_newapi_accounts_with_secret(db_path.clone()).await
        {
            Ok(accounts) => accounts,
            Err(err) => {
                tracing::warn!(err = %err, "load newapi accounts failed");
                if !wait_for_retry_or_shutdown(&mut notify).await {
                    break;
                }
                continue;
            }
        };
        let remote_accounts = match storage::list_remote_accounts_with_secret(db_path.clone()).await
        {
            Ok(accounts) => accounts,
            Err(err) => {
                tracing::warn!(err = %err, "load remote accounts failed");
                if !wait_for_retry_or_shutdown(&mut notify).await {
                    break;
                }
                continue;
            }
        };
        let managed_channels_by_account = if settings.newapi_managed_channel_missing_prompt_enabled
            || settings.newapi_managed_channel_sync_multiplier_enabled
        {
            match storage::list_channels(db_path.clone()).await {
                Ok(channels) => group_managed_channels_by_account(channels),
                Err(err) => {
                    tracing::warn!(err = %err, "load managed channels failed");
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        let mut completed_ids =
            match storage::get_newapi_accounts_checkins_today(db_path.clone()).await {
                Ok(checkins) => checkins
                    .completed_account_ids
                    .into_iter()
                    .collect::<HashSet<_>>(),
                Err(err) => {
                    tracing::warn!(err = %err, "load newapi account checkins failed");
                    HashSet::new()
                }
            };

        for mut account in newapi_accounts {
            run_newapi_account_maintenance(
                db_path.clone(),
                &http_client,
                &settings,
                &mut account,
                &managed_channels_by_account,
                &mut completed_ids,
            )
            .await;
        }

        for mut account in remote_accounts {
            run_sub2api_account_maintenance(
                db_path.clone(),
                &http_client,
                &settings,
                &mut account,
                &managed_channels_by_account,
            )
            .await;
        }

        if !wait_for_interval_or_shutdown(interval, &mut notify).await {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LowBalanceAlertAction, decide_low_balance_alert_action};

    #[test]
    fn low_balance_alert_is_silently_consumed_when_notifications_are_disabled() {
        assert_eq!(
            decide_low_balance_alert_action(10.0, Some(8.0), false, false),
            LowBalanceAlertAction::MarkNotifiedSilently
        );
    }

    #[test]
    fn low_balance_alert_is_published_when_notifications_are_enabled() {
        assert_eq!(
            decide_low_balance_alert_action(10.0, Some(8.0), false, true),
            LowBalanceAlertAction::PublishAndMarkNotified
        );
    }

    #[test]
    fn low_balance_alert_is_cleared_after_balance_recovers_even_if_notifications_are_disabled() {
        assert_eq!(
            decide_low_balance_alert_action(10.0, Some(12.0), true, false),
            LowBalanceAlertAction::ClearNotified
        );
    }

    #[test]
    fn disabled_threshold_clears_existing_low_balance_state() {
        assert_eq!(
            decide_low_balance_alert_action(0.0, Some(8.0), true, true),
            LowBalanceAlertAction::ClearNotified
        );
    }
}
