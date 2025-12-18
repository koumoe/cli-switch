use anyhow::Context as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use cliswitch::{app, server, storage};
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use rusqlite::params;
use tao::dpi::LogicalSize;
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
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
}

#[derive(Debug, Default)]
struct DesktopState {
    window_visible: bool,
    close_request_inflight: bool,
    close_prompt_open: bool,
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

fn quit_app(server_handle: &tokio::task::JoinHandle<()>, control_flow: &mut ControlFlow) {
    server_handle.abort();
    *control_flow = ControlFlow::Exit;
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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

        let updated_at_ms = now_ms();
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
    server_handle: &tokio::task::JoinHandle<()>,
    window: &tao::window::Window,
    webview: &wry::WebView,
    tray_id: &tray_icon::TrayIconId,
    tray_show: &MenuItem,
    tray_hide: &MenuItem,
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
                quit_app(server_handle, control_flow);
            }
        }
        UserEvent::CloseRequested(settings) => {
            state.close_request_inflight = false;
            match settings.close_behavior {
                storage::CloseBehavior::Quit => {
                    quit_app(server_handle, control_flow);
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
                                            pricing_auto_update_enabled: None,
                                            pricing_auto_update_interval_hours: None,
                                            close_behavior: Some(
                                                storage::CloseBehavior::MinimizeToTray,
                                            ),
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
                            quit_app(server_handle, control_flow);
                        }
                    }
                }
            }
        }
    }
}

pub async fn run(port: u16) -> anyhow::Result<()> {
    let data_dir = app::default_data_dir()?;
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("创建数据目录失败：{}", data_dir.display()))?;

    let db_path = app::db_path(&data_dir);
    storage::init_db(&db_path).with_context(|| "初始化 SQLite 失败")?;

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{addr}"))?;
    let actual_addr = listener.local_addr().context("读取监听地址失败")?;
    let base_url = format!("http://{actual_addr}");

    let server_db_path = db_path.clone();
    let server_handle = tokio::spawn(async move {
        if let Err(err) = server::serve_with_listener(listener, server_db_path, false).await {
            tracing::error!(err = %err, "backend serve failed");
        }
    });

    wait_for_health(&base_url).await?;

    // 创建菜单
    let menu = Menu::new();
    let edit_menu = Submenu::new("Edit", true);
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
        .with_minimizable(true);

    #[cfg(target_os = "macos")]
    let window_builder = window_builder.with_automatic_window_tabbing(false);

    let window = window_builder.build(&event_loop).context("创建窗口失败")?;

    let tray_menu = Menu::new();
    let tray_show = MenuItem::with_id("tray_show", "显示窗口", true, None);
    let tray_hide = MenuItem::with_id("tray_hide", "隐藏窗口", true, None);
    let tray_quit = MenuItem::with_id("tray_quit", "退出", true, None);
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
        .with_icon(build_default_tray_icon().context("创建托盘图标失败")?)
        .with_icon_as_template(true)
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

    let tray_show_id = tray_show.id().clone();
    let tray_hide_id = tray_hide.id().clone();
    let tray_quit_id = tray_quit.id().clone();
    let tray_id = tray_icon.id().clone();
    let mut state = DesktopState {
        window_visible: true,
        close_request_inflight: false,
        close_prompt_open: false,
    };
    tray_show.set_enabled(!state.window_visible);
    tray_hide.set_enabled(state.window_visible);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if handle_close_requested(&event, &mut state, &proxy, &db_path) {
            return;
        }

        if let Event::UserEvent(ev) = event {
            handle_user_event(
                ev,
                &mut state,
                control_flow,
                &server_handle,
                &window,
                &webview,
                &tray_id,
                &tray_show,
                &tray_hide,
                &tray_show_id,
                &tray_hide_id,
                &tray_quit_id,
                &db_path,
            );
        }

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

fn build_default_tray_icon() -> Result<tray_icon::Icon, tray_icon::BadIcon> {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    let cx = (size as f32 - 1.0) / 2.0;
    let cy = (size as f32 - 1.0) / 2.0;
    let r = 12.0f32;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist2 = dx * dx + dy * dy;
            let inside = dist2 <= r * r;
            let idx = ((y * size + x) * 4) as usize;
            if inside {
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 255;
            } else {
                rgba[idx + 3] = 0;
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, size, size)
}
