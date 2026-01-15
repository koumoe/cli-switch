use anyhow::Context as _;
use serde::Deserialize;
use sha2::Digest as _;
use std::path::{Path, PathBuf};

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

    let tmp = out.with_extension("partial");
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .with_context(|| format!("create file failed: {}", tmp.display()))?;

    let mut hasher = sha2::Sha256::new();
    let mut stream = res.bytes_stream();
    use futures_util::StreamExt as _;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| "read download chunk failed")?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await.ok();
    drop(file);

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
        return Ok(ManagedNodePaths {
            node_path: node,
            npm_path: npm,
        });
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
        return Ok(ManagedNodePaths {
            node_path: node,
            npm_path: npm,
        });
    }
}

pub async fn ensure_managed_node_installed(
    client: &reqwest::Client,
    data_dir: &Path,
) -> anyhow::Result<ManagedNodePaths> {
    let version = resolve_latest_lts_version(client).await?;
    let dist = node_dist_for_current_platform(&version)?;

    let install_base = data_dir.join("nodejs");
    let install_root = install_base.join(&dist.dir_name);

    // If already installed, reuse it.
    if let Ok(paths) = managed_node_paths_from_root(&install_root) {
        return Ok(paths);
    }

    let downloads = install_base.join("downloads");
    let archive_path = downloads.join(&dist.filename);

    // Verify checksum against official SHASUMS256.
    let shasums = download_text(client, &dist.shasums_url).await?;
    let expected = expected_sha256(&shasums, &dist.filename)
        .with_context(|| format!("sha256 not found in SHASUMS256.txt: {}", dist.filename))?;

    let actual = download_to_file_with_sha256(client, &dist.url, &archive_path).await?;
    if !actual.eq_ignore_ascii_case(&expected) {
        anyhow::bail!(
            "sha256 mismatch for {}: expected={} actual={}",
            dist.filename,
            expected,
            actual
        );
    }

    // Extract (blocking) to avoid slowing down the async runtime.
    let kind = dist.kind;
    let archive_path2 = archive_path.clone();
    let install_base2 = install_base.clone();
    tokio::task::spawn_blocking(move || extract_archive(&archive_path2, kind, &install_base2))
        .await
        .context("extract task join failed")?
        .context("extract node archive failed")?;

    managed_node_paths_from_root(&install_root)
}
