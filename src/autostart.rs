use anyhow::Context as _;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::io::ErrorKind;
#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(target_os = "windows")]
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

pub const AUTO_START_APP_NAME: &str = "CliSwitch";
const AUTO_START_ARGS: &[&str] = &["--autostart"];

#[cfg(target_os = "windows")]
const WINDOWS_RUN_REGKEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
#[cfg(target_os = "windows")]
const WINDOWS_STARTUP_APPROVED_REGKEY: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";

fn current_exe_utf8() -> anyhow::Result<String> {
    let exe = std::env::current_exe().context("读取当前可执行文件路径失败")?;
    let exe = exe
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("可执行文件路径包含非 UTF-8 字符：{}", exe.display()))?
        .to_string();
    Ok(exe)
}

fn build_launcher(exe: &str) -> anyhow::Result<auto_launch::AutoLaunch> {
    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder.set_app_name(AUTO_START_APP_NAME);
    builder.set_app_path(exe);
    builder.set_args(AUTO_START_ARGS);
    #[cfg(target_os = "macos")]
    builder.set_use_launch_agent(true);
    builder.build().map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn set_enabled(enabled: bool) -> anyhow::Result<()> {
    if !auto_launch::AutoLaunch::is_support() {
        return Ok(());
    }

    let exe = current_exe_utf8()?;
    let launcher = build_launcher(&exe)?;

    if enabled {
        if registration_matches_current_config(&exe)? {
            tracing::debug!("autostart already enabled with current config");
            return Ok(());
        }
        launcher.enable().map_err(|e| anyhow::anyhow!("{e}"))?;
        tracing::info!("autostart enabled");
    } else {
        if !registration_exists()? {
            tracing::debug!("autostart already disabled");
            return Ok(());
        }
        launcher.disable().map_err(|e| anyhow::anyhow!("{e}"))?;
        tracing::info!("autostart disabled");
    }

    Ok(())
}

pub fn is_enabled() -> anyhow::Result<bool> {
    if !auto_launch::AutoLaunch::is_support() {
        return Ok(false);
    }

    let exe = current_exe_utf8()?;
    registration_matches_current_config(&exe)
}

#[cfg(any(target_os = "macos", test))]
fn program_arguments(exe: &str) -> Vec<String> {
    let mut args = Vec::with_capacity(1 + AUTO_START_ARGS.len());
    args.push(exe.to_string());
    args.extend(AUTO_START_ARGS.iter().map(|arg| (*arg).to_string()));
    args
}

#[cfg(target_os = "macos")]
fn registration_exists() -> anyhow::Result<bool> {
    Ok(macos_launch_agent_file()?.exists())
}

#[cfg(target_os = "macos")]
fn registration_matches_current_config(exe: &str) -> anyhow::Result<bool> {
    let file = macos_launch_agent_file()?;
    let actual = match fs::read_to_string(&file) {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(err).with_context(|| format!("读取自启动配置失败：{}", file.display()));
        }
    };

    let expected = render_macos_launch_agent_plist(AUTO_START_APP_NAME, &program_arguments(exe));
    Ok(actual == expected)
}

#[cfg(target_os = "macos")]
fn macos_launch_agent_file() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{AUTO_START_APP_NAME}.plist")))
}

#[cfg(target_os = "linux")]
fn registration_exists() -> anyhow::Result<bool> {
    Ok(linux_autostart_file()?.exists())
}

#[cfg(target_os = "linux")]
fn registration_matches_current_config(exe: &str) -> anyhow::Result<bool> {
    let file = linux_autostart_file()?;
    let actual = match fs::read_to_string(&file) {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(err).with_context(|| format!("读取自启动配置失败：{}", file.display()));
        }
    };

    let expected = render_linux_desktop_entry(AUTO_START_APP_NAME, exe, AUTO_START_ARGS);
    Ok(actual == expected)
}

#[cfg(target_os = "linux")]
fn linux_autostart_file() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?
        .join(".config")
        .join("autostart")
        .join(format!("{AUTO_START_APP_NAME}.desktop")))
}

#[cfg(target_os = "windows")]
fn registration_exists() -> anyhow::Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = match hkcu.open_subkey_with_flags(WINDOWS_RUN_REGKEY, KEY_READ) {
        Ok(run) => run,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err).context("读取 Windows 自启动注册表失败"),
    };
    Ok(run.get_value::<String, _>(AUTO_START_APP_NAME).is_ok())
}

