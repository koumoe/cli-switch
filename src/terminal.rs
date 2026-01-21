use anyhow::Context as _;
use std::path::{Path, PathBuf};

const BEGIN_MARKER: &str = "# >>> cliswitch cli-tools shims >>>";
const END_MARKER: &str = "# <<< cliswitch cli-tools shims <<<";

#[derive(Debug, Clone)]
pub struct EnsureCliToolShimResult {
    pub shim_dir: PathBuf,
    pub shim_path: PathBuf,
    pub touched_files: Vec<PathBuf>,
}

fn home_dir() -> anyhow::Result<PathBuf> {
    let ud = directories::UserDirs::new().context("无法定位用户目录（UserDirs）")?;
    Ok(ud.home_dir().to_path_buf())
}

pub fn cli_tools_shim_dir() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?.join(".cliswitch").join("bin"))
}

pub fn cli_tool_shim_path(tool_bin: &str) -> anyhow::Result<PathBuf> {
    let shim_dir = cli_tools_shim_dir()?;
    Ok(tool_shim_path(&shim_dir, tool_bin))
}

pub fn remove_cli_tool_shim(tool_bin: &str) -> anyhow::Result<bool> {
    let shim_dir = match cli_tools_shim_dir() {
        Ok(p) => p,
        Err(_) => return Ok(false),
    };
    if !shim_dir.is_dir() {
        return Ok(false);
    }
    let p = tool_shim_path(&shim_dir, tool_bin);
    if !p.is_file() {
        return Ok(false);
    }
    std::fs::remove_file(&p).with_context(|| format!("remove shim failed: {}", p.display()))?;
    Ok(true)
}

fn sh_single_quote(s: &str) -> String {
    // POSIX-safe single-quote escaping: ' -> '\''.
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

fn read_text_or_empty_if_missing(path: &Path) -> anyhow::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).with_context(|| format!("read failed: {}", path.display())),
    }
}

fn write_if_changed(path: &Path, content: &str) -> anyhow::Result<bool> {
    let prev = read_text_or_empty_if_missing(path)?;
    if prev == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir failed: {}", parent.display()))?;
    }
    std::fs::write(path, content).with_context(|| format!("write failed: {}", path.display()))?;
    Ok(true)
}

fn upsert_marked_block(path: &Path, block_lines: &[String]) -> anyhow::Result<bool> {
    let prev = read_text_or_empty_if_missing(path)?;
    let lines: Vec<&str> = prev.lines().collect();
    let mut out: Vec<String> = Vec::new();

    let mut in_block = false;
    let mut found_block = false;

    for line in lines {
        if line == BEGIN_MARKER {
            // Replace (and de-dup) any existing block.
            in_block = true;
            found_block = true;
            out.extend(block_lines.iter().cloned());
            continue;
        }
        if in_block {
            if line == END_MARKER {
                in_block = false;
            }
            continue;
        }
        out.push(line.to_string());
    }

    if !found_block {
        if !out.is_empty() && !out.last().map(|s| s.is_empty()).unwrap_or(true) {
            out.push(String::new());
        }
        out.extend(block_lines.iter().cloned());
    }

    let mut next = out.join("\n");
    next.push('\n');

    write_if_changed(path, &next)
}

#[cfg(all(not(windows), target_os = "macos"))]
fn pick_shell_rc_file(home: &Path) -> PathBuf {
    // macOS default shell is zsh; GUI apps may not carry a reliable $SHELL.
    let shell = std::env::var("SHELL").ok();
    let shell_name = shell
        .as_deref()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|s| s.to_str());

    match shell_name {
        Some("bash") => home.join(".bashrc"),
        _ => home.join(".zshrc"),
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn pick_shell_rc_file(home: &Path) -> PathBuf {
    let shell = std::env::var("SHELL").ok();
    let shell_name = shell
        .as_deref()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|s| s.to_str());

    match shell_name {
        Some("zsh") => home.join(".zshrc"),
        Some("bash") => home.join(".bashrc"),
        _ => {
            let bashrc = home.join(".bashrc");
            if bashrc.exists() {
                return bashrc;
            }
            let zshrc = home.join(".zshrc");
            if zshrc.exists() {
                return zshrc;
            }
            home.join(".profile")
        }
    }
}

