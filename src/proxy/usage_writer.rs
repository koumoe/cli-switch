use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::mpsc;

use crate::storage;

const USAGE_QUEUE_CAPACITY: usize = 256;
const SQLITE_BUSY_MAX_RETRIES: usize = 3;
const SQLITE_BUSY_RETRY_BASE_DELAY: Duration = Duration::from_millis(50);
const SQLITE_BUSY_RETRY_MAX_DELAY: Duration = Duration::from_secs(2);

static WRITERS: OnceLock<Mutex<HashMap<PathBuf, mpsc::Sender<storage::CreateUsageEvent>>>> =
    OnceLock::new();

pub(crate) fn try_enqueue(db_path: PathBuf, event: storage::CreateUsageEvent) -> bool {
    let tx = get_or_spawn_writer(&db_path);
    match tx.try_send(event) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => false,
        Err(mpsc::error::TrySendError::Closed(event)) => {
            // The writer task may have been cancelled (e.g. runtime shutdown). Recreate once.
            let tx = recreate_writer(db_path);
            tx.try_send(event).is_ok()
        }
    }
}

fn get_or_spawn_writer(db_path: &PathBuf) -> mpsc::Sender<storage::CreateUsageEvent> {
    let writers = WRITERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = writers.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(tx) = map.get(db_path) {
        if !tx.is_closed() {
            return tx.clone();
        }
        map.remove(db_path);
    }

    let (tx, rx) = mpsc::channel(USAGE_QUEUE_CAPACITY);
    map.insert(db_path.clone(), tx.clone());
    drop(map);

    spawn_writer_task(db_path.clone(), rx);
    tx
}

fn recreate_writer(db_path: PathBuf) -> mpsc::Sender<storage::CreateUsageEvent> {
    let writers = WRITERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = writers.lock().unwrap_or_else(|e| e.into_inner());
    map.remove(&db_path);

    let (tx, rx) = mpsc::channel(USAGE_QUEUE_CAPACITY);
    map.insert(db_path.clone(), tx.clone());
    drop(map);

    spawn_writer_task(db_path, rx);
    tx
}

fn spawn_writer_task(db_path: PathBuf, mut rx: mpsc::Receiver<storage::CreateUsageEvent>) {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            insert_with_retry(db_path.clone(), event).await;
        }
    });
}

async fn insert_with_retry(db_path: PathBuf, event: storage::CreateUsageEvent) {
    let mut delay = SQLITE_BUSY_RETRY_BASE_DELAY;
    for attempt in 0..=SQLITE_BUSY_MAX_RETRIES {
        let res = storage::insert_usage_event(db_path.clone(), event.clone()).await;
        match res {
            Ok(()) => return,
            Err(e) if attempt < SQLITE_BUSY_MAX_RETRIES && is_sqlite_busy(&e) => {
                tracing::debug!(
                    attempt = attempt + 1,
                    err = %e,
                    "insert usage event hit sqlite busy; retrying"
                );
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(SQLITE_BUSY_RETRY_MAX_DELAY);
            }
            Err(e) => {
                tracing::warn!(err = %e, "insert usage event failed");
                return;
            }
        }
    }
}

fn is_sqlite_busy(e: &anyhow::Error) -> bool {
    e.chain().any(|cause| {
        let Some(err) = cause.downcast_ref::<rusqlite::Error>() else {
            return false;
        };
        let rusqlite::Error::SqliteFailure(ffi_err, _) = err else {
            return false;
        };
        matches!(
            ffi_err.code,
            rusqlite::ffi::ErrorCode::DatabaseBusy | rusqlite::ffi::ErrorCode::DatabaseLocked
        )
    })
}
