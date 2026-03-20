mod bindings;
mod projects;
mod schema;
mod sessions;

pub(super) use super::{StorageError, now_ms, with_conn};
pub(super) use crate::cli_tools::CliToolId;
use serde::{Deserialize, Serialize};

use crate::i18n::AppLocale;

pub use bindings::{
    consume_pairing_token, create_pairing_token, deactivate_chat_binding, list_chat_bindings,
    resolve_chat_binding, update_chat_binding_locale,
};
pub use projects::{list_bridge_known_projects, upsert_bridge_known_project};
pub(in crate::storage) use schema::ensure_chat_bridge_schema;
pub use sessions::{
    count_active_bridge_sessions_for_platform, create_bridge_session, get_bridge_session,
    get_default_bridge_session_for_platform, list_bridge_sessions_for_platform,
    set_default_bridge_session_for_platform, stop_all_bridge_sessions_for_platform,
    stop_bridge_session, update_bridge_session,
};

pub const DEFAULT_PAIRING_TOKEN_EXPIRES_MINUTES: i64 = 10;
pub const MAX_PAIRING_TOKEN_EXPIRES_MINUTES: i64 = 24 * 60;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ChatPlatform {
    Telegram,
    Discord,
    WhatsApp,
}

impl ChatPlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            ChatPlatform::Telegram => "telegram",
            ChatPlatform::Discord => "discord",
            ChatPlatform::WhatsApp => "whatsapp",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ChatPlatform::Telegram => "Telegram",
            ChatPlatform::Discord => "Discord",
            ChatPlatform::WhatsApp => "WhatsApp",
        }
    }

    fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw.trim() {
            "telegram" => Ok(ChatPlatform::Telegram),
            "discord" => Ok(ChatPlatform::Discord),
            "whatsapp" => Ok(ChatPlatform::WhatsApp),
            other => anyhow::bail!("unknown chat platform: {other}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgePermissionMode {
    Safe,
    Yolo,
}

impl BridgePermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            BridgePermissionMode::Safe => "safe",
            BridgePermissionMode::Yolo => "yolo",
        }
    }

    fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw.trim() {
            "safe" => Ok(BridgePermissionMode::Safe),
            "yolo" => Ok(BridgePermissionMode::Yolo),
            other => anyhow::bail!("unknown bridge permission mode: {other}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeSessionStatus {
    Running,
    Idle,
    Stopped,
}

impl BridgeSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            BridgeSessionStatus::Running => "running",
            BridgeSessionStatus::Idle => "idle",
            BridgeSessionStatus::Stopped => "stopped",
        }
    }

    fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw.trim() {
            "running" => Ok(BridgeSessionStatus::Running),
            "idle" => Ok(BridgeSessionStatus::Idle),
            "stopped" => Ok(BridgeSessionStatus::Stopped),
            other => anyhow::bail!("unknown bridge session status: {other}"),
        }
    }

    pub fn is_active(self) -> bool {
        !matches!(self, BridgeSessionStatus::Stopped)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatBridgeBinding {
    pub id: i64,
    pub platform: ChatPlatform,
    pub platform_user_id: String,
    pub display_name: Option<String>,
    pub preferred_locale: String,
    pub bound_at_ms: i64,
    pub is_active: bool,
}

impl ChatBridgeBinding {
    pub fn locale(&self) -> AppLocale {
        AppLocale::parse_or_default(&self.preferred_locale)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePairingTokenInput {
    pub platform: ChatPlatform,
    pub expires_in_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatBridgePairingToken {
    pub token: String,
    pub platform: ChatPlatform,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeKnownProject {
    pub path: String,
    pub display_name: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeSession {
    pub id: i64,
    pub alias: Option<String>,
    pub platform: ChatPlatform,
    pub cli_type: CliToolId,
    pub cli_session_ref: Option<String>,
    pub project_id: Option<String>,
    pub project_name: String,
    pub working_dir: String,
    pub permission_mode: BridgePermissionMode,
    pub status: BridgeSessionStatus,
    pub is_default: bool,
    pub created_at_ms: i64,
    pub last_active_ms: i64,
}

#[derive(Debug, Clone)]
pub struct CreateBridgeSessionInput {
    pub platform: ChatPlatform,
    pub alias: Option<String>,
    pub cli_type: CliToolId,
    pub cli_session_ref: Option<String>,
    pub project_id: Option<String>,
    pub project_name: String,
    pub working_dir: String,
    pub permission_mode: BridgePermissionMode,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateBridgeSessionInput {
    pub cli_session_ref: Option<Option<String>>,
    pub status: Option<BridgeSessionStatus>,
    pub is_default: Option<bool>,
    pub last_active_ms: Option<i64>,
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}
