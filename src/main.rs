#![cfg_attr(
    all(feature = "desktop", target_os = "windows"),
    windows_subsystem = "windows"
)]

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use cliswitch::{app, logging, server, storage};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing::Level;

#[cfg(feature = "desktop")]
mod desktop;

fn maybe_disable_macos_debug_system_logs() {
    #[cfg(target_os = "macos")]
    {
        let running_under_debugger = std::env::var_os("OS_ACTIVITY_DT_MODE").is_some();
        let already_configured = std::env::var_os("OS_ACTIVITY_MODE").is_some();

        if running_under_debugger && !already_configured {
            unsafe {
                std::env::set_var("OS_ACTIVITY_MODE", "disable");
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "cliswitch",
    version,
    about = "Local CLI proxy + routing + stats"
)]
struct Cli {
    #[arg(long, hide = true, default_value_t = false)]
    autostart: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[cfg(feature = "desktop")]
fn default_command() -> Command {
    Command::Desktop { port: 3210 }
}

#[cfg(not(feature = "desktop"))]
fn default_command() -> Command {
    Command::Serve {
        port: 3210,
        open: true,
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    Serve {
        #[arg(long, default_value_t = 3210)]
        port: u16,
        #[arg(long = "no-open", action = clap::ArgAction::SetFalse, default_value_t = true)]
        open: bool,
    },
    #[cfg(feature = "desktop")]
    Desktop {
        #[arg(long, default_value_t = 3210)]
        port: u16,
    },
    Migrate,
}

fn main() -> anyhow::Result<()> {
    maybe_disable_macos_debug_system_logs();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("初始化 Tokio Runtime 失败")?;
    runtime.block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let launched_by_autostart = cli.autostart;

    let cmd = cli.command.unwrap_or_else(default_command);

    let data_dir = app::default_data_dir()?;
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("创建数据目录失败：{}", data_dir.display()))?;
    let db_path = app::db_path(&data_dir);
    storage::init_db(&db_path).with_context(|| "初始化 SQLite 失败")?;

    let settings = storage::get_app_settings(db_path.clone())
        .await
        .unwrap_or_default();
    logging::init(&data_dir, settings.log_level)?;

    match cmd {
        Command::Serve { port, open } => {
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            tracing::event!(
                Level::INFO,
                addr = %addr,
                db = %db_path.display(),
                "cliswitch listening"
            );

            server::serve(addr, db_path, open).await
        }
        #[cfg(feature = "desktop")]
        Command::Desktop { port } => {
            desktop::run(port, data_dir, db_path, launched_by_autostart).await
        }
        Command::Migrate => {
            println!("ok: {}", db_path.display());
            Ok(())
        }
    }
}
