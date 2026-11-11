//! Managed, verified one-click updates. No save dialog, shell, or command script.
use reclaim::updater::{self, Release};
use std::path::{Path, PathBuf};

pub fn supported() -> bool {
    cfg!(any(target_os = "linux", windows))
}

#[cfg(any(windows, test))]
fn windows_installer_arguments() -> Vec<String> {
    [
        "/SILENT",
        "/SUPPRESSMSGBOXES",
        "/NORESTART",
        "/CLOSEAPPLICATIONS",
        "/NORESTARTAPPLICATIONS",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// Private per-user folder for staged update packages. Each download gets its
/// own `pending-*` folder, removed after installing or on the next launch.
fn cache_root() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("Reclaim/updates"));
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|p| p.join("Library/Caches/Reclaim/updates"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .map(|p| p.join("reclaim/updates"));
    let path = base.ok_or("Could not locate the private update cache")?;
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

fn new_stage() -> Result<tempfile::TempDir, String> {
    tempfile::Builder::new()
        .prefix(STAGE_PREFIX)
        .tempdir_in(cache_root()?)
        .map_err(|e| e.to_string())
}

const STAGE_PREFIX: &str = "pending-";

/// Download and verify an installer the user opens themselves (macOS packages).
/// It stays staged until the next launch, then [`cleanup_staged_updates`] removes it.
pub fn stage_installer(release: &Release, progress: impl Fn(u64, u64)) -> Result<PathBuf, String> {
    let stage = new_stage()?;
    let package = stage.path().join(&release.asset);
    updater::download_installer(release, &package, progress)?;
    updater::verify_installer_file(release, &package)?;
    Ok(stage.keep().join(&release.asset))
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
    // Dropping `stage` deletes the package on every path, including errors;
    // only the Windows hand-off keeps it, because the installer is still running.
    let stage = new_stage()?;
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
        phase("Starting installer…");
        let directory = stage.keep();
        let package = directory.join(&release.asset);
        crate::native_process::spawn(&package, &windows_installer_arguments())?;
        Ok(())
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

/// Remove installers left from earlier updates so they do not use disk space.
/// Only `pending-*` folders created by Reclaim inside its own cache are removed;
/// symbolic links are never followed. Locked files are retried next launch.
pub fn cleanup_staged_updates() {
    if let Ok(root) = cache_root() {
        remove_stages(&root);
    }
}

fn remove_stages(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        // DirEntry::file_type does not follow links, so a planted link is skipped.
        let is_stage = entry
            .file_name()
            .to_string_lossy()
            .starts_with(STAGE_PREFIX)
            && entry.file_type().is_ok_and(|kind| kind.is_dir());
        if is_stage && let Err(error) = std::fs::remove_dir_all(entry.path()) {
            log::warn!("Could not remove a staged update yet: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_update_uses_direct_installer_handoff() {
        assert_eq!(
            super::windows_installer_arguments(),
            [
                "/SILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/CLOSEAPPLICATIONS",
                "/NORESTARTAPPLICATIONS"
            ]
        );
        let installer = include_str!("../packaging/windows/reclaim.iss");
        assert!(installer.contains("RestartApplications=yes"));
        assert!(installer.contains("Check: WizardSilent"));
        assert!(!installer.contains("reclaim-update.exe"));
    }

    #[test]
    fn cleanup_removes_only_staged_update_folders() {
        let root = tempfile::tempdir().unwrap();
        let stage = root.path().join("pending-abc");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("reclaim-old-setup.exe"), b"old installer").unwrap();
        std::fs::write(root.path().join("pending-file"), b"not a folder").unwrap();
        std::fs::create_dir(root.path().join("keep")).unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("user.txt"), b"user data").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), root.path().join("pending-link")).unwrap();

        super::remove_stages(root.path());

        assert!(!stage.exists());
        assert!(root.path().join("pending-file").exists());
        assert!(root.path().join("keep").exists());
        assert!(outside.path().join("user.txt").exists());
    }

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
