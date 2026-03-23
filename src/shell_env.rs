use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::sync::{LazyLock, RwLock};

#[cfg(unix)]
use anyhow::Context as _;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt as _;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
struct ShellEnvSnapshot {
    vars: HashMap<OsString, OsString>,
    path: Option<OsString>,
}

static SNAPSHOT: LazyLock<RwLock<ShellEnvSnapshot>> =
    LazyLock::new(|| RwLock::new(ShellEnvSnapshot::default()));

#[cfg(unix)]
static REFRESH_STARTED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
const SHELL_CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(unix)]
const SHELL_REFRESH_INTERVAL: Duration = Duration::from_secs(10 * 60);
#[cfg(unix)]
const ENV_CAPTURE_SENTINEL: &[u8] = b"__CLISWITCH_SHELL_ENV__";
#[cfg(unix)]
const ENV_CAPTURE_SCRIPT: &str = "printf '%s\\0' '__CLISWITCH_SHELL_ENV__'; command env -0";

pub fn init() {
    #[cfg(unix)]
    refresh_snapshot("startup");
}

pub fn spawn_refresh_task() {
    #[cfg(unix)]
    {
        if REFRESH_STARTED.swap(true, Ordering::AcqRel) {
            return;
        }

        tokio::spawn(async {
            let mut interval = tokio::time::interval(SHELL_REFRESH_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                refresh_snapshot("periodic refresh");
            }
        });
    }
}

pub fn shell_path() -> Option<OsString> {
    read_snapshot().path
}

pub fn shell_vars() -> HashMap<OsString, OsString> {
    read_snapshot().vars
}

pub fn is_excluded_var(key: &OsStr) -> bool {
    let key = key.to_string_lossy();
    matches!(
        key.as_ref(),
        "_" | "SHLVL" | "PWD" | "OLDPWD" | "TERM" | "TERM_PROGRAM" | "TERM_SESSION_ID"
    ) || key.starts_with("COMP_")
}

pub(crate) fn apply_to_command(cmd: &mut std::process::Command) {
    #[cfg(unix)]
    {
        for (key, _) in std::env::vars_os() {
            if is_excluded_var(&key) {
                cmd.env_remove(&key);
            }
        }

        for (key, value) in shell_vars() {
            if key == OsStr::new("PATH") {
                continue;
            }
            if is_excluded_var(&key) {
                continue;
            }
            cmd.env(&key, value);
        }
    }

    #[cfg(windows)]
    {
        let _ = cmd;
    }
}

fn read_snapshot() -> ShellEnvSnapshot {
    match SNAPSHOT.read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

#[cfg(unix)]
fn write_snapshot(snapshot: ShellEnvSnapshot) {
    match SNAPSHOT.write() {
        Ok(mut guard) => *guard = snapshot,
        Err(poisoned) => *poisoned.into_inner() = snapshot,
    }
}

#[cfg(unix)]
fn refresh_snapshot(reason: &str) {
    match capture_shell_snapshot() {
        Ok(snapshot) => write_snapshot(snapshot),
        Err(err) => tracing::warn!(err = %err, reason, "capture shell environment failed"),
    }
}

#[cfg(unix)]
fn capture_shell_snapshot() -> anyhow::Result<ShellEnvSnapshot> {
    let started = Instant::now();
    let mut errors = Vec::<String>::new();

    for shell in candidate_shells() {
        for args in [["-ilc", ENV_CAPTURE_SCRIPT], ["-lc", ENV_CAPTURE_SCRIPT]] {
            let Some(timeout) = remaining_timeout(started) else {
                break;
            };
            match capture_with_args(&shell, &args, timeout) {
                Ok(snapshot) => return Ok(snapshot),
                Err(err) => errors.push(format!("{} {}: {err}", shell.display(), args[0])),
            }
        }
    }

    let detail = if errors.is_empty() {
        "no shell capture attempts were executed".to_string()
    } else {
        errors.join("; ")
    };
    anyhow::bail!("shell env capture failed: {detail}");
}

#[cfg(unix)]
fn remaining_timeout(started: Instant) -> Option<Duration> {
    SHELL_CAPTURE_TIMEOUT.checked_sub(started.elapsed())
}

#[cfg(unix)]
fn candidate_shells() -> Vec<PathBuf> {
    let mut out = Vec::<PathBuf>::new();

    if let Some(shell) = std::env::var_os("SHELL")
        && !shell.is_empty()
    {
        out.push(PathBuf::from(shell));
    }

    let fallback = if cfg!(target_os = "macos") {
        PathBuf::from("/bin/zsh")
    } else {
        PathBuf::from("/bin/bash")
    };

    if !out.iter().any(|item| item == &fallback) {
        out.push(fallback);
    }

    out
}

#[cfg(unix)]
fn capture_with_args(
    shell: &Path,
    args: &[&str; 2],
    timeout: Duration,
) -> anyhow::Result<ShellEnvSnapshot> {
    let mut cmd = std::process::Command::new(shell);
    cmd.args(args);
    crate::process::command_silent(&mut cmd);
    let out = crate::process::command_output_with_timeout(&mut cmd, timeout)
        .with_context(|| format!("spawn {} {}", shell.display(), args[0]))?;

    if !out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        anyhow::bail!(
            "exit={:?} stdout={} stderr={}",
            out.status.code(),
            stdout,
            stderr
        );
    }

    parse_shell_env_snapshot(&out.stdout)
        .with_context(|| format!("parse {} {}", shell.display(), args[0]))
}

