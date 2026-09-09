//! Opt-in bridge for Codex's documented `notify` completion hook.
//! This module never installs a hook and never stores notification bodies.
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub(crate) enum NotifyHttpError {
    #[error("invalid activity notification token")]
    InvalidToken,
    #[error("invalid Codex notify JSON: {0}")]
    InvalidPayload(String),
    #[error("missing Codex notify field: {0}")]
    MissingField(&'static str),
    #[error("thread was not observed or still has a running request")]
    NotObserved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RendezvousFile {
    pid: u32,
    port: u16,
    token: String,
}

fn process_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
        result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let mut code = 0;
        let ok = unsafe { GetExitCodeProcess(handle, &mut code) } != 0;
        unsafe {
            CloseHandle(handle);
        }
        const STILL_ACTIVE: u32 = 259;
        return ok && code == STILL_ACTIVE;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        true
    }
}

fn read_live_file(path: &Path) -> Option<RendezvousFile> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > 8 * 1024 {
        return None;
    }
    let file: RendezvousFile = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    process_is_alive(file.pid).then_some(file)
}

#[derive(Debug)]
struct Registration {
    path: PathBuf,
    token: String,
}

#[derive(Debug)]
pub(crate) struct RegistrationGuard {
    path: PathBuf,
    token: String,
}

impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        if let Ok(file) = std::fs::read_to_string(&self.path)
            && serde_json::from_str::<RendezvousFile>(&file)
                .ok()
                .is_some_and(|value| value.token == self.token)
        {
            let _ = std::fs::remove_file(&self.path);
        }
        if let Ok(mut registrations) = registrations().lock() {
            registrations.retain(|entry| entry.path != self.path);
        }
    }
}

fn registrations() -> &'static Mutex<Vec<Registration>> {
    static REG: OnceLock<Mutex<Vec<Registration>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Vec::new()))
}

fn rendezvous_path(data_dir: &Path) -> PathBuf {
    data_dir.join(format!(
        "cliswitch-activity-notify-{}-{}.json",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}

/// Register a local-only rendezvous file for this server. The returned guard owns cleanup.
pub(crate) fn register(data_dir: &Path, port: u16) -> anyhow::Result<RegistrationGuard> {
    std::fs::create_dir_all(data_dir)?;
    let token = uuid::Uuid::new_v4().to_string();
    let path = rendezvous_path(data_dir);
    let temp_path = path.with_extension("json.tmp");
    let file = RendezvousFile {
        pid: std::process::id(),
        port,
        token: token.clone(),
    };
    let bytes = serde_json::to_vec(&file)?;
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut handle = options.open(&temp_path)?;
    use std::io::Write as _;
    handle.write_all(&bytes)?;
    handle.sync_all().ok();
    std::fs::rename(&temp_path, &path)?;
    let guard = RegistrationGuard { path, token };
    registrations().lock().unwrap().push(Registration {
        path: guard.path.clone(),
        token: guard.token.clone(),
    });
    Ok(guard)
}

pub(crate) fn tokens_for(data_dir: &Path) -> Vec<String> {
    let mut tokens: Vec<String> = registrations()
        .lock()
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| entry.path.parent() == Some(data_dir))
                .map(|entry| entry.token.clone())
                .collect()
        })
        .unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .starts_with("cliswitch-activity-notify-")
                    && n.to_string_lossy().ends_with(".json")
            }) {
                continue;
            }
            if let Some(file) = read_live_file(&path) {
                tokens.push(file.token);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn safe_turn_id(value: &str) -> Option<&str> {
    (!value.is_empty() && value.len() <= 256 && value.chars().all(|ch| !ch.is_control()))
        .then_some(value)
}

#[derive(Debug, Deserialize)]
struct NotifyPayload {
    r#type: Option<String>,
    #[serde(rename = "thread-id")]
    thread_id: Option<String>,
    #[serde(rename = "turn-id")]
    turn_id: Option<String>,
}

struct ParsedNotify {
    thread_id: String,
    turn_id: String,
}

fn parse_payload(payload: &str) -> Result<Option<ParsedNotify>, NotifyHttpError> {
    let payload: NotifyPayload = serde_json::from_str(payload)
        .map_err(|error| NotifyHttpError::InvalidPayload(error.to_string()))?;
    if payload.r#type.as_deref() != Some("agent-turn-complete") {
        return Ok(None);
    }
    let thread_id = payload
        .thread_id
        .filter(|v| !v.is_empty())
        .ok_or(NotifyHttpError::MissingField("thread-id"))?;
    let turn_id = payload
        .turn_id
        .filter(|v| safe_turn_id(v).is_some())
        .ok_or(NotifyHttpError::MissingField("turn-id"))?;
    Ok(Some(ParsedNotify { thread_id, turn_id }))
}

