//! Managed, verified one-click updates. No save dialog, shell, or command script.
use reclaim::updater::{self, Release};
#[cfg(any(target_os = "linux", windows))]
use std::path::Path;
use std::path::PathBuf;

pub fn supported() -> bool {
    cfg!(any(target_os = "linux", windows))
}

fn cache_root() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("Reclaim/updates"));
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .map(|p| p.join("reclaim/updates"));
    let path = base.ok_or("Could not locate the private update cache")?;
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn update(
    release: &Release,
    progress: impl Fn(u64, u64),
    phase: impl Fn(&'static str),
) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(not(any(target_os = "linux", windows)))]
    let _ = &executable;
    if updater::installer_download(release).is_none() {
        #[cfg(target_os = "linux")]
        {
            phase("Downloading and verifying update…");
            updater::install_with_progress(release, progress)?;
            phase("Restarting Reclaim…");
            crate::native_process::spawn(&executable, &[])?;
            return Ok(());
        }
        #[cfg(not(target_os = "linux"))]
        return Err("Automatic updates require an installed copy of Reclaim".into());
    }
    #[cfg(target_os = "linux")]
    packagekit_preflight()?;
    #[cfg(windows)]
    if !executable
        .parent()
        .is_some_and(|p| p.join("installed-by-inno").is_file())
    {
        return Err("Run the Windows setup once to enable automatic updates".into());
    }
    let stage = tempfile::Builder::new()
        .prefix("pending-")
        .tempdir_in(cache_root()?)
        .map_err(|e| e.to_string())?;
    let package = stage.path().join(&release.asset);
    phase("Downloading and verifying update…");
    updater::download_installer(release, &package, progress)?;
    updater::verify_installer_file(release, &package)?;
    #[cfg(target_os = "linux")]
    {
        phase("Installing update…");
        install_deb(&package, false)?;
        phase("Restarting Reclaim…");
        crate::native_process::spawn(&executable, &[])?;
        Ok(())
    }
    #[cfg(windows)]
    {
        phase("Preparing automatic restart…");
        windows::handoff(stage, release, &executable)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    Err("Automatic updates are unavailable on this platform".into())
}

#[cfg(target_os = "linux")]
fn packagekit_preflight() -> Result<(), String> {
    let connection = zbus::blocking::Connection::system().map_err(|e| e.to_string())?;
    let daemon = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        "/org/freedesktop/PackageKit",
        "org.freedesktop.PackageKit",
    )
    .map_err(|e| e.to_string())?;
    let _: String = daemon
        .get_property("BackendName")
        .map_err(|e| format!("Automatic package updates require PackageKit: {e}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_deb(package: &Path, simulate: bool) -> Result<(), String> {
    let connection = zbus::blocking::Connection::system().map_err(|e| e.to_string())?;
    let daemon = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        "/org/freedesktop/PackageKit",
        "org.freedesktop.PackageKit",
    )
    .map_err(|e| e.to_string())?;
    let path: zbus::zvariant::OwnedObjectPath = daemon
        .call("CreateTransaction", &())
        .map_err(|e| e.to_string())?;
    let transaction = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.PackageKit",
        path,
        "org.freedesktop.PackageKit.Transaction",
    )
    .map_err(|e| e.to_string())?;
    let _: () = transaction
        .call("SetHints", &(vec!["interactive=true", "background=false"],))
        .map_err(|e| e.to_string())?;
    let mut signals = transaction
        .receive_all_signals()
        .map_err(|e| e.to_string())?;
    // PackageKit's SIMULATE bit is used only by the explicit integration test.
    // Normal local installs use flags=0; the HTTPS + SHA-256 check is done first.
    let flags = if simulate { 1u64 << 2 } else { 0 };
    let files = vec![
        package
            .to_str()
            .ok_or("Update cache path is not valid Unicode")?,
    ];
    let reply: Option<()> = transaction
        .call_with_flags(
            "InstallFiles",
            zbus::proxy::MethodFlags::AllowInteractiveAuth.into(),
            &(flags, files),
        )
        .map_err(|e| e.to_string())?;
    reply.ok_or("The package service did not acknowledge the update")?;
    let mut failure = None;
    for signal in &mut signals {
        match signal.header().member().map(|m| m.as_str()) {
            Some("ErrorCode") => {
                let (_, details): (u32, String) =
                    signal.body().deserialize().map_err(|e| e.to_string())?;
                failure = Some(details);
            }
            Some("Finished") => {
                let (exit, _duration): (u32, u32) =
                    signal.body().deserialize().map_err(|e| e.to_string())?;
                return packagekit_result(exit, failure);
            }
            _ => {}
        }
    }
    Err("The package service disconnected before the update finished".into())
}

