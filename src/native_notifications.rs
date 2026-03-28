use anyhow::Context as _;

const APP_DISPLAY_NAME: &str = "CliSwitch";

#[cfg(target_os = "macos")]
const MACOS_BUNDLE_ID: &str = "com.koumoe.cliswitch";

#[cfg(target_os = "windows")]
const WINDOWS_AUMID: &str = "com.koumoe.cliswitch";
#[cfg(target_os = "windows")]
const WINDOWS_SHORTCUT_NAME: &str = "CliSwitch";

pub(crate) fn initialize() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        notify_rust::set_application(MACOS_BUNDLE_ID).with_context(|| {
            format!("set macOS notification bundle id failed: {MACOS_BUNDLE_ID}")
        })?;
    }

    #[cfg(target_os = "windows")]
    {
        ensure_windows_notification_shortcut()?;
    }

    Ok(())
}

pub(crate) fn show(title: &str, body: &str) -> anyhow::Result<()> {
    let mut notification = notify_rust::Notification::new();
    notification
        .appname(APP_DISPLAY_NAME)
        .summary(title)
        .body(body);

    #[cfg(target_os = "windows")]
    {
        notification.app_id(WINDOWS_AUMID);
    }

    notification
        .show()
        .map(|_| ())
        .context("show native system notification failed")
}

#[cfg(target_os = "windows")]
fn ensure_windows_notification_shortcut() -> anyhow::Result<()> {
    use std::path::PathBuf;

    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize, IPersistFile, StructuredStorage::PROPVARIANT,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{
        IShellLinkW, SetCurrentProcessExplicitAppUserModelID, ShellLink,
    };
    use windows::core::{GUID, HSTRING, Interface, PCWSTR};

    const WINDOWS_TOAST_PROPERTY_GUID: GUID =
        GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3);
    const WINDOWS_APP_ID_PROPERTY_KEY: PROPERTYKEY = PROPERTYKEY {
        fmtid: WINDOWS_TOAST_PROPERTY_GUID,
        pid: 5,
    };
    const RPC_E_CHANGED_MODE_HRESULT: i32 = -2147417850;

    let appdata = std::env::var_os("APPDATA").context("APPDATA is not set")?;
    let shortcut_dir = PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");
    std::fs::create_dir_all(&shortcut_dir).with_context(|| {
        format!(
            "create start menu directory failed: {}",
            shortcut_dir.display()
        )
    })?;

    let shortcut_path = shortcut_dir.join(format!("{WINDOWS_SHORTCUT_NAME}.lnk"));
    if shortcut_path.exists() {
        std::fs::remove_file(&shortcut_path).with_context(|| {
            format!("remove stale shortcut failed: {}", shortcut_path.display())
        })?;
    }

    let exe_path = std::env::current_exe().context("resolve current exe path failed")?;
    let work_dir = exe_path
        .parent()
        .context("current exe has no parent directory")?;
    let app_id = HSTRING::from(WINDOWS_AUMID);
    let app_id_pcw = PCWSTR::from_raw(app_id.as_ptr());

    let needs_uninit = unsafe {
        match CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() {
            Ok(()) => true,
            Err(err) if err.code().0 == RPC_E_CHANGED_MODE_HRESULT => false,
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "initialize COM for toast shortcut failed: {err}"
                ));
            }
        }
    };

    let result = unsafe {
        SetCurrentProcessExplicitAppUserModelID(app_id_pcw)
            .context("set process AppUserModelID failed")?;

        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .context("create IShellLinkW failed")?;

        let exe = HSTRING::from(exe_path.to_string_lossy().as_ref());
        let exe_pcw = PCWSTR::from_raw(exe.as_ptr());
        shell_link
            .SetPath(exe_pcw)
            .context("set shortcut target failed")?;
        shell_link
            .SetIconLocation(exe_pcw, 0)
            .context("set shortcut icon failed")?;

        let work_dir_h = HSTRING::from(work_dir.to_string_lossy().as_ref());
        shell_link
            .SetWorkingDirectory(PCWSTR::from_raw(work_dir_h.as_ptr()))
            .context("set shortcut working directory failed")?;

        let description = HSTRING::from(APP_DISPLAY_NAME);
        shell_link
            .SetDescription(PCWSTR::from_raw(description.as_ptr()))
            .context("set shortcut description failed")?;

        let property_store: IPropertyStore = shell_link
            .cast()
            .context("cast shell link to property store failed")?;
        let app_id = PROPVARIANT::from(WINDOWS_AUMID);
        property_store
            .SetValue(
                &WINDOWS_APP_ID_PROPERTY_KEY as *const PROPERTYKEY,
                &app_id as *const PROPVARIANT,
            )
            .context("set AppUserModelID on shortcut failed")?;
        property_store
            .Commit()
            .context("commit shortcut property store failed")?;

        let shortcut = HSTRING::from(shortcut_path.to_string_lossy().as_ref());
        let persist_file: IPersistFile = shell_link
            .cast()
            .context("cast shell link to persist file failed")?;
        persist_file
            .Save(PCWSTR::from_raw(shortcut.as_ptr()), true)
            .with_context(|| format!("save shortcut failed: {}", shortcut_path.display()))
    };

    if needs_uninit {
        unsafe {
            CoUninitialize();
        }
    }

    result.map_err(anyhow::Error::from)
}
