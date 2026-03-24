use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use crate::chat_bridge::weixin::{WeixinControl, WeixinStatus};
use crate::chat_bridge::whatsapp_web::{WhatsAppWebControl, WhatsAppWebStatus};
use crate::storage;
use crate::update;

#[derive(Clone)]
pub struct AppState {
    pub listen_addr: SocketAddr,
    pub db_path: Arc<PathBuf>,
    pub http_client: reqwest::Client,
    pub settings_notify: watch::Sender<u64>,
    pub settings_cache: watch::Sender<Arc<storage::AppSettings>>,
    pub settings_cache_rx: watch::Receiver<Arc<storage::AppSettings>>,
    pub channels_cache: watch::Sender<Arc<Vec<storage::Channel>>>,
    pub channels_cache_rx: watch::Receiver<Arc<Vec<storage::Channel>>>,
    pub update_runtime: Arc<tokio::sync::Mutex<update::UpdateRuntime>>,
    pub whatsapp_control_tx: mpsc::Sender<WhatsAppWebControl>,
    pub whatsapp_status_rx: watch::Receiver<WhatsAppWebStatus>,
    pub weixin_control_tx: mpsc::Sender<WeixinControl>,
    pub weixin_status_rx: watch::Receiver<WeixinStatus>,
}

impl AppState {
    pub(crate) fn db_path(&self) -> PathBuf {
        self.db_path.as_ref().clone()
    }

    pub(crate) fn data_dir(&self) -> PathBuf {
        data_dir_from_db_path(self.db_path.as_path())
    }

    pub(crate) fn settings_snapshot(&self) -> Arc<storage::AppSettings> {
        self.settings_cache_rx.borrow().clone()
    }

    pub(crate) fn channels_snapshot(&self) -> Arc<Vec<storage::Channel>> {
        self.channels_cache_rx.borrow().clone()
    }
}

pub(crate) fn data_dir_from_db_path(db_path: &Path) -> PathBuf {
    match db_path.parent() {
        Some(p) => p.to_path_buf(),
        None => {
            tracing::error!(db_path = %db_path.display(), "db_path has no parent; fallback to current dir");
            PathBuf::from(".")
        }
    }
}