#[cfg(target_os = "windows")]
fn registration_matches_current_config(exe: &str) -> anyhow::Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = match hkcu.open_subkey_with_flags(WINDOWS_RUN_REGKEY, KEY_READ) {
        Ok(run) => run,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err).context("读取 Windows 自启动注册表失败"),
    };

    let current = match run.get_value::<String, _>(AUTO_START_APP_NAME) {
        Ok(value) => value,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err).context("读取 Windows 自启动命令失败"),
    };

    if current != render_windows_run_value(exe, AUTO_START_ARGS) {
        return Ok(false);
    }

    Ok(windows_startup_approved_enabled(&hkcu)?.unwrap_or(true))
}

#[cfg(target_os = "windows")]
fn windows_startup_approved_enabled(hkcu: &RegKey) -> anyhow::Result<Option<bool>> {
    let reg = match hkcu.open_subkey_with_flags(WINDOWS_STARTUP_APPROVED_REGKEY, KEY_READ) {
        Ok(reg) => reg,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).context("读取 Windows StartupApproved 失败"),
    };

    let raw = match reg.get_raw_value(AUTO_START_APP_NAME) {
        Ok(value) => value,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).context("读取 Windows StartupApproved 条目失败"),
    };

    if raw.bytes.len() < 8 {
        return Ok(None);
    }

    Ok(Some(raw.bytes.iter().rev().take(8).all(|byte| *byte == 0)))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn registration_exists() -> anyhow::Result<bool> {
    Ok(false)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn registration_matches_current_config(_exe: &str) -> anyhow::Result<bool> {
    Ok(false)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn home_dir() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("无法读取 HOME 目录"))
}

#[cfg(any(target_os = "windows", test))]
fn render_windows_run_value(exe: &str, args: &[&str]) -> String {
    format!("{} {}", exe, args.join(" "))
}

#[cfg(any(target_os = "linux", test))]
fn render_linux_desktop_entry(app_name: &str, exe: &str, args: &[&str]) -> String {
    format!(
        "[Desktop Entry]\n\
            Type=Application\n\
            Version=1.0\n\
            Name={}\n\
            Comment={}startup script\n\
            Exec={} {}\n\
            StartupNotify=false\n\
            Terminal=false",
        app_name,
        app_name,
        exe,
        args.join(" ")
    )
}

#[cfg(any(target_os = "macos", test))]
fn render_macos_launch_agent_plist(app_name: &str, args: &[String]) -> String {
    let section = args
        .iter()
        .map(|arg| format!("<string>{}</string>", arg))
        .collect::<String>();

    format!(
        "{}\n{}\n\
            <plist version=\"1.0\">\n  \
            <dict>\n  \
                <key>Label</key>\n  \
                <string>{}</string>\n  \
                <key>ProgramArguments</key>\n  \
                <array>{}</array>\n  \
                <key>RunAtLoad</key>\n  \
                <true/>\n  \
            </dict>\n\
            </plist>",
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">"#,
        app_name,
        section
    )
}

#[cfg(test)]
mod tests {
    use super::{
        AUTO_START_APP_NAME, AUTO_START_ARGS, program_arguments, render_linux_desktop_entry,
        render_macos_launch_agent_plist, render_windows_run_value,
    };

    #[test]
    fn renders_windows_run_value_like_auto_launch() {
        let exe = r"C:\Program Files\CliSwitch\cliswitch.exe";
        let rendered = render_windows_run_value(exe, AUTO_START_ARGS);
        assert_eq!(
            rendered,
            r"C:\Program Files\CliSwitch\cliswitch.exe --autostart"
        );
    }

    #[test]
    fn renders_linux_desktop_entry_like_auto_launch() {
        let rendered =
            render_linux_desktop_entry(AUTO_START_APP_NAME, "/usr/bin/cliswitch", AUTO_START_ARGS);
        assert_eq!(
            rendered,
            "[Desktop Entry]\n\
            Type=Application\n\
            Version=1.0\n\
            Name=CliSwitch\n\
            Comment=CliSwitchstartup script\n\
            Exec=/usr/bin/cliswitch --autostart\n\
            StartupNotify=false\n\
            Terminal=false"
        );
    }

    #[test]
    fn renders_macos_launch_agent_plist_like_auto_launch() {
        let rendered = render_macos_launch_agent_plist(
            AUTO_START_APP_NAME,
            &program_arguments("/Applications/CliSwitch.app/Contents/MacOS/cliswitch"),
        );
        assert_eq!(
            rendered,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
            <plist version=\"1.0\">\n  \
            <dict>\n  \
                <key>Label</key>\n  \
                <string>CliSwitch</string>\n  \
                <key>ProgramArguments</key>\n  \
                <array><string>/Applications/CliSwitch.app/Contents/MacOS/cliswitch</string><string>--autostart</string></array>\n  \
                <key>RunAtLoad</key>\n  \
                <true/>\n  \
            </dict>\n\
            </plist>"
        );
    }
}
