use anyhow::Context as _;
use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use cliswitch::events::AppEvent;
use cliswitch::i18n::AppLocale;
use cliswitch::{events, i18n, server, storage, update};
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use rusqlite::params;
use serde::Serialize;
use tao::dpi::LogicalSize;
#[cfg(target_os = "macos")]
use tao::platform::macos::{EventLoopWindowTargetExtMacOS, WindowBuilderExtMacOS};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{WindowBuilder, WindowId},
};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use wry::WebViewBuilder;

#[derive(Debug, Clone)]
enum UserEvent {
    TrayIcon(TrayIconEvent),
    Menu(MenuEvent),
    Ipc(String),
    CloseRequested(storage::AppSettings),
    BackendEvent(AppEvent),
    Sub2ApiAuthToken {
        request_id: String,
        window_id: WindowId,
        token: String,
        refresh_token: String,
    },
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum CloseDecisionAction {
    Cancel,
    MinimizeToTray,
    Quit,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
enum IpcMessage {
    #[serde(rename = "close-decision")]
    CloseDecision {
        action: CloseDecisionAction,
        remember: bool,
    },
    #[serde(rename = "set-locale")]
    SetLocale { locale: String },
    #[serde(rename = "request-quit")]
    RequestQuit,
    #[serde(rename = "request-restart-backend")]
    RequestRestartBackend,
    #[serde(rename = "request-sub2api-auth")]
    RequestSub2ApiAuth {
        request_id: String,
        base_url: String,
    },
    #[serde(rename = "ui-ready")]
    UiReady,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type")]
enum Sub2ApiAuthIpcMessage {
    #[serde(rename = "sub2api-auth-token")]
    Token {
        token: String,
        refresh_token: String,
    },
}

#[derive(Debug, Default)]
struct DesktopState {
    window_visible: bool,
    dock_visible: bool,
    close_request_inflight: bool,
    close_prompt_open: bool,
    locale: AppLocale,
    system_notifications: cliswitch::events::SystemNotificationSettings,
    ui_ready: bool,
    pending_remote_group_added: Vec<cliswitch::events::RemoteGroupAddedAlert>,
    pending_managed_channel_missing: Vec<cliswitch::events::RemoteManagedChannelMissingPrompt>,
    pending_managed_channel_multiplier:
        Vec<cliswitch::events::RemoteManagedChannelMultiplierPrompt>,
}

struct Sub2ApiAuthWindow {
    request_id: String,
    _window: tao::window::Window,
    _webview: wry::WebView,
}

#[derive(Debug, Serialize)]
struct Sub2ApiAuthResultEvent {
    request_id: String,
    token: Option<String>,
    refresh_token: Option<String>,
    cancelled: bool,
    error: Option<String>,
}

const SUB2API_AUTH_INIT_SCRIPT: &str = r#"
(() => {
  const AUTH_KEY = "auth_token";
  const REFRESH_KEY = "refresh_token";
  const EXTRA_CLEAR_KEYS = ["auth_user", "token_expires_at"];
  let lastSignature = "";

  const readAuth = () => {
    try {
      const accessToken = window.localStorage?.getItem(AUTH_KEY);
      const refreshToken = window.localStorage?.getItem(REFRESH_KEY);
      return {
        token: typeof accessToken === "string" ? accessToken.trim() : "",
        refresh_token: typeof refreshToken === "string" ? refreshToken.trim() : "",
      };
    } catch (_) {
      return { token: "", refresh_token: "" };
    }
  };

  const clearAuthState = () => {
    try {
      const storage = window.localStorage;
      if (storage) {
        storage.removeItem(AUTH_KEY);
        storage.removeItem(REFRESH_KEY);
        for (const key of EXTRA_CLEAR_KEYS) {
          storage.removeItem(key);
        }
      }
    } catch (_) {}

    try {
      window.sessionStorage?.removeItem("auth_expired");
    } catch (_) {}
  };

  const emit = () => {
    const payload = readAuth();
    if (!payload.token || !payload.refresh_token) return;
    const signature = `${payload.token}\n${payload.refresh_token}`;
    if (signature === lastSignature) return;
    lastSignature = signature;
    try {
      window.ipc?.postMessage(JSON.stringify({
        type: "sub2api-auth-token",
        token: payload.token,
        refresh_token: payload.refresh_token,
      }));
    } catch (_) {}
  };

  const scheduleEmit = () => window.setTimeout(emit, 0);

  try {
    const originalSetItem = window.Storage?.prototype?.setItem;
    if (typeof originalSetItem === "function") {
      window.Storage.prototype.setItem = function(key, value) {
        originalSetItem.call(this, key, value);
        if (this === window.localStorage && (key === AUTH_KEY || key === REFRESH_KEY)) {
          scheduleEmit();
        }
      };
    }
  } catch (_) {}

  clearAuthState();
  window.addEventListener("load", emit);
  window.addEventListener("storage", emit);
  window.setInterval(emit, 1000);
})();
"#;

fn dispatch_custom_event<T: Serialize>(webview: &wry::WebView, name: &str, detail: &T) {
    let detail_json = match serde_json::to_string(detail) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(err = %e, event = name, "serialize webview event detail failed");
            return;
        }
    };
    let detail_json_str = match serde_json::to_string(&detail_json) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(err = %e, event = name, "escape webview event detail json failed");
            return;
        }
    };
    let script = format!(
        r#"try {{ window.dispatchEvent(new CustomEvent({name:?}, {{ detail: JSON.parse({detail_json_str}) }})); }} catch (e) {{}}"#,
    );
    if let Err(e) = webview.evaluate_script(&script) {
        tracing::warn!(err = %e, event = name, "webview evaluate_script failed");
    }
}

