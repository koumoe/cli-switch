use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::watch;

use crate::update;

#[derive(Clone)]
pub struct AppState {
    pub listen_addr: SocketAddr,
    pub db_path: Arc<PathBuf>,
    pub http_client: reqwest::Client,
    pub settings_notify: watch::Sender<u64>,
    pub update_runtime: Arc<tokio::sync::Mutex<update::UpdateRuntime>>,
}

impl AppState {
    pub(crate) fn db_path(&self) -> PathBuf {
        self.db_path.as_ref().clone()
    }

    pub(crate) fn data_dir(&self) -> PathBuf {
        data_dir_from_db_path(self.db_path.as_path())
    }
}

pub(crate) fn data_dir_from_db_path(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default()
}
