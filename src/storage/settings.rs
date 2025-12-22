use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::logging::LogLevel;

use super::{now_ms, with_conn};

const KEY_PRICING_AUTO_UPDATE_ENABLED: &str = "pricing_auto_update_enabled";
const KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS: &str = "pricing_auto_update_interval_hours";
const KEY_CLOSE_BEHAVIOR: &str = "close_behavior";
const KEY_AUTO_START_ENABLED: &str = "auto_start_enabled";
const KEY_AUTO_START_LAUNCH_MODE: &str = "auto_start_launch_mode";
const KEY_APP_AUTO_UPDATE_ENABLED: &str = "app_auto_update_enabled";
const KEY_AUTO_DISABLE_ENABLED: &str = "auto_disable_enabled";
const KEY_AUTO_DISABLE_WINDOW_MINUTES: &str = "auto_disable_window_minutes";
const KEY_AUTO_DISABLE_FAILURE_TIMES: &str = "auto_disable_failure_times";
const KEY_AUTO_DISABLE_DISABLE_MINUTES: &str = "auto_disable_disable_minutes";
const KEY_LOG_LEVEL: &str = "log_level";

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
    pub app_auto_update_enabled: bool,
    pub auto_disable_enabled: bool,
    pub auto_disable_window_minutes: i64,
    pub auto_disable_failure_times: i64,
    pub auto_disable_disable_minutes: i64,
    pub log_level: LogLevel,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            pricing_auto_update_enabled: false,
            pricing_auto_update_interval_hours: 24,
            close_behavior: CloseBehavior::Ask,
            auto_start_enabled: false,
            auto_start_launch_mode: AutoStartLaunchMode::ShowWindow,
            app_auto_update_enabled: false,
            auto_disable_enabled: false,
            auto_disable_window_minutes: 3,
            auto_disable_failure_times: 5,
            auto_disable_disable_minutes: 30,
            log_level: LogLevel::Warning,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppSettingsPatch {
    pub pricing_auto_update_enabled: Option<bool>,
    pub pricing_auto_update_interval_hours: Option<i64>,
    pub close_behavior: Option<CloseBehavior>,
    pub auto_start_enabled: Option<bool>,
    pub auto_start_launch_mode: Option<AutoStartLaunchMode>,
    pub app_auto_update_enabled: Option<bool>,
    pub auto_disable_enabled: Option<bool>,
    pub auto_disable_window_minutes: Option<i64>,
    pub auto_disable_failure_times: Option<i64>,
    pub auto_disable_disable_minutes: Option<i64>,
    pub log_level: Option<LogLevel>,
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

fn parse_bool(v: &str) -> bool {
    let v = v.trim();
    v == "1" || v.eq_ignore_ascii_case("true")
}

pub async fn get_app_settings(db_path: PathBuf) -> anyhow::Result<AppSettings> {
    with_conn(db_path, move |conn| {
        let mut out = AppSettings::default();

        if let Some(v) = get_setting(conn, KEY_PRICING_AUTO_UPDATE_ENABLED)? {
            out.pricing_auto_update_enabled = parse_bool(&v);
        }
        if let Some(v) = get_setting(conn, KEY_PRICING_AUTO_UPDATE_INTERVAL_HOURS)?
            && let Ok(n) = v.trim().parse::<i64>()
        {
            out.pricing_auto_update_interval_hours = n;
        }
        if let Some(v) = get_setting(conn, KEY_CLOSE_BEHAVIOR)? {
            match v.trim() {
                "ask" => out.close_behavior = CloseBehavior::Ask,
                "minimize_to_tray" => out.close_behavior = CloseBehavior::MinimizeToTray,
                "quit" => out.close_behavior = CloseBehavior::Quit,
                _ => {}
            }
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_START_ENABLED)? {
            out.auto_start_enabled = parse_bool(&v);
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_START_LAUNCH_MODE)? {
            match v.trim() {
                "show_window" => out.auto_start_launch_mode = AutoStartLaunchMode::ShowWindow,
                "minimize_to_tray" => {
                    out.auto_start_launch_mode = AutoStartLaunchMode::MinimizeToTray;
                }
                _ => {}
            }
        }
        if let Some(v) = get_setting(conn, KEY_APP_AUTO_UPDATE_ENABLED)? {
            out.app_auto_update_enabled = parse_bool(&v);
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_ENABLED)? {
            out.auto_disable_enabled = parse_bool(&v);
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_WINDOW_MINUTES)?
            && let Ok(n) = v.trim().parse::<i64>()
        {
            out.auto_disable_window_minutes = n;
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_FAILURE_TIMES)?
            && let Ok(n) = v.trim().parse::<i64>()
        {
            out.auto_disable_failure_times = n;
        }
        if let Some(v) = get_setting(conn, KEY_AUTO_DISABLE_DISABLE_MINUTES)?
            && let Ok(n) = v.trim().parse::<i64>()
        {
            out.auto_disable_disable_minutes = n;
        }
        if let Some(v) = get_setting(conn, KEY_LOG_LEVEL)? {
            match v.trim() {
                "none" | "off" => out.log_level = LogLevel::None,
                "debug" => out.log_level = LogLevel::Debug,
                "info" => out.log_level = LogLevel::Info,
                "warn" | "warning" => out.log_level = LogLevel::Warning,
                "error" => out.log_level = LogLevel::Error,
                _ => {}
            }
        }

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
        if let Some(v) = patch.app_auto_update_enabled {
            set_setting(
                conn,
                KEY_APP_AUTO_UPDATE_ENABLED,
                if v { "true" } else { "false" },
                updated_at_ms,
            )?;
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
        if let Some(v) = patch.log_level {
            set_setting(conn, KEY_LOG_LEVEL, v.as_str(), updated_at_ms)?;
        }
        Ok(())
    })
    .await?;

    get_app_settings(db_path).await
}
