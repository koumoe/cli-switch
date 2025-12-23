use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[cfg(not(target_os = "windows"))]
use std::ffi::{OsStr, OsString};

use crate::events::{self, AppEvent};

const GITHUB_OWNER: &str = "koumoe";
const GITHUB_REPO: &str = "cli-switch";

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Idle,
    Checking,
    Downloading,
    Staging,
    Ready,
    Error,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Idle => "idle",
            Stage::Checking => "checking",
            Stage::Downloading => "downloading",
            Stage::Staging => "staging",
            Stage::Ready => "ready",
            Stage::Error => "error",
        }
    }
}

#[derive(Debug, Default)]
pub struct UpdateRuntime {
    pub stage: Stage,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub downloading_version: Option<String>,
    pub download_percent: Option<u8>,
    pub download_total_bytes: Option<u64>,
    pub download_downloaded_bytes: u64,
    pub error: Option<String>,
}

impl UpdateRuntime {
    fn reset_download_state(&mut self) {
        self.downloading_version = None;
        self.download_percent = None;
        self.download_total_bytes = None;
        self.download_downloaded_bytes = 0;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub auto_update_enabled: bool,
    pub stage: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub pending_version: Option<String>,
    pub download_percent: Option<u8>,
    pub error: Option<String>,
}

fn snapshot_status(rt: &UpdateRuntime, pending_version: Option<String>) -> UpdateStatus {
    UpdateStatus {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        auto_update_enabled: false,
        stage: rt.stage.as_str().to_string(),
        latest_version: rt.latest_version.clone(),
        update_available: rt.update_available,
        pending_version,
        download_percent: if rt.stage == Stage::Downloading {
            rt.download_percent
        } else {
            None
        },
        error: rt.error.clone(),
    }
}

fn publish_status(rt: &UpdateRuntime, pending_version: Option<String>) {
    events::publish(AppEvent::UpdateStatus(snapshot_status(rt, pending_version)));
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpdate {
    pub version: String,
    pub staged_executable: PathBuf,
    pub downloaded_at_ms: i64,
    pub asset_name: String,
}

fn normalize_version_tag(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

fn current_platform_key() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "macos-arm64",
        ("macos", "x86_64") => "macos-x64",
        ("windows", "x86_64") => "windows-x64",
        ("windows", "aarch64") => "windows-arm64",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        _ => "unknown",
    }
}

fn updates_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("updates")
}

fn pending_path(data_dir: &Path) -> PathBuf {
    updates_dir(data_dir).join("pending.json")
}

fn update_downloads_dir(data_dir: &Path) -> PathBuf {
    updates_dir(data_dir).join("downloads")
}

fn update_staged_dir(data_dir: &Path) -> PathBuf {
    updates_dir(data_dir).join("staged")
}

fn collect_update_versions(data_dir: &Path) -> anyhow::Result<std::collections::HashMap<String, SystemTime>> {
    fn collect_from(
        dir: &Path,
        acc: &mut std::collections::HashMap<String, SystemTime>,
    ) -> anyhow::Result<()> {
        let entries = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e).with_context(|| format!("read dir failed: {}", dir.display())),
        };

        for entry in entries {
            let entry = entry.with_context(|| format!("read dir entry failed: {}", dir.display()))?;
            let ty = entry
                .file_type()
                .with_context(|| format!("read file type failed: {}", entry.path().display()))?;
            if !ty.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.is_empty() {
                continue;
            }
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            acc.entry(name)
                .and_modify(|t| {
                    if mtime > *t {
                        *t = mtime;
                    }
                })
                .or_insert(mtime);
        }
        Ok(())
    }

    let mut out = std::collections::HashMap::new();
    collect_from(&update_downloads_dir(data_dir), &mut out)?;
    collect_from(&update_staged_dir(data_dir), &mut out)?;
    Ok(out)
}

