//! Launch the updater / application through native APIs, without a shell.
use std::path::Path;

#[cfg(target_os = "linux")]
pub struct Child(libc::pid_t);
#[cfg(windows)]
pub struct Child(windows_sys::Win32::Foundation::HANDLE);
#[cfg(not(any(target_os = "linux", windows)))]
pub struct Child;

#[cfg(target_os = "linux")]
pub fn spawn(program: &Path, arguments: &[String]) -> Result<Child, String> {
    let environment = std::env::vars_os()
        .map(|(key, value)| {
            let mut item = key;
            item.push("=");
            item.push(value);
            item
        })
        .collect();
    spawn_with_environment(program, arguments, environment)
}

/// Like [`spawn`], with exactly `environment` (`KEY=value` items) instead of
/// this process's own.
#[cfg(target_os = "linux")]
pub fn spawn_with_environment(
    program: &Path,
    arguments: &[String],
    environment: Vec<std::ffi::OsString>,
) -> Result<Child, String> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let program = CString::new(program.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    let mut args = vec![program.clone()];
    for arg in arguments {
        args.push(CString::new(arg.as_bytes()).map_err(|e| e.to_string())?);
    }
    let argv: Vec<_> = args
        .iter()
        .map(|s| s.as_ptr().cast_mut())
        .chain(Some(std::ptr::null_mut()))
        .collect();
    let environment: Vec<_> = environment
        .into_iter()
        .map(|item| CString::new(item.as_bytes()).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let envp: Vec<_> = environment
        .iter()
        .map(|s| s.as_ptr().cast_mut())
        .chain(Some(std::ptr::null_mut()))
        .collect();
    let mut pid = 0;
    // SAFETY: all terminated argument/environment strings and pointer arrays
    // remain live through posix_spawn. No inherited custom file actions.
    let result = unsafe {
        libc::posix_spawn(
            &mut pid,
            program.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            argv.as_ptr(),
            envp.as_ptr(),
        )
    };
    if result != 0 {
        return Err(std::io::Error::from_raw_os_error(result).to_string());
    }
    Ok(Child(pid))
}

#[cfg(windows)]
pub fn spawn(program: &Path, arguments: &[String]) -> Result<Child, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::Threading::*;
    let application: Vec<_> = program.as_os_str().encode_wide().chain(Some(0)).collect();
    let program_text = program
        .to_str()
        .ok_or_else(|| reclaim::i18n::diagnostic("Update path is not valid Unicode"))?;
    let mut parts = vec![quote_argument(program_text)];
    for arg in arguments {
        if arg.contains('\0') {
            return Err(reclaim::i18n::diagnostic("Invalid update argument"));
        }
        parts.push(quote_argument(arg));
    }
    let mut command: Vec<_> = parts.join(" ").encode_utf16().chain(Some(0)).collect();
    // audit-allow(SEC-UNINIT): Win32 documents zeroed STARTUPINFOW as the initial state.
    // SAFETY: STARTUPINFOW is plain old data; all zero bytes is its documented initial value.
    let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    // audit-allow(SEC-UNINIT): an output structure that CreateProcessW fills in.
    // SAFETY: PROCESS_INFORMATION is plain old data; CreateProcessW overwrites it.
    let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: owned terminated strings, initialized structures, no inherited
    // handles, explicit executable path, and no shell interpretation.
    let ok = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            CREATE_NO_WINDOW,
            std::ptr::null(),
            std::ptr::null(),
            &startup,
            &mut process,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // SAFETY: CreateProcessW returned this thread handle and nothing else owns it.
    unsafe {
        windows_sys::Win32::Foundation::CloseHandle(process.hThread);
    }
    Ok(Child(process.hProcess))
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn spawn(_program: &Path, _arguments: &[String]) -> Result<Child, String> {
    Err("Automatic restart is unavailable on this platform".into())
}

#[cfg(target_os = "linux")]
impl Drop for Child {
    fn drop(&mut self) {
        // Reap an already-exited child; a relaunched app outlives its old parent.
        // SAFETY: self.0 is a PID from posix_spawn, and WNOHANG allows a null status.
        unsafe {
            libc::waitpid(self.0, std::ptr::null_mut(), libc::WNOHANG);
        }
    }
}

impl Child {
    /// Waits for the child to exit and returns its exit code.
    #[cfg(any(test, target_os = "linux"))]
    pub fn wait(self) -> Result<u32, String> {
        #[cfg(target_os = "linux")]
        {
            let mut status = 0;
            loop {
                // SAFETY: self owns a child PID returned by posix_spawn.
                if unsafe { libc::waitpid(self.0, &mut status, 0) } >= 0 {
                    break;
                }
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::Interrupted {
                    return Err(error.to_string());
                }
            }
            if libc::WIFEXITED(status) {
                return Ok(libc::WEXITSTATUS(status) as u32);
            }
            Err(reclaim::i18n::diagnostic(
                "Update process terminated unexpectedly",
            ))
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Threading::*;
            // SAFETY: self.0 is the live process handle this Child owns.
            if unsafe { WaitForSingleObject(self.0, INFINITE) } != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut code = 0;
            // SAFETY: self.0 is the live process handle and code a valid out-pointer.
            if unsafe { GetExitCodeProcess(self.0, &mut code) } == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(code)
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        Err(reclaim::i18n::diagnostic(
            "Native process waiting is unavailable",
        ))
    }
}

#[cfg(windows)]
impl Drop for Child {
    fn drop(&mut self) {
        // SAFETY: Child owns this process handle and closes it exactly once.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

// Windows' command line escaping rules: backslashes before quotes and the
// closing quote must be doubled. Paths with spaces/Unicode remain one argument.
#[cfg(any(windows, test))]
fn quote_argument(argument: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in argument.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            result.push_str(&"\\".repeat(slashes * 2 + 1));
        } else {
            result.push_str(&"\\".repeat(slashes));
        }
        slashes = 0;
        result.push(ch);
    }
    result.push_str(&"\\".repeat(slashes * 2));
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_arguments_preserve_spaces_quotes_and_trailing_slashes() {
        assert_eq!(super::quote_argument(""), "\"\"");
        assert_eq!(
            super::quote_argument("תיקיה with spaces"),
            "\"תיקיה with spaces\""
        );
        assert_eq!(super::quote_argument("a\"b"), "\"a\\\"b\"");
        assert_eq!(super::quote_argument("C:\\path\\"), "\"C:\\path\\\\\"");
    }
    #[test]
    fn native_child_probe() {}
    #[test]
    #[cfg(any(target_os = "linux", windows))]
    fn native_launch_waits_for_a_child_at_a_unicode_path_with_spaces() {
        let folder = tempfile::tempdir().unwrap();
        let program = folder.path().join(if cfg!(windows) {
            "בדיקה with spaces.exe"
        } else {
            "בדיקה with spaces"
        });
        std::fs::copy(std::env::current_exe().unwrap(), &program).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert_eq!(
            super::spawn(
                &program,
                &[
                    "--exact".into(),
                    "native_process::tests::native_child_probe".into()
                ]
            )
            .unwrap()
            .wait()
            .unwrap(),
            0
        );
    }
}
