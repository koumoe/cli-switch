use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::i18n::AppLocale;
use crate::logging::LogLevel;

use super::{now_ms, with_conn};

const KEY_PRICING_AUTO_UPDATE_ENABLED: &str = "pricing_auto_update_enabled";
const KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS: &str = "pricing_auto_update_interval_hours";
const KEY_CLOSE_BEHAVIOR: &str = "close_behavior";
const KEY_AUTO_START_ENABLED: &str = "auto_start_enabled";
const KEY_AUTO_START_LAUNCH_MODE: &str = "auto_start_launch_mode";
const KEY_SERVER_LAN_ACCESSIBLE: &str = "server_lan_accessible";
const KEY_UI_LOCALE: &str = "ui_locale";
const KEY_APP_AUTO_UPDATE_ENABLED: &str = "app_auto_update_enabled";
const KEY_GEMINI_CLI_AUTO_UPDATE_ENABLED: &str = "gemini_cli_auto_update_enabled";
const KEY_CLAUDE_CODE_AUTO_UPDATE_ENABLED: &str = "claude_code_auto_update_enabled";
const KEY_CODEX_AUTO_UPDATE_ENABLED: &str = "codex_auto_update_enabled";
const KEY_CLI_TOOLS_NPM_PATH: &str = "cli_tools_npm_path";
const KEY_CLI_TOOLS_NODE_PATH: &str = "cli_tools_node_path";
const KEY_AUTO_DISABLE_ENABLED: &str = "auto_disable_enabled";
const KEY_AUTO_DISABLE_WINDOW_MINUTES: &str = "auto_disable_window_minutes";
const KEY_AUTO_DISABLE_FAILURE_TIMES: &str = "auto_disable_failure_times";
const KEY_AUTO_DISABLE_DISABLE_MINUTES: &str = "auto_disable_disable_minutes";
const KEY_ANTHROPIC_COUNT_TOKENS_MOCK_ENABLED: &str = "anthropic_count_tokens_mock_enabled";
const KEY_LOG_LEVEL: &str = "log_level";
const KEY_LOG_RETENTION_DAYS: &str = "log_retention_days";
const KEY_CHAT_BRIDGE_ENABLED: &str = "chat_bridge_enabled";
const KEY_CHAT_BRIDGE_TELEGRAM_ENABLED: &str = "chat_bridge_telegram_enabled";
const KEY_CHAT_BRIDGE_TELEGRAM_BOT_TOKEN: &str = "chat_bridge_telegram_bot_token";
const KEY_CHAT_BRIDGE_DISCORD_ENABLED: &str = "chat_bridge_discord_enabled";
const KEY_CHAT_BRIDGE_DISCORD_BOT_TOKEN: &str = "chat_bridge_discord_bot_token";
const KEY_CHAT_BRIDGE_WHATSAPP_ENABLED: &str = "chat_bridge_whatsapp_enabled";
const KEY_CHAT_BRIDGE_WEIXIN_ENABLED: &str = "chat_bridge_weixin_enabled";
const KEY_CHAT_BRIDGE_TURN_TIMEOUT_MINUTES: &str = "chat_bridge_turn_timeout_minutes";
const KEY_CHAT_BRIDGE_ALLOW_NEW_PROJECTS: &str = "chat_bridge_allow_new_projects";
const KEY_NEWAPI_MANAGED_CHANNEL_MISSING_PROMPT_ENABLED: &str =
    "newapi_managed_channel_missing_prompt_enabled";
const KEY_NEWAPI_MANAGED_CHANNEL_SYNC_MULTIPLIER_ENABLED: &str =
    "newapi_managed_channel_sync_multiplier_enabled";
const KEY_NEWAPI_MANAGED_CHANNEL_SYNC_FREE_MULTIPLIER_ENABLED: &str =
    "newapi_managed_channel_sync_free_multiplier_enabled";
const DEFAULT_CHAT_BRIDGE_TURN_TIMEOUT_MINUTES: i64 = 0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    Ask,
    MinimizeToTray,
    Quit,
}

