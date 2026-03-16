use anyhow::Context as _;
use directories::UserDirs;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::cli_tools::CliToolId;
use crate::storage::{AppSettings, BridgePermissionMode, BridgeSession};

use super::{CliAdapter, CliInvocation, ValidateResult, build_command, build_std_command};
const GEMINI_LIST_SESSIONS_TIMEOUT: Duration = Duration::from_secs(15);
const LEGACY_GEMINI_SESSION_MATCH_WINDOW_MS: i64 = 10 * 60 * 1000;
pub const LEGACY_GEMINI_UNTRACKED_SESSION_REF: &str = "__cliswitch_untracked_gemini_session__";

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeminiListedSession {
    index: usize,
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeminiStoredSession {
    id: String,
    start_time_ms: i64,
}

pub struct GeminiAdapter;

#[async_trait::async_trait]
impl CliAdapter for GeminiAdapter {
    fn tool(&self) -> CliToolId {
        CliToolId::Gemini
    }

    fn build_invocation(
        &self,
        prompt: &str,
        session: &BridgeSession,
        settings: &AppSettings,
        streaming: bool,
        resume_existing: bool,
    ) -> anyhow::Result<CliInvocation> {
        let mut cmd = build_command("gemini", settings)?;
        cmd.arg("-p").arg(prompt);
        cmd.arg("--output-format")
            .arg(if streaming { "stream-json" } else { "json" });

        match session.permission_mode {
            BridgePermissionMode::Safe => {
                cmd.arg("--approval-mode").arg("auto_edit");
            }
            BridgePermissionMode::Yolo => {
                cmd.arg("--yolo");
            }
        }

        if resume_existing {
            let session_ref = session.cli_session_ref.as_deref().ok_or_else(|| {
                anyhow::anyhow!("gemini session resume requested without session ref")
            })?;
            if session_ref == LEGACY_GEMINI_UNTRACKED_SESSION_REF {
                anyhow::bail!(
                    "gemini session resume is disabled because the session was created before session_id capture was available"
                );
            }
            cmd.arg("--resume").arg(session_ref);
        }

        cmd.current_dir(&session.working_dir);
        Ok(CliInvocation {
            command: cmd,
            final_output_path: None,
        })
    }

    fn extract_session_ref(&self, output: &str) -> Option<String> {
        extract_session_id_from_output(output)
    }

    async fn validate_session_ref(
        &self,
        session: &BridgeSession,
        settings: &AppSettings,
    ) -> anyhow::Result<ValidateResult> {
        let Some(session_ref) = session.cli_session_ref.as_deref() else {
            return Ok(ValidateResult::Valid);
        };

        let listed =
            match run_gemini_list_sessions(settings.clone(), session.working_dir.clone()).await {
                Ok(stdout) => match parse_list_sessions_output(&stdout) {
                    Ok(items) => items,
                    Err(err) => {
                        return Ok(ValidateResult::Invalid(format!(
                            "gemini --list-sessions 输出无法安全解析：{err}"
                        )));
                    }
                },
                Err(err) => {
                    return Ok(ValidateResult::Invalid(format!(
                        "执行 gemini --list-sessions 失败：{err}"
                    )));
                }
            };
        let local_sessions =
            load_gemini_stored_sessions_for_workdir(Path::new(&session.working_dir))?;
        let resolved_ref = if session_ref == LEGACY_GEMINI_UNTRACKED_SESSION_REF {
            resolve_legacy_gemini_session_ref_from_sessions(session.created_at_ms, &local_sessions)
        } else {
            resolve_listed_gemini_session_ref(session_ref, &listed)
        };
        let Some(resolved_ref) = resolved_ref else {
            return Ok(ValidateResult::Invalid(
                "Gemini 当前项目下未找到对应会话，可能已被外部删除或当前 CLI 版本无法安全恢复。"
                    .to_string(),
            ));
        };

        if !listed.iter().any(|item| item.id == resolved_ref) {
            return Ok(ValidateResult::Invalid(
                "Gemini 本地历史里找到了候选 session_id，但当前项目的会话列表里没有它，可能已被外部删除。"
                    .to_string(),
            ));
        }

        Ok(if resolved_ref == session_ref {
            ValidateResult::Valid
        } else {
            ValidateResult::Corrected(resolved_ref)
        })
    }
}

async fn run_gemini_list_sessions(
    settings: AppSettings,
    working_dir: String,
) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || {
        let mut cmd = build_std_command("gemini", &settings)?;
        cmd.arg("--list-sessions");
        cmd.current_dir(&working_dir);
        let output =
            crate::process::command_output_with_timeout(&mut cmd, GEMINI_LIST_SESSIONS_TIMEOUT)
                .with_context(|| "run gemini --list-sessions failed")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit status {}", output.status)
            };
            anyhow::bail!("{detail}");
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .context("wait gemini --list-sessions task failed")?
}

