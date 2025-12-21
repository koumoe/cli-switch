use std::sync::{Mutex, OnceLock};

use tokio::sync::broadcast;

use crate::update;

#[derive(Debug, Clone)]
pub enum AppEvent {
    UpdateStatus(update::UpdateStatus),
    UsageChanged { at_ms: i64 },
}

fn sender() -> &'static broadcast::Sender<AppEvent> {
    static SENDER: OnceLock<broadcast::Sender<AppEvent>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, _rx) = broadcast::channel(1024);
        tx
    })
}

fn last_update_status_cell() -> &'static Mutex<Option<update::UpdateStatus>> {
    static CELL: OnceLock<Mutex<Option<update::UpdateStatus>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

pub fn subscribe() -> broadcast::Receiver<AppEvent> {
    sender().subscribe()
}

pub fn last_update_status() -> Option<update::UpdateStatus> {
    last_update_status_cell()
        .lock()
        .ok()
        .and_then(|v| v.clone())
}

pub fn publish(event: AppEvent) {
    if let AppEvent::UpdateStatus(ref status) = event {
        if let Ok(mut guard) = last_update_status_cell().lock() {
            *guard = Some(status.clone());
        }
    }
    let _ = sender().send(event);
}
