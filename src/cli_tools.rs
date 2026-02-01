use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CMD_VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const NPM_BIN_TIMEOUT: Duration = Duration::from_secs(5);
const NPM_INSTALL_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const NPM_REGISTRY_PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

pub const NPM_REGISTRY_OFFICIAL: &str = "https://registry.npmjs.org";
pub const NPM_REGISTRY_NPMMIRROR: &str = "https://registry.npmmirror.com";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CliToolId {
    Gemini,
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CliToolDef {
    pub id: CliToolId,
    pub name: &'static str,
    pub bin: &'static str,
    pub npm_package: &'static str,
}

pub const CLI_TOOLS: &[CliToolDef] = &[
    CliToolDef {
        id: CliToolId::Gemini,
        name: "Gemini CLI",
        bin: "gemini",
        npm_package: "@google/gemini-cli",
    },
    CliToolDef {
        id: CliToolId::Claude,
        name: "Claude Code",
        bin: "claude",
        npm_package: "@anthropic-ai/claude-code",
    },
    CliToolDef {
        id: CliToolId::Codex,
        name: "Codex",
        bin: "codex",
        npm_package: "@openai/codex",
    },
];

#[derive(Debug, Clone)]
pub struct CmdOutput {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

pub fn os_name() -> &'static str {
    std::env::consts::OS
}

/// CliSwitch-managed npm global prefix for installing CLI tools.
///
/// We intentionally do NOT install into the user's system/global npm prefix to avoid
/// permission issues and platform differences (brew/choco/apt/winget, etc.).
pub fn cli_tools_npm_prefix_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("cli_tools").join("npm")
}

async fn probe_registry_latency(client: &reqwest::Client, base: &str) -> Option<Duration> {
    let url = format!("{base}/-/ping");
    let started = Instant::now();
    let res = client
        .get(url)
        .timeout(NPM_REGISTRY_PROBE_TIMEOUT)
        .send()
        .await
        .ok()?;
    res.status().is_success().then_some(started.elapsed())
}

