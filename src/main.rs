use anyhow::Context as _;
use clap::{Parser, Subcommand};
use cliswitch::{app, server, storage};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing::Level;

#[cfg(feature = "desktop")]
mod desktop;

#[derive(Parser, Debug)]
#[command(
    name = "cliswitch",
    version,
    about = "Local CLI proxy + routing + stats"
)]
struct Cli {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("CLISWITCH_LOG")
                .ok()
                .unwrap_or_else(|| "info".to_string()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or_else(default_command) {
        Command::Serve { port, open } => {
            let data_dir = app::default_data_dir()?;
            std::fs::create_dir_all(&data_dir)
                .with_context(|| format!("创建数据目录失败：{}", data_dir.display()))?;

            let db_path = app::db_path(&data_dir);
            storage::init_db(&db_path).with_context(|| "初始化 SQLite 失败")?;

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
        Command::Desktop { port } => desktop::run(port).await,
        Command::Migrate => {
            let data_dir = app::default_data_dir()?;
            std::fs::create_dir_all(&data_dir)
                .with_context(|| format!("创建数据目录失败：{}", data_dir.display()))?;
            let db_path = app::db_path(&data_dir);
            storage::init_db(&db_path).with_context(|| "初始化 SQLite 失败")?;
            println!("ok: {}", db_path.display());
            Ok(())
        }
    }
}
