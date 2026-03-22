use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CMD_VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const NPM_BIN_TIMEOUT: Duration = Duration::from_secs(5);
const NPM_INSTALL_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const NPM_LIST_TIMEOUT: Duration = Duration::from_secs(8);
const BREW_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const NPM_REGISTRY_PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
const GEO_IPINFO_TIMEOUT: Duration = Duration::from_millis(1200);

pub const NPM_REGISTRY_OFFICIAL: &str = "https://registry.npmjs.org";
pub const NPM_REGISTRY_NPMMIRROR: &str = "https://registry.npmmirror.com";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CliToolId {
    Gemini,
    Claude,
    Codex,
}

impl CliToolId {
    pub const fn as_str(self) -> &'static str {
        match self {
            CliToolId::Gemini => "gemini",
            CliToolId::Claude => "claude",
            CliToolId::Codex => "codex",
        }
    }
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CliToolInstallMethod {
    /// Installed/updated by CliSwitch into our managed npm prefix.
    ManagedNpmPrefix,
    /// Installed/updated by Homebrew (macOS).
    Brew,
    /// Installed/updated by the user's global npm prefix.
    Npm,
    /// Unknown/unsupported installer.
    Other,
}

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

pub fn cli_tools_npm_prefix_bin_dir(prefix_dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        prefix_dir.to_path_buf()
    }
    #[cfg(not(windows))]
    {
        prefix_dir.join("bin")
    }
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

#[derive(Debug, Clone, serde::Deserialize)]
struct IpInfoResponse {
    #[serde(default)]
    country: Option<String>,
}

async fn detect_country_code(client: &reqwest::Client) -> Option<String> {
    let res = client
        .get("https://ipinfo.io/json")
        .timeout(GEO_IPINFO_TIMEOUT)
        .send()
        .await
        .ok()?;

    if !res.status().is_success() {
        return None;
    }

    let ipinfo: IpInfoResponse = res.json().await.ok()?;
    let c = ipinfo.country?.trim().to_uppercase();
    (!c.is_empty()).then_some(c)
}

/// Pick the npm registry for CLI tools installation/update.
///
/// Policy:
/// - No user manual override.
/// - Prefer CN-friendly registry when the current public IP is in CN.
/// - If geo-detection fails, fall back to quick reachability/latency probes.
pub async fn pick_cli_tools_npm_registry(client: &reqwest::Client) -> String {
    let preferred = match detect_country_code(client).await.as_deref() {
        Some("CN") => NPM_REGISTRY_NPMMIRROR,
        Some(_) => NPM_REGISTRY_OFFICIAL,
        None => "",
    };

    let other = if preferred == NPM_REGISTRY_NPMMIRROR {
        NPM_REGISTRY_OFFICIAL
    } else {
        NPM_REGISTRY_NPMMIRROR
    };

    if !preferred.is_empty() {
        // Verify reachability quickly; if preferred is down but the other is up, switch.
        let (p_ok, o_ok) = tokio::join!(
            probe_registry_latency(client, preferred),
            probe_registry_latency(client, other),
        );
        if p_ok.is_some() {
            return preferred.to_string();
        }
        if o_ok.is_some() {
            return other.to_string();
        }
        return preferred.to_string();
    }

    // Geo-detection failed; pick the better reachable registry.
    let (npmjs, mirror) = tokio::join!(
        probe_registry_latency(client, NPM_REGISTRY_OFFICIAL),
        probe_registry_latency(client, NPM_REGISTRY_NPMMIRROR),
    );

    match (npmjs, mirror) {
        (None, Some(_)) => NPM_REGISTRY_NPMMIRROR.to_string(),
        (Some(_), None) => NPM_REGISTRY_OFFICIAL.to_string(),
        (Some(a), Some(b)) => {
            // Keep npmjs unless it is noticeably slower.
            if b + Duration::from_millis(100) < a {
                NPM_REGISTRY_NPMMIRROR.to_string()
            } else {
                NPM_REGISTRY_OFFICIAL.to_string()
            }
        }
        _ => NPM_REGISTRY_OFFICIAL.to_string(),
    }
}