fn dispatch_sub2api_auth_result(
    webview: &wry::WebView,
    request_id: impl Into<String>,
    token: Option<String>,
    refresh_token: Option<String>,
    cancelled: bool,
    error: Option<String>,
) {
    dispatch_custom_event(
        webview,
        "cliswitch-sub2api-auth-result",
        &Sub2ApiAuthResultEvent {
            request_id: request_id.into(),
            token,
            refresh_token,
            cancelled,
            error,
        },
    );
}

fn create_sub2api_auth_window(
    event_loop: &tao::event_loop::EventLoopWindowTarget<UserEvent>,
    proxy: &tao::event_loop::EventLoopProxy<UserEvent>,
    request_id: String,
    base_url: String,
) -> anyhow::Result<Sub2ApiAuthWindow> {
    let fixed_size = LogicalSize::new(1120.0, 760.0);
    let window = WindowBuilder::new()
        .with_title("sub2api Login")
        .with_inner_size(fixed_size)
        .with_min_inner_size(LogicalSize::new(960.0, 640.0))
        .with_window_icon(build_window_icon().ok())
        .build(event_loop)
        .context("创建 sub2api 登录窗口失败")?;
    let window_id = window.id();
    let proxy_for_auth = proxy.clone();
    let request_id_for_auth = request_id.clone();
    let webview = WebViewBuilder::new()
        .with_initialization_script(SUB2API_AUTH_INIT_SCRIPT)
        .with_url(&base_url)
        .with_ipc_handler(move |req| {
            let Ok(message) = serde_json::from_str::<Sub2ApiAuthIpcMessage>(req.body()) else {
                return;
            };
            match message {
                Sub2ApiAuthIpcMessage::Token {
                    token,
                    refresh_token,
                } => {
                    let token = token.trim().to_string();
                    let refresh_token = refresh_token.trim().to_string();
                    if token.is_empty() || refresh_token.is_empty() {
                        return;
                    }
                    tracing::info!(
                        request_id = %request_id_for_auth,
                        window_id = ?window_id,
                        token_len = token.len(),
                        refresh_token_len = refresh_token.len(),
                        "captured sub2api auth tokens from webview"
                    );
                    let _ = proxy_for_auth.send_event(UserEvent::Sub2ApiAuthToken {
                        request_id: request_id_for_auth.clone(),
                        window_id,
                        token,
                        refresh_token,
                    });
                }
            }
        })
        .build(&window)
        .context("创建 sub2api 登录 WebView 失败")?;

    tracing::info!(
        request_id = %request_id,
        base_url = %base_url,
        window_id = ?window_id,
        "opened sub2api auth window with cleared local auth state"
    );

    Ok(Sub2ApiAuthWindow {
        request_id,
        _window: window,
        _webview: webview,
    })
}

fn detect_desktop_locale() -> AppLocale {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(key)
            && let Some(locale) = AppLocale::parse(&v)
        {
            return locale;
        }
    }
    AppLocale::default()
}

fn fallback_edit_menu_title(locale: AppLocale) -> &'static str {
    match locale {
        AppLocale::ZhCN => "编辑",
        AppLocale::EnUS => "Edit",
    }
}

fn fallback_tray_show(locale: AppLocale) -> &'static str {
    match locale {
        AppLocale::ZhCN => "显示窗口",
        AppLocale::EnUS => "Show Window",
    }
}

fn fallback_tray_hide(locale: AppLocale) -> &'static str {
    match locale {
        AppLocale::ZhCN => "隐藏窗口",
        AppLocale::EnUS => "Hide Window",
    }
}

fn fallback_tray_quit(locale: AppLocale) -> &'static str {
    match locale {
        AppLocale::ZhCN => "退出",
        AppLocale::EnUS => "Quit",
    }
}

fn desktop_text(locale: AppLocale, key: &str, fallback: &str) -> String {
    i18n::render_optional(locale, key, &BTreeMap::<String, String>::new())
        .unwrap_or_else(|| fallback.to_string())
}

fn desktop_args<const N: usize>(pairs: [(&str, String); N]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in pairs {
        out.insert(key.to_string(), value);
    }
    out
}

fn desktop_text_args(
    locale: AppLocale,
    key: &str,
    args: &BTreeMap<String, String>,
    fallback: String,
) -> String {
    i18n::render_optional(locale, key, args).unwrap_or(fallback)
}

fn low_balance_notification_title(locale: AppLocale) -> String {
    desktop_text(
        locale,
        "desktop.notifications.lowBalance.title",
        match locale {
            AppLocale::ZhCN => "CliSwitch 余额告警",
            AppLocale::EnUS => "CliSwitch Low Balance",
        },
    )
}

fn remote_group_added_notification_title(locale: AppLocale) -> String {
    desktop_text(
        locale,
        "desktop.notifications.remoteGroupAdded.title",
        match locale {
            AppLocale::ZhCN => "CliSwitch 远端分组新增",
            AppLocale::EnUS => "CliSwitch Remote Group Added",
        },
    )
}

