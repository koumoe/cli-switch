use anyhow::Context as _;
use directories::ProjectDirs;
use std::path::{Path, PathBuf};

pub fn default_data_dir() -> anyhow::Result<PathBuf> {
    let proj = ProjectDirs::from("com", "cliswitch", "CliSwitch")
        .context("无法定位用户数据目录（ProjectDirs）")?;
    Ok(proj.data_dir().to_path_buf())
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("cliswitch.sqlite3")
}

pub fn logs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}
