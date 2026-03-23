use async_trait::async_trait;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command as TokioCommand;

use crate::cli_tools::{CliExecEnv, CliToolId};
use crate::i18n::AppLocale;
use crate::storage::{self, AppSettings, BridgeSession};

mod claude;
mod codex;
mod gemini;

pub use claude::ClaudeAdapter;
pub use codex::CodexAdapter;
pub use gemini::{GeminiAdapter, LEGACY_GEMINI_UNTRACKED_SESSION_REF};

pub struct CliInvocation {
    pub command: TokioCommand,
    pub final_output_path: Option<PathBuf>,
}

pub enum ValidateResult {
    Valid,
    Corrected(String),
    Invalid(String),
}

#[async_trait]
pub trait CliAdapter: Send + Sync {
    fn tool(&self) -> CliToolId;

    fn build_invocation(
        &self,
        prompt: &str,
        session: &BridgeSession,
        settings: &AppSettings,
        streaming: bool,
        resume_existing: bool,
    ) -> anyhow::Result<CliInvocation>;

    fn extract_session_ref(&self, _output: &str) -> Option<String> {
        None
    }

    async fn validate_session_ref(
        &self,
        _session: &BridgeSession,
        _settings: &AppSettings,
        _locale: AppLocale,
    ) -> anyhow::Result<ValidateResult> {
        Ok(ValidateResult::Valid)
    }
}

pub fn adapter_for(tool: CliToolId) -> &'static dyn CliAdapter {
    static CLAUDE: ClaudeAdapter = ClaudeAdapter;
    static CODEX: CodexAdapter = CodexAdapter;
    static GEMINI: GeminiAdapter = GeminiAdapter;

    match tool {
        CliToolId::Claude => &CLAUDE,
        CliToolId::Codex => &CODEX,
        CliToolId::Gemini => &GEMINI,
    }
}

pub fn cli_type_label(tool: CliToolId) -> &'static str {
    match tool {
        CliToolId::Gemini => "Gemini CLI",
        CliToolId::Claude => "Claude Code",
        CliToolId::Codex => "Codex",
    }
}

fn setting_path_dir(raw: Option<&str>) -> Option<PathBuf> {
    let value = raw?.trim();
    if value.is_empty() {
        return None;
    }

    let path = PathBuf::from(value);
    if path.is_dir() {
        return Some(path);
    }
    if path.is_file() {
        return path.parent().map(|item| item.to_path_buf());
    }
    None
}

fn joined_child_path(program_path: &Path, settings: &AppSettings) -> Option<OsString> {
    let mut dirs = Vec::<PathBuf>::new();
    if let Some(parent) = program_path.parent() {
        dirs.push(parent.to_path_buf());
    }
    if let Some(dir) = setting_path_dir(settings.cli_tools_node_path.as_deref()) {
        dirs.push(dir);
    }
    if let Some(dir) = setting_path_dir(settings.cli_tools_npm_path.as_deref()) {
        dirs.push(dir);
    }
    crate::cli_tools::joined_child_path_for_dirs(&dirs)
}

fn base_async_command(program_path: &Path, settings: &AppSettings) -> TokioCommand {
    #[cfg(target_os = "windows")]
    {
        let is_cmd = program_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("cmd") || value.eq_ignore_ascii_case("bat"))
            .unwrap_or(false);
        if is_cmd {
            let mut cmd = TokioCommand::new("cmd");
            cmd.args(["/C", &program_path.to_string_lossy()]);
            crate::shell_env::apply_to_command(cmd.as_std_mut());
            if let Some(path) = joined_child_path(program_path, settings) {
                cmd.env("PATH", path);
            }
            crate::process::command_silent(cmd.as_std_mut());
            return cmd;
        }
    }

    let mut cmd = TokioCommand::new(program_path);
    crate::shell_env::apply_to_command(cmd.as_std_mut());
    if let Some(path) = joined_child_path(program_path, settings) {
        cmd.env("PATH", path);
    }
    crate::process::command_silent(cmd.as_std_mut());
    cmd
}

fn base_std_command(program_path: &Path, settings: &AppSettings) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        let is_cmd = program_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("cmd") || value.eq_ignore_ascii_case("bat"))
            .unwrap_or(false);
        if is_cmd {
            let mut cmd = std::process::Command::new("cmd");
            cmd.args(["/C", &program_path.to_string_lossy()]);
            crate::shell_env::apply_to_command(&mut cmd);
            if let Some(path) = joined_child_path(program_path, settings) {
                cmd.env("PATH", path);
            }
            crate::process::command_silent(&mut cmd);
            return cmd;
        }
    }

    let mut cmd = std::process::Command::new(program_path);
    crate::shell_env::apply_to_command(&mut cmd);
    if let Some(path) = joined_child_path(program_path, settings) {
        cmd.env("PATH", path);
    }
    crate::process::command_silent(&mut cmd);
    cmd
}

pub fn build_command(bin: &str, settings: &AppSettings) -> anyhow::Result<TokioCommand> {
    let exec_env = CliExecEnv::new(
        settings.cli_tools_npm_path.as_deref(),
        settings.cli_tools_node_path.as_deref(),
    );
    let program_path = exec_env
        .find_executable(bin)
        .unwrap_or_else(|| PathBuf::from(bin));
    let mut cmd = base_async_command(&program_path, settings);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);
    Ok(cmd)
}

pub fn build_std_command(
    bin: &str,
    settings: &AppSettings,
) -> anyhow::Result<std::process::Command> {
    let exec_env = CliExecEnv::new(
        settings.cli_tools_npm_path.as_deref(),
        settings.cli_tools_node_path.as_deref(),
    );
    let program_path = exec_env
        .find_executable(bin)
        .unwrap_or_else(|| PathBuf::from(bin));
    let mut cmd = base_std_command(&program_path, settings);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    Ok(cmd)
}

pub fn permission_mode_label(mode: storage::BridgePermissionMode) -> &'static str {
    match mode {
        storage::BridgePermissionMode::Safe => "Safe",
        storage::BridgePermissionMode::Yolo => "YOLO",
    }
}
