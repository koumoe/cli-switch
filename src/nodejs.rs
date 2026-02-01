use anyhow::Context as _;
use serde::Deserialize;
use sha2::Digest as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::cli_tools;
use crate::events::{self, AppEvent, NpmEnvInstallProgress};
use crate::process;

fn publish_progress(
    stage: &str,
    version: Option<&str>,
    percent: Option<u8>,
    total_bytes: Option<u64>,
    downloaded_bytes: Option<u64>,
    message: Option<String>,
) {
    events::publish(AppEvent::NpmEnvInstallProgress(NpmEnvInstallProgress {
        stage: stage.to_string(),
        version: version.map(|v| v.to_string()),
        percent,
        total_bytes,
        downloaded_bytes,
        message,
    }));
}

#[derive(Debug, Clone, Copy)]
enum ArchiveKind {
    TarGz,
    TarXz,
    Zip,
}

#[derive(Debug, Clone)]
pub struct ManagedNodePaths {
    pub node_path: PathBuf,
    pub npm_path: PathBuf,
}

#[derive(Debug, Clone)]
struct NodeDist {
    url: String,
    shasums_url: String,
    filename: String,
    dir_name: String,
    kind: ArchiveKind,
}

#[derive(Debug, Clone, Deserialize)]
struct NodeIndexEntry {
    // Example: "v22.11.0"
    version: String,
    // Node uses either string (e.g. "Jod") or false.
    lts: serde_json::Value,
}

async fn resolve_latest_lts_version(client: &reqwest::Client) -> anyhow::Result<String> {
    // https://nodejs.org/dist/index.json is sorted by version descending.
    let text = download_text(client, "https://nodejs.org/dist/index.json").await?;
    let list: Vec<NodeIndexEntry> =
        serde_json::from_str(&text).with_context(|| "parse nodejs dist index.json failed")?;

    for e in list {
        if matches!(e.lts, serde_json::Value::String(_)) {
            return Ok(e.version.trim_start_matches('v').to_string());
        }
    }
    anyhow::bail!("no LTS version found in nodejs dist index.json")
}

fn node_dist_for_current_platform(version: &str) -> anyhow::Result<NodeDist> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let (platform, kind) = match (os, arch) {
        ("macos", "aarch64") => ("darwin-arm64", ArchiveKind::TarGz),
        ("macos", "x86_64") => ("darwin-x64", ArchiveKind::TarGz),
        ("linux", "aarch64") => ("linux-arm64", ArchiveKind::TarXz),
        ("linux", "x86_64") => ("linux-x64", ArchiveKind::TarXz),
        ("windows", "aarch64") => ("win-arm64", ArchiveKind::Zip),
        ("windows", "x86_64") => ("win-x64", ArchiveKind::Zip),
        _ => anyhow::bail!("unsupported platform: os={os} arch={arch}"),
    };

    let filename = match kind {
        ArchiveKind::TarGz => format!("node-v{version}-{platform}.tar.gz"),
        ArchiveKind::TarXz => format!("node-v{version}-{platform}.tar.xz"),
        ArchiveKind::Zip => format!("node-v{version}-{platform}.zip"),
    };
    let base = format!("https://nodejs.org/dist/v{version}");
    Ok(NodeDist {
        url: format!("{base}/{filename}"),
        shasums_url: format!("{base}/SHASUMS256.txt"),
        dir_name: format!("node-v{version}-{platform}"),
        filename,
        kind,
    })
}

async fn download_text(client: &reqwest::Client, url: &str) -> anyhow::Result<String> {
    let res = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            format!("CliSwitch/{}", env!("CARGO_PKG_VERSION")),
        )
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .with_context(|| format!("download failed: {url}"))?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("download failed: {status} {body}");
    }
    Ok(res.text().await.unwrap_or_default())
}

fn expected_sha256(shasums_text: &str, filename: &str) -> Option<String> {
    for line in shasums_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: "<sha256>  <filename>"
        let mut parts = line.split_whitespace();
        let sha = parts.next()?;
        let file = parts.next()?;
        if file == filename {
            return Some(sha.to_string());
        }
    }
    None
}