fn remote_group_added_body(
    locale: AppLocale,
    alert: &cliswitch::events::RemoteGroupAddedAlert,
) -> String {
    let args = desktop_args([
        ("base_url", alert.account_base_url.clone()),
        ("group", alert.group_name.clone()),
    ]);
    desktop_text_args(
        locale,
        "desktop.notifications.remoteGroupAdded.body",
        &args,
        match locale {
            AppLocale::ZhCN => format!(
                "检测到账号 {} 新增远端分组 {}，可按需创建托管渠道",
                alert.account_base_url, alert.group_name
            ),
            AppLocale::EnUS => format!(
                "Detected a new remote group {} on {}. Create a managed channel if needed.",
                alert.group_name, alert.account_base_url
            ),
        },
    )
}

fn low_balance_notification_body(
    locale: AppLocale,
    alert: &cliswitch::events::RemoteLowBalanceAlert,
) -> String {
    let args = desktop_args([
        ("base_url", alert.account_base_url.clone()),
        ("balance", alert.balance_text.clone()),
    ]);
    desktop_text_args(
        locale,
        "desktop.notifications.lowBalance.body",
        &args,
        match locale {
            AppLocale::ZhCN => {
                format!("{} 余额 {}", alert.account_base_url, alert.balance_text)
            }
            AppLocale::EnUS => {
                format!("{} balance {}", alert.account_base_url, alert.balance_text)
            }
        },
    )
}

fn managed_channel_notification_title(locale: AppLocale) -> String {
    desktop_text(
        locale,
        "desktop.notifications.managedChannel.title",
        match locale {
            AppLocale::ZhCN => "CliSwitch 渠道变更通知",
            AppLocale::EnUS => "CliSwitch Channel Change",
        },
    )
}

fn managed_resource_label(
    locale: AppLocale,
    provider: storage::ManagedRemoteProvider,
) -> &'static str {
    match (locale, provider) {
        (AppLocale::ZhCN, storage::ManagedRemoteProvider::Newapi) => "token",
        (AppLocale::ZhCN, storage::ManagedRemoteProvider::Sub2Api) => "key",
        (AppLocale::EnUS, storage::ManagedRemoteProvider::Newapi) => "token",
        (AppLocale::EnUS, storage::ManagedRemoteProvider::Sub2Api) => "key",
    }
}

fn managed_channel_missing_body(
    locale: AppLocale,
    event: &cliswitch::events::RemoteManagedChannelMissingPrompt,
) -> String {
    let args = desktop_args([("channel", event.channel_name.clone())]);
    let resource_label = managed_resource_label(locale, event.provider);
    if event.missing_group && event.missing_resource {
        return desktop_text_args(
            locale,
            "desktop.notifications.managedChannel.tokenAndGroupMissingBody",
            &args,
            match locale {
                AppLocale::ZhCN => format!(
                    "检测到渠道 {} 对应的远端 {} 和分组均未在列表中找到，请选择禁用或删除",
                    event.channel_name, resource_label
                ),
                AppLocale::EnUS => format!(
                    "Remote {} and group for channel {} were not found in the latest list. Disable or delete the local channel.",
                    resource_label, event.channel_name
                ),
            },
        );
    }
    if event.missing_group {
        return desktop_text_args(
            locale,
            "desktop.notifications.managedChannel.groupMissingBody",
            &args,
            match locale {
                AppLocale::ZhCN => format!(
                    "检测到渠道 {} 对应的远端分组未在列表中找到，请选择禁用或删除",
                    event.channel_name
                ),
                AppLocale::EnUS => format!(
                    "Remote group for channel {} was not found in the latest list. Disable or delete the local channel.",
                    event.channel_name
                ),
            },
        );
    }
    desktop_text_args(
        locale,
        "desktop.notifications.managedChannel.tokenMissingBody",
        &args,
        match locale {
            AppLocale::ZhCN => format!(
                "检测到渠道 {} 对应的远端 {} 未在列表中找到，请选择禁用或删除",
                event.channel_name, resource_label
            ),
            AppLocale::EnUS => format!(
                "Remote {} for channel {} was not found in the latest list. Disable or delete the local channel.",
                resource_label, event.channel_name
            ),
        },
    )
}

fn managed_channel_multiplier_body(
    locale: AppLocale,
    event: &cliswitch::events::RemoteManagedChannelMultiplierPrompt,
) -> String {
    let args = desktop_args([
        ("channel", event.channel_name.clone()),
        (
            "local_multiplier",
            format!("{:.2}", event.current_multiplier),
        ),
        (
            "remote_multiplier",
            format!("{:.2}", event.remote_multiplier),
        ),
    ]);
    desktop_text_args(
        locale,
        "desktop.notifications.managedChannel.multiplierBody",
        &args,
        match locale {
            AppLocale::ZhCN => format!(
                "检测到渠道 {} 的倍率不一致：本地 ×{:.2}，远端 ×{:.2}，请确认是否更新",
                event.channel_name, event.current_multiplier, event.remote_multiplier
            ),
            AppLocale::EnUS => format!(
                "Channel {} multiplier mismatch: local ×{:.2}, remote ×{:.2}. Please confirm the update.",
                event.channel_name, event.current_multiplier, event.remote_multiplier
            ),
        },
    )
}

fn upsert_pending_missing_prompt(
    queue: &mut Vec<cliswitch::events::RemoteManagedChannelMissingPrompt>,
    prompt: &cliswitch::events::RemoteManagedChannelMissingPrompt,
) {
    if let Some(existing) = queue
        .iter_mut()
        .find(|item| item.channel_id == prompt.channel_id)
    {
        *existing = prompt.clone();
        return;
    }
    queue.push(prompt.clone());
}

