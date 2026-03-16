use crate::cli_tools::CliToolId;
use crate::storage::{AppSettings, BridgePermissionMode, BridgeSession};

use super::{CliAdapter, CliInvocation, build_command};

pub struct ClaudeAdapter;

#[async_trait::async_trait]
impl CliAdapter for ClaudeAdapter {
    fn tool(&self) -> CliToolId {
        CliToolId::Claude
    }

    fn build_invocation(
        &self,
        prompt: &str,
        session: &BridgeSession,
        settings: &AppSettings,
        streaming: bool,
        resume_existing: bool,
    ) -> anyhow::Result<CliInvocation> {
        let mut cmd = build_command("claude", settings)?;
        cmd.arg("-p").arg(prompt);
        cmd.arg("--output-format")
            .arg(if streaming { "stream-json" } else { "json" });
        if streaming {
            cmd.arg("--verbose");
            cmd.arg("--include-partial-messages");
        }

        match session.permission_mode {
            BridgePermissionMode::Safe => {
                cmd.arg("--permission-mode").arg("acceptEdits");
            }
            BridgePermissionMode::Yolo => {
                cmd.arg("--dangerously-skip-permissions");
            }
        }

        if let Some(session_ref) = session.cli_session_ref.as_deref() {
            if resume_existing {
                cmd.arg("--resume").arg(session_ref);
            } else {
                cmd.arg("--session-id").arg(session_ref);
            }
        }

        cmd.current_dir(&session.working_dir);
        Ok(CliInvocation {
            command: cmd,
            final_output_path: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session(session_ref: Option<&str>) -> BridgeSession {
        BridgeSession {
            id: 1,
            alias: None,
            platform: crate::storage::ChatPlatform::Telegram,
            cli_type: CliToolId::Claude,
            cli_session_ref: session_ref.map(str::to_string),
            project_id: Some("/tmp/demo".to_string()),
            project_name: "demo".to_string(),
            working_dir: "/tmp".to_string(),
            permission_mode: BridgePermissionMode::Safe,
            status: crate::storage::BridgeSessionStatus::Idle,
            is_default: true,
            created_at_ms: 1,
            last_active_ms: 1,
        }
    }

    fn command_args(invocation: &CliInvocation) -> Vec<String> {
        invocation
            .command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn claude_resume_uses_resume_with_session_id_value_only() {
        let adapter = ClaudeAdapter;
        let invocation = adapter
            .build_invocation(
                "hello",
                &test_session(Some("123e4567-e89b-12d3-a456-426614174000")),
                &AppSettings::default(),
                false,
                true,
            )
            .expect("build invocation");

        let args = command_args(&invocation);
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--resume", "123e4567-e89b-12d3-a456-426614174000"])
        );
        assert!(!args.iter().any(|arg| arg == "--session-id"));
    }

    #[test]
    fn claude_initial_turn_keeps_session_id_without_resume() {
        let adapter = ClaudeAdapter;
        let invocation = adapter
            .build_invocation(
                "hello",
                &test_session(Some("123e4567-e89b-12d3-a456-426614174000")),
                &AppSettings::default(),
                false,
                false,
            )
            .expect("build invocation");

        let args = command_args(&invocation);
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--session-id", "123e4567-e89b-12d3-a456-426614174000"])
        );
        assert!(!args.iter().any(|arg| arg == "--resume"));
    }

    #[test]
    fn claude_streaming_requires_verbose_flag() {
        let adapter = ClaudeAdapter;
        let invocation = adapter
            .build_invocation(
                "hello",
                &test_session(Some("123e4567-e89b-12d3-a456-426614174000")),
                &AppSettings::default(),
                true,
                true,
            )
            .expect("build invocation");

        let args = command_args(&invocation);
        assert!(args.iter().any(|arg| arg == "--verbose"));
        assert!(args.iter().any(|arg| arg == "--include-partial-messages"));
    }
}
