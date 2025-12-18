use anyhow::Context as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use cliswitch::{app, server, storage};
use muda::{Menu, PredefinedMenuItem, Submenu};
use tao::dpi::LogicalSize;
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

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

    let event_loop = EventLoop::new();
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

    let webview = WebViewBuilder::new()
        .with_url(&base_url)
        .build(&window)
        .context("创建 WebView 失败")?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            server_handle.abort();
            *control_flow = ControlFlow::Exit;
        }

        let _ = &webview;
        let _ = &menu;
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