fn keep_two_versions(
    versions: &std::collections::HashMap<String, SystemTime>,
    primary: &str,
    preferred_secondary: &str,
) -> std::collections::HashSet<String> {
    let mut keep = std::collections::HashSet::new();
    keep.insert(primary.to_string());
    if versions.contains_key(preferred_secondary) && preferred_secondary != primary {
        keep.insert(preferred_secondary.to_string());
        return keep;
    }

    let mut candidates: Vec<(&String, &SystemTime)> = versions
        .iter()
        .filter(|(v, _)| v.as_str() != primary)
        .collect();
    candidates.sort_by(|(va, ta), (vb, tb)| tb.cmp(ta).then_with(|| vb.cmp(va)));
    if let Some((v, _)) = candidates.first() {
        keep.insert((*v).clone());
    }
    keep
}

fn cleanup_update_artifacts_with_keep(
    data_dir: &Path,
    keep: &std::collections::HashSet<String>,
) -> anyhow::Result<()> {
    fn cleanup_dir(root: &Path, keep: &std::collections::HashSet<String>) -> anyhow::Result<()> {
        let entries = match std::fs::read_dir(root) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e).with_context(|| format!("read dir failed: {}", root.display())),
        };

        for entry in entries {
            let entry = entry.with_context(|| format!("read dir entry failed: {}", root.display()))?;
            let path = entry.path();
            let ty = entry
                .file_type()
                .with_context(|| format!("read file type failed: {}", path.display()))?;
            if !ty.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if keep.contains(&name) {
                continue;
            }
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("remove dir failed: {}", path.display()))?;
        }
        Ok(())
    }

    cleanup_dir(&update_downloads_dir(data_dir), keep)?;
    cleanup_dir(&update_staged_dir(data_dir), keep)?;
    Ok(())
}

pub fn load_pending_update(data_dir: &Path) -> Option<PendingUpdate> {
    let path = pending_path(data_dir);
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<PendingUpdate>(&bytes).ok()
}

fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid path: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create dir failed: {}", parent.display()))?;

    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("write tmp failed: {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("rename tmp failed: {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

async fn github_latest_release(client: &reqwest::Client) -> anyhow::Result<GitHubRelease> {
    let url = format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest");
    let res = client
        .get(&url)
        .header(
            reqwest::header::USER_AGENT,
            format!("CliSwitch/{}", env!("CARGO_PKG_VERSION")),
        )
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .with_context(|| "request github latest release failed")?;

    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("github response not ok: {status} {body}");
    }

    res.json::<GitHubRelease>()
        .await
        .with_context(|| "parse github release json failed")
}

fn pick_asset(release: &GitHubRelease) -> Option<(&GitHubAsset, Option<&GitHubAsset>)> {
    let key = current_platform_key();
    if key == "unknown" {
        return None;
    }
    let key_lower = key.to_ascii_lowercase();

    let mut best: Option<&GitHubAsset> = None;
    for a in &release.assets {
        let name_lower = a.name.to_ascii_lowercase();
        if !name_lower.contains(&key_lower) {
            continue;
        }
        if name_lower.ends_with(".zip") || name_lower.ends_with(".tar.gz") {
            best = Some(a);
            break;
        }
    }

    let asset = best?;
    let sha256_name = format!("{}.sha256", asset.name);
    let sha256_asset = release.assets.iter().find(|a| a.name == sha256_name);
    Some((asset, sha256_asset))
}

pub async fn check_latest(
    client: &reqwest::Client,
    runtime: std::sync::Arc<tokio::sync::Mutex<UpdateRuntime>>,
    data_dir: &Path,
) -> UpdateCheck {
    {
        let rt = runtime.lock().await;
        if matches!(
            rt.stage,
            Stage::Checking | Stage::Downloading | Stage::Staging
        ) {
            tracing::debug!(stage = ?rt.stage, "update check skipped: runtime busy");
            return UpdateCheck {
                current_version: env!("CARGO_PKG_VERSION").to_string(),
                latest_version: rt.latest_version.clone(),
                update_available: rt.update_available,
            };
        }
    }

    {
        let mut rt = runtime.lock().await;
        rt.stage = Stage::Checking;
        rt.error = None;
        rt.reset_download_state();
    }

    let current = semver::Version::parse(env!("CARGO_PKG_VERSION")).ok();
    let mut latest_str: Option<String> = None;
    let mut available = false;
    let mut err: Option<String> = None;

    match github_latest_release(client).await {
        Ok(release) => {
            let tag = normalize_version_tag(&release.tag_name).to_string();
            latest_str = Some(tag.clone());
            match (current, semver::Version::parse(&tag)) {
                (Some(cur), Ok(lat)) => {
                    available = lat > cur;
                }
                _ => {
                    err = Some("版本号解析失败".to_string());
                }
            }
        }
        Err(e) => {
            err = Some(e.to_string());
        }
    }

    let pending = load_pending_update(data_dir);
    let pending_version = pending.as_ref().map(|p| p.version.clone());
    {
        let mut rt = runtime.lock().await;
        rt.latest_version = latest_str.clone();
        rt.update_available = available;
        rt.error = err.clone();
        if err.is_some() {
            rt.stage = Stage::Error;
        } else if pending.is_some() {
            rt.stage = Stage::Ready;
        } else {
            rt.stage = Stage::Idle;
        }
        rt.reset_download_state();
        publish_status(&rt, pending_version);
    }

    UpdateCheck {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        latest_version: latest_str,
        update_available: available,
    }
}

pub async fn get_status(
    runtime: std::sync::Arc<tokio::sync::Mutex<UpdateRuntime>>,
    data_dir: &Path,
    auto_update_enabled: bool,
) -> UpdateStatus {
    let pending = load_pending_update(data_dir);
    let pending_version = pending.as_ref().map(|p| p.version.clone());
    let mut rt = runtime.lock().await;
    if pending.is_some() && rt.stage != Stage::Downloading {
        rt.stage = Stage::Ready;
    }

    UpdateStatus {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        auto_update_enabled,
        stage: rt.stage.as_str().to_string(),
        latest_version: rt.latest_version.clone(),
        update_available: rt.update_available,
        pending_version,
        download_percent: if rt.stage == Stage::Downloading {
            rt.download_percent
        } else {
            None
        },
        error: rt.error.clone(),
    }
}

pub async fn spawn_download_latest(
    client: reqwest::Client,
    runtime: std::sync::Arc<tokio::sync::Mutex<UpdateRuntime>>,
    data_dir: PathBuf,
) -> bool {
    {
        let mut rt = runtime.lock().await;
        if matches!(
            rt.stage,
            Stage::Checking | Stage::Downloading | Stage::Staging
        ) {
            tracing::debug!(stage = ?rt.stage, "update download skipped: runtime busy");
            return false;
        }
        rt.stage = Stage::Checking;
        rt.error = None;
        rt.reset_download_state();
    }

    let release = match github_latest_release(&client).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(err = %e, "update download check failed: github latest release");
            let pending_version = load_pending_update(&data_dir).map(|p| p.version);
            let mut rt = runtime.lock().await;
            rt.stage = Stage::Error;
            rt.error = Some(e.to_string());
            rt.reset_download_state();
            publish_status(&rt, pending_version);
            return false;
        }
    };

    let latest = normalize_version_tag(&release.tag_name).to_string();
    let (available, version_err) = match (
        semver::Version::parse(env!("CARGO_PKG_VERSION")),
        semver::Version::parse(&latest),
    ) {
        (Ok(cur), Ok(lat)) => (lat > cur, None),
        _ => (false, Some("版本号解析失败".to_string())),
    };

    if let Some(err) = version_err {
        tracing::warn!(err = %err, latest = %latest, "update download check failed: version parse");
        let mut rt = runtime.lock().await;
        rt.latest_version = Some(latest);
        rt.update_available = false;
        rt.stage = Stage::Error;
        rt.error = Some(err);
        rt.reset_download_state();
        publish_status(&rt, load_pending_update(&data_dir).map(|p| p.version));
        return false;
    }

    let pending = load_pending_update(&data_dir);
    if pending.as_ref().is_some_and(|p| p.version == latest) {
        tracing::debug!(latest = %latest, "update download skipped: already pending");
        let mut rt = runtime.lock().await;
        rt.latest_version = Some(latest);
        rt.update_available = available;
        rt.stage = Stage::Ready;
        rt.error = None;
        rt.reset_download_state();
        publish_status(&rt, pending.map(|p| p.version));
        return false;
    }

    if !available {
        tracing::debug!(latest = %latest, "update download skipped: no update available");
        let mut rt = runtime.lock().await;
        rt.latest_version = Some(latest);
        rt.update_available = false;
        rt.stage = Stage::Idle;
        rt.error = None;
        rt.reset_download_state();
        publish_status(&rt, load_pending_update(&data_dir).map(|p| p.version));
        return false;
    }

    if pick_asset(&release).is_none() {
        tracing::warn!(latest = %latest, platform = %current_platform_key(), "update download skipped: no matching asset");
        let mut rt = runtime.lock().await;
        rt.latest_version = Some(latest);
        rt.update_available = true;
        rt.stage = Stage::Error;
        rt.error = Some("未找到适配当前平台的 Release 资源".to_string());
        rt.reset_download_state();
        publish_status(&rt, load_pending_update(&data_dir).map(|p| p.version));
        return false;
    }

    {
        tracing::info!(latest = %latest, "update download started");
        let mut rt = runtime.lock().await;
        rt.latest_version = Some(latest);
        rt.update_available = true;
        rt.stage = Stage::Downloading;
        rt.error = None;
        rt.download_percent = Some(0);
        rt.download_total_bytes = None;
        rt.download_downloaded_bytes = 0;
        publish_status(&rt, None);
    }

    tokio::spawn(async move {
        let res = download_latest_inner(&client, &data_dir, runtime.clone()).await;
        if let Err(e) = res {
            tracing::warn!(err = %e, "update download failed");
            let pending_version = load_pending_update(&data_dir).map(|p| p.version);
            let mut rt = runtime.lock().await;
            rt.stage = Stage::Error;
            rt.error = Some(e.to_string());
            rt.reset_download_state();
            publish_status(&rt, pending_version);
        }
    });

    true
}