impl CloseBehavior {
    fn as_str(self) -> &'static str {
        match self {
            CloseBehavior::Ask => "ask",
            CloseBehavior::MinimizeToTray => "minimize_to_tray",
            CloseBehavior::Quit => "quit",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoStartLaunchMode {
    ShowWindow,
    MinimizeToTray,
}

impl AutoStartLaunchMode {
    fn as_str(self) -> &'static str {
        match self {
            AutoStartLaunchMode::ShowWindow => "show_window",
            AutoStartLaunchMode::MinimizeToTray => "minimize_to_tray",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub pricing_auto_update_enabled: bool,
    pub pricing_auto_update_interval_hours: i64,
    pub close_behavior: CloseBehavior,
    pub auto_start_enabled: bool,
    pub auto_start_launch_mode: AutoStartLaunchMode,
    pub server_lan_accessible: bool,
    pub ui_locale: AppLocale,
    pub app_auto_update_enabled: bool,
    pub gemini_cli_auto_update_enabled: bool,
    pub claude_code_auto_update_enabled: bool,
    pub codex_auto_update_enabled: bool,
    pub cli_tools_npm_path: Option<String>,
    pub cli_tools_node_path: Option<String>,
    pub auto_disable_enabled: bool,
    pub auto_disable_window_minutes: i64,
    pub auto_disable_failure_times: i64,
    pub auto_disable_disable_minutes: i64,
    pub anthropic_count_tokens_mock_enabled: bool,
    pub log_level: LogLevel,
    pub log_retention_days: i64,
    pub chat_bridge_enabled: bool,
    pub chat_bridge_telegram_enabled: bool,
    #[serde(skip_serializing)]
    pub chat_bridge_telegram_bot_token: Option<String>,
    pub chat_bridge_telegram_bot_token_configured: bool,
    pub chat_bridge_discord_enabled: bool,
    #[serde(skip_serializing)]
    pub chat_bridge_discord_bot_token: Option<String>,
    pub chat_bridge_discord_bot_token_configured: bool,
    pub chat_bridge_whatsapp_enabled: bool,
    pub chat_bridge_weixin_enabled: bool,
    pub chat_bridge_turn_timeout_minutes: i64,
    pub chat_bridge_allow_new_projects: bool,
    pub newapi_managed_channel_missing_prompt_enabled: bool,
    pub newapi_managed_channel_sync_multiplier_enabled: bool,
    pub newapi_managed_channel_sync_free_multiplier_enabled: bool,
    #[serde(default)]
    pub has_invalid_values: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            pricing_auto_update_enabled: false,
            pricing_auto_update_interval_hours: 24,
            close_behavior: CloseBehavior::Ask,
            auto_start_enabled: false,
            auto_start_launch_mode: AutoStartLaunchMode::ShowWindow,
            server_lan_accessible: false,
            ui_locale: AppLocale::default(),
            app_auto_update_enabled: false,
            gemini_cli_auto_update_enabled: false,
            claude_code_auto_update_enabled: false,
            codex_auto_update_enabled: false,
            cli_tools_npm_path: None,
            cli_tools_node_path: None,
            auto_disable_enabled: false,
            auto_disable_window_minutes: 3,
            auto_disable_failure_times: 5,
            auto_disable_disable_minutes: 30,
            anthropic_count_tokens_mock_enabled: false,
            log_level: LogLevel::Warning,
            log_retention_days: 30,
            chat_bridge_enabled: false,
            chat_bridge_telegram_enabled: false,
            chat_bridge_telegram_bot_token: None,
            chat_bridge_telegram_bot_token_configured: false,
            chat_bridge_discord_enabled: false,
            chat_bridge_discord_bot_token: None,
            chat_bridge_discord_bot_token_configured: false,
            chat_bridge_whatsapp_enabled: false,
            chat_bridge_weixin_enabled: false,
            chat_bridge_turn_timeout_minutes: DEFAULT_CHAT_BRIDGE_TURN_TIMEOUT_MINUTES,
            chat_bridge_allow_new_projects: false,
            newapi_managed_channel_missing_prompt_enabled: true,
            newapi_managed_channel_sync_multiplier_enabled: true,
            newapi_managed_channel_sync_free_multiplier_enabled: false,
            has_invalid_values: false,
        }
    }
}

impl AppSettings {
    pub fn chat_bridge_turn_timeout(&self) -> Option<Duration> {
        if self.chat_bridge_turn_timeout_minutes == 0 {
            return None;
        }

        u64::try_from(self.chat_bridge_turn_timeout_minutes)
            .ok()
            .filter(|minutes| *minutes >= 1)
            .or_else(|| {
                u64::try_from(DEFAULT_CHAT_BRIDGE_TURN_TIMEOUT_MINUTES)
                    .ok()
                    .filter(|minutes| *minutes >= 1)
            })
            .map(|minutes| Duration::from_secs(minutes.saturating_mul(60)))
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppSettingsPatch {
    pub pricing_auto_update_enabled: Option<bool>,
    pub pricing_auto_update_interval_hours: Option<i64>,
    pub close_behavior: Option<CloseBehavior>,
    pub auto_start_enabled: Option<bool>,
    pub auto_start_launch_mode: Option<AutoStartLaunchMode>,
    pub server_lan_accessible: Option<bool>,
    pub ui_locale: Option<AppLocale>,
    pub app_auto_update_enabled: Option<bool>,
    pub gemini_cli_auto_update_enabled: Option<bool>,
    pub claude_code_auto_update_enabled: Option<bool>,
    pub codex_auto_update_enabled: Option<bool>,
    pub cli_tools_npm_path: Option<String>,
    pub cli_tools_node_path: Option<String>,
    pub auto_disable_enabled: Option<bool>,
    pub auto_disable_window_minutes: Option<i64>,
    pub auto_disable_failure_times: Option<i64>,
    pub auto_disable_disable_minutes: Option<i64>,
    pub anthropic_count_tokens_mock_enabled: Option<bool>,
    pub log_level: Option<LogLevel>,
    pub log_retention_days: Option<i64>,
    pub chat_bridge_enabled: Option<bool>,
    pub chat_bridge_telegram_enabled: Option<bool>,
    pub chat_bridge_telegram_bot_token: Option<String>,
    pub chat_bridge_discord_enabled: Option<bool>,
    pub chat_bridge_discord_bot_token: Option<String>,
    pub chat_bridge_whatsapp_enabled: Option<bool>,
    pub chat_bridge_weixin_enabled: Option<bool>,
    pub chat_bridge_turn_timeout_minutes: Option<i64>,
    pub chat_bridge_allow_new_projects: Option<bool>,
    pub newapi_managed_channel_missing_prompt_enabled: Option<bool>,
    pub newapi_managed_channel_sync_multiplier_enabled: Option<bool>,
    pub newapi_managed_channel_sync_free_multiplier_enabled: Option<bool>,
}

fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

fn set_setting(
    conn: &Connection,
    key: &str,
    value: &str,
    updated_at_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"
        INSERT INTO app_settings (key, value, updated_at_ms)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET
          value = excluded.value,
          updated_at_ms = excluded.updated_at_ms
        "#,
        params![key, value, updated_at_ms],
    )?;
    Ok(())
}

static INVALID_SETTINGS_ONCE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn warn_invalid_setting_once(key: &'static str, raw_value: &str, reason: impl FnOnce() -> String) {
    let value = raw_value.trim();
    let id = format!("{key}={value}");
    let set = INVALID_SETTINGS_ONCE.get_or_init(|| Mutex::new(HashSet::new()));
    let mut set = set.lock().unwrap_or_else(|e| e.into_inner());
    if !set.insert(id) {
        return;
    }
    let reason = reason();
    tracing::warn!(
        key,
        value = %value,
        reason = %reason,
        "invalid app setting; using default"
    );
}

fn parse_bool_setting(
    key: &'static str,
    raw_value: &str,
    invalid: &mut bool,
    default: bool,
) -> bool {
    let v = raw_value.trim();
    if v == "1" || v.eq_ignore_ascii_case("true") {
        return true;
    }
    if v == "0" || v.eq_ignore_ascii_case("false") {
        return false;
    }
    *invalid = true;
    warn_invalid_setting_once(key, raw_value, || "invalid bool".to_string());
    default
}

fn parse_i64_setting(key: &'static str, raw_value: &str, invalid: &mut bool) -> Option<i64> {
    let v = raw_value.trim();
    match v.parse::<i64>() {
        Ok(n) => Some(n),
        Err(e) => {
            *invalid = true;
            warn_invalid_setting_once(key, raw_value, || format!("invalid i64: {e}"));
            None
        }
    }
}

pub async fn get_app_settings(db_path: PathBuf) -> anyhow::Result<AppSettings> {
    with_conn(db_path, move |conn| {
        let mut out = AppSettings::default();
        let mut has_invalid_values = false;

        if let Some(v) = get_setting(conn, KEY_PRICING_AUTO_UPDATE_ENABLED)? {
            out.pricing_auto_update_enabled = parse_bool_setting(
                KEY_PRICING_AUTO_UPDATE_ENABLED,
                &v,
                &mut has_invalid_values,
                out.pricing_auto_update_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS)?
            && let Some(n) = parse_i64_setting(
                KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS,
                &v,
                &mut has_invalid_values,
            )
        {
            out.pricing_auto_update_interval_hours = n;
        }
        if let Some(v) = get_setting(conn, KEY_CLOSE_BEHAVIOR)? {
            match v.trim() {
                "ask" => out.close_behavior = CloseBehavior::Ask,
                "minimize_to_tray" => out.close_behavior = CloseBehavior::MinimizeToTray,
                "quit" => out.close_behavior = CloseBehavior::Quit,
                _ => {
                    has_invalid_values = true;
                    warn_invalid_setting_once(KEY_CLOSE_BEHAVIOR, &v, || {
                        "unknown close_behavior".to_string()
                    });
                }
            }
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_START_ENABLED)? {
            out.auto_start_enabled = parse_bool_setting(
                KEY_AUTO_START_ENABLED,
                &v,
                &mut has_invalid_values,
                out.auto_start_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_START_LAUNCH_MODE)? {
            match v.trim() {
                "show_window" => out.auto_start_launch_mode = AutoStartLaunchMode::ShowWindow,
                "minimize_to_tray" => {
                    out.auto_start_launch_mode = AutoStartLaunchMode::MinimizeToTray;
                }
                _ => {
                    has_invalid_values = true;
                    warn_invalid_setting_once(KEY_AUTO_START_LAUNCH_MODE, &v, || {
                        "unknown auto_start_launch_mode".to_string()
                    });
                }
            }
        }
        if let Some(v) = get_setting(conn, KEY_SERVER_LAN_ACCESSIBLE)? {
            out.server_lan_accessible = parse_bool_setting(
                KEY_SERVER_LAN_ACCESSIBLE,
                &v,
                &mut has_invalid_values,
                out.server_lan_accessible,
            );
        }
        if let Some(v) = get_setting(conn, KEY_UI_LOCALE)? {
            out.ui_locale = AppLocale::parse_or_default(&v);
        }
        if let Some(v) = get_setting(conn, KEY_APP_AUTO_UPDATE_ENABLED)? {
            out.app_auto_update_enabled = parse_bool_setting(
                KEY_APP_AUTO_UPDATE_ENABLED,
                &v,
                &mut has_invalid_values,
                out.app_auto_update_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_GEMINI_CLI_AUTO_UPDATE_ENABLED)? {
            out.gemini_cli_auto_update_enabled = parse_bool_setting(
                KEY_GEMINI_CLI_AUTO_UPDATE_ENABLED,
                &v,
                &mut has_invalid_values,
                out.gemini_cli_auto_update_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_CLAUDE_CODE_AUTO_UPDATE_ENABLED)? {
            out.claude_code_auto_update_enabled = parse_bool_setting(
                KEY_CLAUDE_CODE_AUTO_UPDATE_ENABLED,
                &v,
                &mut has_invalid_values,
                out.claude_code_auto_update_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_CODEX_AUTO_UPDATE_ENABLED)? {
            out.codex_auto_update_enabled = parse_bool_setting(
                KEY_CODEX_AUTO_UPDATE_ENABLED,
                &v,
                &mut has_invalid_values,
                out.codex_auto_update_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_CLI_TOOLS_NPM_PATH)? {
            let s = v.trim();
            if !s.is_empty() {
                out.cli_tools_npm_path = Some(s.to_string());
            }
        }
        if let Some(v) = get_setting(conn, KEY_CLI_TOOLS_NODE_PATH)? {
            let s = v.trim();
            if !s.is_empty() {
                out.cli_tools_node_path = Some(s.to_string());
            }
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_ENABLED)? {
            out.auto_disable_enabled = parse_bool_setting(
                KEY_AUTO_DISABLE_ENABLED,
                &v,
                &mut has_invalid_values,
                out.auto_disable_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_WINDOW_MINUTES)?
            && let Some(n) =
                parse_i64_setting(KEY_AUTO_DISABLE_WINDOW_MINUTES, &v, &mut has_invalid_values)
        {
            out.auto_disable_window_minutes = n;
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_FAILURE_TIMES)?
            && let Some(n) =
                parse_i64_setting(KEY_AUTO_DISABLE_FAILURE_TIMES, &v, &mut has_invalid_values)
        {
            out.auto_disable_failure_times = n;
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_DISABLE_MINUTES)?
            && let Some(n) = parse_i64_setting(
                KEY_AUTO_DISABLE_DISABLE_MINUTES,
                &v,
                &mut has_invalid_values,
            )
        {
            out.auto_disable_disable_minutes = n;
        }
        if let Some(v) = get_setting(conn, KEY_ANTHROPIC_COUNT_TOKENS_MOCK_ENABLED)? {
            out.anthropic_count_tokens_mock_enabled = parse_bool_setting(
                KEY_ANTHROPIC_COUNT_TOKENS_MOCK_ENABLED,
                &v,
                &mut has_invalid_values,
                out.anthropic_count_tokens_mock_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_LOG_LEVEL)? {
            match v.trim() {
                "none" | "off" => out.log_level = LogLevel::None,
                "debug" => out.log_level = LogLevel::Debug,
                "info" => out.log_level = LogLevel::Info,
                "warn" | "warning" => out.log_level = LogLevel::Warning,
                "error" => out.log_level = LogLevel::Error,
                _ => {
                    has_invalid_values = true;
                    warn_invalid_setting_once(KEY_LOG_LEVEL, &v, || {
                        "unknown log_level".to_string()
                    });
                }
            }
        }
        if let Some(v) = get_setting(conn, KEY_LOG_RETENTION_DAYS)?
            && let Some(n) = parse_i64_setting(KEY_LOG_RETENTION_DAYS, &v, &mut has_invalid_values)
        {
            out.log_retention_days = n;
        }
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_ENABLED)? {
            out.chat_bridge_enabled = parse_bool_setting(
                KEY_CHAT_BRIDGE_ENABLED,
                &v,
                &mut has_invalid_values,
                out.chat_bridge_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_TELEGRAM_ENABLED)? {
            out.chat_bridge_telegram_enabled = parse_bool_setting(
                KEY_CHAT_BRIDGE_TELEGRAM_ENABLED,
                &v,
                &mut has_invalid_values,
                out.chat_bridge_telegram_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_TELEGRAM_BOT_TOKEN)? {
            let s = v.trim();
            if !s.is_empty() {
                out.chat_bridge_telegram_bot_token = Some(s.to_string());
            }
        }
        out.chat_bridge_telegram_bot_token_configured = out
            .chat_bridge_telegram_bot_token
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_DISCORD_ENABLED)? {
            out.chat_bridge_discord_enabled = parse_bool_setting(
                KEY_CHAT_BRIDGE_DISCORD_ENABLED,
                &v,
                &mut has_invalid_values,
                out.chat_bridge_discord_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_DISCORD_BOT_TOKEN)? {
            let s = v.trim();
            if !s.is_empty() {
                out.chat_bridge_discord_bot_token = Some(s.to_string());
            }
        }
        out.chat_bridge_discord_bot_token_configured = out
            .chat_bridge_discord_bot_token
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_WHATSAPP_ENABLED)? {
            out.chat_bridge_whatsapp_enabled = parse_bool_setting(
                KEY_CHAT_BRIDGE_WHATSAPP_ENABLED,
                &v,
                &mut has_invalid_values,
                out.chat_bridge_whatsapp_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_WEIXIN_ENABLED)? {
            out.chat_bridge_weixin_enabled = parse_bool_setting(
                KEY_CHAT_BRIDGE_WEIXIN_ENABLED,
                &v,
                &mut has_invalid_values,
                out.chat_bridge_weixin_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_TURN_TIMEOUT_MINUTES)?
            && let Some(n) = parse_i64_setting(
                KEY_CHAT_BRIDGE_TURN_TIMEOUT_MINUTES,
                &v,
                &mut has_invalid_values,
            )
        {
            if n >= 0 {
                out.chat_bridge_turn_timeout_minutes = n;
            } else {
                has_invalid_values = true;
                warn_invalid_setting_once(KEY_CHAT_BRIDGE_TURN_TIMEOUT_MINUTES, &v, || {
                    "must be >= 0".to_string()
                });
            }
        }
        if let Some(v) = get_setting(conn, KEY_CHAT_BRIDGE_ALLOW_NEW_PROJECTS)? {
            out.chat_bridge_allow_new_projects = parse_bool_setting(
                KEY_CHAT_BRIDGE_ALLOW_NEW_PROJECTS,
                &v,
                &mut has_invalid_values,
                out.chat_bridge_allow_new_projects,
            );
        }
        if let Some(v) = get_setting(conn, KEY_NEWAPI_MANAGED_CHANNEL_MISSING_PROMPT_ENABLED)? {
            out.newapi_managed_channel_missing_prompt_enabled = parse_bool_setting(
                KEY_NEWAPI_MANAGED_CHANNEL_MISSING_PROMPT_ENABLED,
                &v,
                &mut has_invalid_values,
                out.newapi_managed_channel_missing_prompt_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_NEWAPI_MANAGED_CHANNEL_SYNC_MULTIPLIER_ENABLED)? {
            out.newapi_managed_channel_sync_multiplier_enabled = parse_bool_setting(
                KEY_NEWAPI_MANAGED_CHANNEL_SYNC_MULTIPLIER_ENABLED,
                &v,
                &mut has_invalid_values,
                out.newapi_managed_channel_sync_multiplier_enabled,
            );
        }
        if let Some(v) = get_setting(conn, KEY_NEWAPI_MANAGED_CHANNEL_SYNC_FREE_MULTIPLIER_ENABLED)?
        {
            out.newapi_managed_channel_sync_free_multiplier_enabled = parse_bool_setting(
                KEY_NEWAPI_MANAGED_CHANNEL_SYNC_FREE_MULTIPLIER_ENABLED,
                &v,
                &mut has_invalid_values,
                out.newapi_managed_channel_sync_free_multiplier_enabled,
            );
        }

        out.has_invalid_values = has_invalid_values;
        Ok(out)
    })
    .await
}

pub async fn update_app_settings(
    db_path: PathBuf,
    patch: AppSettingsPatch,
) -> anyhow::Result<AppSettings> {
    let db_path2 = db_path.clone();
    with_conn(db_path2, move |conn| {
        let updated_at_ms = now_ms();
        if let Some(v) = patch.pricing_auto_update_enabled {
            set_setting(
                conn,
                KEY_PRICING_AUTO_UPDATE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.pricing_auto_update_interval_hours {
            set_setting(
                conn,
                KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.close_behavior {
            set_setting(conn, KEY_CLOSE_BEHAVIOR, v.as_str(), updated_at_ms)?;
        }
        if let Some(v) = patch.auto_start_enabled {
            set_setting(
                conn,
                KEY_AUTO_START_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.auto_start_launch_mode {
            set_setting(conn, KEY_AUTO_START_LAUNCH_MODE, v.as_str(), updated_at_ms)?;
        }
        if let Some(v) = patch.server_lan_accessible {
            set_setting(
                conn,
                KEY_SERVER_LAN_ACCESSIBLE,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.ui_locale {
            set_setting(conn, KEY_UI_LOCALE, v.as_str(), updated_at_ms)?;
        }
        if let Some(v) = patch.app_auto_update_enabled {
            set_setting(
                conn,
                KEY_APP_AUTO_UPDATE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.gemini_cli_auto_update_enabled {
            set_setting(
                conn,
                KEY_GEMINI_CLI_AUTO_UPDATE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.claude_code_auto_update_enabled {
            set_setting(
                conn,
                KEY_CLAUDE_CODE_AUTO_UPDATE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.codex_auto_update_enabled {
            set_setting(
                conn,
                KEY_CODEX_AUTO_UPDATE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.cli_tools_npm_path {
            set_setting(conn, KEY_CLI_TOOLS_NPM_PATH, v.trim(), updated_at_ms)?;
        }
        if let Some(v) = patch.cli_tools_node_path {
            set_setting(conn, KEY_CLI_TOOLS_NODE_PATH, v.trim(), updated_at_ms)?;
        }
        if let Some(v) = patch.auto_disable_enabled {
            set_setting(
                conn,
                KEY_AUTO_DISABLE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.auto_disable_window_minutes {
            set_setting(
                conn,
                KEY_AUTO_DISABLE_WINDOW_MINUTES,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.auto_disable_failure_times {
            set_setting(
                conn,
                KEY_AUTO_DISABLE_FAILURE_TIMES,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.auto_disable_disable_minutes {
            set_setting(
                conn,
                KEY_AUTO_DISABLE_DISABLE_MINUTES,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.anthropic_count_tokens_mock_enabled {
            set_setting(
                conn,
                KEY_ANTHROPIC_COUNT_TOKENS_MOCK_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.log_level {
            set_setting(conn, KEY_LOG_LEVEL, v.as_str(), updated_at_ms)?;
        }
        if let Some(v) = patch.log_retention_days {
            set_setting(conn, KEY_LOG_RETENTION_DAYS, &v.to_string(), updated_at_ms)?;
        }
        if let Some(v) = patch.chat_bridge_enabled {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.chat_bridge_telegram_enabled {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_TELEGRAM_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.chat_bridge_telegram_bot_token {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_TELEGRAM_BOT_TOKEN,
                v.trim(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.chat_bridge_discord_enabled {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_DISCORD_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.chat_bridge_discord_bot_token {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_DISCORD_BOT_TOKEN,
                v.trim(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.chat_bridge_whatsapp_enabled {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_WHATSAPP_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.chat_bridge_weixin_enabled {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_WEIXIN_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.chat_bridge_turn_timeout_minutes {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_TURN_TIMEOUT_MINUTES,
                &v.to_string(),
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.chat_bridge_allow_new_projects {
            set_setting(
                conn,
                KEY_CHAT_BRIDGE_ALLOW_NEW_PROJECTS,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.newapi_managed_channel_missing_prompt_enabled {
            set_setting(
                conn,
                KEY_NEWAPI_MANAGED_CHANNEL_MISSING_PROMPT_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.newapi_managed_channel_sync_multiplier_enabled {
            set_setting(
                conn,
                KEY_NEWAPI_MANAGED_CHANNEL_SYNC_MULTIPLIER_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        if let Some(v) = patch.newapi_managed_channel_sync_free_multiplier_enabled {
            set_setting(
                conn,
                KEY_NEWAPI_MANAGED_CHANNEL_SYNC_FREE_MULTIPLIER_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
        }
        Ok(())
    })
    .await?;

    get_app_settings(db_path).await
}