fn upsert_pending_multiplier_prompt(
    queue: &mut Vec<cliswitch::events::RemoteManagedChannelMultiplierPrompt>,
    prompt: &cliswitch::events::RemoteManagedChannelMultiplierPrompt,
) {
    if let Some(existing) = queue
        .iter_mut()
        .find(|item| item.channel_id == prompt.channel_id)
    {
        *existing = prompt.clone();
        return;
    }
    queue.push(prompt.clone());
}

fn push_pending_remote_group_added(
    queue: &mut Vec<cliswitch::events::RemoteGroupAddedAlert>,
    alert: &cliswitch::events::RemoteGroupAddedAlert,
) {
    let exists = queue.iter().any(|item| {
        item.provider == alert.provider
            && item.account_id == alert.account_id
            && item.group_id == alert.group_id
            && item.group_name == alert.group_name
    });
    if !exists {
        queue.push(alert.clone());
    }
}

fn show_system_notification(title: &str, body: &str) -> anyhow::Result<()> {
    crate::native_notifications::show(title, body)
}

fn apply_desktop_locale(locale: AppLocale, menus: LocalizableMenus<'_>) {
    menus.edit_menu.set_text(desktop_text(
        locale,
        "desktop.editMenuTitle",
        fallback_edit_menu_title(locale),
    ));
    menus.tray_show.set_text(desktop_text(
        locale,
        "desktop.tray.show",
        fallback_tray_show(locale),
    ));
    menus.tray_hide.set_text(desktop_text(
        locale,
        "desktop.tray.hide",
        fallback_tray_hide(locale),
    ));
    menus.tray_quit.set_text(desktop_text(
        locale,
        "desktop.tray.quit",
        fallback_tray_quit(locale),
    ));
}

#[derive(Clone, Copy)]
struct LocalizableMenus<'a> {
    edit_menu: &'a Submenu,
    tray_show: &'a MenuItem,
    tray_hide: &'a MenuItem,
    tray_quit: &'a MenuItem,
}

fn apply_window_visible(
    window: &tao::window::Window,
    state: &mut DesktopState,
    tray_show: &MenuItem,
    tray_hide: &MenuItem,
    visible: bool,
    focus: bool,
) {
    state.window_visible = visible;
    window.set_visible(visible);
    if visible && focus {
        window.set_focus();
    }
    tray_show.set_enabled(!visible);
    tray_hide.set_enabled(visible);
}

#[cfg(target_os = "macos")]
fn sync_macos_dock_visibility(
    target: &tao::event_loop::EventLoopWindowTarget<UserEvent>,
    state: &mut DesktopState,
) {
    let desired = state.window_visible;
    if state.dock_visible == desired {
        return;
    }
    target.set_dock_visibility(desired);
    state.dock_visible = desired;
}

fn request_close_behavior(
    proxy: tao::event_loop::EventLoopProxy<UserEvent>,
    db_path: std::path::PathBuf,
) {
    tokio::spawn(async move {
        let settings = match storage::get_app_settings(db_path).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed");
                storage::AppSettings::default()
            }
        };
        let _ = proxy.send_event(UserEvent::CloseRequested(settings));
    });
}

fn quit_app(
    data_dir: &std::path::Path,
    server_handle: &tokio::task::JoinHandle<()>,
    control_flow: &mut ControlFlow,
    restart_after_update: bool,
) {
    let res = if restart_after_update {
        update::apply_pending_on_exit_and_restart(data_dir)
    } else {
        update::apply_pending_on_exit(data_dir)
    };
    if let Err(e) = res {
        tracing::warn!(err = %e, "apply pending update on exit failed");
    }
    server_handle.abort();
    *control_flow = ControlFlow::Exit;
}

