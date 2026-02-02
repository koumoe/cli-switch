use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::watch;
use tokio::time::Duration;

use crate::cli_tools::CLI_TOOLS;
use crate::{autostart, log_files, nodejs, storage, update};

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
                let changed = tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => Ok(()),
                    changed = notify.changed() => changed,
                };
                if changed.is_err() {
                    break;
                }
                continue;
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
                let changed = tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => Ok(()),
                    changed = notify.changed() => changed,
                };
                if changed.is_err() {
                    break;
                }
                continue;
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
    let http_client = reqwest::Client::builder()
        .user_agent(format!("CliSwitch/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .ok();

    loop {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed");
                let changed = tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => Ok(()),
                    changed = notify.changed() => changed,
                };
                if changed.is_err() {
                    break;
                }
                continue;
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
                let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
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
                let changed = tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => Ok(()),
                    changed = notify.changed() => changed,
                };
                if changed.is_err() {
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

        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            changed = notify.changed() => {
                if changed.is_err() { break; }
                continue;
            }
        }
    }
}