#[cfg(not(windows))]
fn ensure_unix_path_has_shim_dir(shim_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let home = home_dir()?;
    let rc = pick_shell_rc_file(&home);

    let export_line = if let Ok(rel) = shim_dir.strip_prefix(&home) {
        let rel = rel.to_string_lossy();
        format!("export PATH=\"$HOME/{rel}:$PATH\"")
    } else {
        let abs = shim_dir.to_string_lossy();
        format!("export PATH=\"{abs}:$PATH\"")
    };

    let block_lines = vec![
        BEGIN_MARKER.to_string(),
        export_line,
        END_MARKER.to_string(),
    ];

    let changed = upsert_marked_block(&rc, &block_lines)
        .with_context(|| format!("update shell rc failed: {}", rc.display()))?;
    Ok(if changed { vec![rc] } else { vec![] })
}

#[cfg(windows)]
fn ensure_windows_path_has_shim_dir(shim_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    // Best-effort: persist to the current user's PATH.
    use winreg::RegKey;
    use winreg::RegValue;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_SZ, RegType};

    let shim = shim_dir.to_str().ok_or_else(|| {
        anyhow::anyhow!("shim dir contains invalid utf-8: {}", shim_dir.display())
    })?;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .context("open HKCU\\\\Environment failed")?;

    fn reg_value_to_string(v: &RegValue) -> String {
        if v.bytes.len() < 2 {
            return String::new();
        }
        let mut u16s: Vec<u16> = v
            .bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        while matches!(u16s.last(), Some(0)) {
            u16s.pop();
        }
        String::from_utf16_lossy(&u16s)
    }

    fn string_to_reg_value(s: &str, vtype: RegType) -> RegValue {
        let mut bytes: Vec<u8> = Vec::with_capacity((s.len() + 1) * 2);
        for u in s.encode_utf16().chain(std::iter::once(0)) {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        RegValue { vtype, bytes }
    }

    // Preserve the original value type to avoid breaking %VAR% entries in PATH.
    let (current, vtype) = match env.get_raw_value("Path") {
        Ok(raw) if raw.vtype == REG_SZ || raw.vtype == REG_EXPAND_SZ => {
            (reg_value_to_string(&raw), raw.vtype)
        }
        _ => (String::new(), REG_EXPAND_SZ),
    };
    let mut parts: Vec<String> = current
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let contains = parts.iter().any(|p| p.eq_ignore_ascii_case(shim));
    if contains {
        return Ok(vec![]);
    }
    parts.push(shim.to_string());
    let next = parts.join(";");

    env.set_raw_value("Path", &string_to_reg_value(&next, vtype))
        .context("write HKCU\\\\Environment\\\\Path failed")?;
    crate::process::notify_env_changed();

    Ok(vec![])
}

fn tool_shim_path(shim_dir: &Path, tool_bin: &str) -> PathBuf {
    #[cfg(windows)]
    {
        return shim_dir.join(format!("{tool_bin}.cmd"));
    }
    #[cfg(not(windows))]
    {
        shim_dir.join(tool_bin)
    }
}