fn restart_backend(
    server_handle: &mut tokio::task::JoinHandle<()>,
    backend_port: u16,
    db_path: &std::path::Path,
) {
    server_handle.abort();

    let db_path = db_path.to_path_buf();
    *server_handle = tokio::spawn(async move {
        let settings = match storage::get_app_settings(db_path.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(err = %e, "load app settings failed; using defaults");
                storage::AppSettings::default()
            }
        };

        let bind_ip = if settings.server_lan_accessible {
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        } else {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        };
        let addr = SocketAddr::new(bind_ip, backend_port);

        let mut last_err: Option<anyhow::Error> = None;
        for _ in 0..50 {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => {
                    if let Err(err) = server::serve_with_listener(listener, db_path, false).await {
                        tracing::error!(err = %err, "backend serve failed");
                    }
                    return;
                }
                Err(e) => {
                    last_err = Some(anyhow::anyhow!(e));
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        if let Some(e) = last_err {
            tracing::error!(addr = %addr, err = %e, "restart backend failed: bind timeout");
        } else {
            tracing::error!(addr = %addr, "restart backend failed: bind timeout");
        }
    });
}

fn persist_close_behavior_sync(db_path: &std::path::Path, behavior: storage::CloseBehavior) {
    let value = match behavior {
        storage::CloseBehavior::Ask => "ask",
        storage::CloseBehavior::MinimizeToTray => "minimize_to_tray",
        storage::CloseBehavior::Quit => "quit",
    };

    let res: anyhow::Result<()> = (|| {
        let conn = rusqlite::Connection::open(db_path)?;

        let updated_at_ms = storage::now_ms();
        conn.execute(
            r#"
            INSERT INTO app_settings (key, value, updated_at_ms)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at_ms = excluded.updated_at_ms
            "#,
            params!["close_behavior", value, updated_at_ms],
        )?;
        Ok(())
    })();

    if let Err(e) = res {
        tracing::warn!(err = %e, "persist close_behavior failed");
    }
}

fn handle_close_requested(
    event: &Event<UserEvent>,
    state: &mut DesktopState,
    proxy: &tao::event_loop::EventLoopProxy<UserEvent>,
    db_path: &std::path::Path,
    main_window_id: WindowId,
    auth_windows: &mut HashMap<WindowId, Sub2ApiAuthWindow>,
    webview: &wry::WebView,
) -> bool {
    let Event::WindowEvent {
        event, window_id, ..
    } = event
    else {
        return false;
    };
    let WindowEvent::CloseRequested = event else {
        return false;
    };

    if *window_id != main_window_id {
        if let Some(auth_window) = auth_windows.remove(window_id) {
            tracing::info!(
                request_id = %auth_window.request_id,
                window_id = ?window_id,
                "sub2api auth window closed before new tokens were captured"
            );
            dispatch_sub2api_auth_result(webview, auth_window.request_id, None, None, true, None);
            return true;
        }
        return false;
    }

    if state.close_prompt_open || state.close_request_inflight {
        return true;
    }

    state.close_request_inflight = true;
    request_close_behavior(proxy.clone(), db_path.to_path_buf());
    true
}

#[allow(clippy::too_many_arguments)]
fn handle_user_event(
    ev: UserEvent,
    state: &mut DesktopState,
    control_flow: &mut ControlFlow,
    event_loop: &tao::event_loop::EventLoopWindowTarget<UserEvent>,
    proxy: &tao::event_loop::EventLoopProxy<UserEvent>,
    server_handle: &mut tokio::task::JoinHandle<()>,
    data_dir: &std::path::Path,
    window: &tao::window::Window,
    webview: &wry::WebView,
    auth_windows: &mut HashMap<WindowId, Sub2ApiAuthWindow>,
    backend_port: u16,
    tray_id: &tray_icon::TrayIconId,
    edit_menu: &Submenu,
    tray_show: &MenuItem,
    tray_hide: &MenuItem,
    tray_quit: &MenuItem,
    tray_show_id: &MenuId,
    tray_hide_id: &MenuId,
    tray_quit_id: &MenuId,
    db_path: &std::path::Path,
) {
    match ev {
        UserEvent::TrayIcon(e) => {
            if e.id() != tray_id {
                return;
            }
            let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            else {
                return;
            };
            let next = !state.window_visible;
            apply_window_visible(window, state, tray_show, tray_hide, next, true);
        }
        UserEvent::Menu(e) => {
            let id = &e.id;
            if id == tray_show_id {
                apply_window_visible(window, state, tray_show, tray_hide, true, true);
            } else if id == tray_hide_id {
                apply_window_visible(window, state, tray_show, tray_hide, false, false);
            } else if id == tray_quit_id {
                quit_app(data_dir, server_handle, control_flow, false);
            }
        }
        UserEvent::CloseRequested(settings) => {
            state.close_request_inflight = false;
            match settings.close_behavior {
                storage::CloseBehavior::Quit => {
                    quit_app(data_dir, server_handle, control_flow, false);
                }
                storage::CloseBehavior::MinimizeToTray => {
                    apply_window_visible(window, state, tray_show, tray_hide, false, false);
                }
                storage::CloseBehavior::Ask => {
                    state.close_prompt_open = true;
                    if webview
                        .evaluate_script(
                            r#"window.dispatchEvent(new Event("cliswitch-close-requested"));"#,
                        )
                        .is_err()
                    {
                        state.close_prompt_open = false;
                        apply_window_visible(window, state, tray_show, tray_hide, false, false);
                    }
                }
            }
        }
        UserEvent::Ipc(msg) => {
            let Ok(parsed) = serde_json::from_str::<IpcMessage>(&msg) else {
                return;
            };

            match parsed {
                IpcMessage::CloseDecision { action, remember } => {
                    state.close_prompt_open = false;
                    match action {
                        CloseDecisionAction::Cancel => {}
                        CloseDecisionAction::MinimizeToTray => {
                            apply_window_visible(window, state, tray_show, tray_hide, false, false);
                            if remember {
                                let db_path = db_path.to_path_buf();
                                tokio::spawn(async move {
                                    let _ = storage::update_app_settings(
                                        db_path,
                                        storage::AppSettingsPatch {
                                            close_behavior: Some(
                                                storage::CloseBehavior::MinimizeToTray,
                                            ),
                                            ..Default::default()
                                        },
                                    )
                                    .await;
                                });
                            }
                        }
                        CloseDecisionAction::Quit => {
                            if remember {
                                persist_close_behavior_sync(db_path, storage::CloseBehavior::Quit);
                            }
                            quit_app(data_dir, server_handle, control_flow, false);
                        }
                    }
                }
                IpcMessage::SetLocale { locale } => {
                    let next = AppLocale::parse_or_default(&locale);
                    if next != state.locale {
                        state.locale = next;
                        apply_desktop_locale(
                            next,
                            LocalizableMenus {
                                edit_menu,
                                tray_show,
                                tray_hide,
                                tray_quit,
                            },
                        );
                    }
                }
                IpcMessage::RequestQuit => {
                    quit_app(data_dir, server_handle, control_flow, true);
                }
                IpcMessage::RequestRestartBackend => {
                    restart_backend(server_handle, backend_port, db_path);
                }
                IpcMessage::RequestSub2ApiAuth {
                    request_id,
                    base_url,
                } => {
                    let base_url = base_url.trim().to_string();
                    if base_url.is_empty() {
                        dispatch_sub2api_auth_result(
                            webview,
                            request_id,
                            None,
                            None,
                            false,
                            Some("base_url is required".to_string()),
                        );
                        return;
                    }

                    let already_open = auth_windows
                        .values()
                        .any(|item| item.request_id == request_id);
                    if already_open {
                        return;
                    }

                    tracing::info!(
                        request_id = %request_id,
                        base_url = %base_url,
                        "received sub2api desktop auth request"
                    );

                    match create_sub2api_auth_window(
                        event_loop,
                        proxy,
                        request_id.clone(),
                        base_url,
                    ) {
                        Ok(auth_window) => {
                            auth_windows.insert(auth_window._window.id(), auth_window);
                        }
                        Err(err) => {
                            dispatch_sub2api_auth_result(
                                webview,
                                request_id,
                                None,
                                None,
                                false,
                                Some(err.to_string()),
                            );
                        }
                    }
                }
                IpcMessage::UiReady => {
                    state.ui_ready = true;
                    if let Some(status) = events::last_update_status() {
                        let _ = proxy
                            .send_event(UserEvent::BackendEvent(AppEvent::UpdateStatus(status)));
                    }
                    for prompt in std::mem::take(&mut state.pending_managed_channel_missing) {
                        dispatch_custom_event(
                            webview,
                            "cliswitch-remote-managed-channel-missing",
                            &prompt,
                        );
                    }
                    for prompt in std::mem::take(&mut state.pending_managed_channel_multiplier) {
                        dispatch_custom_event(
                            webview,
                            "cliswitch-remote-managed-channel-multiplier",
                            &prompt,
                        );
                    }
                    for alert in std::mem::take(&mut state.pending_remote_group_added) {
                        dispatch_custom_event(webview, "cliswitch-remote-group-added", &alert);
                    }
                }
            }
        }
        UserEvent::Sub2ApiAuthToken {
            request_id,
            window_id,
            token,
            refresh_token,
        } => {
            let Some(_auth_window) = auth_windows.remove(&window_id) else {
                return;
            };
            dispatch_sub2api_auth_result(
                webview,
                request_id,
                Some(token),
                Some(refresh_token),
                false,
                None,
            );
        }
        UserEvent::BackendEvent(ev) => {
            if let AppEvent::SystemNotificationSettingsChanged(ref next) = ev {
                state.system_notifications = next.clone();
            }
            if let AppEvent::RemoteLowBalanceAlert(ref alert) = ev {
                if state.system_notifications.enabled
                    && state.system_notifications.remote_low_balance_enabled
                {
                    let title = low_balance_notification_title(state.locale);
                    let body = low_balance_notification_body(state.locale, alert);
                    if let Err(err) = show_system_notification(&title, &body) {
                        tracing::warn!(err = %err, account_id = %alert.account_id, "show low balance system notification failed");
                    }
                }
            }
            if let AppEvent::RemoteGroupAddedAlert(ref alert) = ev {
                if state.system_notifications.enabled
                    && state.system_notifications.remote_group_added_enabled
                {
                    let title = remote_group_added_notification_title(state.locale);
                    let body = remote_group_added_body(state.locale, alert);
                    if let Err(err) = show_system_notification(&title, &body) {
                        tracing::warn!(err = %err, account_id = %alert.account_id, group_name = %alert.group_name, "show remote group added system notification failed");
                    }
                }
            }
            if let AppEvent::RemoteManagedChannelMissingPrompt(ref prompt) = ev {
                if state.system_notifications.enabled
                    && state
                        .system_notifications
                        .remote_managed_channel_missing_enabled
                {
                    let title = managed_channel_notification_title(state.locale);
                    let body = managed_channel_missing_body(state.locale, prompt);
                    if let Err(err) = show_system_notification(&title, &body) {
                        tracing::warn!(err = %err, channel_id = %prompt.channel_id, "show managed channel missing system notification failed");
                    }
                }
                apply_window_visible(window, state, tray_show, tray_hide, true, true);
            }
            if let AppEvent::RemoteManagedChannelMultiplierPrompt(ref prompt) = ev {
                if state.system_notifications.enabled
                    && state
                        .system_notifications
                        .remote_managed_channel_multiplier_enabled
                {
                    let title = managed_channel_notification_title(state.locale);
                    let body = managed_channel_multiplier_body(state.locale, prompt);
                    if let Err(err) = show_system_notification(&title, &body) {
                        tracing::warn!(err = %err, channel_id = %prompt.channel_id, "show managed channel multiplier system notification failed");
                    }
                }
            }

            if !state.ui_ready {
                match ev {
                    AppEvent::RemoteGroupAddedAlert(ref alert) => {
                        push_pending_remote_group_added(
                            &mut state.pending_remote_group_added,
                            alert,
                        );
                    }
                    AppEvent::RemoteManagedChannelMissingPrompt(ref prompt) => {
                        upsert_pending_missing_prompt(
                            &mut state.pending_managed_channel_missing,
                            prompt,
                        );
                    }
                    AppEvent::RemoteManagedChannelMultiplierPrompt(ref prompt) => {
                        upsert_pending_multiplier_prompt(
                            &mut state.pending_managed_channel_multiplier,
                            prompt,
                        );
                    }
                    _ => {}
                }
                return;
            }
            match ev {
                AppEvent::UpdateStatus(status) => {
                    dispatch_custom_event(webview, "cliswitch-update-status", &status);
                }
                AppEvent::UsageChanged { at_ms } => {
                    dispatch_custom_event(
                        webview,
                        "cliswitch-usage-changed",
                        &serde_json::json!({ "at_ms": at_ms }),
                    );
                }
                AppEvent::ChannelsChanged { at_ms } => {
                    dispatch_custom_event(
                        webview,
                        "cliswitch-channels-changed",
                        &serde_json::json!({ "at_ms": at_ms }),
                    );
                }
                AppEvent::NpmEnvInstallProgress(progress) => {
                    dispatch_custom_event(webview, "cliswitch-npm-env-install-progress", &progress);
                }
                AppEvent::SystemNotificationSettingsChanged(_) => {}
                AppEvent::RemoteLowBalanceAlert(alert) => {
                    dispatch_custom_event(webview, "cliswitch-remote-low-balance-alert", &alert);
                }
                AppEvent::RemoteGroupAddedAlert(alert) => {
                    dispatch_custom_event(webview, "cliswitch-remote-group-added", &alert);
                }
                AppEvent::RemoteManagedChannelCreated(_) => {}
                AppEvent::RemoteManagedChannelMissingPrompt(prompt) => {
                    dispatch_custom_event(
                        webview,
                        "cliswitch-remote-managed-channel-missing",
                        &prompt,
                    );
                }
                AppEvent::RemoteManagedChannelMultiplierPrompt(prompt) => {
                    dispatch_custom_event(
                        webview,
                        "cliswitch-remote-managed-channel-multiplier",
                        &prompt,
                    );
                }
            }
        }
    }
}

pub async fn run(
    port: u16,
    data_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    launched_by_autostart: bool,
) -> anyhow::Result<()> {
    if let Err(err) = crate::native_notifications::initialize() {
        tracing::warn!(err = %err, "initialize native notifications failed");
    }

    let settings = match storage::get_app_settings(db_path.clone()).await {
        Ok(s) => Some(s),
        Err(e) => {
            // Safe-mode: if settings can't be loaded, use defaults.
            tracing::warn!(err = %e, "load app settings failed; using defaults");
            None
        }
    };

    let start_hidden = if !launched_by_autostart {
        false
    } else {
        match settings.as_ref() {
            Some(settings) => {
                settings.auto_start_launch_mode == storage::AutoStartLaunchMode::MinimizeToTray
            }
            None => {
                // Safe-mode: if settings can't be loaded, don't hide the window.
                false
            }
        }
    };
    let initial_window_visible = !start_hidden;

    let bind_ip = if settings.as_ref().is_some_and(|s| s.server_lan_accessible) {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };

    let addr = SocketAddr::new(bind_ip, port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{addr}"))?;
    let actual_addr = listener.local_addr().context("读取监听地址失败")?;
    // Even if the server binds to 0.0.0.0, the desktop WebView should still connect via localhost.
    let base_url = format!("http://127.0.0.1:{}", actual_addr.port());
    tracing::info!(addr = %actual_addr, base_url = %base_url, "desktop backend ready");

    let server_db_path = db_path.clone();
    let mut server_handle = tokio::spawn(async move {
        if let Err(err) = server::serve_with_listener(listener, server_db_path, false).await {
            tracing::error!(err = %err, "backend serve failed");
        }
    });

    wait_for_health(&base_url).await?;

    // 创建菜单
    let menu = Menu::new();
    let initial_locale = settings
        .as_ref()
        .map(|s| s.ui_locale)
        .unwrap_or_else(detect_desktop_locale);
    let edit_menu = Submenu::new(
        desktop_text(
            initial_locale,
            "desktop.editMenuTitle",
            fallback_edit_menu_title(initial_locale),
        ),
        true,
    );
    edit_menu
        .append_items(&[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ])
        .ok();
    menu.append(&edit_menu).ok();

    #[cfg(target_os = "macos")]
    menu.init_for_nsapp();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    MenuEvent::set_event_handler({
        let proxy = proxy.clone();
        Some(move |event| {
            let _ = proxy.send_event(UserEvent::Menu(event));
        })
    });

    TrayIconEvent::set_event_handler({
        let proxy = proxy.clone();
        Some(move |event| {
            let _ = proxy.send_event(UserEvent::TrayIcon(event));
        })
    });

    let fixed_size = LogicalSize::new(1000.0, 680.0);
    let window_icon = match build_window_icon() {
        Ok(icon) => Some(icon),
        Err(e) => {
            tracing::warn!(err = %e, "build window icon failed");
            None
        }
    };
    let window_builder = WindowBuilder::new()
        .with_title("CliSwitch")
        .with_window_icon(window_icon)
        .with_inner_size(fixed_size)
        .with_min_inner_size(fixed_size)
        .with_max_inner_size(fixed_size)
        .with_resizable(false)
        .with_maximizable(false)
        .with_minimizable(true)
        .with_visible(initial_window_visible);

    #[cfg(target_os = "macos")]
    let window_builder = window_builder.with_automatic_window_tabbing(false);

    let window = window_builder.build(&event_loop).context("创建窗口失败")?;

    let tray_menu = Menu::new();
    let tray_show = MenuItem::with_id(
        "tray_show",
        desktop_text(
            initial_locale,
            "desktop.tray.show",
            fallback_tray_show(initial_locale),
        ),
        true,
        None,
    );
    let tray_hide = MenuItem::with_id(
        "tray_hide",
        desktop_text(
            initial_locale,
            "desktop.tray.hide",
            fallback_tray_hide(initial_locale),
        ),
        true,
        None,
    );
    let tray_quit = MenuItem::with_id(
        "tray_quit",
        desktop_text(
            initial_locale,
            "desktop.tray.quit",
            fallback_tray_quit(initial_locale),
        ),
        true,
        None,
    );
    tray_menu
        .append_items(&[
            &tray_show,
            &tray_hide,
            &PredefinedMenuItem::separator(),
            &tray_quit,
        ])
        .ok();

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("CliSwitch")
        .with_icon(build_tray_icon().context("创建托盘图标失败")?)
        .with_icon_as_template(false)
        .with_menu_on_left_click(false)
        .build()
        .context("初始化托盘失败")?;

    let proxy_for_webview = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_url(&base_url)
        .with_ipc_handler(move |req| {
            let msg = req.body().clone();
            let _ = proxy_for_webview.send_event(UserEvent::Ipc(msg));
        })
        .build(&window)
        .context("创建 WebView 失败")?;

    {
        let proxy = proxy.clone();
        tokio::spawn(async move {
            let mut rx = events::subscribe();
            let mut last_usage_emit = tokio::time::Instant::now() - Duration::from_secs(10);
            loop {
                let ev = match rx.recv().await {
                    Ok(e) => e,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                };

                if let AppEvent::UsageChanged { .. } = ev {
                    let now = tokio::time::Instant::now();
                    if now.duration_since(last_usage_emit) < Duration::from_secs(1) {
                        continue;
                    }
                    last_usage_emit = now;
                }

                let _ = proxy.send_event(UserEvent::BackendEvent(ev));
            }
        });
    }

    let tray_show_id = tray_show.id().clone();
    let tray_hide_id = tray_hide.id().clone();
    let tray_quit_id = tray_quit.id().clone();
    let tray_id = tray_icon.id().clone();
    let mut state = DesktopState {
        window_visible: initial_window_visible,
        dock_visible: true,
        close_request_inflight: false,
        close_prompt_open: false,
        locale: initial_locale,
        system_notifications: settings
            .as_ref()
            .map(cliswitch::events::SystemNotificationSettings::from_settings)
            .unwrap_or_default(),
        ui_ready: false,
        pending_remote_group_added: Vec::new(),
        pending_managed_channel_missing: Vec::new(),
        pending_managed_channel_multiplier: Vec::new(),
    };
    tray_show.set_enabled(!state.window_visible);
    tray_hide.set_enabled(state.window_visible);
    let mut auth_windows = HashMap::<WindowId, Sub2ApiAuthWindow>::new();

    event_loop.run(move |event, event_loop_target, control_flow| {
        *control_flow = ControlFlow::Wait;

        if handle_close_requested(
            &event,
            &mut state,
            &proxy,
            &db_path,
            window.id(),
            &mut auth_windows,
            &webview,
        ) {
            return;
        }

        if let Event::UserEvent(ev) = event {
            handle_user_event(
                ev,
                &mut state,
                control_flow,
                event_loop_target,
                &proxy,
                &mut server_handle,
                &data_dir,
                &window,
                &webview,
                &mut auth_windows,
                actual_addr.port(),
                &tray_id,
                &edit_menu,
                &tray_show,
                &tray_hide,
                &tray_quit,
                &tray_show_id,
                &tray_hide_id,
                &tray_quit_id,
                &db_path,
            );
        }

        #[cfg(target_os = "macos")]
        sync_macos_dock_visibility(event_loop_target, &mut state);

        let _ = &webview;
        let _ = &menu;
        let _ = &tray_icon;
    })
}

async fn wait_for_health(base_url: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder().build()?;
    let url = format!("{base_url}/api/health");

    for _ in 0..50 {
        match client
            .get(&url)
            .timeout(Duration::from_millis(200))
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    Err(anyhow::anyhow!("后端启动超时：{url}"))
}

fn build_window_icon() -> anyhow::Result<tao::window::Icon> {
    let target_size = 256u32;

    let bytes = include_bytes!("../assets/logo.png");
    let img = image::load_from_memory(bytes).context("读取 assets/logo.png 失败")?;
    let img = img.resize_exact(
        target_size,
        target_size,
        image::imageops::FilterType::Lanczos3,
    );
    let rgba = img.to_rgba8().into_raw();
    tao::window::Icon::from_rgba(rgba, target_size, target_size)
        .map_err(|e| anyhow::anyhow!("构造窗口 Icon 失败：{e}"))
}

fn build_tray_icon() -> anyhow::Result<tray_icon::Icon> {
    let target_size = if cfg!(target_os = "macos") {
        18u32
    } else {
        32u32
    };

    let bytes = include_bytes!("../assets/logo.png");
    let img = image::load_from_memory(bytes).context("读取 assets/logo.png 失败")?;
    let img = img.resize_exact(
        target_size,
        target_size,
        image::imageops::FilterType::Lanczos3,
    );
    let rgba = img.to_rgba8().into_raw();
    let icon = tray_icon::Icon::from_rgba(rgba, target_size, target_size)
        .map_err(|e| anyhow::anyhow!("构造托盘 Icon 失败：{e}"))?;
    Ok(icon)
}