#[cfg(unix)]
fn parse_shell_env_snapshot(stdout: &[u8]) -> anyhow::Result<ShellEnvSnapshot> {
    let Some(offset) = find_sentinel(stdout) else {
        anyhow::bail!("sentinel not found in shell env output");
    };

    let mut vars = HashMap::<OsString, OsString>::new();
    for entry in stdout[offset..].split(|byte| *byte == 0) {
        if entry.is_empty() {
            continue;
        }
        let Some(eq_index) = entry.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        if eq_index == 0 {
            continue;
        }

        let key = OsString::from_vec(entry[..eq_index].to_vec());
        let value = OsString::from_vec(entry[eq_index + 1..].to_vec());
        vars.insert(key, value);
    }

    if vars.is_empty() {
        anyhow::bail!("shell env output was empty after parsing");
    }

    let path = vars.get(OsStr::new("PATH")).cloned();
    Ok(ShellEnvSnapshot { vars, path })
}

#[cfg(unix)]
fn find_sentinel(stdout: &[u8]) -> Option<usize> {
    let mut needle = Vec::with_capacity(ENV_CAPTURE_SENTINEL.len() + 1);
    needle.extend_from_slice(ENV_CAPTURE_SENTINEL);
    needle.push(0);

    stdout
        .windows(needle.len())
        .position(|window| window == needle.as_slice())
        .map(|index| index + needle.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_vars_match_expected_names() {
        assert!(is_excluded_var(OsStr::new("_")));
        assert!(is_excluded_var(OsStr::new("PWD")));
        assert!(is_excluded_var(OsStr::new("COMP_WORDBREAKS")));
        assert!(!is_excluded_var(OsStr::new("PATH")));
        assert!(!is_excluded_var(OsStr::new("GOPATH")));
    }

    #[cfg(unix)]
    #[test]
    fn parse_shell_env_snapshot_uses_sentinel_and_nul_entries() {
        let mut stdout = b"shell banner\n".to_vec();
        stdout.extend_from_slice(ENV_CAPTURE_SENTINEL);
        stdout.push(0);
        stdout.extend_from_slice(b"PATH=/custom/bin:/usr/bin\0");
        stdout.extend_from_slice(b"GOPATH=/tmp/go\0");

        let snapshot = parse_shell_env_snapshot(&stdout).expect("parse shell env snapshot");
        assert_eq!(
            snapshot.path.as_deref(),
            Some(OsStr::new("/custom/bin:/usr/bin"))
        );
        assert_eq!(
            snapshot
                .vars
                .get(OsStr::new("GOPATH"))
                .map(OsString::as_os_str),
            Some(OsStr::new("/tmp/go"))
        );
    }
}