fn fallback_executable_dirs() -> Vec<PathBuf> {
    fallback_executable_dirs_with_home(
        std::env::var_os("HOME").as_deref().map(Path::new),
        std::env::var_os("NVM_DIR").as_deref().map(Path::new),
    )
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn push_nvm_bins(out: &mut Vec<PathBuf>, home: &Path, nvm_dir: Option<&Path>) {
    let nvm_dir = nvm_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| home.join(".nvm"));

    let versions_dir = nvm_dir.join("versions").join("node");
    let Ok(read_dir) = std::fs::read_dir(&versions_dir) else {
        return;
    };

    let mut bins: Vec<PathBuf> = Vec::new();
    for e in read_dir.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let bin = p.join("bin");
        if bin.is_dir() {
            bins.push(bin);
        }
    }

    // Keep deterministic ordering; don't blow up PATH for users with many versions installed.
    bins.sort();
    out.extend(bins.into_iter().take(20));
}

fn fallback_executable_dirs_with_home(home: Option<&Path>, nvm_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // Common install locations that are often missing from GUI-app PATH.
        out.push(PathBuf::from("/opt/homebrew/bin"));
        out.push(PathBuf::from("/opt/homebrew/sbin"));
        out.push(PathBuf::from("/usr/local/bin"));
        out.push(PathBuf::from("/usr/local/sbin"));
        out.push(PathBuf::from("/usr/bin"));
        out.push(PathBuf::from("/bin"));
        out.push(PathBuf::from("/usr/sbin"));
        out.push(PathBuf::from("/sbin"));

        if let Some(home) = home {
            out.push(home.join(".local").join("bin"));
            out.push(home.join(".cargo").join("bin"));
            out.push(home.join(".volta").join("bin"));
            out.push(home.join(".asdf").join("bin"));
            out.push(home.join(".asdf").join("shims"));
            out.push(home.join(".npm-global").join("bin"));
            out.push(home.join(".yarn").join("bin"));
            out.push(
                home.join(".fnm")
                    .join("aliases")
                    .join("default")
                    .join("bin"),
            );
            out.push(home.join(".local").join("share").join("mise").join("shims"));
            push_nvm_bins(&mut out, home, nvm_dir);
        }
    }

    #[cfg(target_os = "linux")]
    {
        out.push(PathBuf::from("/usr/local/bin"));
        out.push(PathBuf::from("/usr/local/sbin"));
        out.push(PathBuf::from("/usr/bin"));
        out.push(PathBuf::from("/bin"));
        out.push(PathBuf::from("/usr/sbin"));
        out.push(PathBuf::from("/sbin"));

        // linuxbrew
        out.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin"));
        out.push(PathBuf::from("/home/linuxbrew/.linuxbrew/sbin"));

        if let Some(home) = home {
            out.push(home.join(".local").join("bin"));
            out.push(home.join(".cargo").join("bin"));
            out.push(home.join(".volta").join("bin"));
            out.push(home.join(".asdf").join("bin"));
            out.push(home.join(".asdf").join("shims"));
            out.push(home.join(".linuxbrew").join("bin"));
            out.push(home.join(".linuxbrew").join("sbin"));
            out.push(home.join(".npm-global").join("bin"));
            out.push(home.join(".yarn").join("bin"));
            out.push(
                home.join(".fnm")
                    .join("aliases")
                    .join("default")
                    .join("bin"),
            );
            out.push(home.join(".local").join("share").join("mise").join("shims"));
            push_nvm_bins(&mut out, home, nvm_dir);
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Node installer
        if let Some(p) = std::env::var_os("ProgramFiles") {
            out.push(PathBuf::from(p).join("nodejs"));
        }
        if let Some(p) = std::env::var_os("ProgramFiles(x86)") {
            out.push(PathBuf::from(p).join("nodejs"));
        }

        // Global npm bin dir
        if let Some(appdata) = std::env::var_os("APPDATA") {
            out.push(PathBuf::from(appdata).join("npm"));
        }

        // Package managers
        out.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
        if let Some(userprofile) = std::env::var_os("USERPROFILE") {
            out.push(PathBuf::from(&userprofile).join("scoop").join("shims"));
            out.push(PathBuf::from(userprofile).join(".volta").join("bin"));
        }

        // nvm-windows
        if let Some(nvm_home) = std::env::var_os("NVM_HOME") {
            out.push(PathBuf::from(nvm_home));
        }
        if let Some(nvm_symlink) = std::env::var_os("NVM_SYMLINK") {
            out.push(PathBuf::from(nvm_symlink));
        }

        // User-level Node installs
        if let Some(localappdata) = std::env::var_os("LOCALAPPDATA") {
            out.push(PathBuf::from(&localappdata).join("Programs").join("nodejs"));
        }
        if let Some(localappdata) = std::env::var_os("LOCALAPPDATA") {
            out.push(
                PathBuf::from(localappdata)
                    .join("Microsoft")
                    .join("WindowsApps"),
            );
        }
    }

    // Dedup, keep order.
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

pub fn find_executable_in_path(name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(path_var) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path_var));
    }
    dirs.extend(fallback_executable_dirs());

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

    for dir in dirs {
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

#[derive(Debug, Clone)]
pub struct DetectedCliTool {
    pub installed: bool,
    pub version: Option<String>,
    pub install_method: CliToolInstallMethod,
    pub install_path: Option<PathBuf>,
    pub installer_path: Option<PathBuf>,
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
    let mut out: Vec<PathBuf> = Vec::new();
    out.extend_from_slice(extra_dirs);
    if let Some(env_path) = std::env::var_os("PATH") {
        out.extend(std::env::split_paths(&env_path));
    }
    out.extend(fallback_executable_dirs());

    // Dedup, keep order.
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    out.retain(|p| seen.insert(p.clone()));

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

    pub fn command_for(&self, program: &str) -> Option<std::process::Command> {
        let program_path = self.find_executable(program)?;
        Some(self.cmd(program_path))
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

    pub fn npm_install_global_with_registry(
        &self,
        pkg: &str,
        registry: Option<&str>,
    ) -> anyhow::Result<CmdOutput> {
        let npm = self
            .npm
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("npm not found"))?;

        let mut cmd = self.cmd(npm);
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

    pub fn npm_global_has_package(&self, pkg: &str) -> bool {
        let Some(npm) = self.npm.as_ref() else {
            return false;
        };

        let mut cmd = self.cmd(npm.clone());
        cmd.args(["list", "-g", pkg]);
        let out = crate::process::command_output_with_timeout(&mut cmd, NPM_LIST_TIMEOUT);
        out.is_ok_and(|o| o.status.success())
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

#[cfg(target_os = "macos")]
fn brew_list_contains(brew: &Path, args: &[&str], needle: &str) -> bool {
    let mut cmd = std::process::Command::new(brew);
    cmd.args(args);
    crate::process::command_silent(&mut cmd);
    let out = crate::process::command_output_with_timeout(&mut cmd, CMD_VERSION_TIMEOUT);
    let Ok(out) = out else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.lines().any(|l| l.trim() == needle)
}

#[cfg(target_os = "macos")]
fn brew_detect_cli_tool_install_method(
    brew: &Path,
    tool: CliToolId,
) -> Option<CliToolInstallMethod> {
    match tool {
        CliToolId::Gemini => brew_list_contains(brew, &["list", "--formula"], "gemini-cli")
            .then_some(CliToolInstallMethod::Brew),
        CliToolId::Codex => brew_list_contains(brew, &["list", "--cask"], "codex")
            .then_some(CliToolInstallMethod::Brew),
        CliToolId::Claude => None,
    }
}

#[cfg(not(target_os = "macos"))]
fn brew_detect_cli_tool_install_method(
    _brew: &Path,
    _tool: CliToolId,
) -> Option<CliToolInstallMethod> {
    None
}

pub fn detect_cli_tool(env: &CliExecEnv, data_dir: &Path, def: &CliToolDef) -> DetectedCliTool {
    let tools_prefix_dir = cli_tools_npm_prefix_dir(data_dir);
    let managed_bin_dir = cli_tools_npm_prefix_bin_dir(&tools_prefix_dir);

    let managed_tool_path =
        resolve_program_from_user_path(def.bin, &managed_bin_dir.to_string_lossy());
    let managed_present = managed_tool_path.is_some();
    let mut resolved_path = env.find_executable(def.bin);

    // If we resolve to our own terminal shim, treat it as "no system path" and rely on
    // managed-prefix detection.
    if let Ok(shim_path) = crate::terminal::cli_tool_shim_path(def.bin)
        && resolved_path.as_ref() == Some(&shim_path)
    {
        resolved_path = None;
    }

    let install_path = managed_tool_path.clone().or(resolved_path);

    // Install method & installer path.
    let mut method = CliToolInstallMethod::Other;
    let mut installer_path: Option<PathBuf> = None;

    if install_path.is_some() && managed_present {
        method = CliToolInstallMethod::ManagedNpmPrefix;
        installer_path = env.resolved_npm_path().map(|p| p.to_path_buf());
    } else if install_path.is_some()
        && let Some(brew) = find_executable_in_path("brew")
        && let Some(m) = brew_detect_cli_tool_install_method(&brew, def.id)
    {
        method = m;
        installer_path = Some(brew);
    }

    if method == CliToolInstallMethod::Other
        && install_path.is_some()
        && env.npm_global_has_package(def.npm_package)
    {
        method = CliToolInstallMethod::Npm;
        installer_path = env.resolved_npm_path().map(|p| p.to_path_buf());
    }

    let version = install_path
        .as_ref()
        .and_then(|p| env.try_get_cmd_version_by_path(p))
        .map(|v| normalize_version_string(&v));

    let installed = install_path.is_some();
    DetectedCliTool {
        installed,
        version,
        install_method: method,
        install_path,
        installer_path,
    }
}

pub fn brew_upgrade_cli_tool(brew: &Path, tool: CliToolId) -> anyhow::Result<CmdOutput> {
    let mut cmd = std::process::Command::new(brew);
    match tool {
        CliToolId::Gemini => cmd.args(["upgrade", "gemini-cli"]),
        CliToolId::Codex => cmd.args(["upgrade", "--cask", "codex"]),
        CliToolId::Claude => {
            anyhow::bail!("Claude Code does not support brew upgrade")
        }
    };

    crate::process::command_silent(&mut cmd);
    let out = crate::process::command_output_with_timeout(&mut cmd, BREW_TIMEOUT)
        .with_context(|| format!("run brew upgrade for {tool:?} failed"))?;

    Ok(CmdOutput {
        status: out.status,
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    })
}

#[cfg(test)]
mod path_tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-cli-tools-test-{}-{}",
            uuid::Uuid::new_v4(),
            name
        ))
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn fallback_executable_dirs_includes_common_user_dirs_and_nvm_bins() {
        let home = tmp_dir("home");
        std::fs::create_dir_all(&home).expect("create tmp home dir");

        // Simulate an nvm installation.
        let nvm_bin = home
            .join(".nvm")
            .join("versions")
            .join("node")
            .join("v1.2.3")
            .join("bin");
        std::fs::create_dir_all(&nvm_bin).expect("create nvm bin dir");

        // Do NOT rely on environment NVM_DIR (CI may set it). Use our temp home instead.
        let dirs = fallback_executable_dirs_with_home(Some(&home), None);
        assert!(dirs.contains(&home.join(".local").join("bin")));
        assert!(dirs.contains(&home.join(".asdf").join("shims")));
        assert!(dirs.contains(&nvm_bin));

        let _ = std::fs::remove_dir_all(&home);
    }
}

#[cfg(test)]
mod detect_tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-cli-tools-test-{}-{}",
            uuid::Uuid::new_v4(),
            name
        ))
    }

    #[test]
    fn detect_cli_tool_prefers_managed_prefix_over_system() {
        let data_dir = tmp_dir("data");
        let prefix_dir = cli_tools_npm_prefix_dir(&data_dir);
        let bin_dir = cli_tools_npm_prefix_bin_dir(&prefix_dir);
        std::fs::create_dir_all(&bin_dir).expect("create managed bin dir");

        // Use a unique bin name to avoid interacting with user's environment.
        let bin = "cliswitch-test-tool-bin";

        #[cfg(windows)]
        let tool_file = format!("{bin}.cmd");
        #[cfg(not(windows))]
        let tool_file = bin.to_string();

        let tool_path = bin_dir.join(&tool_file);
        std::fs::write(&tool_path, b"").expect("write dummy tool file");

        let env = CliExecEnv::new(None, None);
        let def = CliToolDef {
            id: CliToolId::Codex,
            name: "Test Tool",
            bin,
            npm_package: "cliswitch-test-tool-pkg",
        };

        let detected = detect_cli_tool(&env, &data_dir, &def);
        assert!(detected.installed);
        assert_eq!(
            detected.install_method,
            CliToolInstallMethod::ManagedNpmPrefix
        );
        assert_eq!(detected.install_path.as_deref(), Some(tool_path.as_path()));

        let _ = std::fs::remove_dir_all(&data_dir);
    }
}