/// Validate and deliver a request received by this process. Request bodies are not retained.
pub(crate) fn accept_codex_notification(
    _data_dir: &Path,
    token: &str,
    auth_token: &str,
    payload: &str,
) -> Result<(), NotifyHttpError> {
    if token.is_empty() || auth_token != token {
        return Err(NotifyHttpError::InvalidToken);
    }
    let Some(parsed) = parse_payload(payload)? else {
        return Ok(());
    };
    crate::activity::deliver_codex_turn(&parsed.thread_id, &parsed.turn_id)
        .map_err(|_| NotifyHttpError::NotObserved)
}

/// Send a Codex notify JSON payload to every live CliSwitch instance in `data_dir`.
/// The payload is parsed only for allowlisted fields and is never persisted.
pub async fn deliver_codex_notification(data_dir: &Path, payload: &str) -> anyhow::Result<()> {
    let Some(parsed) = parse_payload(payload).map_err(|error| anyhow::anyhow!(error))? else {
        return Ok(());
    };
    let body = serde_json::json!({ "type": "agent-turn-complete", "thread-id": parsed.thread_id, "turn-id": parsed.turn_id });
    if !data_dir.is_dir() {
        return Ok(());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(400))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    for entry in std::fs::read_dir(data_dir).context("read activity rendezvous directory")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("cliswitch-activity-notify-") || !name.ends_with(".json") {
            continue;
        }
        let Some(rendezvous) = read_live_file(&entry.path()) else {
            let _ = std::fs::remove_file(entry.path());
            continue;
        };
        let url = format!(
            "http://127.0.0.1:{}/api/activities/codex-notify",
            rendezvous.port
        );
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let response = tokio::time::timeout(
            remaining,
            client
                .post(url)
                .bearer_auth(rendezvous.token)
                .json(&body)
                .send(),
        )
        .await
        .ok()
        .and_then(Result::ok);
        let _ = response;
    }
    Ok(())
}

