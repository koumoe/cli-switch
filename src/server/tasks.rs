use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::watch;
use tokio::time::Duration;

use crate::{autostart, storage, update};

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
        if update::load_pending_update(&data_dir).is_none() {
            let _ = update::spawn_download_latest(
                http_client.clone(),
                update_runtime.clone(),
                data_dir.clone(),
            )
            .await;
        } else {
            let _ = update::get_status(update_runtime.clone(), &data_dir, true).await;
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
