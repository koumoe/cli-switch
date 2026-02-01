use anyhow::Context as _;
use std::io::Read as _;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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

pub(crate) fn command_output_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
) -> anyhow::Result<Output> {
    // Ensure we don't accidentally block on stdin.
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let program = cmd.get_program().to_os_string();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_os_string()).collect();

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawn command failed: {:?} {:?}", program, args))?;

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();

    let stdout_handle = std::thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(ref mut r) = stdout {
            let _ = r.read_to_end(&mut out);
        }
        out
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(ref mut r) = stderr {
            let _ = r.read_to_end(&mut out);
        }
        out
    });

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            let _ = child.kill();
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();

    if timed_out {
        anyhow::bail!(
            "command timed out after {:?}: {:?} {:?}",
            timeout,
            program,
            args
        );
    }

    Ok(Output {
        status,
        stdout,
        stderr,
    })
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