/// Command suitable for a user to add to an existing Codex `notify` setting.
pub fn codex_notify_command(data_dir: &Path) -> Vec<String> {
    vec![
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "cliswitch".to_string()),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "activity-notify".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::{self, ActivityGuard, ActivityStatus};

    #[test]
    fn notify_accepts_only_allowlisted_ids_and_marks_the_observed_turn() {
        let thread = uuid::Uuid::new_v4().to_string();
        let turn = uuid::Uuid::new_v4().to_string();
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut guard = ActivityGuard::proxy(
            &request_id,
            crate::storage::Protocol::Openai,
            Some((thread.clone(), Some(turn.clone()))),
        );
        guard.finish(ActivityStatus::ResponseFinished);
        let payload = serde_json::json!({
            "type": "agent-turn-complete",
            "thread-id": thread,
            "turn-id": turn,
            "cwd": r#"C:\Users\测试\project"#,
            "input-messages": ["秘密 \"不要保存\""],
            "last-assistant-message": "secret",
        })
        .to_string();
        accept_codex_notification(Path::new("/tmp"), "token", "token", &payload).unwrap();
        let entry = activity::snapshot()
            .entries
            .into_iter()
            .find(|entry| entry.id == format!("codex:{thread}"))
            .unwrap();
        assert_eq!(entry.status, ActivityStatus::Completed);
        assert_eq!(entry.completed_turn_id.as_deref(), Some(turn.as_str()));
        assert!(
            accept_codex_notification(
                Path::new("/tmp"),
                "token",
                "token",
                r#"{"type":"other","input-messages":["secret"]}"#
            )
            .is_ok()
        );
        assert!(accept_codex_notification(Path::new("/tmp"), "token", "wrong", &payload).is_err());
        assert!(
            accept_codex_notification(Path::new("/tmp"), "token", "token", &payload).is_ok(),
            "duplicate turn is idempotent"
        );
    }

    #[test]
    fn rendezvous_is_private_and_raii_cleanup_is_token_scoped() {
        let dir =
            std::env::temp_dir().join(format!("cliswitch-notify-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let guard = register(&dir, 43123).unwrap();
        let path = guard.path.clone();
        assert!(!std::fs::metadata(&path).unwrap().permissions().readonly());
        assert_eq!(tokens_for(&dir), vec![guard.token.clone()]);
        drop(guard);
        assert!(!path.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod late_notify_tests {
    use super::*;
    use crate::activity::{self, ActivityGuard, ActivityStatus};

    #[test]
    fn late_previous_turn_notification_cannot_complete_a_newer_turn() {
        let thread = uuid::Uuid::new_v4().to_string();
        let old_turn = uuid::Uuid::new_v4().to_string();
        let new_turn = uuid::Uuid::new_v4().to_string();
        let mut old = ActivityGuard::proxy(
            "late-old",
            crate::storage::Protocol::Openai,
            Some((thread.clone(), Some(old_turn.clone()))),
        );
        old.finish(ActivityStatus::ResponseFinished);
        let mut new = ActivityGuard::proxy(
            "late-new",
            crate::storage::Protocol::Openai,
            Some((thread.clone(), Some(new_turn.clone()))),
        );
        new.finish(ActivityStatus::ResponseFinished);
        let old_payload = format!(
            r#"{{"type":"agent-turn-complete","thread-id":"{thread}","turn-id":"{old_turn}"}}"#
        );
        assert!(
            accept_codex_notification(Path::new("/tmp"), "token", "token", &old_payload).is_err()
        );
        let new_payload = format!(
            r#"{{"type":"agent-turn-complete","thread-id":"{thread}","turn-id":"{new_turn}"}}"#
        );
        accept_codex_notification(Path::new("/tmp"), "token", "token", &new_payload).unwrap();
        assert_eq!(
            activity::snapshot()
                .entries
                .into_iter()
                .find(|entry| entry.id == format!("codex:{thread}"))
                .unwrap()
                .completed_turn_id
                .as_deref(),
            Some(new_turn.as_str())
        );
    }

    #[test]
    fn pending_new_turn_rejects_old_notification_without_mutating_revision() {
        let thread = uuid::Uuid::new_v4().to_string();
        let old_turn = uuid::Uuid::new_v4().to_string();
        let new_turn = uuid::Uuid::new_v4().to_string();
        let mut old = ActivityGuard::proxy(
            "pending-old",
            crate::storage::Protocol::Openai,
            Some((thread.clone(), Some(old_turn.clone()))),
        );
        old.finish(ActivityStatus::ResponseFinished);
        let new = ActivityGuard::proxy(
            "pending-new",
            crate::storage::Protocol::Openai,
            Some((thread.clone(), Some(new_turn))),
        );
        let payload = format!(
            r#"{{"type":"agent-turn-complete","thread-id":"{thread}","turn-id":"{old_turn}"}}"#
        );
        assert!(accept_codex_notification(Path::new("/tmp"), "token", "token", &payload).is_err());
        assert_eq!(
            activity::snapshot()
                .entries
                .into_iter()
                .find(|entry| entry.id == format!("codex:{thread}"))
                .unwrap()
                .status,
            ActivityStatus::Running
        );
        drop(new);
    }

    #[test]
    fn notification_pending_while_running_is_retried_when_that_request_finishes() {
        let thread = uuid::Uuid::new_v4().to_string();
        let turn = uuid::Uuid::new_v4().to_string();
        let mut request = ActivityGuard::proxy(
            "pending-same-turn",
            crate::storage::Protocol::Openai,
            Some((thread.clone(), Some(turn.clone()))),
        );
        let payload = format!(
            r#"{{"type":"agent-turn-complete","thread-id":"{thread}","turn-id":"{turn}"}}"#
        );
        assert!(accept_codex_notification(Path::new("/tmp"), "token", "token", &payload).is_ok());
        assert_eq!(
            activity::snapshot()
                .entries
                .iter()
                .find(|entry| entry.thread_id.as_deref() == Some(&thread))
                .unwrap()
                .status,
            ActivityStatus::Running
        );
        request.finish(ActivityStatus::ResponseFinished);
        assert_eq!(
            activity::snapshot()
                .entries
                .iter()
                .find(|entry| entry.thread_id.as_deref() == Some(&thread))
                .unwrap()
                .completed_turn_id
                .as_deref(),
            Some(turn.as_str())
        );
    }
}
