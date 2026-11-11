//! Report startup failures before the Windows GUI (or its renderer) exists.
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
struct FileLogger;
static LOGGER: FileLogger = FileLogger;

fn append(path: &Path, message: &str) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    // Never follow a symlink planted where the log should be (CWE-59).
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
    }
    let mut file = options.open(path)?;
    writeln!(
        file,
        "{} {message}",
        chrono::DateTime::<chrono::Utc>::from(std::time::SystemTime::now()).to_rfc3339()
    )?;
    file.sync_data()
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            write(&format!(
                "{} {}: {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }
    }
    fn flush(&self) {}
}

pub fn write(message: &str) {
    if let Some(path) = LOG_PATH.get() {
        let _ = append(path, message);
    }
}

/// Per-user log folders, most preferred first. Unix never falls back to the
/// shared temporary directory, where another account could plant the folder
/// or a symlink in it (CWE-377, CWE-59). `LOCALAPPDATA` also lets tests redirect it.
fn log_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        directories.push(PathBuf::from(local).join("Reclaim/logs"));
    }
    #[cfg(windows)]
    // audit-allow(SEC-TEMP): Windows keeps %TEMP% inside the user's own profile.
    directories.push(std::env::temp_dir().join("Reclaim/logs"));
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    directories.extend(home.map(|home| home.join("Library/Logs/Reclaim")));
    #[cfg(not(any(windows, target_os = "macos")))]
    directories.extend(
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|home| home.join(".local/state")))
            .map(|state| state.join("reclaim/logs")),
    );
    directories
}

fn create_private_dir(directory: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory)
}

pub fn install() {
    for directory in log_directories() {
        if create_private_dir(&directory).is_err() {
            continue;
        }
        let path = directory.join("reclaim.log");
        if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > 1024 * 1024) {
            let _ = std::fs::remove_file(directory.join("reclaim.previous.log"));
            let _ = std::fs::rename(&path, directory.join("reclaim.previous.log"));
        }
        if append(
            &path,
            &format!(
                // audit-allow(I18N-MISSING): a line in the diagnostic log, not interface text.
                "Starting Reclaim {} on {} {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        )
        .is_ok()
        {
            let _ = LOG_PATH.set(path);
            break;
        }
    }
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    std::panic::set_hook(Box::new(|info| {
        let details = format!("{info}\n{}", std::backtrace::Backtrace::force_capture());
        report(
            &reclaim::i18n::t("Reclaim encountered an unexpected error and must close."),
            &details,
        );
    }));
    #[cfg(windows)]
    // SAFETY: the callback has the Windows ABI and remains valid for the process lifetime.
    unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::SetUnhandledExceptionFilter(Some(
            native_exception,
        ));
    }
}

/// Log a fatal problem and show it. `summary` is already translated; `details`
/// is technical and stays in its original language.
pub fn report(summary: &str, details: &str) {
    use reclaim::i18n;
    write(&format!("ERROR: {summary}\n{details}"));
    let location = LOG_PATH
        .get()
        .map(|path| i18n::message("Diagnostic log: {0}", &[&path.display().to_string()]))
        .unwrap_or_else(|| i18n::t("The diagnostic log could not be written."));
    let display_details: String = details.chars().take(2000).collect();
    let hint = i18n::t(
        "If this is a virtual machine, install or update its graphics drivers or guest additions.",
    );
    let message = [summary, &display_details, &location, &hint].join("\n\n");
    let _ = writeln!(std::io::stderr(), "{message}");
    #[cfg(windows)]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static SHOWING: AtomicBool = AtomicBool::new(false);
        if SHOWING.swap(true, Ordering::SeqCst) {
            return;
        }
        let text: Vec<u16> = message
            .chars()
            .take(8000)
            .collect::<String>()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let title: Vec<u16> = reclaim::i18n::t("Reclaim could not continue")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: both strings are terminated and alive until the modal call returns.
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
                    | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
        SHOWING.store(false, Ordering::SeqCst);
    }
}

/// # Safety
///
/// Called only by Windows as the unhandled-exception filter, with a pointer
/// that is either null or valid for the duration of the call.
#[cfg(windows)]
unsafe extern "system" fn native_exception(
    exception: *const windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS,
) -> i32 {
    // Do not attempt to resume execution after a native crash.
    let code = if exception.is_null() {
        0
    } else {
        // SAFETY: Windows supplies the exception structure for the duration of this callback.
        let record = unsafe { (*exception).ExceptionRecord };
        if record.is_null() {
            0
        } else {
            // SAFETY: a non-null record inside the supplied exception is valid here too.
            unsafe { (*record).ExceptionCode }
        }
    };
    report(
        &reclaim::i18n::t("Reclaim stopped because of a Windows exception."),
        // audit-allow(I18N-MISSING): technical detail shown after the translated summary.
        &format!("Exception code: 0x{code:08X}"),
    );
    windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_EXECUTE_HANDLER
}

#[cfg(test)]
mod tests {
    // Run the real panic hook in a separate process so other tests keep their
    // panic handlers and a genuine hook failure cannot take down this suite.
    #[cfg(not(windows))]
    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "Test-only subprocess isolates a deliberately panicking test harness; production code never launches helpers"
    )]
    fn panic_is_saved_before_the_process_exits() {
        let directory = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "diagnostics::tests::panic_hook_child",
                "--nocapture",
            ])
            .env("RECLAIM_PANIC_CHECK", "1")
            .env("LOCALAPPDATA", directory.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        let log =
            std::fs::read_to_string(directory.path().join("Reclaim/logs/reclaim.log")).unwrap();
        assert!(log.contains("Starting Reclaim"));
        assert!(log.contains("intentional diagnostic test panic"));
        assert!(log.contains("src/diagnostics.rs"));
    }

    #[cfg(not(windows))]
    #[test]
    fn panic_hook_child() {
        if std::env::var_os("RECLAIM_PANIC_CHECK").is_some() {
            super::install();
            panic!("intentional diagnostic test panic");
        }
    }
    #[test]
    fn reports_are_appended_without_overwriting_the_first_failure() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("reclaim.log");
        super::append(&path, "graphics initialization failed").unwrap();
        super::append(&path, "native exception 0xC0000005").unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("graphics initialization failed"));
        assert!(content.contains("native exception 0xC0000005"));
    }
}
