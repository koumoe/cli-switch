use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{PathBuf};

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
            let pathext = std::env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".EXE;.CMD;.BAT;.COM"));
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
    let out = std::process::Command::new(program_path)
        .arg("--version")
        .output()
        .ok()?;
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
        let cleaned = token.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+'));
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
    let npm = find_executable_in_path("npm")
        .ok_or_else(|| anyhow::anyhow!("npm not found in PATH"))?;

    let out = std::process::Command::new(npm)
        .args(["install", "-g", pkg, "--no-fund", "--no-audit"])
        .output()
        .with_context(|| format!("run npm install -g {pkg} failed"))?;

    Ok(CmdOutput {
        status: out.status,
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    })
}
