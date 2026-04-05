use anyhow::Context as _;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;
#[cfg(any(target_os = "macos", test))]
use std::sync::OnceLock;

#[cfg(any(target_os = "macos", test))]
use regex::Regex;
#[cfg(target_os = "windows")]
use std::io::ErrorKind;
#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(target_os = "windows")]
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

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
        #[cfg(target_os = "windows")]
        set_windows_run_value(&exe)?;
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

    let Some(actual) = parse_macos_launch_agent_plist(&actual) else {
        return Ok(false);
    };

    Ok(actual
        == MacosLaunchAgent {
            label: AUTO_START_APP_NAME.to_string(),
            program_arguments: program_arguments(exe),
            run_at_load: true,
        })
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

    let Some(actual) = parse_linux_desktop_entry(&actual) else {
        return Ok(false);
    };

    Ok(actual == expected_linux_desktop_entry(AUTO_START_APP_NAME, exe, AUTO_START_ARGS))
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
fn set_windows_run_value(exe: &str) -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(WINDOWS_RUN_REGKEY, KEY_SET_VALUE)
        .context("读取 Windows 自启动注册表失败")?
        .set_value(
            AUTO_START_APP_NAME,
            &render_windows_run_value(exe, AUTO_START_ARGS),
        )
        .context("写入 Windows 自启动命令失败")?;
    Ok(())
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

#[cfg(any(target_os = "linux", target_os = "windows", test))]
fn render_command_with_args(exe: &str, args: &[&str]) -> String {
    if args.is_empty() {
        exe.to_string()
    } else {
        format!("{exe} {}", args.join(" "))
    }
}

#[cfg(any(target_os = "windows", test))]
fn render_windows_run_value(exe: &str, args: &[&str]) -> String {
    let exe = if exe.contains([' ', '\t']) {
        format!("\"{exe}\"")
    } else {
        exe.to_string()
    };
    render_command_with_args(&exe, args)
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn parse_bool_flag(value: &str) -> Option<bool> {
    match value.trim() {
        value if value.eq_ignore_ascii_case("true") => Some(true),
        value if value.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, PartialEq, Eq)]
struct LinuxDesktopEntry {
    entry_type: String,
    name: String,
    exec: String,
    startup_notify: bool,
    terminal: bool,
}

#[cfg(any(target_os = "linux", test))]
fn expected_linux_desktop_entry(app_name: &str, exe: &str, args: &[&str]) -> LinuxDesktopEntry {
    LinuxDesktopEntry {
        entry_type: "Application".to_string(),
        name: app_name.to_string(),
        exec: render_command_with_args(exe, args),
        startup_notify: false,
        terminal: false,
    }
}

#[cfg(any(target_os = "linux", test))]
fn parse_linux_desktop_entry(contents: &str) -> Option<LinuxDesktopEntry> {
    let mut in_desktop_entry = false;
    let mut entry_type = None;
    let mut name = None;
    let mut exec = None;
    let mut startup_notify = None;
    let mut terminal = None;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        let (key, value) = line.split_once('=')?;
        match key.trim() {
            "Type" => entry_type = Some(value.trim().to_string()),
            "Name" => name = Some(value.trim().to_string()),
            "Exec" => exec = Some(value.trim().to_string()),
            "StartupNotify" => startup_notify = parse_bool_flag(value),
            "Terminal" => terminal = parse_bool_flag(value),
            _ => {}
        }
    }

    Some(LinuxDesktopEntry {
        entry_type: entry_type?,
        name: name?,
        exec: exec?,
        startup_notify: startup_notify?,
        terminal: terminal?,
    })
}

