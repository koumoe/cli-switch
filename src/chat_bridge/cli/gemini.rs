use anyhow::Context as _;
use serde_json::Value;
use std::time::Duration;

use crate::chat_bridge::i18n::{args, t, t_args};
use crate::cli_tools::CliToolId;
use crate::i18n::AppLocale;
use crate::storage::{AppSettings, BridgePermissionMode, BridgeSession};

use super::{CliAdapter, CliInvocation, ValidateResult, build_command, build_std_command};
const GEMINI_LIST_SESSIONS_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeminiListedSession {
    id: String,
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
        locale: AppLocale,
    ) -> anyhow::Result<ValidateResult> {
        let Some(session_ref) = session.cli_session_ref.as_deref() else {
            return Ok(ValidateResult::Valid);
        };

        let listed =
            match run_gemini_list_sessions(settings.clone(), session.working_dir.clone()).await {
                Ok(stdout) => match parse_list_sessions_output(&stdout) {
                    Ok(items) => items,
                    Err(err) => {
                        return Ok(ValidateResult::Invalid(t_args(
                            locale,
                            "error.gemini_list_sessions_parse_failed",
                            &args([("detail", err.to_string())]),
                        )));
                    }
                },
                Err(err) => {
                    return Ok(ValidateResult::Invalid(t_args(
                        locale,
                        "error.gemini_list_sessions_failed",
                        &args([("detail", err.to_string())]),
                    )));
                }
            };
        if !listed.iter().any(|item| item.id == session_ref) {
            return Ok(ValidateResult::Invalid(t(
                locale,
                "error.gemini_session_missing",
            )));
        }

        Ok(ValidateResult::Valid)
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

        let Some((_, rest)) = line.split_once(". ") else {
            anyhow::bail!("missing index separator in line: {line}");
        };
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
        sessions.push(GeminiListedSession { id: id.to_string() });
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
                    id: "11111111-1111-1111-1111-111111111111".to_string(),
                },
                GeminiListedSession {
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

}
