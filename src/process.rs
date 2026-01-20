use std::process::Command;

/// Best-effort: prevent console windows from flashing when a GUI app spawns a console process.
pub(crate) fn command_silent(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;

        // https://learn.microsoft.com/windows/win32/procthread/process-creation-flags
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// Best-effort: notify Windows that user environment variables changed (e.g. PATH).
#[cfg(windows)]
pub(crate) fn notify_env_changed() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };

    // lParam must be a null-terminated wide string.
    let wide: Vec<u16> = "Environment\0".encode_utf16().collect();
    unsafe {
        // Ignore result; callers treat this as best-effort.
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            wide.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            2000,
            std::ptr::null_mut(),
        );
    }
}
