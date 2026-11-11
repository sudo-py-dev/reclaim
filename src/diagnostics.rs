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
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
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

pub fn install() {
    let preferred = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Reclaim/logs");
    for directory in [preferred, std::env::temp_dir().join("Reclaim/logs")] {
        if std::fs::create_dir_all(&directory).is_err() {
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
            "Reclaim encountered an unexpected error and must close.",
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

pub fn report(summary: &str, details: &str) {
    write(&format!("ERROR: {summary}\n{details}"));
    let location = LOG_PATH
        .get()
        .map(|path| format!("\n\nDiagnostic log: {}", path.display()))
        .unwrap_or_else(|| "\n\nThe diagnostic log could not be written.".into());
    let display_details: String = details.chars().take(2000).collect();
    let message = format!(
        "{summary}\n\n{display_details}{location}\n\nIf this is a virtual machine, install or update its graphics drivers / Guest Additions."
    );
    eprintln!("{message}");
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
        let title: Vec<u16> = "Reclaim: launch or crash error"
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
            unsafe { (*record).ExceptionCode }
        }
    };
    report(
        "Reclaim stopped because of a Windows exception.",
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