async fn download_to_file_with_sha256(
    client: &reqwest::Client,
    url: &str,
    out: &Path,
    version: &str,
) -> anyhow::Result<String> {
    use tokio::io::AsyncWriteExt as _;

    if let Some(parent) = out.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create dir failed: {}", parent.display()))?;
    }

    let res = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            format!("CliSwitch/{}", env!("CARGO_PKG_VERSION")),
        )
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .with_context(|| format!("download failed: {url}"))?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("download failed: {status} {body}");
    }

    let total = res.content_length().filter(|t| *t > 0);
    publish_progress(
        "downloading_archive",
        Some(version),
        total.map(|_| 0),
        total,
        Some(0),
        None,
    );

    let tmp = out.with_extension("partial");
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .with_context(|| format!("create file failed: {}", tmp.display()))?;

    let mut hasher = sha2::Sha256::new();
    let mut stream = res.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_percent: Option<u8> = None;
    let mut last_reported_downloaded: u64 = 0;
    let mut last_reported_at = Instant::now();
    use futures_util::StreamExt as _;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| "read download chunk failed")?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        hasher.update(&chunk);
        file.write_all(&chunk).await?;

        if let Some(total) = total {
            let mut percent = ((downloaded.saturating_mul(100)) / total) as u8;
            if percent > 100 {
                percent = 100;
            }
            if last_percent != Some(percent) {
                last_percent = Some(percent);
                publish_progress(
                    "downloading_archive",
                    Some(version),
                    Some(percent),
                    Some(total),
                    Some(downloaded),
                    None,
                );
            }
        } else {
            // Best-effort progress when server doesn't provide Content-Length.
            let should_report = downloaded.saturating_sub(last_reported_downloaded) >= 1024 * 1024
                || last_reported_at.elapsed() >= std::time::Duration::from_millis(500);
            if should_report {
                last_reported_downloaded = downloaded;
                last_reported_at = Instant::now();
                publish_progress(
                    "downloading_archive",
                    Some(version),
                    None,
                    None,
                    Some(downloaded),
                    None,
                );
            }
        }
    }
    file.flush().await.ok();
    drop(file);

    if total.is_some() {
        publish_progress(
            "downloading_archive",
            Some(version),
            Some(100),
            total,
            Some(downloaded),
            None,
        );
    }

    tokio::fs::rename(&tmp, out)
        .await
        .with_context(|| format!("rename failed: {} -> {}", tmp.display(), out.display()))?;

    Ok(hex::encode(hasher.finalize()))
}

