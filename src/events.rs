use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tokio::sync::broadcast;

use crate::storage;
use crate::update;

#[derive(Debug, Clone, Serialize)]
pub struct NpmEnvInstallProgress {
    pub stage: String,
    pub version: Option<String>,
    pub percent: Option<u8>,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewApiLowBalanceAlert {
    pub account_id: String,
    pub base_url: String,
    pub balance_text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewApiManagedChannelMissingPrompt {
    pub channel_id: String,
    pub channel_name: String,
    pub account_id: String,
    pub account_base_url: String,
    pub group_name: Option<String>,
    pub token_name: Option<String>,
    pub missing_group: bool,
    pub missing_token: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewApiManagedChannelCreated {
    pub channel_id: String,
    pub channel_name: String,
    pub account_id: String,
    pub account_base_url: String,
    pub group_name: Option<String>,
    pub token_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewApiManagedChannelMultiplierPrompt {
    pub channel_id: String,
    pub channel_name: String,
    pub account_id: String,
    pub account_base_url: String,
    pub group_name: Option<String>,
    pub current_multiplier: f64,
    pub remote_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SystemNotificationSettings {
    pub enabled: bool,
    pub newapi_low_balance_enabled: bool,
    pub newapi_managed_channel_missing_enabled: bool,
    pub newapi_managed_channel_multiplier_enabled: bool,
}

impl Default for SystemNotificationSettings {
    fn default() -> Self {
        Self::from_settings(&storage::AppSettings::default())
    }
}

impl SystemNotificationSettings {
    pub fn from_settings(settings: &storage::AppSettings) -> Self {
        Self {
            enabled: settings.system_notifications_enabled,
            newapi_low_balance_enabled: settings.newapi_low_balance_system_notification_enabled,
            newapi_managed_channel_missing_enabled: settings
                .newapi_managed_channel_missing_system_notification_enabled,
            newapi_managed_channel_multiplier_enabled: settings
                .newapi_managed_channel_multiplier_system_notification_enabled,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    UpdateStatus(update::UpdateStatus),
    UsageChanged { at_ms: i64 },
    ChannelsChanged { at_ms: i64 },
    NpmEnvInstallProgress(NpmEnvInstallProgress),
    SystemNotificationSettingsChanged(SystemNotificationSettings),
    NewApiLowBalanceAlert(NewApiLowBalanceAlert),
    NewApiManagedChannelCreated(NewApiManagedChannelCreated),
    NewApiManagedChannelMissingPrompt(NewApiManagedChannelMissingPrompt),
    NewApiManagedChannelMultiplierPrompt(NewApiManagedChannelMultiplierPrompt),
}

fn sender() -> &'static broadcast::Sender<AppEvent> {
    static SENDER: OnceLock<broadcast::Sender<AppEvent>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, _rx) = broadcast::channel(1024);
        tx
    })
}

fn last_update_status_cell() -> &'static Mutex<Option<update::UpdateStatus>> {
    static CELL: OnceLock<Mutex<Option<update::UpdateStatus>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

pub fn subscribe() -> broadcast::Receiver<AppEvent> {
    sender().subscribe()
}

pub fn last_update_status() -> Option<update::UpdateStatus> {
    last_update_status_cell()
        .lock()
        .ok()
        .and_then(|v| v.clone())
}

pub fn publish(event: AppEvent) {
    if let AppEvent::UpdateStatus(ref status) = event
        && let Ok(mut guard) = last_update_status_cell().lock()
    {
        *guard = Some(status.clone());
    }
    let _ = sender().send(event);
}