async fn download_text(client: &reqwest::Client, url: &str) -> anyhow::Result<String> {
    let res = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            format!("CliSwitch/{}", env!("CARGO_PKG_VERSION")),
        )
        .timeout(std::time::Duration::from_secs(20))
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

async fn download_to_file_with_sha256(
    client: &reqwest::Client,
    url: &str,
    out: &Path,
    runtime: std::sync::Arc<tokio::sync::Mutex<UpdateRuntime>>,
) -> anyhow::Result<String> {
    use tokio::io::AsyncWriteExt as _;

    let res = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            format!("CliSwitch/{}", env!("CARGO_PKG_VERSION")),
        )
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .with_context(|| format!("download failed: {url}"))?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("download failed: {status} {body}");
    }

    let total = res.content_length().filter(|t| *t > 0);
    {
        let mut rt = runtime.lock().await;
        rt.download_total_bytes = total;
        rt.download_downloaded_bytes = 0;
        rt.download_percent = total.map(|_| 0);
    }

    if let Some(parent) = out.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create dir failed: {}", parent.display()))?;
    }

    let mut file = tokio::fs::File::create(out)
        .await
        .with_context(|| format!("create file failed: {}", out.display()))?;

    let mut hasher = sha2::Sha256::new();
    let mut stream = res.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_percent: Option<u8> = None;
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
                let status = {
                    let mut rt = runtime.lock().await;
                    rt.download_downloaded_bytes = downloaded;
                    rt.download_percent = Some(percent);
                    snapshot_status(&rt, None)
                };
                events::publish(AppEvent::UpdateStatus(status));
            }
        }
    }
    file.flush().await.ok();

    if total.is_some() {
        let status = {
            let mut rt = runtime.lock().await;
            rt.download_downloaded_bytes = downloaded;
            rt.download_percent = Some(100);
            snapshot_status(&rt, None)
        };
        events::publish(AppEvent::UpdateStatus(status));
    }

    Ok(hex::encode(hasher.finalize()))
}