fn extract_archive(archive_path: &Path, kind: ArchiveKind, dest_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("create dir failed: {}", dest_dir.display()))?;

    match kind {
        ArchiveKind::TarGz => {
            let f = std::fs::File::open(archive_path)
                .with_context(|| format!("open archive failed: {}", archive_path.display()))?;
            let dec = flate2::read::GzDecoder::new(f);
            let mut ar = tar::Archive::new(dec);
            ar.unpack(dest_dir)
                .with_context(|| format!("extract tar.gz failed: {}", archive_path.display()))?;
        }
        ArchiveKind::TarXz => {
            let f = std::fs::File::open(archive_path)
                .with_context(|| format!("open archive failed: {}", archive_path.display()))?;
            let dec = xz2::read::XzDecoder::new(f);
            let mut ar = tar::Archive::new(dec);
            ar.unpack(dest_dir)
                .with_context(|| format!("extract tar.xz failed: {}", archive_path.display()))?;
        }
        ArchiveKind::Zip => {
            let f = std::fs::File::open(archive_path)
                .with_context(|| format!("open archive failed: {}", archive_path.display()))?;
            let mut z = zip::ZipArchive::new(f)
                .with_context(|| format!("read zip failed: {}", archive_path.display()))?;
            for i in 0..z.len() {
                let mut file = z.by_index(i)?;
                let outpath = match file.enclosed_name() {
                    Some(p) => dest_dir.join(p),
                    None => continue,
                };
                if file.is_dir() {
                    std::fs::create_dir_all(&outpath)?;
                    continue;
                }
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
    }
    Ok(())
}

fn managed_node_paths_from_root(root: &Path) -> anyhow::Result<ManagedNodePaths> {
    #[cfg(target_os = "windows")]
    {
        let node = root.join("node.exe");
        let npm = root.join("npm.cmd");
        if !node.is_file() {
            anyhow::bail!("managed node missing: {}", node.display());
        }
        if !npm.is_file() {
            anyhow::bail!("managed npm missing: {}", npm.display());
        }
        Ok(ManagedNodePaths {
            node_path: node,
            npm_path: npm,
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        let node = root.join("bin").join("node");
        let npm = root.join("bin").join("npm");
        if !node.is_file() {
            anyhow::bail!("managed node missing: {}", node.display());
        }
        if !npm.is_file() {
            anyhow::bail!("managed npm missing: {}", npm.display());
        }
        Ok(ManagedNodePaths {
            node_path: node,
            npm_path: npm,
        })
    }
}

#[cfg(target_os = "windows")]
const SYSTEM_NODE_BASENAME: &str = "node.exe";
#[cfg(target_os = "windows")]
const SYSTEM_NPM_BASENAME: &str = "npm.cmd";

#[cfg(not(target_os = "windows"))]
const SYSTEM_NODE_BASENAME: &str = "node";
#[cfg(not(target_os = "windows"))]
const SYSTEM_NPM_BASENAME: &str = "npm";

fn detect_node_npm_in_dirs(dirs: &[PathBuf]) -> Option<ManagedNodePaths> {
    for dir in dirs {
        let node = dir.join(SYSTEM_NODE_BASENAME);
        let npm = dir.join(SYSTEM_NPM_BASENAME);
        if !node.is_file() || !npm.is_file() {
            continue;
        }

        // Verify the binaries are actually executable.
        if cli_tools::try_get_cmd_version_at(&node).is_none()
            || cli_tools::try_get_cmd_version_at(&npm).is_none()
        {
            continue;
        }

        return Some(ManagedNodePaths {
            node_path: node,
            npm_path: npm,
        });
    }
    None
}

fn candidate_system_node_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(v) = std::env::var("ProgramFiles") {
            dirs.push(PathBuf::from(v).join("nodejs"));
        }
        if let Ok(v) = std::env::var("ProgramFiles(x86)") {
            dirs.push(PathBuf::from(v).join("nodejs"));
        }
        if let Ok(v) = std::env::var("LOCALAPPDATA") {
            let base = PathBuf::from(v);
            dirs.push(base.join("Programs").join("nodejs"));
            dirs.push(base.join("nodejs"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Common Homebrew prefixes (GUI apps often don't inherit shell PATH).
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }

    #[cfg(target_os = "linux")]
    {
        // Distro packages and common manual installs.
        dirs.push(PathBuf::from("/usr/local/bin"));
    }

    // Common fallback.
    dirs.push(PathBuf::from("/usr/bin"));
    dirs.push(PathBuf::from("/bin"));

    // Dedup, keep order.
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    dirs.retain(|p| seen.insert(p.clone()));
    dirs
}

fn detect_system_node_npm() -> Option<ManagedNodePaths> {
    detect_node_npm_in_dirs(&candidate_system_node_dirs())
}

fn run_command(program: &Path, args: &[&str]) -> anyhow::Result<std::process::Output> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    process::command_silent(&mut cmd);
    cmd.output()
        .with_context(|| format!("run command failed: {} {:?}", program.display(), args))
}

#[cfg(target_os = "windows")]
fn find_winget() -> Option<PathBuf> {
    cli_tools::find_executable_in_path("winget").or_else(|| {
        let local = std::env::var_os("LOCALAPPDATA")?;
        let p = PathBuf::from(local)
            .join("Microsoft")
            .join("WindowsApps")
            .join("winget.exe");
        p.is_file().then_some(p)
    })
}

#[cfg(target_os = "windows")]
fn try_install_node_with_winget() -> anyhow::Result<()> {
    let winget = find_winget().ok_or_else(|| anyhow::anyhow!("winget not found"))?;

    publish_progress(
        "system_install",
        None,
        None,
        None,
        None,
        Some("Installing Node.js via winget...".to_string()),
    );

    // Keep it non-interactive for a desktop app.
    let out = run_command(
        &winget,
        &[
            "install",
            "-e",
            "--id",
            "OpenJS.NodeJS",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
            "--silent",
        ],
    )?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        anyhow::bail!(
            "winget install failed: exit={:?} stdout={} stderr={}",
            out.status.code(),
            stdout,
            stderr
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn find_brew() -> Option<PathBuf> {
    cli_tools::find_executable_in_path("brew").or_else(|| {
        for p in ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"] {
            let pb = PathBuf::from(p);
            if pb.is_file() {
                return Some(pb);
            }
        }
        None
    })
}

#[cfg(target_os = "macos")]
fn try_install_node_with_brew() -> anyhow::Result<()> {
    let brew = find_brew().ok_or_else(|| anyhow::anyhow!("brew not found"))?;

    publish_progress(
        "system_install",
        None,
        None,
        None,
        None,
        Some("Installing Node.js via Homebrew...".to_string()),
    );

    let out = run_command(&brew, &["install", "node"])?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        anyhow::bail!(
            "brew install node failed: exit={:?} stdout={} stderr={}",
            out.status.code(),
            stdout,
            stderr
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn find_linux_pkg_manager() -> Option<(PathBuf, &'static str)> {
    // Prefer apt-get for scripting; otherwise try dnf/yum.
    const APT_GET_PATHS: [&str; 2] = ["/usr/bin/apt-get", "/bin/apt-get"];
    const DNF_PATHS: [&str; 2] = ["/usr/bin/dnf", "/bin/dnf"];
    const YUM_PATHS: [&str; 2] = ["/usr/bin/yum", "/bin/yum"];

    for (name, paths) in [
        ("apt-get", &APT_GET_PATHS[..]),
        ("dnf", &DNF_PATHS[..]),
        ("yum", &YUM_PATHS[..]),
    ] {
        if let Some(p) = cli_tools::find_executable_in_path(name) {
            return Some((p, name));
        }
        for raw in paths {
            let p = PathBuf::from(raw);
            if p.is_file() {
                return Some((p, name));
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn try_install_node_with_linux_pkg_manager() -> anyhow::Result<()> {
    let (pm, name) =
        find_linux_pkg_manager().ok_or_else(|| anyhow::anyhow!("no apt/dnf/yum found"))?;

    publish_progress(
        "system_install",
        None,
        None,
        None,
        None,
        Some(format!("Installing Node.js via {name}...")),
    );

    // Don't use sudo here (desktop app, no TTY). If permissions are missing, fall back.
    let out = run_command(&pm, &["install", "-y", "nodejs", "npm"])?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        anyhow::bail!(
            "{name} install failed: exit={:?} stdout={} stderr={}",
            out.status.code(),
            stdout,
            stderr
        );
    }
    Ok(())
}

async fn try_install_node_with_system_package_manager() -> anyhow::Result<()> {
    let join = tokio::task::spawn_blocking(|| -> anyhow::Result<()> {
        #[cfg(target_os = "windows")]
        {
            try_install_node_with_winget()
        }
        #[cfg(target_os = "macos")]
        {
            try_install_node_with_brew()
        }
        #[cfg(target_os = "linux")]
        {
            try_install_node_with_linux_pkg_manager()
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            anyhow::bail!("unsupported os for system package install");
        }
    })
    .await;

    match join {
        Ok(v) => v,
        Err(e) => Err(anyhow::anyhow!("system install task join failed: {e}")),
    }
}

pub async fn ensure_npm_env_installed(
    client: &reqwest::Client,
    data_dir: &Path,
) -> anyhow::Result<ManagedNodePaths> {
    publish_progress("start", None, None, None, None, None);

    // 1) If the system already has node/npm (but our GUI process can't see PATH), detect
    //    well-known installation locations and pin to absolute paths.
    publish_progress(
        "checking_system",
        None,
        None,
        None,
        None,
        Some("Checking system Node.js/npm locations...".to_string()),
    );
    if let Some(paths) = detect_system_node_npm() {
        publish_progress("done", None, Some(100), None, None, None);
        return Ok(paths);
    }

    // 2) Try system package manager (winget/brew/apt/yum...) if available.
    if let Err(e) = try_install_node_with_system_package_manager().await {
        publish_progress(
            "system_install_failed",
            None,
            None,
            None,
            None,
            Some(format!(
                "System install failed, fallback to bundled Node.js: {e}"
            )),
        );
    }

    // Re-detect after installation.
    if let Some(paths) = detect_system_node_npm() {
        publish_progress("done", None, Some(100), None, None, None);
        return Ok(paths);
    }

    // 3) Fallback: download official Node.js dist into app data dir (no admin required).
    ensure_managed_node_installed(client, data_dir).await
}

pub async fn ensure_managed_node_installed(
    client: &reqwest::Client,
    data_dir: &Path,
) -> anyhow::Result<ManagedNodePaths> {
    publish_progress("start", None, None, None, None, None);
    publish_progress("resolving_version", None, None, None, None, None);

    let version = match resolve_latest_lts_version(client).await {
        Ok(v) => v,
        Err(e) => {
            publish_progress("error", None, None, None, None, Some(e.to_string()));
            return Err(e);
        }
    };

    let dist = match node_dist_for_current_platform(&version) {
        Ok(v) => v,
        Err(e) => {
            publish_progress(
                "error",
                Some(&version),
                None,
                None,
                None,
                Some(e.to_string()),
            );
            return Err(e);
        }
    };

    let install_base = data_dir.join("nodejs");
    let install_root = install_base.join(&dist.dir_name);

    // If already installed, reuse it.
    if let Ok(paths) = managed_node_paths_from_root(&install_root) {
        publish_progress("done", Some(&version), Some(100), None, None, None);
        return Ok(paths);
    }

    let downloads = install_base.join("downloads");
    let archive_path = downloads.join(&dist.filename);

    // Verify checksum against official SHASUMS256.
    publish_progress(
        "downloading_shasums",
        Some(&version),
        None,
        None,
        None,
        None,
    );
    let shasums = match download_text(client, &dist.shasums_url).await {
        Ok(v) => v,
        Err(e) => {
            publish_progress(
                "error",
                Some(&version),
                None,
                None,
                None,
                Some(e.to_string()),
            );
            return Err(e);
        }
    };
    let expected = match expected_sha256(&shasums, &dist.filename) {
        Some(v) => v,
        None => {
            let msg = format!("sha256 not found in SHASUMS256.txt: {}", dist.filename);
            publish_progress("error", Some(&version), None, None, None, Some(msg.clone()));
            anyhow::bail!("{msg}");
        }
    };

    let actual =
        match download_to_file_with_sha256(client, &dist.url, &archive_path, &version).await {
            Ok(v) => v,
            Err(e) => {
                publish_progress(
                    "error",
                    Some(&version),
                    None,
                    None,
                    None,
                    Some(e.to_string()),
                );
                return Err(e);
            }
        };
    publish_progress("verifying_sha256", Some(&version), None, None, None, None);
    if !actual.eq_ignore_ascii_case(&expected) {
        let msg = format!(
            "sha256 mismatch for {}: expected={} actual={}",
            dist.filename, expected, actual
        );
        publish_progress("error", Some(&version), None, None, None, Some(msg.clone()));
        anyhow::bail!("{msg}");
    }

    // Extract (blocking) to avoid slowing down the async runtime.
    publish_progress("extracting", Some(&version), None, None, None, None);
    let kind = dist.kind;
    let archive_path2 = archive_path.clone();
    let install_base2 = install_base.clone();
    let join =
        tokio::task::spawn_blocking(move || extract_archive(&archive_path2, kind, &install_base2))
            .await;
    let join = match join {
        Ok(v) => v,
        Err(e) => {
            let err = anyhow::anyhow!("extract task join failed: {e}");
            publish_progress(
                "error",
                Some(&version),
                None,
                None,
                None,
                Some(err.to_string()),
            );
            return Err(err);
        }
    };
    if let Err(e) = join.context("extract node archive failed") {
        publish_progress(
            "error",
            Some(&version),
            None,
            None,
            None,
            Some(e.to_string()),
        );
        return Err(e);
    }

    let paths = match managed_node_paths_from_root(&install_root) {
        Ok(v) => v,
        Err(e) => {
            publish_progress(
                "error",
                Some(&version),
                None,
                None,
                None,
                Some(e.to_string()),
            );
            return Err(e);
        }
    };

    publish_progress("done", Some(&version), Some(100), None, None, None);
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn detect_node_npm_in_dirs_finds_executables() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir =
            std::env::temp_dir().join(format!("cliswitch-nodejs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let node = dir.join("node");
        let npm = dir.join("npm");

        std::fs::write(
            &node,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"v25.0.0\"; else echo \"ok\"; fi\n",
        )
        .unwrap();
        std::fs::write(
            &npm,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"10.0.0\"; else echo \"ok\"; fi\n",
        )
        .unwrap();

        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();

        let found =
            detect_node_npm_in_dirs(std::slice::from_ref(&dir)).expect("should detect node/npm");
        assert_eq!(found.node_path, node);
        assert_eq!(found.npm_path, npm);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