fn extract_session_id_from_output(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed)
        && let Some(session_id) = extract_session_id_from_value(&value)
    {
        return Some(session_id);
    }

    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(session_id) = extract_session_id_from_value(&value) {
            return Some(session_id);
        }
    }

    None
}

fn extract_session_id_from_value(value: &Value) -> Option<String> {
    value
        .get("session_id")
        .or_else(|| value.get("sessionId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn resolve_legacy_gemini_session_ref_from_sessions(
    created_at_ms: i64,
    sessions: &[GeminiStoredSession],
) -> Option<String> {
    let mut candidates = sessions
        .iter()
        .filter_map(|session| {
            let diff = session.start_time_ms.abs_diff(created_at_ms) as i64;
            (diff <= LEGACY_GEMINI_SESSION_MATCH_WINDOW_MS).then_some((
                diff,
                session.start_time_ms,
                session.id.clone(),
            ))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let first = candidates.first()?;
    if candidates.get(1).is_some_and(|next| next.0 == first.0) {
        return None;
    }
    Some(first.2.clone())
}

fn resolve_listed_gemini_session_ref(
    session_ref: &str,
    listed: &[GeminiListedSession],
) -> Option<String> {
    if listed.iter().any(|item| item.id == session_ref) {
        return Some(session_ref.to_string());
    }

    let index = session_ref.parse::<usize>().ok()?;
    listed
        .iter()
        .find(|item| item.index == index)
        .map(|item| item.id.clone())
}

fn find_gemini_project_temp_dir(working_dir: &Path) -> anyhow::Result<Option<PathBuf>> {
    let Some(home_dir) = UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) else {
        return Ok(None);
    };
    let gemini_tmp_root = home_dir.join(".gemini").join("tmp");
    if !gemini_tmp_root.is_dir() {
        return Ok(None);
    }

    let wanted = canonical_path_string(working_dir);
    for entry in fs::read_dir(&gemini_tmp_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let marker_path = path.join(".project_root");
        let Ok(raw) = fs::read_to_string(&marker_path) else {
            continue;
        };
        let marker = raw.trim();
        if marker.is_empty() {
            continue;
        }
        if canonical_path_string(Path::new(marker)) == wanted {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn canonical_path_string(path: &Path) -> String {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let value = resolved.to_string_lossy().to_string();
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

fn load_gemini_stored_sessions(chats_dir: &Path) -> anyhow::Result<Vec<GeminiStoredSession>> {
    if !chats_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in fs::read_dir(chats_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if !path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.starts_with("session-"))
        {
            continue;
        }

        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(id) = value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(start_time_raw) = value.get("startTime").and_then(Value::as_str) else {
            continue;
        };
        let Some(start_time_ms) = parse_rfc3339_ms(start_time_raw) else {
            continue;
        };
        sessions.push(GeminiStoredSession {
            id: id.to_string(),
            start_time_ms,
        });
    }

    sessions.sort_by_key(|item| item.start_time_ms);
    Ok(sessions)
}

fn load_gemini_stored_sessions_for_workdir(
    working_dir: &Path,
) -> anyhow::Result<Vec<GeminiStoredSession>> {
    let temp_dir = find_gemini_project_temp_dir(working_dir)?;
    let Some(temp_dir) = temp_dir else {
        return Ok(Vec::new());
    };
    load_gemini_stored_sessions(&temp_dir.join("chats"))
}

fn parse_rfc3339_ms(raw: &str) -> Option<i64> {
    let parsed = OffsetDateTime::parse(raw.trim(), &Rfc3339).ok()?;
    Some((parsed.unix_timestamp_nanos() / 1_000_000) as i64)
}

fn parse_list_sessions_output(output: &str) -> anyhow::Result<Vec<GeminiListedSession>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        anyhow::bail!("empty output");
    }
    if trimmed == "No previous sessions found for this project." {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Available sessions for this project") {
            continue;
        }
        if !matches!(line.as_bytes().first(), Some(b'0'..=b'9')) {
            continue;
        }

        let Some((index_part, rest)) = line.split_once(". ") else {
            anyhow::bail!("missing index separator in line: {line}");
        };
        let index = index_part
            .parse::<usize>()
            .map_err(|err| anyhow::anyhow!("invalid session index in line {line}: {err}"))?;
        let Some(rest) = rest.strip_suffix(']') else {
            anyhow::bail!("missing trailing ] in line: {line}");
        };
        let Some((_, id_part)) = rest.rsplit_once(" [") else {
            anyhow::bail!("missing session id segment in line: {line}");
        };
        let id = id_part.trim();
        if id.is_empty() {
            anyhow::bail!("empty session id in line: {line}");
        }
        sessions.push(GeminiListedSession {
            index,
            id: id.to_string(),
        });
    }

    if sessions.is_empty() {
        anyhow::bail!("no session rows found");
    }
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_session_id_from_pretty_json_output() {
        let output = r#"{
  "session_id": "gemini-session-123",
  "response": "hello"
}"#;
        assert_eq!(
            extract_session_id_from_output(output).as_deref(),
            Some("gemini-session-123")
        );
    }

    #[test]
    fn extract_session_id_from_stream_json_output() {
        let output = r#"{"type":"init","session_id":"stream-session-1","model":"gemini-2.5-pro"}
{"type":"message","role":"assistant","content":"hello","delta":true}"#;
        assert_eq!(
            extract_session_id_from_output(output).as_deref(),
            Some("stream-session-1")
        );
    }

    #[test]
    fn parse_list_sessions_output_reads_ids() {
        let output = r#"
Available sessions for this project (2):
  1. Initial auth fix (2 days ago) [11111111-1111-1111-1111-111111111111]
  2. Investigate session drift (Just now, current) [22222222-2222-2222-2222-222222222222]
"#;

        let parsed = parse_list_sessions_output(output).expect("parse sessions");
        assert_eq!(
            parsed,
            vec![
                GeminiListedSession {
                    index: 1,
                    id: "11111111-1111-1111-1111-111111111111".to_string(),
                },
                GeminiListedSession {
                    index: 2,
                    id: "22222222-2222-2222-2222-222222222222".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_list_sessions_output_handles_empty_project() {
        let parsed = parse_list_sessions_output("No previous sessions found for this project.")
            .expect("parse empty output");
        assert!(parsed.is_empty());
    }

    #[test]
    fn resolve_listed_gemini_session_ref_upgrades_numeric_index_to_uuid() {
        let listed = vec![
            GeminiListedSession {
                index: 1,
                id: "11111111-1111-1111-1111-111111111111".to_string(),
            },
            GeminiListedSession {
                index: 2,
                id: "22222222-2222-2222-2222-222222222222".to_string(),
            },
        ];

        assert_eq!(
            resolve_listed_gemini_session_ref("2", &listed).as_deref(),
            Some("22222222-2222-2222-2222-222222222222")
        );
    }

    #[test]
    fn resolve_legacy_gemini_session_ref_prefers_nearest_start_time() {
        let sessions = vec![
            GeminiStoredSession {
                id: "old".to_string(),
                start_time_ms: 1_000,
            },
            GeminiStoredSession {
                id: "expected".to_string(),
                start_time_ms: 5_500,
            },
            GeminiStoredSession {
                id: "new".to_string(),
                start_time_ms: 30_000,
            },
        ];

        assert_eq!(
            resolve_legacy_gemini_session_ref_from_sessions(5_000, &sessions).as_deref(),
            Some("expected")
        );
    }

    #[test]
    fn resolve_legacy_gemini_session_ref_rejects_ambiguous_match() {
        let sessions = vec![
            GeminiStoredSession {
                id: "left".to_string(),
                start_time_ms: 4_000,
            },
            GeminiStoredSession {
                id: "right".to_string(),
                start_time_ms: 6_000,
            },
        ];

        assert_eq!(
            resolve_legacy_gemini_session_ref_from_sessions(5_000, &sessions),
            None
        );
    }
}