fn extract_executable(
    archive_path: &Path,
    asset_name: &str,
    version: &str,
    data_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let staged_dir = updates_dir(data_dir).join("staged").join(version);
    std::fs::create_dir_all(&staged_dir)
        .with_context(|| format!("create staged dir failed: {}", staged_dir.display()))?;

    let exe_name = if cfg!(target_os = "windows") {
        "CliSwitch.exe"
    } else {
        "cliswitch"
    };
    let staged_exe = staged_dir.join(exe_name);

    if asset_name.to_ascii_lowercase().ends_with(".zip") {
        let f = std::fs::File::open(archive_path)
            .with_context(|| format!("open zip failed: {}", archive_path.display()))?;
        let mut zip = zip::ZipArchive::new(f).context("open zip archive failed")?;

        let want_suffix = if cfg!(target_os = "macos") {
            ".app/Contents/MacOS/cliswitch"
        } else if cfg!(target_os = "windows") {
            "/CliSwitch.exe"
        } else {
            "/cliswitch"
        };

        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let name = file.name().to_string();
            if !name.ends_with(want_suffix) {
                continue;
            }

            let mut out = std::fs::File::create(&staged_exe)?;
            std::io::copy(&mut file, &mut out)?;
            set_executable_perm(&staged_exe)?;
            return Ok(staged_exe);
        }

        anyhow::bail!("zip 内未找到可执行文件：{want_suffix}");
    }

    if asset_name.to_ascii_lowercase().ends_with(".tar.gz") {
        let f = std::fs::File::open(archive_path)
            .with_context(|| format!("open tar.gz failed: {}", archive_path.display()))?;
        let gz = flate2::read::GzDecoder::new(f);
        let mut ar = tar::Archive::new(gz);

        for entry in ar.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();
            if !path_str.ends_with("/cliswitch") && path_str != "cliswitch" {
                continue;
            }
            entry.unpack(&staged_exe)?;
            set_executable_perm(&staged_exe)?;
            return Ok(staged_exe);
        }

        anyhow::bail!("tar.gz 内未找到可执行文件：cliswitch");
    }

    anyhow::bail!("不支持的更新包格式：{asset_name}");
}