/// Pick a registry override for CLI tools installation/update.
///
/// - If `user_override` is set, always use it.
/// - Otherwise, best-effort probe and prefer a faster reachable registry (CN networks commonly
///   benefit from npmmirror). If we can't decide, return `None` to let npm use its own defaults.
pub async fn pick_cli_tools_npm_registry(
    client: &reqwest::Client,
    user_override: Option<&str>,
) -> Option<String> {
    let override_clean = user_override.map(|s| s.trim()).filter(|s| !s.is_empty());
    if let Some(v) = override_clean {
        return Some(v.to_string());
    }

    let npmjs = probe_registry_latency(client, NPM_REGISTRY_OFFICIAL).await;
    let mirror = probe_registry_latency(client, NPM_REGISTRY_NPMMIRROR).await;

    match (npmjs, mirror) {
        (None, Some(_)) => Some(NPM_REGISTRY_NPMMIRROR.to_string()),
        (Some(a), Some(b)) => {
            // Keep the default if npmjs is not noticeably slower.
            if b + Duration::from_millis(100) < a {
                Some(NPM_REGISTRY_NPMMIRROR.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn find_executable_in_path(name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }

    let path_var = std::env::var_os("PATH")?;
    let paths = std::env::split_paths(&path_var);

    #[cfg(target_os = "windows")]
    let candidate_names: Vec<OsString> = {
        let base = OsString::from(name);
        if std::path::Path::new(name).extension().is_some() {
            vec![base]
        } else {
            let pathext = std::env::var_os("PATHEXT")
                .unwrap_or_else(|| OsString::from(".EXE;.CMD;.BAT;.COM"));
            let exts: Vec<_> = pathext
                .to_string_lossy()
                .split(';')
                .filter_map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .collect();
            let mut out = Vec::with_capacity(1 + exts.len());
            for ext in exts {
                out.push(OsString::from(format!("{name}{ext}")));
            }
            if out.is_empty() {
                out.push(base);
            }
            out
        }
    };

    #[cfg(not(target_os = "windows"))]
    let candidate_names: Vec<OsString> = vec![OsString::from(name)];

    for dir in paths {
        for cand in &candidate_names {
            let p = dir.join(cand);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn try_get_cmd_version(program: &str) -> Option<String> {
    let program_path = find_executable_in_path(program)?;
    let mut cmd = std::process::Command::new(program_path);
    cmd.arg("--version");
    crate::process::command_silent(&mut cmd);
    let out = crate::process::command_output_with_timeout(&mut cmd, CMD_VERSION_TIMEOUT).ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }
    if !stderr.is_empty() {
        return Some(stderr);
    }
    None
}

fn extract_semver(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| {
            !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')
        });
        let cleaned = cleaned.strip_prefix('v').unwrap_or(cleaned);
        if semver::Version::parse(cleaned).is_ok() {
            return Some(cleaned.to_string());
        }
    }
    None
}

pub fn normalize_version_string(raw: &str) -> String {
    extract_semver(raw).unwrap_or_else(|| raw.trim().to_string())
}

pub fn npm_available() -> bool {
    find_executable_in_path("npm").is_some()
}

pub fn try_get_npm_version() -> Option<String> {
    try_get_cmd_version("npm")
}

pub fn try_get_node_version() -> Option<String> {
    try_get_cmd_version("node")
}

pub fn npm_install_global(pkg: &str) -> anyhow::Result<CmdOutput> {
    let npm =
        find_executable_in_path("npm").ok_or_else(|| anyhow::anyhow!("npm not found in PATH"))?;

    let mut cmd = std::process::Command::new(npm);
    cmd.args(["install", "-g", pkg, "--no-fund", "--no-audit"]);
    crate::process::command_silent(&mut cmd);
    let out = crate::process::command_output_with_timeout(&mut cmd, NPM_INSTALL_TIMEOUT)
        .with_context(|| format!("run npm install -g {pkg} failed"))?;

    Ok(CmdOutput {
        status: out.status,
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    })
}

#[derive(Debug, Clone, Default)]
pub struct CliExecEnv {
    npm: Option<PathBuf>,
    node: Option<PathBuf>,
    npm_global_bin: Option<PathBuf>,
    child_path: Option<OsString>,
    extra_dirs: Vec<PathBuf>,
}

fn candidate_names(name: &str) -> Vec<OsString> {
    #[cfg(target_os = "windows")]
    {
        let base = OsString::from(name);
        if std::path::Path::new(name).extension().is_some() {
            vec![base]
        } else {
            let pathext = std::env::var_os("PATHEXT")
                .unwrap_or_else(|| OsString::from(".EXE;.CMD;.BAT;.COM"));
            let exts: Vec<_> = pathext
                .to_string_lossy()
                .split(';')
                .filter_map(|s| {
                    let t = s.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .collect();
            let mut out = Vec::with_capacity(1 + exts.len());
            for ext in exts {
                out.push(OsString::from(format!("{name}{ext}")));
            }
            if out.is_empty() {
                out.push(base);
            }
            out
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![OsString::from(name)]
    }
}

fn find_executable_in_dirs(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }

    let candidates = candidate_names(name);
    for dir in dirs {
        for cand in &candidates {
            let p = dir.join(cand);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub(crate) fn resolve_program_from_user_path(name: &str, user_path: &str) -> Option<PathBuf> {
    let p = PathBuf::from(user_path.trim());
    if p.is_file() {
        return Some(p);
    }
    if p.is_dir() {
        return find_executable_in_dirs(name, &[p]);
    }
    None
}

pub(crate) fn try_get_cmd_version_at(program_path: &Path) -> Option<String> {
    if !program_path.is_file() {
        return None;
    }

    // Ensure scripts like npm(.cmd) can find sibling node by prefixing PATH with the program dir.
    let program_dir = program_path.parent().map(|d| d.to_path_buf())?;
    let mut dirs: Vec<PathBuf> = vec![program_dir];
    if let Some(env_path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&env_path));
    }
    let joined = std::env::join_paths(dirs).ok();

    #[cfg(target_os = "windows")]
    {
        let is_cmd = program_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("cmd") || s.eq_ignore_ascii_case("bat"))
            .unwrap_or(false);
        if is_cmd {
            let p = program_path.to_string_lossy().to_string();
            let mut cmd = std::process::Command::new("cmd");
            cmd.args(["/C", &p, "--version"]);
            if let Some(p) = &joined {
                cmd.env("PATH", p);
            }
            crate::process::command_silent(&mut cmd);
            let out =
                crate::process::command_output_with_timeout(&mut cmd, CMD_VERSION_TIMEOUT).ok()?;
            if !out.status.success() {
                return None;
            }
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if !stdout.is_empty() {
                return Some(stdout);
            }
            if !stderr.is_empty() {
                return Some(stderr);
            }
            return None;
        }
    }

    let mut cmd = std::process::Command::new(program_path);
    cmd.arg("--version");
    if let Some(p) = &joined {
        cmd.env("PATH", p);
    }
    crate::process::command_silent(&mut cmd);
    let out = crate::process::command_output_with_timeout(&mut cmd, CMD_VERSION_TIMEOUT).ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }
    if !stderr.is_empty() {
        return Some(stderr);
    }
    None
}

fn resolve_program(name: &str, user_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = user_path
        && !p.trim().is_empty()
        && let Some(found) = resolve_program_from_user_path(name, p)
    {
        return Some(found);
    }
    find_executable_in_path(name)
}

fn user_path_dir(user_path: Option<&str>) -> Option<PathBuf> {
    let raw = user_path?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let p = PathBuf::from(trimmed);
    if p.is_dir() {
        return Some(p);
    }
    if p.is_file() {
        return p.parent().map(|d| d.to_path_buf());
    }
    None
}

fn program_dir(p: Option<&PathBuf>) -> Option<PathBuf> {
    p.and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

fn join_child_path(extra_dirs: &[PathBuf]) -> Option<OsString> {
    if extra_dirs.is_empty() {
        return None;
    }
    let mut out: Vec<PathBuf> = Vec::new();
    out.extend_from_slice(extra_dirs);
    if let Some(env_path) = std::env::var_os("PATH") {
        out.extend(std::env::split_paths(&env_path));
    }
    std::env::join_paths(out).ok()
}

impl CliExecEnv {
    pub fn new(npm_path: Option<&str>, node_path: Option<&str>) -> Self {
        let npm = resolve_program("npm", npm_path);
        let node = resolve_program("node", node_path);

        let mut child_dirs: Vec<PathBuf> = Vec::new();
        if let Some(dir) = program_dir(node.as_ref()) {
            child_dirs.push(dir);
        }
        if let Some(dir) = program_dir(npm.as_ref()) {
            child_dirs.push(dir);
        }
        if let Some(dir) = user_path_dir(node_path) {
            child_dirs.push(dir);
        }
        if let Some(dir) = user_path_dir(npm_path) {
            child_dirs.push(dir);
        }

        // Dedup, keep order.
        let mut seen = std::collections::HashSet::<PathBuf>::new();
        child_dirs.retain(|p| seen.insert(p.clone()));

        let child_path = join_child_path(&child_dirs);

        let mut env = Self {
            npm,
            node,
            npm_global_bin: None,
            child_path,
            extra_dirs: child_dirs,
        };

        let global_bin = env.compute_npm_global_bin_dir();
        env.npm_global_bin = global_bin.clone();
        if let Some(global_bin) = global_bin
            && !env.extra_dirs.contains(&global_bin)
        {
            env.extra_dirs.push(global_bin);
        }

        env
    }

    pub fn npm_available(&self) -> bool {
        self.npm.is_some()
    }

    pub fn resolved_npm_path(&self) -> Option<&Path> {
        self.npm.as_deref()
    }

    pub fn resolved_node_path(&self) -> Option<&Path> {
        self.node.as_deref()
    }

    pub fn node_bin_dir(&self) -> Option<PathBuf> {
        self.node
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }

    pub fn npm_global_bin_dir(&self) -> Option<PathBuf> {
        self.npm_global_bin.clone()
    }

    pub fn npm_global_bin_dir_for_prefix(&self, prefix: &Path) -> Option<PathBuf> {
        let npm = self.npm.as_ref()?;
        let mut cmd = self.cmd(npm.clone());
        cmd.env("npm_config_prefix", prefix.as_os_str());
        cmd.args(["bin", "-g"]);
        let out = crate::process::command_output_with_timeout(&mut cmd, NPM_BIN_TIMEOUT).ok()?;
        if !out.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if stdout.is_empty() {
            return None;
        }
        let p = PathBuf::from(stdout);
        p.is_dir().then_some(p)
    }

    pub fn try_get_npm_version(&self) -> Option<String> {
        self.try_get_cmd_version("npm")
    }

    pub fn try_get_node_version(&self) -> Option<String> {
        self.try_get_cmd_version("node")
    }

    pub fn find_executable(&self, name: &str) -> Option<PathBuf> {
        find_executable_in_path(name).or_else(|| find_executable_in_dirs(name, &self.extra_dirs))
    }

    fn cmd(&self, program_path: PathBuf) -> std::process::Command {
        // On Windows, npm is commonly a .cmd shim. CreateProcess can't execute it directly,
        // so we route through `cmd /C`.
        #[cfg(target_os = "windows")]
        {
            let is_cmd = program_path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("cmd") || s.eq_ignore_ascii_case("bat"))
                .unwrap_or(false);
            if is_cmd {
                let p = program_path.to_string_lossy().to_string();
                let mut cmd = std::process::Command::new("cmd");
                cmd.args(["/C", &p]);
                if let Some(p) = &self.child_path {
                    cmd.env("PATH", p);
                }
                crate::process::command_silent(&mut cmd);
                return cmd;
            }
        }

        let mut cmd = std::process::Command::new(program_path);
        if let Some(p) = &self.child_path {
            cmd.env("PATH", p);
        }
        crate::process::command_silent(&mut cmd);
        cmd
    }

    pub fn try_get_cmd_version(&self, program: &str) -> Option<String> {
        let program_path = self.find_executable(program)?;
        let mut cmd = self.cmd(program_path);
        cmd.arg("--version");
        let out =
            crate::process::command_output_with_timeout(&mut cmd, CMD_VERSION_TIMEOUT).ok()?;
        if !out.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !stdout.is_empty() {
            return Some(stdout);
        }
        if !stderr.is_empty() {
            return Some(stderr);
        }
        None
    }

    pub fn try_get_cmd_version_by_path(&self, program_path: &Path) -> Option<String> {
        if !program_path.is_file() {
            return None;
        }
        let mut cmd = self.cmd(program_path.to_path_buf());
        cmd.arg("--version");
        let out =
            crate::process::command_output_with_timeout(&mut cmd, CMD_VERSION_TIMEOUT).ok()?;
        if !out.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !stdout.is_empty() {
            return Some(stdout);
        }
        if !stderr.is_empty() {
            return Some(stderr);
        }
        None
    }

    fn compute_npm_global_bin_dir(&self) -> Option<PathBuf> {
        let npm = self.npm.as_ref()?;
        let mut cmd = self.cmd(npm.clone());
        cmd.args(["bin", "-g"]);
        let out = crate::process::command_output_with_timeout(&mut cmd, NPM_BIN_TIMEOUT).ok()?;
        if !out.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if stdout.is_empty() {
            return None;
        }
        let p = PathBuf::from(stdout);
        p.is_dir().then_some(p)
    }

    pub fn npm_install_global(&self, pkg: &str) -> anyhow::Result<CmdOutput> {
        let npm = self
            .npm
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("npm not found"))?;

        let mut cmd = self.cmd(npm);
        cmd.args(["install", "-g", pkg, "--no-fund", "--no-audit"]);
        let out = crate::process::command_output_with_timeout(&mut cmd, NPM_INSTALL_TIMEOUT)
            .with_context(|| format!("run npm install -g {pkg} failed"))?;

        Ok(CmdOutput {
            status: out.status,
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        })
    }

    pub fn npm_install_global_to_prefix(
        &self,
        pkg: &str,
        prefix: &Path,
        registry: Option<&str>,
    ) -> anyhow::Result<CmdOutput> {
        let npm = self
            .npm
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("npm not found"))?;

        let mut cmd = self.cmd(npm);
        cmd.env("npm_config_prefix", prefix.as_os_str());
        if let Some(registry) = registry.map(|s| s.trim())
            && !registry.is_empty()
        {
            cmd.env("npm_config_registry", registry);
        }

        cmd.args(["install", "-g", pkg, "--no-fund", "--no-audit"]);
        let out = crate::process::command_output_with_timeout(&mut cmd, NPM_INSTALL_TIMEOUT)
            .with_context(|| format!("run npm install -g {pkg} failed"))?;

        Ok(CmdOutput {
            status: out.status,
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        })
    }
}
