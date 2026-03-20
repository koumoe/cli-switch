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
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;

        // Create a new process group so we can kill the whole tree on timeout (npm frequently
        // spawns child processes; killing only the parent can leave children holding stdout/stderr
        // open forever, which would otherwise deadlock our reader threads).
        unsafe {
            cmd.pre_exec(|| {
                // setpgid(0, 0) => new process group where PGID == PID.
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    // Ensure we don't accidentally block on stdin.
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let program = cmd.get_program().to_os_string();
    let args: Vec<_> = cmd.get_args().map(|a| a.to_os_string()).collect();

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawn command failed: {:?} {:?}", program, args))?;

    let pid = child.id();

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
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            kill_process_tree_best_effort(pid);
            let _ = child.kill();

            // Reap the process in a detached thread so we never block the caller.
            std::thread::spawn(move || {
                let _ = child.wait();
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
            });

            anyhow::bail!(
                "command timed out after {:?}: {:?} {:?}",
                timeout,
                program,
                args
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

pub(crate) fn kill_process_tree_best_effort(pid: u32) {
    if pid == 0 {
        return;
    }

    #[cfg(unix)]
    unsafe {
        // Negative PID => process group.
        let _ = libc::kill(-(pid as i32), libc::SIGKILL);
    }

    #[cfg(windows)]
    {
        // taskkill /T kills the whole process tree.
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
        command_silent(&mut cmd);
        // Best-effort: don't wait here (we're on a timeout path already).
        let _ = cmd.spawn();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn sh_escape_single_quotes(s: &str) -> String {
        // Wrap with single quotes and escape embedded single quotes:
        // abc'def => 'abc'\''def'
        let mut out = String::new();
        out.push('\'');
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

    #[test]
    #[cfg(unix)]
    fn command_output_with_timeout_does_not_deadlock_on_timeout() {
        use std::io::Write as _;

        // Use a tmp file so we can verify that the background child process is killed.
        let tmp = std::env::temp_dir().join(format!(
            "cliswitch-process-timeout-{}.txt",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&tmp);

        // Make sure the directory exists and is writable.
        let mut f = std::fs::File::create(&tmp).expect("create tmp file");
        writeln!(&mut f, "init").ok();
        drop(f);
        let _ = std::fs::remove_file(&tmp);

        let tmp_escaped = sh_escape_single_quotes(&tmp.to_string_lossy());
        let script = format!("sleep 1000 & echo $! > {tmp_escaped}; wait");

        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", &script]);
        command_silent(&mut cmd);

        let started = Instant::now();
        let res = command_output_with_timeout(&mut cmd, Duration::from_millis(200));
        assert!(res.is_err(), "expected timeout error");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timeout should return quickly"
        );

        let pid: i32 = std::fs::read_to_string(&tmp)
            .expect("read pid file")
            .trim()
            .parse()
            .expect("parse pid");

        // Wait a little for kill to take effect.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let alive = unsafe { libc::kill(pid, 0) == 0 };
            if !alive || Instant::now() >= deadline {
                assert!(!alive, "child process still alive after timeout: pid={pid}");
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let _ = std::fs::remove_file(&tmp);
    }
}