#[cfg(unix)]
fn set_executable_perm(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let perm = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(path, perm)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable_perm(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

async fn download_latest_inner(
    client: &reqwest::Client,
    data_dir: &Path,
    runtime: std::sync::Arc<tokio::sync::Mutex<UpdateRuntime>>,
) -> anyhow::Result<()> {
    let release = github_latest_release(client).await?;
    let latest = normalize_version_tag(&release.tag_name).to_string();

    {
        let mut rt = runtime.lock().await;
        rt.latest_version = Some(latest.clone());
        rt.downloading_version = Some(latest.clone());
    }

    let pending = load_pending_update(data_dir);
    if pending.as_ref().is_some_and(|p| p.version == latest) {
        let mut rt = runtime.lock().await;
        rt.update_available = true;
        rt.stage = Stage::Ready;
        rt.reset_download_state();
        rt.error = None;
        return Ok(());
    }

    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
    let latest_v = semver::Version::parse(&latest)?;
    let available = latest_v > current;

    {
        let mut rt = runtime.lock().await;
        rt.update_available = available;
    }

    if !available {
        let mut rt = runtime.lock().await;
        rt.stage = Stage::Idle;
        rt.reset_download_state();
        rt.error = None;
        return Ok(());
    }

    let (asset, sha256_asset) =
        pick_asset(&release).context("未找到适配当前平台的 Release 资源")?;

    let archive_path = updates_dir(data_dir)
        .join("downloads")
        .join(&latest)
        .join(&asset.name);

    let expected_sha256 = if let Some(sha) = sha256_asset {
        let text = download_text(client, &sha.browser_download_url).await?;
        text.split_whitespace()
            .next()
            .map(|s| s.trim().to_ascii_lowercase())
    } else {
        None
    };

    let actual_sha256 = download_to_file_with_sha256(
        client,
        &asset.browser_download_url,
        &archive_path,
        runtime.clone(),
    )
    .await?;
    if let Some(expected) = expected_sha256
        && !expected.is_empty()
        && expected != actual_sha256
    {
        tracing::error!(expected = %expected, actual = %actual_sha256, "update sha256 checksum mismatch");
        anyhow::bail!("sha256 校验失败：expected={expected} actual={actual_sha256}");
    }

    {
        let mut rt = runtime.lock().await;
        rt.stage = Stage::Staging;
        publish_status(&rt, load_pending_update(data_dir).map(|p| p.version));
    }

    let staged_exe = tokio::task::spawn_blocking({
        let archive_path = archive_path.clone();
        let asset_name = asset.name.clone();
        let latest = latest.clone();
        let data_dir = data_dir.to_path_buf();
        move || extract_executable(&archive_path, &asset_name, &latest, &data_dir)
    })
    .await
    .context("等待解压任务失败")??;

    let pending = PendingUpdate {
        version: latest.clone(),
        staged_executable: staged_exe.clone(),
        downloaded_at_ms: crate::storage::now_ms(),
        asset_name: asset.name.clone(),
    };
    let json = serde_json::to_vec_pretty(&pending)?;
    atomic_write(&pending_path(data_dir), &json)?;
    tracing::info!(version = %latest, staged = %staged_exe.display(), "update download completed");

    match collect_update_versions(data_dir) {
        Ok(versions) => {
            let keep = keep_two_versions(&versions, &latest, env!("CARGO_PKG_VERSION"));
            if let Err(e) = cleanup_update_artifacts_with_keep(data_dir, &keep) {
                tracing::warn!(err = %e, keep = ?keep, "update artifacts cleanup failed");
            }
        }
        Err(e) => {
            tracing::warn!(err = %e, "collect update versions failed, cleanup skipped");
        }
    }

    let mut rt = runtime.lock().await;
    rt.stage = Stage::Ready;
    rt.reset_download_state();
    rt.error = None;
    publish_status(&rt, Some(latest.clone()));
    Ok(())
}

#[cfg(target_os = "windows")]
const WINDOWS_APPLY_SCRIPT_TEMPLATE: &str = "@echo off\r\n\
setlocal\r\n\
set \"SRC={src}\"\r\n\
set \"DST={dst}\"\r\n\
set \"PENDING={pending}\"\r\n\
set \"STAGED={staged}\"\r\n\
:retry\r\n\
ping -n 2 127.0.0.1 >nul\r\n\
copy /Y \"%SRC%\" \"%DST%\" >nul\r\n\
if errorlevel 1 goto retry\r\n\
del /F /Q \"%PENDING%\" >nul 2>nul\r\n\
del /F /Q \"%STAGED%\" >nul 2>nul\r\n\
{restart}\
endlocal\r\n";

pub fn apply_pending_on_exit(data_dir: &Path) -> anyhow::Result<bool> {
    apply_pending_on_exit_inner(data_dir, false)
}

pub fn apply_pending_on_exit_and_restart(data_dir: &Path) -> anyhow::Result<bool> {
    apply_pending_on_exit_inner(data_dir, true)
}

fn apply_pending_on_exit_inner(data_dir: &Path, restart: bool) -> anyhow::Result<bool> {
    let pending = match load_pending_update(data_dir) {
        Some(p) => p,
        None => return Ok(false),
    };

    if !pending.staged_executable.is_file() {
        tracing::error!(path = %pending.staged_executable.display(), "staged update file not found");
        anyhow::bail!(
            "已下载的更新文件不存在：{}",
            pending.staged_executable.display()
        );
    }

    tracing::info!(
        version = %pending.version,
        restart,
        staged = %pending.staged_executable.display(),
        "applying pending update on exit"
    );

    match collect_update_versions(data_dir) {
        Ok(versions) => {
            let keep = keep_two_versions(&versions, &pending.version, env!("CARGO_PKG_VERSION"));
            if let Err(e) = cleanup_update_artifacts_with_keep(data_dir, &keep) {
                tracing::warn!(err = %e, keep = ?keep, "update artifacts cleanup failed");
            }
        }
        Err(e) => {
            tracing::warn!(err = %e, "collect update versions failed, cleanup skipped");
        }
    }

    let target = std::env::current_exe().context("读取当前可执行文件路径失败")?;

    #[cfg(target_os = "windows")]
    {
        let script = updates_dir(data_dir)
            .join("apply")
            .join(format!("apply-{}.cmd", pending.version));
        if let Some(parent) = script.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir failed: {}", parent.display()))?;
        }

        let escape_for_set = |s: &str| s.replace('"', "^\"");
        let restart_snippet = if restart {
            "start \"\" \"%DST%\"\r\n"
        } else {
            ""
        };
        let script_body = WINDOWS_APPLY_SCRIPT_TEMPLATE
            .replace(
                "{src}",
                &escape_for_set(&pending.staged_executable.display().to_string()),
            )
            .replace("{dst}", &escape_for_set(&target.display().to_string()))
            .replace(
                "{pending}",
                &escape_for_set(&pending_path(data_dir).display().to_string()),
            )
            .replace(
                "{staged}",
                &escape_for_set(&pending.staged_executable.display().to_string()),
            )
            .replace("{restart}", restart_snippet);
        std::fs::write(&script, script_body.as_bytes())
            .with_context(|| format!("write apply script failed: {}", script.display()))?;

        std::process::Command::new("cmd")
            .arg("/C")
            .arg(script.as_os_str())
            .spawn()
            .with_context(|| "spawn windows apply script failed")?;

        return Ok(true);
    }

    #[cfg(not(target_os = "windows"))]
    {
        spawn_apply_helper_after_exit(data_dir, &target, &pending, restart)?;
        Ok(true)
    }
}

#[cfg(not(target_os = "windows"))]
fn spawn_apply_helper_after_exit(
    data_dir: &Path,
    target: &Path,
    pending: &PendingUpdate,
    restart: bool,
) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let parent_pid = std::process::id().to_string();
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let now = crate::storage::now_ms();

    let file_name = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("cliswitch");
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid exe path: {}", target.display()))?;
    let backup = parent.join(format!("{file_name}.bak.{now}"));
    let temp = parent.join(format!("{file_name}.new.{now}"));

    let app = {
        #[cfg(target_os = "macos")]
        {
            target
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .filter(|p| p.extension().is_some_and(|ext| ext == OsStr::new("app")))
                .map(|p| p.to_path_buf())
        }
        #[cfg(not(target_os = "macos"))]
        {
            None::<PathBuf>
        }
    };

    let script = updates_dir(data_dir)
        .join("apply")
        .join(format!("apply-cliswitch.{}.{}.sh", pending.version, now));
    let log = updates_dir(data_dir)
        .join("apply")
        .join(format!("apply-cliswitch.{}.{}.log", pending.version, now));

    let pending_json = pending_path(data_dir);
    let staged = pending.staged_executable.clone();
    let restart_flag = if restart { "1" } else { "0" };

    let script_body = r#"#!/bin/sh
set -u

TARGET="$1"
APP="$2"
PID="$3"
PENDING="$4"
STAGED="$5"
BACKUP="$6"
TEMP="$7"
RESTART="$8"
LOG="$9"
shift 9

mkdir -p "$(dirname "$LOG")" >/dev/null 2>&1 || true
exec >>"$LOG" 2>&1

echo "[apply] started at $(date)"
echo "[apply] target=$TARGET"
echo "[apply] app=$APP"
echo "[apply] pid=$PID"
echo "[apply] pending=$PENDING"
echo "[apply] staged=$STAGED"
echo "[apply] backup=$BACKUP"
echo "[apply] temp=$TEMP"
echo "[apply] restart=$RESTART"

while kill -0 "$PID" 2>/dev/null; do
  sleep 1
done

echo "[apply] parent exited at $(date)"

attempt=0
while :; do
  attempt=$((attempt+1))

  rm -f "$TEMP" >/dev/null 2>&1 || true

  if ! cp -f "$STAGED" "$TEMP"; then
    echo "[apply] copy staged failed (attempt=$attempt)"
  else
    chmod 755 "$TEMP" >/dev/null 2>&1 || true

    if ! mv -f "$TARGET" "$BACKUP"; then
      echo "[apply] backup current exe failed (attempt=$attempt)"
    else
      if mv -f "$TEMP" "$TARGET"; then
        break
      fi
      echo "[apply] replace current exe failed (attempt=$attempt)"
      mv -f "$BACKUP" "$TARGET" >/dev/null 2>&1 || true
    fi
  fi

  if [ $attempt -ge 60 ]; then
    echo "[apply] apply failed after $attempt attempts"
    exit 1
  fi
  sleep 1
done

echo "[apply] replace ok at $(date)"

if [ "$APP" != "-" ] && [ -n "$APP" ]; then
  if command -v codesign >/dev/null 2>&1; then
    echo "[apply] codesign started at $(date)"
    start=$(date +%s)
    if ! codesign --force --sign - "$APP"; then
      echo "[apply] codesign (no --deep) failed, retry with --deep"
      codesign --force --deep --sign - "$APP" || true
    fi
    end=$(date +%s)
    echo "[apply] codesign finished in $((end-start))s at $(date)"
  else
    echo "[apply] codesign not found, skipped"
  fi
fi

rm -f "$PENDING" >/dev/null 2>&1 || true
rm -f "$STAGED" >/dev/null 2>&1 || true
rm -f "$0" >/dev/null 2>&1 || true

echo "[apply] cleanup done at $(date)"

if [ "$RESTART" = "1" ]; then
  echo "[apply] restarting at $(date)"
  if [ "$APP" != "-" ] && [ -n "$APP" ]; then
    exec open -n "$APP" --args "$@"
  else
    exec "$TARGET" "$@"
  fi
fi

echo "[apply] exit at $(date)"
exit 0
"#;

    if let Some(parent) = script.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir failed: {}", parent.display()))?;
    }
    std::fs::write(&script, script_body.as_bytes())
        .with_context(|| format!("write apply helper failed: {}", script.display()))?;
    set_executable_perm(&script)?;

    let mut cmd = std::process::Command::new("sh");
    cmd.arg(script.as_os_str())
        .arg(target.as_os_str())
        .arg(
            app.as_ref()
                .map(|p| p.as_os_str())
                .unwrap_or(OsStr::new("-")),
        )
        .arg(parent_pid)
        .arg(pending_json.as_os_str())
        .arg(staged.as_os_str())
        .arg(backup.as_os_str())
        .arg(temp.as_os_str())
        .arg(restart_flag)
        .arg(log.as_os_str());
    cmd.args(args);
    cmd.spawn().with_context(|| "spawn apply helper failed")?;
    tracing::info!(
        script = %script.display(),
        log = %log.display(),
        restart,
        "spawned update apply helper"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_two_versions_prefers_secondary_when_present() {
        let mut versions = std::collections::HashMap::new();
        versions.insert(
            "1.0.0".to_string(),
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
        );
        versions.insert(
            "1.1.0".to_string(),
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2),
        );

        let keep = keep_two_versions(&versions, "1.1.0", "1.0.0");
        assert_eq!(keep.len(), 2);
        assert!(keep.contains("1.1.0"));
        assert!(keep.contains("1.0.0"));
    }

    #[test]
    fn keep_two_versions_falls_back_to_most_recent() {
        let mut versions = std::collections::HashMap::new();
        versions.insert(
            "0.9.0".to_string(),
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
        );
        versions.insert(
            "1.0.0".to_string(),
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2),
        );
        versions.insert(
            "1.1.0".to_string(),
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3),
        );

        let keep = keep_two_versions(&versions, "1.1.0", "2.0.0");
        assert_eq!(keep.len(), 2);
        assert!(keep.contains("1.1.0"));
        assert!(keep.contains("1.0.0"));
        assert!(!keep.contains("0.9.0"));
    }

    #[test]
    fn cleanup_update_artifacts_removes_old_versions() {
        let data_dir = std::env::temp_dir()
            .join(format!("cliswitch-update-test-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&data_dir);

        let versions = ["0.9.0", "1.0.0", "1.1.0"];
        for v in versions {
            std::fs::create_dir_all(update_downloads_dir(&data_dir).join(v)).unwrap();
            std::fs::create_dir_all(update_staged_dir(&data_dir).join(v)).unwrap();
        }
        std::fs::write(update_downloads_dir(&data_dir).join("keep.txt"), b"ok").unwrap();

        let keep: std::collections::HashSet<String> =
            ["1.1.0".to_string(), "1.0.0".to_string()].into_iter().collect();
        cleanup_update_artifacts_with_keep(&data_dir, &keep).unwrap();

        assert!(update_downloads_dir(&data_dir).join("1.1.0").is_dir());
        assert!(update_downloads_dir(&data_dir).join("1.0.0").is_dir());
        assert!(!update_downloads_dir(&data_dir).join("0.9.0").exists());
        assert!(update_downloads_dir(&data_dir).join("keep.txt").is_file());

        assert!(update_staged_dir(&data_dir).join("1.1.0").is_dir());
        assert!(update_staged_dir(&data_dir).join("1.0.0").is_dir());
        assert!(!update_staged_dir(&data_dir).join("0.9.0").exists());

        std::fs::remove_dir_all(&data_dir).unwrap();
    }
}
