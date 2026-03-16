use serde_json::Value;

use crate::cli_tools::CliToolId;
use crate::storage::{AppSettings, BridgePermissionMode, BridgeSession};

use super::{CliAdapter, CliInvocation, build_command};

pub struct CodexAdapter;

#[async_trait::async_trait]
impl CliAdapter for CodexAdapter {
    fn tool(&self) -> CliToolId {
        CliToolId::Codex
    }

    fn build_invocation(
        &self,
        prompt: &str,
        session: &BridgeSession,
        settings: &AppSettings,
        streaming: bool,
        resume_existing: bool,
    ) -> anyhow::Result<CliInvocation> {
        let mut cmd = build_command("codex", settings)?;

        if resume_existing {
            let session_ref = session.cli_session_ref.as_deref().ok_or_else(|| {
                anyhow::anyhow!("codex session resume requested without session ref")
            })?;
            cmd.arg("exec").arg("resume").arg(session_ref).arg(prompt);
        } else {
            cmd.arg("exec").arg(prompt);
        }

        cmd.arg("--json");

        let final_output_path = if streaming {
            None
        } else {
            let path = std::env::temp_dir().join(format!(
                "cliswitch-codex-output-{}-{}.txt",
                session.id,
                uuid::Uuid::new_v4()
            ));
            cmd.arg("-o").arg(&path);
            Some(path)
        };

        match session.permission_mode {
            BridgePermissionMode::Safe => {
                cmd.arg("--full-auto");
            }
            BridgePermissionMode::Yolo => {
                cmd.arg("--dangerously-bypass-approvals-and-sandbox");
            }
        }

        cmd.current_dir(&session.working_dir);
        Ok(CliInvocation {
            command: cmd,
            final_output_path,
        })
    }

    fn extract_session_ref(&self, output: &str) -> Option<String> {
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
                continue;
            };

            if value.get("type").and_then(Value::as_str) == Some("session_meta")
                && let Some(id) = value
                    .pointer("/payload/id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            {
                return Some(id.to_string());
            }

            if value.get("type").and_then(Value::as_str) == Some("thread.started")
                && let Some(thread_id) = value
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .or_else(|| value.pointer("/payload/thread_id").and_then(Value::as_str))
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            {
                return Some(thread_id.to_string());
            }
        }

        None
    }
}
