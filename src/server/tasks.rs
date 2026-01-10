use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::watch;
use tokio::time::Duration;

use crate::{autostart, log_files, storage, update};
use crate::cli_tools::CLI_TOOLS;

use super::handlers::pricing::run_pricing_sync;
use super::state::data_dir_from_db_path;

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
                storage::AppSettings::default()
            }
        };

        if !settings.pricing_auto_update_enabled {
            if notify.changed().await.is_err() {
                break;
            }
            continue;
        }

        let hours = settings.pricing_auto_update_interval_hours.clamp(1, 8760);
        if let Err(e) = run_pricing_sync(&http_client, db_path.clone()).await {
            tracing::warn!(err = %e, "pricing auto sync failed");
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs((hours as u64) * 3600)) => {}
            changed = notify.changed() => {
                if changed.is_err() { break; }
                continue;
            }
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
                storage::AppSettings::default()
            }
        };

        if !settings.app_auto_update_enabled {
            if notify.changed().await.is_err() {
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
        )
        .await;

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            changed = notify.changed() => {
                if changed.is_err() { break; }
                continue;
            }
        }
    }
}

pub(crate) async fn cli_tools_auto_update_loop(db_path: PathBuf, mut notify: watch::Receiver<u64>) {
    let interval = Duration::from_secs(24 * 3600);

    loop {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed");
                storage::AppSettings::default()
            }
        };

        let enabled = settings.gemini_cli_auto_update_enabled
            || settings.claude_code_auto_update_enabled
            || settings.codex_auto_update_enabled;

        if !enabled {
            if notify.changed().await.is_err() {
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

        let res = tokio::task::spawn_blocking(move || {
            if !crate::cli_tools::npm_available() {
                return;
            }
            for d in to_update {
                match crate::cli_tools::npm_install_global(d.npm_package) {
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
        })
        .await;

        if let Err(e) = res {
            tracing::warn!(err = %e, "cli tool auto update task join failed");
        }

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            changed = notify.changed() => {
                if changed.is_err() { break; }
                continue;
            }
        }
    }
}

pub(crate) async fn apply_autostart_setting(db_path: PathBuf) {
    let settings = match storage::get_app_settings(db_path).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(err = %e, "load app settings failed");
            storage::AppSettings::default()
        }
    };

    let desired = settings.auto_start_enabled;
    let _ = tokio::task::spawn_blocking(move || {
        let actual = autostart::is_enabled().unwrap_or(false);
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
                storage::AppSettings::default()
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

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            changed = notify.changed() => {
                if changed.is_err() { break; }
                continue;
            }
        }
    }
}