#[cfg(test)]
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
#[derive(Debug, PartialEq, Eq)]
struct MacosLaunchAgent {
    label: String,
    program_arguments: Vec<String>,
    run_at_load: bool,
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_launch_agent_plist(contents: &str) -> Option<MacosLaunchAgent> {
    static LABEL_RE: OnceLock<Regex> = OnceLock::new();
    static PROGRAM_ARGUMENTS_RE: OnceLock<Regex> = OnceLock::new();
    static STRING_RE: OnceLock<Regex> = OnceLock::new();
    static RUN_AT_LOAD_RE: OnceLock<Regex> = OnceLock::new();

    let label = LABEL_RE
        .get_or_init(|| {
            Regex::new(r"(?s)<key>\s*Label\s*</key>\s*<string>\s*(.*?)\s*</string>")
                .expect("label regex should compile")
        })
        .captures(contents)?
        .get(1)?
        .as_str()
        .trim()
        .to_string();

    let program_arguments_section = PROGRAM_ARGUMENTS_RE
        .get_or_init(|| {
            Regex::new(r"(?s)<key>\s*ProgramArguments\s*</key>\s*<array>(.*?)</array>")
                .expect("program arguments regex should compile")
        })
        .captures(contents)?
        .get(1)?
        .as_str();

    let program_arguments = STRING_RE
        .get_or_init(|| {
            Regex::new(r"(?s)<string>\s*(.*?)\s*</string>").expect("string regex should compile")
        })
        .captures_iter(program_arguments_section)
        .map(|captures| captures[1].trim().to_string())
        .collect::<Vec<_>>();

    let run_at_load = RUN_AT_LOAD_RE
        .get_or_init(|| {
            Regex::new(r"(?s)<key>\s*RunAtLoad\s*</key>\s*<(true|false)\s*/>")
                .expect("run at load regex should compile")
        })
        .captures(contents)
        .and_then(|captures| captures.get(1))
        .and_then(|value| parse_bool_flag(value.as_str()))?;

    Some(MacosLaunchAgent {
        label,
        program_arguments,
        run_at_load,
    })
}

#[cfg(test)]
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
        AUTO_START_APP_NAME, AUTO_START_ARGS, MacosLaunchAgent, expected_linux_desktop_entry,
        parse_linux_desktop_entry, parse_macos_launch_agent_plist, program_arguments,
        render_linux_desktop_entry, render_macos_launch_agent_plist, render_windows_run_value,
    };

    #[test]
    fn renders_windows_run_value_with_quoted_executable_when_needed() {
        let exe = r"C:\Program Files\CliSwitch\cliswitch.exe";
        let rendered = render_windows_run_value(exe, AUTO_START_ARGS);
        assert_eq!(
            rendered,
            r#""C:\Program Files\CliSwitch\cliswitch.exe" --autostart"#
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
    fn parses_linux_desktop_entry_without_relying_on_comment_text() {
        let parsed = parse_linux_desktop_entry(
            "[Desktop Entry]\n\
            Type=Application\n\
            Name=CliSwitch\n\
            Comment=CliSwitch startup script\n\
            Exec=/usr/bin/cliswitch --autostart\n\
            StartupNotify=false\n\
            Terminal=false",
        );

        assert_eq!(
            parsed,
            Some(expected_linux_desktop_entry(
                AUTO_START_APP_NAME,
                "/usr/bin/cliswitch",
                AUTO_START_ARGS,
            ))
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

    #[test]
    fn parses_macos_launch_agent_plist_without_relying_on_whitespace() {
        let parsed = parse_macos_launch_agent_plist(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <plist version=\"1.0\">\n\
                <dict>\n\
                    <key>ProgramArguments</key>\n\
                    <array>\n\
                        <string>/Applications/CliSwitch.app/Contents/MacOS/cliswitch</string>\n\
                        <string>--autostart</string>\n\
                    </array>\n\
                    <key>RunAtLoad</key>\n\
                    <true/>\n\
                    <key>Label</key>\n\
                    <string>CliSwitch</string>\n\
                </dict>\n\
            </plist>",
        );

        assert_eq!(
            parsed,
            Some(MacosLaunchAgent {
                label: AUTO_START_APP_NAME.to_string(),
                program_arguments: program_arguments(
                    "/Applications/CliSwitch.app/Contents/MacOS/cliswitch",
                ),
                run_at_load: true,
            })
        );
    }
}