#[cfg(any(target_os = "linux", test))]
fn packagekit_result(exit: u32, failure: Option<String>) -> Result<(), String> {
    if exit == 1 && failure.is_none() {
        Ok(())
    } else {
        Err(failure.unwrap_or_else(|| {
            format!("Package update did not complete (status {exit}). Reclaim was left open.")
        }))
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        io::{Read, Write},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Threading::*,
    };
    const HELPER: &str = "reclaim-update.exe";

    struct Parent(HANDLE);
    impl Parent {
        fn open(pid: u32) -> Result<Self, String> {
            if pid == 0 || pid == std::process::id() {
                return Err("Invalid updater parent process".into());
            }
            // Hold the specific process, so PID reuse cannot confuse the wait.
            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Self(handle))
        }
        fn wait(&self) -> Result<(), String> {
            if unsafe { WaitForSingleObject(self.0, INFINITE) } == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error().to_string())
            }
        }
    }
    impl Drop for Parent {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    pub(super) fn handoff(
        stage: tempfile::TempDir,
        release: &Release,
        executable: &Path,
    ) -> Result<(), String> {
        let helper = stage.path().join(HELPER);
        std::fs::copy(executable, &helper).map_err(|e| e.to_string())?;
        let manifest = stage.path().join("update.json");
        let json = serde_json::json!({
            "schema":1, "parent":std::process::id(), "target":executable.to_str().ok_or("Update path is not valid Unicode")?,
            "version":release.version.to_string(), "asset":release.asset, "url":release.url, "digest":release.digest, "size":release.size
        });
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&manifest)
            .map_err(|e| e.to_string())?;
        file.write_all(json.to_string().as_bytes())
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        crate::native_process::spawn(
            &helper,
            &[
                "--apply-update".into(),
                manifest
                    .to_str()
                    .ok_or("Update path is not valid Unicode")?
                    .into(),
            ],
        )?;
        let start = Instant::now();
        while !stage.path().join("ready").exists() {
            if let Ok(error) = std::fs::read_to_string(stage.path().join("failed")) {
                return Err(error);
            }
            if start.elapsed() > Duration::from_secs(30) {
                return Err("The update helper did not start. Reclaim was left open.".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // The verified helper now owns the handoff; preserve its files while
        // the old application closes and the installer replaces its executable.
        let _owned_cache = stage.keep();
        Ok(())
    }

    pub(super) fn apply(manifest: &Path) -> Result<(), String> {
        let directory = manifest.parent().ok_or("Update has no cache directory")?;
        let result = apply_inner(manifest);
        if let Err(error) = &result {
            let _ = std::fs::write(directory.join("failed"), error);
        }
        result
    }
    fn apply_inner(manifest: &Path) -> Result<(), String> {
        let directory = manifest.parent().ok_or("Update has no cache directory")?;
        if std::env::current_exe()
            .map_err(|e| e.to_string())?
            .canonicalize()
            .map_err(|e| e.to_string())?
            != directory
                .join(HELPER)
                .canonicalize()
                .map_err(|e| e.to_string())?
        {
            return Err("Invalid updater location".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(manifest)
            .map_err(|e| e.to_string())?
            .take(16385)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16384 {
            return Err("Invalid update manifest size".into());
        }
        let json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let text = |name: &str| {
            json[name]
                .as_str()
                .ok_or_else(|| format!("Missing update field: {name}"))
        };
        if json["schema"].as_u64() != Some(1) {
            return Err("Invalid update manifest".into());
        }
        let target = PathBuf::from(text("target")?);
        if !target.is_absolute()
            || target
                .file_name()
                .is_none_or(|n| !n.to_string_lossy().eq_ignore_ascii_case("reclaim.exe"))
            || !target
                .parent()
                .is_some_and(|p| p.join("installed-by-inno").is_file())
        {
            return Err("Update target is not an installed Reclaim application".into());
        }
        let release = Release {
            version: semver::Version::parse(text("version")?).map_err(|e| e.to_string())?,
            asset: text("asset")?.into(),
            notes: String::new(),
            url: text("url")?.into(),
            digest: text("digest")?.into(),
            size: json["size"].as_u64().ok_or("Missing update size")?,
        };
        if release.version < semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap() {
            return Err("Refusing an older update".into());
        }
        if release.asset != "reclaim-windows-x86_64-setup.exe" {
            return Err("Invalid Windows update asset".into());
        }
        let package = directory.join(&release.asset);
        updater::verify_installer_file(&release, &package)?;
        let parent = Parent::open(
            json["parent"]
                .as_u64()
                .and_then(|p| u32::try_from(p).ok())
                .ok_or("Invalid updater parent")?,
        )?;
        std::fs::write(directory.join("ready"), b"ready").map_err(|e| e.to_string())?;
        parent.wait()?;
        let installation = (|| {
            updater::verify_installer_file(&release, &package)?;
            // Probe access without truncating or writing: another app instance
            // must not make an unattended upgrade queue a replacement on reboot.
            drop(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&target)
                    .map_err(|e| format!("Close other Reclaim windows before updating: {e}"))?,
            );
            let arguments = vec![
                "/VERYSILENT".into(),
                "/SUPPRESSMSGBOXES".into(),
                "/NORESTART".into(),
                "/RESTARTEXITCODE=3010".into(),
                "/SP-".into(),
                "/NOCLOSEAPPLICATIONS".into(),
                format!("/DIR={}", target.parent().unwrap().display()),
                format!("/LOG={}", directory.join("installer.log").display()),
            ];
            let exit = crate::native_process::spawn(&package, &arguments)?.wait()?;
            if exit != 0 {
                return Err(format!(
                    "Windows update installer failed (exit {exit}). See {}",
                    directory.join("installer.log").display()
                ));
            }
            Ok(())
        })();
        // Restart either the updated application or the existing installation
        // after an installer failure. Never delete the working application.
        let launched = crate::native_process::spawn(&target, &[]);
        installation?;
        launched?;
        std::fs::write(directory.join("complete"), b"complete").map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(package);
        Ok(())
    }
}

pub fn cleanup_completed_updates() {
    if !supported() {
        return;
    }
    let Ok(root) = cache_root() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with("pending-")
            && entry.path().join("complete").is_file()
        {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

pub fn run_helper_if_requested() -> Option<Result<(), String>> {
    #[cfg(windows)]
    {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.first().is_some_and(|a| a == "--apply-update") {
            return Some(if args.len() == 2 {
                windows::apply(Path::new(&args[1]))
            } else {
                Err("Invalid updater arguments".into())
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn cancellation_and_install_errors_never_request_a_restart() {
        assert!(super::packagekit_result(1, None).is_ok());
        assert!(super::packagekit_result(3, None).is_err());
        assert!(super::packagekit_result(1, Some("Authorization cancelled".into())).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires the host PackageKit service; does not install anything"]
    fn live_package_service_is_available() {
        super::packagekit_preflight().unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires RECLAIM_TEST_PACKAGE; simulates only, without installing"]
    fn live_package_install_simulation() {
        let path = std::path::PathBuf::from(
            std::env::var_os("RECLAIM_TEST_PACKAGE").expect("explicit package"),
        );
        super::install_deb(&path, true).unwrap();
    }
}
