//! Resolves the config directory Codex CLI reads (`CODEX_HOME`).
//!
//! Codex CLI and the ChatGPT desktop app both default to `~/.codex`, sharing one
//! `config.toml` and one `auth.json`. Pointing the CLI at the local proxy there
//! hijacks the desktop app as well, which then cannot use its ChatGPT account
//! login. When isolation is enabled we move the CLI to `~/.cliswitch/codex` via
//! `CODEX_HOME` and leave `~/.codex` untouched for the desktop app.
//!
//! The desktop app launches from the GUI and does not inherit the shell
//! environment, so it keeps reading `~/.codex` regardless of what we export.

use anyhow::Context as _;
use directories::UserDirs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Mirrors `AppSettings::codex_isolated_home_enabled` so the synchronous path
/// helpers below stay callable without threading settings through every caller.
static ISOLATED: AtomicBool = AtomicBool::new(false);

pub fn set_isolated(enabled: bool) {
    ISOLATED.store(enabled, Ordering::Relaxed);
}

pub fn is_isolated() -> bool {
    ISOLATED.load(Ordering::Relaxed)
}

fn user_home_dir() -> anyhow::Result<PathBuf> {
    let user_dirs = UserDirs::new().context("读取用户目录失败")?;
    Ok(user_dirs.home_dir().to_path_buf())
}

/// The isolated directory CliSwitch manages: `~/.cliswitch/codex`.
///
/// Kept next to the generated shims in `~/.cliswitch/bin` so everything
/// CliSwitch owns lives under one root.
pub fn isolated_dir() -> anyhow::Result<PathBuf> {
    Ok(user_home_dir()?.join(".cliswitch").join("codex"))
}

/// The default shared directory: `~/.codex`.
pub fn default_dir() -> anyhow::Result<PathBuf> {
    Ok(user_home_dir()?.join(".codex"))
}

/// The directory Codex CLI actually reads.
///
/// Isolation wins when enabled, because that is the value we write into the
/// generated shim. Otherwise an explicit `CODEX_HOME` in our own environment
/// wins, matching what a CLI launched from the same shell would resolve.
pub fn resolve() -> anyhow::Result<PathBuf> {
    resolve_for(is_isolated())
}

/// `resolve` against an explicit toggle value, so tests do not have to flip the
/// process-global flag other tests read.
fn resolve_for(isolated: bool) -> anyhow::Result<PathBuf> {
    if isolated {
        return isolated_dir();
    }
    if let Some(path) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(path));
    }
    default_dir()
}

/// Codex refuses a `CODEX_HOME` that does not exist yet, so create it before
/// handing the path to the CLI.
pub fn ensure_dir(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("创建 Codex 配置目录失败：{}", path.display()))
}

/// The `CODEX_HOME` value to inject when launching Codex CLI, or `None` when
/// the CLI should keep resolving the directory on its own.
pub fn env_override() -> anyhow::Result<Option<PathBuf>> {
    if !is_isolated() {
        return Ok(None);
    }
    let dir = isolated_dir()?;
    ensure_dir(&dir)?;
    Ok(Some(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Isolation has to beat an inherited `CODEX_HOME`: the shim we generate is
    /// what defines the CLI's directory once the setting is on.
    #[test]
    fn isolation_wins_over_inherited_env() {
        assert_eq!(resolve_for(true).unwrap(), isolated_dir().unwrap());

        let expected = match std::env::var_os("CODEX_HOME") {
            Some(path) => PathBuf::from(path),
            None => default_dir().unwrap(),
        };
        assert_eq!(resolve_for(false).unwrap(), expected);
    }

    /// Without isolation we inject nothing, leaving the CLI to resolve its own
    /// directory exactly as it would without CliSwitch.
    #[test]
    fn env_override_is_absent_by_default() {
        assert!(!is_isolated());
        assert!(env_override().unwrap().is_none());
    }

    #[test]
    fn isolated_dir_sits_under_cliswitch_root() {
        let dir = isolated_dir().unwrap();
        assert!(dir.ends_with("codex"));
        assert!(dir.parent().unwrap().ends_with(".cliswitch"));
        assert_ne!(dir, default_dir().unwrap());
    }
}
