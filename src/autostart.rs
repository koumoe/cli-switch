use anyhow::Context as _;

pub const AUTO_START_APP_NAME: &str = "CliSwitch";

fn current_exe_utf8() -> anyhow::Result<String> {
    let exe = std::env::current_exe().context("读取当前可执行文件路径失败")?;
    let exe = exe
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("可执行文件路径包含非 UTF-8 字符：{}", exe.display()))?
        .to_string();
    Ok(exe)
}

pub fn set_enabled(enabled: bool) -> anyhow::Result<()> {
    if !auto_launch::AutoLaunch::is_support() {
        return Ok(());
    }

    let exe = current_exe_utf8()?;

    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder.set_app_name(AUTO_START_APP_NAME);
    builder.set_app_path(&exe);
    #[cfg(target_os = "macos")]
    builder.set_use_launch_agent(true);

    let launcher = builder.build().map_err(|e| anyhow::anyhow!("{e}"))?;

    if enabled {
        launcher.enable().map_err(|e| anyhow::anyhow!("{e}"))?;
    } else {
        launcher.disable().map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    Ok(())
}

pub fn is_enabled() -> anyhow::Result<bool> {
    if !auto_launch::AutoLaunch::is_support() {
        return Ok(false);
    }

    let exe = current_exe_utf8()?;

    let mut builder = auto_launch::AutoLaunchBuilder::new();
    builder.set_app_name(AUTO_START_APP_NAME);
    builder.set_app_path(&exe);
    #[cfg(target_os = "macos")]
    builder.set_use_launch_agent(true);
    let launcher = builder.build().map_err(|e| anyhow::anyhow!("{e}"))?;

    launcher.is_enabled().map_err(|e| anyhow::anyhow!("{e}"))
}
