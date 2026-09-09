//! Ephemeral, bounded activity metadata for the desktop pet and activity list.
//!
//! A proxy response ending is deliberately *not* a conversation turn completing.
//! This module never retains request/response bodies, credentials, or inferred session IDs.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;

use crate::events::{self, AppEvent};

const MAX_ENTRIES: usize = 256;
const MAX_LABEL_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    ProxyRequest,
    BridgeTurn,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    Running,
    ResponseFinished,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityEntry {
    pub id: String,
    pub kind: ActivityKind,
    pub status: ActivityStatus,
    pub title: Option<String>,
    pub thread_id: Option<String>,
    pub observed_turn_id: Option<String>,
    pub source: String,
    pub protocol: Option<String>,
    pub model: Option<String>,
    pub project: Option<String>,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub completed_turn_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ActivitySnapshot {
    pub revision: u64,
    pub entries: Vec<ActivityEntry>,
    /// Active requests beyond the bounded list capacity; these are still counted.
    pub omitted_running: u32,
}

#[derive(Default)]
struct Registry {
    state: Mutex<ActivitySnapshot>,
    completed_turns: Mutex<(HashSet<String>, VecDeque<String>)>,
}

fn registry() -> &'static Arc<Registry> {
    static REGISTRY: OnceLock<Arc<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(Registry::default()))
}

impl Registry {
    fn mutate(&self, update: impl FnOnce(&mut ActivitySnapshot) -> bool) {
        let revision = {
            let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
            if !update(&mut state) {
                return;
            }
            state.revision = state.revision.saturating_add(1);
            state.revision
        };
        // Publish outside the lock. Consumers always fetch the newest full snapshot.
        events::publish(AppEvent::ActivityChanged { revision });
    }

