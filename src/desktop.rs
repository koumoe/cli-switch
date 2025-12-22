use anyhow::Context as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use cliswitch::events::AppEvent;
use cliswitch::{events, server, storage, update};
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use rusqlite::params;
use serde::Serialize;
use tao::dpi::LogicalSize;
#[cfg(target_os = "macos")]
use tao::platform::macos::{EventLoopWindowTargetExtMacOS, WindowBuilderExtMacOS};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
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
    #[serde(rename = "ui-ready")]
    UiReady,
}

#[derive(Debug, Default)]
struct DesktopState {
    window_visible: bool,
    dock_visible: bool,
    close_request_inflight: bool,
    close_prompt_open: bool,
    locale: DesktopLocale,
    ui_ready: bool,
}

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum DesktopLocale {
    #[default]
    ZhCN,
    EnUS,
}

impl DesktopLocale {
    fn from_str(input: &str) -> Self {
        let lower = input.trim().to_ascii_lowercase();
        if lower == "zh" || lower.starts_with("zh-") {
            DesktopLocale::ZhCN
        } else {
            DesktopLocale::EnUS
        }
    }

    fn detect() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(key) {
                let lower = v.to_ascii_lowercase();
                if lower.contains("zh") {
                    return DesktopLocale::ZhCN;
                }
                if lower.contains("en") {
                    return DesktopLocale::EnUS;
                }
            }
        }
        DesktopLocale::ZhCN
    }

    fn edit_menu_title(self) -> &'static str {
        match self {
            DesktopLocale::ZhCN => "编辑",
            DesktopLocale::EnUS => "Edit",
        }
    }

    fn tray_show(self) -> &'static str {
        match self {
            DesktopLocale::ZhCN => "显示窗口",
            DesktopLocale::EnUS => "Show Window",
        }
    }

    fn tray_hide(self) -> &'static str {
        match self {
            DesktopLocale::ZhCN => "隐藏窗口",
            DesktopLocale::EnUS => "Hide Window",
        }
    }

    fn tray_quit(self) -> &'static str {
        match self {
            DesktopLocale::ZhCN => "退出",
            DesktopLocale::EnUS => "Quit",
        }
    }
}

fn apply_desktop_locale(locale: DesktopLocale, menus: LocalizableMenus<'_>) {
    menus.edit_menu.set_text(locale.edit_menu_title());
    menus.tray_show.set_text(locale.tray_show());
    menus.tray_hide.set_text(locale.tray_hide());
    menus.tray_quit.set_text(locale.tray_quit());
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

fn persist_close_behavior_sync(db_path: &std::path::Path, behavior: storage::CloseBehavior) {
    let value = match behavior {
        storage::CloseBehavior::Ask => "ask",
        storage::CloseBehavior::MinimizeToTray => "minimize_to_tray",
        storage::CloseBehavior::Quit => "quit",
    };

    let res: anyhow::Result<()> = (|| {
        let conn = rusqlite::Connection::open(db_path)?;
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS app_settings (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL,
              updated_at_ms INTEGER NOT NULL
            )
            "#,
            [],
        )?;

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
    db_path: &std::path::PathBuf,
) -> bool {
    let Event::WindowEvent { event, .. } = event else {
        return false;
    };
    let WindowEvent::CloseRequested = event else {
        return false;
    };

    if state.close_prompt_open || state.close_request_inflight {
        return true;
    }

    state.close_request_inflight = true;
    request_close_behavior(proxy.clone(), db_path.clone());
    true
}

fn handle_user_event(
    ev: UserEvent,
    state: &mut DesktopState,
    control_flow: &mut ControlFlow,
    proxy: &tao::event_loop::EventLoopProxy<UserEvent>,
    server_handle: &tokio::task::JoinHandle<()>,
    data_dir: &std::path::PathBuf,
    window: &tao::window::Window,
    webview: &wry::WebView,
    tray_id: &tray_icon::TrayIconId,
    edit_menu: &Submenu,
    tray_show: &MenuItem,
    tray_hide: &MenuItem,
    tray_quit: &MenuItem,
    tray_show_id: &MenuId,
    tray_hide_id: &MenuId,
    tray_quit_id: &MenuId,
    db_path: &std::path::PathBuf,
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
            let id: MenuId = e.id;
            if &id == tray_show_id {
                apply_window_visible(window, state, tray_show, tray_hide, true, true);
            } else if &id == tray_hide_id {
                apply_window_visible(window, state, tray_show, tray_hide, false, false);
            } else if &id == tray_quit_id {
                quit_app(data_dir.as_path(), server_handle, control_flow, false);
            }
        }
        UserEvent::CloseRequested(settings) => {
            state.close_request_inflight = false;
            match settings.close_behavior {
                storage::CloseBehavior::Quit => {
                    quit_app(data_dir.as_path(), server_handle, control_flow, false);
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
                                let db_path = db_path.clone();
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
                                persist_close_behavior_sync(
                                    db_path.as_path(),
                                    storage::CloseBehavior::Quit,
                                );
                            }
                            quit_app(data_dir.as_path(), server_handle, control_flow, false);
                        }
                    }
                }
                IpcMessage::SetLocale { locale } => {
                    let next = DesktopLocale::from_str(&locale);
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
                    quit_app(data_dir.as_path(), server_handle, control_flow, true);
                }
                IpcMessage::UiReady => {
                    state.ui_ready = true;
                    if let Some(status) = events::last_update_status() {
                        let _ = proxy
                            .send_event(UserEvent::BackendEvent(AppEvent::UpdateStatus(status)));
                    }
                }
            }
        }
        UserEvent::BackendEvent(ev) => {
            if !state.ui_ready {
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
    let settings = storage::get_app_settings(db_path.clone())
        .await
        .unwrap_or_default();
    let start_hidden = launched_by_autostart
        && settings.auto_start_launch_mode == storage::AutoStartLaunchMode::MinimizeToTray;
    let initial_window_visible = !start_hidden;

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{addr}"))?;
    let actual_addr = listener.local_addr().context("读取监听地址失败")?;
    let base_url = format!("http://{actual_addr}");
    tracing::info!(addr = %actual_addr, base_url = %base_url, "desktop backend ready");

    let server_db_path = db_path.clone();
    let server_handle = tokio::spawn(async move {
        if let Err(err) = server::serve_with_listener(listener, server_db_path, false).await {
            tracing::error!(err = %err, "backend serve failed");
        }
    });

    wait_for_health(&base_url).await?;

    // 创建菜单
    let menu = Menu::new();
    let initial_locale = DesktopLocale::detect();
    let edit_menu = Submenu::new(initial_locale.edit_menu_title(), true);
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
    let window_builder = WindowBuilder::new()
        .with_title("CliSwitch")
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
    let tray_show = MenuItem::with_id("tray_show", initial_locale.tray_show(), true, None);
    let tray_hide = MenuItem::with_id("tray_hide", initial_locale.tray_hide(), true, None);
    let tray_quit = MenuItem::with_id("tray_quit", initial_locale.tray_quit(), true, None);
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
        ui_ready: false,
    };
    tray_show.set_enabled(!state.window_visible);
    tray_hide.set_enabled(state.window_visible);

    event_loop.run(move |event, event_loop_target, control_flow| {
        *control_flow = ControlFlow::Wait;

        if handle_close_requested(&event, &mut state, &proxy, &db_path) {
            return;
        }

        if let Event::UserEvent(ev) = event {
            handle_user_event(
                ev,
                &mut state,
                control_flow,
                &proxy,
                &server_handle,
                &data_dir,
                &window,
                &webview,
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
