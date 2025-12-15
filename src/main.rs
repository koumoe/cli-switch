mod proxy;
mod server;
mod storage;

use anyhow::Context as _;
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use tracing::Level;

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

#[derive(Subcommand, Debug)]
enum Command {
    Serve {
        #[arg(long, default_value_t = 3210)]
        port: u16,
    },
    Migrate,
}

fn default_data_dir() -> anyhow::Result<PathBuf> {
    let proj = ProjectDirs::from("com", "cliswitch", "CliSwitch")
        .context("无法定位用户数据目录（ProjectDirs）")?;
    Ok(proj.data_dir().to_path_buf())
}

fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("cliswitch.sqlite3")
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

    match cli.command.unwrap_or(Command::Serve { port: 3210 }) {
        Command::Serve { port } => {
            let data_dir = default_data_dir()?;
            std::fs::create_dir_all(&data_dir)
                .with_context(|| format!("创建数据目录失败：{}", data_dir.display()))?;

            let db_path = db_path(&data_dir);
            storage::init_db(&db_path).with_context(|| "初始化 SQLite 失败")?;

            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            tracing::event!(
                Level::INFO,
                addr = %addr,
                db = %db_path.display(),
                "cliswitch listening"
            );

            server::serve(addr, db_path).await
        }
        Command::Migrate => {
            let data_dir = default_data_dir()?;
            std::fs::create_dir_all(&data_dir)
                .with_context(|| format!("创建数据目录失败：{}", data_dir.display()))?;
            let db_path = db_path(&data_dir);
            storage::init_db(&db_path).with_context(|| "初始化 SQLite 失败")?;
            println!("ok: {}", db_path.display());
            Ok(())
        }
    }
}