fn write_tool_shim(
    shim_path: &Path,
    tool_path: &Path,
    prepend_dirs: &[PathBuf],
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let tool = tool_path.to_str().ok_or_else(|| {
            anyhow::anyhow!("tool path contains invalid utf-8: {}", tool_path.display())
        })?;

        let mut dirs: Vec<String> = Vec::new();
        for d in prepend_dirs {
            if let Some(s) = d.to_str() {
                dirs.push(s.to_string());
            }
        }
        // Dedup (case-insensitive) for Windows paths.
        let mut dedup: Vec<String> = Vec::new();
        for d in dirs {
            if !dedup.iter().any(|p| p.eq_ignore_ascii_case(&d)) {
                dedup.push(d);
            }
        }

        let prefix = if dedup.is_empty() {
            String::new()
        } else {
            format!("set \"PATH={};%PATH%\"\r\n", dedup.join(";"))
        };

        let content = format!(
            "@echo off\r\n\
             setlocal\r\n\
             {prefix}\
             call \"{tool}\" %*\r\n"
        );
        let _ = write_if_changed(shim_path, &content)?;
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        let mut dirs: Vec<String> = Vec::new();
        for d in prepend_dirs {
            let s = d.to_string_lossy().to_string();
            if !dirs.iter().any(|p| p == &s) {
                dirs.push(s);
            }
        }

        let export_line = if dirs.is_empty() {
            String::new()
        } else {
            let mut expr = String::new();
            expr.push_str("export PATH=");
            for (i, d) in dirs.iter().enumerate() {
                if i > 0 {
                    expr.push(':');
                }
                expr.push_str(&sh_single_quote(d));
            }
            expr.push_str(":\"$PATH\"");
            expr
        };

        let content = if export_line.is_empty() {
            format!(
                "#!/usr/bin/env sh\n\
                 # Generated by CliSwitch. Do not edit.\n\
                 exec {} \"$@\"\n",
                sh_single_quote(&tool_path.to_string_lossy())
            )
        } else {
            format!(
                "#!/usr/bin/env sh\n\
                 # Generated by CliSwitch. Do not edit.\n\
                 {export_line}\n\
                 exec {} \"$@\"\n",
                sh_single_quote(&tool_path.to_string_lossy())
            )
        };

        let _ = write_if_changed(shim_path, &content)?;

        // Ensure executable bit.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let perm = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(shim_path, perm)
                .with_context(|| format!("chmod +x failed: {}", shim_path.display()))?;
        }

        Ok(())
    }
}

pub fn ensure_cli_tool_shim(
    tool_bin: &str,
    tool_path: &Path,
    node_bin_dir: Option<&Path>,
    npm_global_bin_dir: Option<&Path>,
) -> anyhow::Result<EnsureCliToolShimResult> {
    let shim_dir = cli_tools_shim_dir()?;
    std::fs::create_dir_all(&shim_dir)
        .with_context(|| format!("create shim dir failed: {}", shim_dir.display()))?;

    let touched_files = {
        #[cfg(windows)]
        {
            ensure_windows_path_has_shim_dir(&shim_dir)?
        }
        #[cfg(not(windows))]
        {
            ensure_unix_path_has_shim_dir(&shim_dir)?
        }
    };

    let shim_path = tool_shim_path(&shim_dir, tool_bin);

    let tool_dir = tool_path.parent().map(|p| p.to_path_buf());
    let mut prepend_dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = tool_dir {
        prepend_dirs.push(d);
    }
    if let Some(d) = npm_global_bin_dir {
        prepend_dirs.push(d.to_path_buf());
    }
    if let Some(d) = node_bin_dir {
        prepend_dirs.push(d.to_path_buf());
    }
    // Dedup, keep order.
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    prepend_dirs.retain(|p| seen.insert(p.clone()));

    write_tool_shim(&shim_path, tool_path, &prepend_dirs)?;

    Ok(EnsureCliToolShimResult {
        shim_dir,
        shim_path,
        touched_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_file(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "cliswitch-terminal-test-{}-{}",
            uuid::Uuid::new_v4(),
            name
        ));
        p
    }

    #[test]
    fn upsert_marked_block_allows_missing_file() {
        let path = tmp_file("rc");
        let _ = std::fs::remove_file(&path);

        let block_lines = vec![
            BEGIN_MARKER.to_string(),
            "export PATH=\"$HOME/.cliswitch/bin:$PATH\"".to_string(),
            END_MARKER.to_string(),
        ];

        let changed = upsert_marked_block(&path, &block_lines).expect("upsert marked block");
        assert!(changed, "expected file to be written");

        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains(BEGIN_MARKER));
        assert!(content.contains(END_MARKER));
    }

    #[test]
    fn upsert_marked_block_rejects_non_utf8_file() {
        let path = tmp_file("rc-non-utf8");
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).expect("write");

        let block_lines = vec![
            BEGIN_MARKER.to_string(),
            "export PATH=\"$HOME/.cliswitch/bin:$PATH\"".to_string(),
            END_MARKER.to_string(),
        ];

        let err = upsert_marked_block(&path, &block_lines)
            .expect_err("expected non-utf8 file to be rejected");
        let msg = format!("{err:#}");
        assert!(msg.contains("read failed:"), "unexpected error: {msg}");
    }

    #[test]
    fn write_if_changed_rejects_non_utf8_file() {
        let path = tmp_file("write-non-utf8");
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).expect("write");

        write_if_changed(&path, "x").expect_err("expected non-utf8 file to be rejected");
    }
}