    fn notify_codex_turn(&self, thread_id: &str, turn_id: &str) -> Result<(), NotifyError> {
        let key = format!("{thread_id}:{turn_id}");
        {
            let completed = self
                .completed_turns
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            if completed.0.contains(&key) {
                return Ok(());
            }
        }
        let (revision, completed_entry) = {
            let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
            let thread_entries: Vec<_> = state
                .entries
                .iter()
                .filter(|entry| entry.thread_id.as_deref() == Some(thread_id))
                .collect();
            // Entries are appended in request-start order; use that order when clocks
            // have millisecond ties so a late previous turn cannot claim the newer one.
            let latest_turn = thread_entries
                .last()
                .and_then(|entry| entry.observed_turn_id.as_deref());
            if latest_turn != Some(turn_id)
                || thread_entries
                    .iter()
                    .any(|entry| entry.status == ActivityStatus::Running)
            {
                return Err(NotifyError::NotObservedOrPending);
            }
            let now = crate::storage::now_ms();
            let mut accepted = false;
            for entry in &mut state.entries {
                if entry.thread_id.as_deref() == Some(thread_id)
                    && entry.observed_turn_id.as_deref() == Some(turn_id)
                    && matches!(
                        entry.status,
                        ActivityStatus::ResponseFinished | ActivityStatus::Unknown
                    )
                {
                    entry.status = ActivityStatus::Completed;
                    entry.updated_at_ms = now;
                    entry.finished_at_ms = Some(now);
                    entry.completed_turn_id = Some(turn_id.to_string());
                    accepted = true;
                }
            }
            if !accepted {
                return Err(NotifyError::NotObservedOrPending);
            }
            let completed_entry = state
                .entries
                .iter()
                .filter(|entry| {
                    entry.thread_id.as_deref() == Some(thread_id)
                        && entry.completed_turn_id.as_deref() == Some(turn_id)
                })
                .max_by_key(|entry| entry.updated_at_ms)
                .cloned();
            state.revision = state.revision.saturating_add(1);
            (state.revision, completed_entry)
        };
        events::publish(AppEvent::ActivityChanged { revision });
        if let Some(entry) = completed_entry {
            events::publish(AppEvent::ActivityCompleted(entry));
        }
        let mut completed = self
            .completed_turns
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        completed.0.insert(key.clone());
        completed.1.push_back(key);
        while completed.1.len() > 1024 {
            if let Some(old) = completed.1.pop_front() {
                completed.0.remove(&old);
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> ActivitySnapshot {
        let mut snapshot = self
            .state
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        // Requests retain independent guards internally, but a verified Codex thread
        // occupies one visible row. A newer finished request must not hide another
        // still-running request from the same thread.
        let mut grouped = HashMap::<String, ActivityEntry>::new();
        for mut entry in std::mem::take(&mut snapshot.entries) {
            let Some(thread_id) = entry.thread_id.as_ref() else {
                snapshot.entries.push(entry);
                continue;
            };
            entry.id = format!("codex:{thread_id}");
            match grouped.entry(entry.id.clone()) {
                std::collections::hash_map::Entry::Vacant(slot) => {
                    slot.insert(entry);
                }
                std::collections::hash_map::Entry::Occupied(mut slot) => {
                    let previous = slot.get_mut();
                    let started_at_ms = previous.started_at_ms.min(entry.started_at_ms);
                    let updated_at_ms = previous.updated_at_ms.max(entry.updated_at_ms);
                    let running = previous.status == ActivityStatus::Running
                        || entry.status == ActivityStatus::Running;
                    if entry.updated_at_ms >= previous.updated_at_ms {
                        *previous = entry;
                    }
                    previous.started_at_ms = started_at_ms;
                    previous.updated_at_ms = updated_at_ms;
                    if running {
                        previous.status = ActivityStatus::Running;
                        previous.finished_at_ms = None;
                        previous.completed_turn_id = None;
                    }
                }
            }
        }
        snapshot.entries.extend(grouped.into_values());
        snapshot.entries.sort_by(|a, b| {
            (b.status == ActivityStatus::Running)
                .cmp(&(a.status == ActivityStatus::Running))
                .then_with(|| b.updated_at_ms.cmp(&a.updated_at_ms))
                .then_with(|| b.id.cmp(&a.id))
        });
        snapshot
    }

    fn start(self: &Arc<Self>, entry: ActivityEntry) -> ActivityGuard {
        let mut retained_id = None;
        self.mutate(|state| {
            if state.entries.len() == MAX_ENTRIES
                && let Some((idx, _)) = state
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| entry.status != ActivityStatus::Running)
                    .min_by_key(|(_, entry)| entry.updated_at_ms)
            {
                state.entries.remove(idx);
            }
            if state.entries.len() < MAX_ENTRIES {
                retained_id = Some(entry.id.clone());
                state.entries.push(entry);
            } else {
                state.omitted_running = state.omitted_running.saturating_add(1);
            }
            true
        });
        ActivityGuard {
            registry: self.clone(),
            retained_id,
            finished: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum NotifyError {
    #[error("thread was not observed or still has a running request")]
    NotObservedOrPending,
}

pub(crate) fn deliver_codex_turn(thread_id: &str, turn_id: &str) -> Result<(), NotifyError> {
    registry().notify_codex_turn(thread_id, turn_id)
}

pub fn snapshot() -> ActivitySnapshot {
    registry().snapshot()
}

fn label(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_LABEL_CHARS)
        .collect()
}

/// Codex emits these headers for both hosted and custom Responses providers.
/// Source: openai/codex codex-api/src/requests/headers.rs, commits
/// a98623511ba433154ec811fc63091617f5945438 (thread_id) and
/// 7c7b4861d88960f7e3bd5b7f30f8351be666dd84 (thread-id).
/// session-id is intentionally excluded: it is not the current thread identity.
pub(crate) fn codex_thread_identity(
    protocol: crate::storage::Protocol,
    path: &str,
    headers: &http::HeaderMap,
) -> Option<(String, Option<String>)> {
    if protocol != crate::storage::Protocol::Openai
        || !matches!(
            path.trim_end_matches('/'),
            "/v1/responses" | "/v1/responses/compact"
        )
    {
        return None;
    }
    fn single<'a>(headers: &'a http::HeaderMap, name: &str) -> Result<Option<&'a str>, ()> {
        let mut values = headers.get_all(name).iter();
        let value = values
            .next()
            .map(|value| value.to_str().map_err(|_| ()))
            .transpose()?;
        if values.next().is_some() {
            return Err(());
        }
        Ok(value)
    }
    // This mirrors Codex's official is_first_party_originator predicate. Originator
    // is client metadata, not authentication, and never authorizes an external action.
    fn known_client(name: &str) -> bool {
        matches!(name, "codex_cli_rs" | "codex-tui" | "codex_vscode") || name.starts_with("Codex ")
    }
    let client = match single(headers, "originator").ok()? {
        Some(originator) => originator,
        None => single(headers, "user-agent").ok()??.split('/').next()?,
    };
    if !known_client(client) {
        return None;
    }
    let hyphen = single(headers, "thread-id").ok()?;
    let underscore = single(headers, "thread_id").ok()?;
    let parse = |value: &str| uuid::Uuid::parse_str(value).ok().filter(|id| !id.is_nil());
    let id = parse(hyphen.or(underscore)?)?;
    if let Some(legacy) = underscore
        && parse(legacy)? != id
    {
        return None;
    }
    let turn_id = match single(headers, "x-codex-turn-metadata").ok()? {
        None => None,
        Some(raw) if raw.len() <= 8 * 1024 => {
            let value = serde_json::from_str::<serde_json::Value>(raw).ok()?;
            if let Some(metadata_thread) =
                value.get("thread_id").and_then(serde_json::Value::as_str)
                && metadata_thread != id.to_string()
            {
                return None;
            }
            value
                .get("turn_id")
                .and_then(serde_json::Value::as_str)
                .and_then(|turn| {
                    (!turn.is_empty()
                        && turn.len() <= 128
                        && turn.chars().all(|ch| !ch.is_control()))
                    .then(|| turn.to_string())
                })
        }
        Some(_) => return None,
    };
    Some((id.to_string(), turn_id))
}

/// One owner per logical incoming request/bridge turn; move it into the response body.
/// Dropping a handler/stream without a trustworthy terminal signal means unknown.
/// Internal channel retries keep the same guard and never create additional entries.
pub(crate) struct ActivityGuard {
    registry: Arc<Registry>,
    retained_id: Option<String>,
    finished: bool,
}

impl ActivityGuard {
    pub(crate) fn proxy(
        request_id: &str,
        protocol: crate::storage::Protocol,
        identity: Option<(String, Option<String>)>,
    ) -> Self {
        let now = crate::storage::now_ms();
        registry().start(ActivityEntry {
            id: format!("request:{request_id}"),
            kind: ActivityKind::ProxyRequest,
            status: ActivityStatus::Running,
            title: None,
            source: if identity.is_some() {
                "codex".to_string()
            } else {
                protocol.as_str().to_string()
            },
            thread_id: identity.as_ref().map(|value| value.0.clone()),
            observed_turn_id: identity.and_then(|value| value.1),
            protocol: Some(protocol.as_str().to_string()),
            model: None,
            project: None,
            started_at_ms: now,
            updated_at_ms: now,
            finished_at_ms: None,
            completed_turn_id: None,
        })
    }

    pub(crate) fn bridge(session: &crate::storage::BridgeSession) -> Self {
        let now = crate::storage::now_ms();
        registry().start(ActivityEntry {
            id: format!("bridge:{}:{}", session.id, uuid::Uuid::new_v4()),
            kind: ActivityKind::BridgeTurn,
            status: ActivityStatus::Running,
            title: session.alias.as_deref().map(label),
            thread_id: None,
            observed_turn_id: None,
            source: session.cli_type.as_str().to_string(),
            protocol: None,
            model: None,
            project: Some(label(&session.project_name)),
            started_at_ms: now,
            updated_at_ms: now,
            finished_at_ms: None,
            completed_turn_id: None,
        })
    }

    pub(crate) fn set_model(&self, model: Option<&str>) {
        let Some(model) = model.map(label) else {
            return;
        };
        self.registry.mutate(|state| {
            let Some(entry) = state
                .entries
                .iter_mut()
                .find(|entry| Some(&entry.id) == self.retained_id.as_ref())
            else {
                return false;
            };
            if entry.model.as_ref() == Some(&model) {
                return false;
            }
            entry.model = Some(model);
            true
        });
    }

    /// First terminal result wins, including when a later notification delivery fails.
    pub(crate) fn finish(&mut self, status: ActivityStatus) {
        if self.finished || status == ActivityStatus::Running {
            return;
        }
        self.finished = true;
        let mut completed_entry = None;
        self.registry.mutate(|state| {
            if let Some(id) = &self.retained_id {
                let Some(entry) = state.entries.iter_mut().find(|entry| &entry.id == id) else {
                    return false;
                };
                // Even an accidental caller cannot turn an HTTP response into a completed turn.
                entry.status = if entry.kind == ActivityKind::ProxyRequest
                    && status == ActivityStatus::Completed
                {
                    ActivityStatus::ResponseFinished
                } else {
                    status
                };
                let now = crate::storage::now_ms();
                entry.updated_at_ms = now;
                entry.finished_at_ms = Some(now);
                if entry.status == ActivityStatus::Completed {
                    completed_entry = Some(entry.clone());
                }
            } else {
                state.omitted_running = state.omitted_running.saturating_sub(1);
            }
            true
        });
        if let Some(entry) = completed_entry {
            events::publish(AppEvent::ActivityCompleted(entry));
        }
    }
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        self.finish(ActivityStatus::Unknown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(registry: &Arc<Registry>, id: usize, kind: ActivityKind) -> ActivityGuard {
        registry.start(ActivityEntry {
            id: id.to_string(),
            kind,
            status: ActivityStatus::Running,
            title: None,
            thread_id: None,
            observed_turn_id: None,
            source: "openai".into(),
            protocol: Some("openai".into()),
            model: None,
            project: None,
            started_at_ms: id as i64,
            updated_at_ms: id as i64,
            finished_at_ms: None,
            completed_turn_id: None,
        })
    }

    #[test]
    fn concurrent_requests_finish_independently_and_retries_reuse_entry() {
        let registry = Arc::new(Registry::default());
        let mut first = start(&registry, 1, ActivityKind::ProxyRequest);
        let second = start(&registry, 2, ActivityKind::ProxyRequest);
        for _ in 0..5 {
            first.set_model(Some("gpt-5"));
        }
        assert_eq!(registry.snapshot().entries.len(), 2);
        first.finish(ActivityStatus::ResponseFinished);
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.entries[0].id, "2");
        assert_eq!(snapshot.entries[0].status, ActivityStatus::Running);
        assert_eq!(snapshot.entries[1].status, ActivityStatus::ResponseFinished);
        drop(second);
        assert!(
            registry
                .snapshot()
                .entries
                .iter()
                .any(|entry| entry.id == "2" && entry.status == ActivityStatus::Unknown)
        );
    }

    #[test]
    fn only_bridge_turn_can_complete_and_terminal_result_is_idempotent() {
        let registry = Arc::new(Registry::default());
        let mut request = start(&registry, 1, ActivityKind::ProxyRequest);
        request.finish(ActivityStatus::Completed);
        let mut turn = start(&registry, 2, ActivityKind::BridgeTurn);
        turn.finish(ActivityStatus::Completed);
        let revision = registry.snapshot().revision;
        turn.finish(ActivityStatus::Failed);
        drop(turn);
        assert_eq!(registry.snapshot().revision, revision);
        let entries = registry.snapshot().entries;
        assert!(
            entries
                .iter()
                .any(|e| e.id == "1" && e.status == ActivityStatus::ResponseFinished)
        );
        assert!(
            entries
                .iter()
                .any(|e| e.id == "2" && e.status == ActivityStatus::Completed)
        );
    }

    #[test]
    fn cancellation_failure_and_disconnection_are_distinct() {
        let registry = Arc::new(Registry::default());
        let mut cancelled = start(&registry, 1, ActivityKind::BridgeTurn);
        let mut failed = start(&registry, 2, ActivityKind::ProxyRequest);
        let unknown = start(&registry, 3, ActivityKind::ProxyRequest);
        cancelled.finish(ActivityStatus::Cancelled);
        failed.finish(ActivityStatus::Failed);
        drop(unknown);
        let entries = registry.snapshot().entries;
        for (id, status) in [
            ("1", ActivityStatus::Cancelled),
            ("2", ActivityStatus::Failed),
            ("3", ActivityStatus::Unknown),
        ] {
            assert!(
                entries
                    .iter()
                    .any(|e| e.id == id && e.status == status && e.finished_at_ms.is_some())
            );
        }
    }

    #[test]
    fn memory_is_bounded_without_evicting_active_entries() {
        let registry = Arc::new(Registry::default());
        let mut guards: Vec<_> = (0..MAX_ENTRIES)
            .map(|id| start(&registry, id, ActivityKind::ProxyRequest))
            .collect();
        let overflow = start(&registry, MAX_ENTRIES, ActivityKind::ProxyRequest);
        assert_eq!(registry.snapshot().entries.len(), MAX_ENTRIES);
        assert_eq!(registry.snapshot().omitted_running, 1);
        drop(overflow);
        assert_eq!(registry.snapshot().omitted_running, 0);
        guards[0].finish(ActivityStatus::ResponseFinished);
        let _replacement = start(&registry, MAX_ENTRIES + 1, ActivityKind::ProxyRequest);
        assert!(!registry.snapshot().entries.iter().any(|e| e.id == "0"));
        assert_eq!(registry.snapshot().entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn codex_headers_are_explicit_validated_and_legacy_compatible() {
        use http::{HeaderMap, HeaderValue};
        let first = "0199a213-81c0-7800-8aa1-bbab2a035a53";
        let second = "67e55044-10b1-426f-9247-bb680e5fe0c8";
        let mut headers = HeaderMap::new();
        headers.insert("originator", HeaderValue::from_static("Codex Desktop"));
        headers.insert("session-id", HeaderValue::from_static(first));
        let extract = |headers: &HeaderMap| {
            codex_thread_identity(crate::storage::Protocol::Openai, "/v1/responses", headers)
        };
        assert_eq!(extract(&headers), None, "session is not thread identity");
        headers.insert("thread_id", HeaderValue::from_static(first));
        assert_eq!(
            extract(&headers).map(|value| value.0).as_deref(),
            Some(first)
        );
        headers.insert("thread-id", HeaderValue::from_static(first));
        assert_eq!(
            extract(&headers).map(|value| value.0).as_deref(),
            Some(first)
        );
        headers.insert("thread_id", HeaderValue::from_static(second));
        assert_eq!(extract(&headers), None, "conflicting aliases are rejected");
        headers.remove("thread_id");
        headers.append("thread-id", HeaderValue::from_static(first));
        assert_eq!(extract(&headers), None, "duplicate headers are rejected");
        headers.insert("thread-id", HeaderValue::from_static("not-a-uuid"));
        assert_eq!(extract(&headers), None);
        headers.insert("thread-id", HeaderValue::from_static(first));
        headers.insert("originator", HeaderValue::from_static("unrelated-client"));
        assert_eq!(extract(&headers), None);
        headers.remove("originator");
        headers.insert("user-agent", HeaderValue::from_static("codex-tui/0.150.1"));
        assert_eq!(
            extract(&headers).map(|value| value.0).as_deref(),
            Some(first)
        );
        assert_eq!(
            codex_thread_identity(
                crate::storage::Protocol::Anthropic,
                "/v1/responses",
                &headers
            ),
            None
        );
        assert_eq!(
            codex_thread_identity(
                crate::storage::Protocol::Openai,
                "/v1/chat/completions",
                &headers
            ),
            None
        );
    }

    #[test]
    fn codex_snapshot_groups_concurrent_requests_without_hiding_a_running_sibling() {
        let registry = Arc::new(Registry::default());
        let mut first = start(&registry, 1, ActivityKind::ProxyRequest);
        let mut second = start(&registry, 2, ActivityKind::ProxyRequest);
        let third = start(&registry, 3, ActivityKind::ProxyRequest);
        registry.mutate(|state| {
            for entry in &mut state.entries {
                entry.thread_id = Some(
                    if entry.id == "3" {
                        "other-thread"
                    } else {
                        "same-thread"
                    }
                    .into(),
                );
                entry.source = "codex".into();
            }
            true
        });
        second.finish(ActivityStatus::ResponseFinished);
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.entries.len(), 2);
        let grouped = snapshot
            .entries
            .iter()
            .find(|entry| entry.id == "codex:same-thread")
            .unwrap();
        assert_eq!(grouped.status, ActivityStatus::Running);
        assert_eq!(grouped.finished_at_ms, None);
        assert_eq!(
            grouped.title, None,
            "thread identity does not imply a title"
        );
        first.finish(ActivityStatus::ResponseFinished);
        let snapshot = registry.snapshot();
        assert_eq!(
            snapshot
                .entries
                .iter()
                .find(|entry| entry.id == "codex:same-thread")
                .unwrap()
                .status,
            ActivityStatus::ResponseFinished
        );
        assert_eq!(snapshot.entries[0].id, "codex:other-thread");
        drop(third);
        assert_eq!(
            registry
                .snapshot()
                .entries
                .iter()
                .find(|entry| entry.id == "codex:other-thread")
                .unwrap()
                .status,
            ActivityStatus::Unknown
        );
        assert_eq!(
            registry.state.lock().unwrap().entries.len(),
            3,
            "guards stay independently addressable"
        );
    }

    #[test]
    fn metadata_is_bounded_and_serialized_status_is_explicit() {
        assert_eq!(label(&"a".repeat(500)).len(), MAX_LABEL_CHARS);
        assert_eq!(label("a\nb\tc"), "abc");
        assert_eq!(
            serde_json::to_string(&ActivityStatus::ResponseFinished).unwrap(),
            "\"response_finished\""
        );
    }
}
