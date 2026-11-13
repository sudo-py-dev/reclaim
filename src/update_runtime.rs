//! Managed, verified one-click updates. No save dialog, shell, or command script.
// Hardened module: no unchecked indexing or arithmetic in shipped code (CWE-125, CWE-190).
#![cfg_attr(
    not(test),
    deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)
)]
use anyhow::{Context, bail};
#[cfg(target_os = "linux")]
use anyhow::{anyhow, ensure};
use reclaim::updater::{self, Release};
use std::path::{Path, PathBuf};

pub fn flatpak_managed() -> bool {
    cfg!(target_os = "linux") && Path::new("/.flatpak-info").exists()
}

pub fn snap_managed() -> bool {
    cfg!(target_os = "linux") && std::env::var("SNAP_NAME").as_deref() == Ok("reclaim")
}

/// Sandboxed packages are updated by their distributor, not native installers.
pub fn package_managed() -> bool {
    flatpak_managed() || snap_managed()
}

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
pub(super) fn cache_root() -> anyhow::Result<PathBuf> {
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
    let path = base
        .with_context(|| reclaim::i18n::diagnostic("Could not locate the private update cache"))?;
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn new_stage() -> anyhow::Result<tempfile::TempDir> {
    Ok(tempfile::Builder::new()
        .prefix(STAGE_PREFIX)
        .tempdir_in(cache_root()?)?)
}

const STAGE_PREFIX: &str = "pending-";

/// Download and verify an installer the user opens themselves (macOS packages).
/// It stays staged until the next launch, then [`cleanup_staged_updates`] removes it.
pub fn stage_installer(release: &Release, progress: impl Fn(u64, u64)) -> anyhow::Result<PathBuf> {
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
) -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    #[cfg(not(any(target_os = "linux", windows)))]
    let _: &PathBuf = &executable;
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
        bail!("Automatic updates require an installed copy of Reclaim");
    }
    #[cfg(target_os = "linux")]
    if !Path::new(PKEXEC).is_file() {
        bail!("Automatic package updates require pkexec (polkit)");
    }
    #[cfg(windows)]
    if !executable
        .parent()
        .is_some_and(|p| p.join("installed-by-inno").is_file())
    {
        bail!("Run the Windows setup once to enable automatic updates");
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
        install_deb(release, &package)?;
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
    bail!("Automatic updates are unavailable on this platform")
}

#[cfg(target_os = "linux")]
const PKEXEC: &str = "/usr/bin/pkexec";

/// Hands a verified package to the privileged step through the desktop's own
/// password prompt. The staged files are untrusted to root: the elevated
/// `/usr/bin/reclaim` checks the signature again over a private copy (see
/// [`run_privileged_deb_update`]). Blocks until the package manager is done.
#[cfg(target_os = "linux")]
fn install_deb(release: &Release, package: &Path) -> anyhow::Result<()> {
    std::fs::write(updater::signature_path(package), &release.signature)?;
    let package = package
        .to_str()
        .with_context(|| reclaim::i18n::diagnostic("Update cache path is not valid Unicode"))?;
    // Always elevate the system-installed binary, never a user-controlled one.
    let arguments = [
        "/usr/bin/reclaim".to_owned(),
        updater::INSTALL_DEB_FLAG.to_owned(),
        release.version.to_string(),
        package.to_owned(),
    ];
    let code = crate::native_process::spawn(Path::new(PKEXEC), &arguments)
        .map_err(|e| anyhow!("Could not ask for administrator permission: {e}"))?
        .wait();
    pkexec_result(code.ok())
}

/// The root side of [`install_deb`], run before any GUI or D-Bus setup.
/// Only the root-only copy that [`updater::protect_deb_update`] verified is
/// passed to apt, with a fixed environment instead of the caller's.
#[cfg(target_os = "linux")]
pub fn run_privileged_deb_update(args: &[std::ffi::OsString]) -> anyhow::Result<()> {
    // SAFETY: geteuid has no preconditions and cannot fail.
    if unsafe { libc::geteuid() } != 0 {
        bail!(reclaim::i18n::diagnostic(
            "This command is used only by Reclaim's updater",
        ));
    }
    std::env::set_current_dir("/")?;
    let package = updater::protect_deb_update(args)?;
    let path = package
        .path
        .to_str()
        .with_context(|| reclaim::i18n::diagnostic("Update path is not valid Unicode"))?;
    let arguments = ["install", "-y", "--", path].map(str::to_owned);
    let environment = [
        "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
        "DEBIAN_FRONTEND=noninteractive",
    ]
    .map(Into::into)
    .into();
    let code = crate::native_process::spawn_with_environment(
        Path::new("/usr/bin/apt-get"),
        &arguments,
        environment,
    )
    .map_err(|e| anyhow!("Could not start the package manager: {e}"))?
    .wait()?;
    // `package` (the protected copy) is removed when it goes out of scope.
    ensure!(
        code == 0,
        "The package manager did not install the update (status {code})"
    );
    Ok(())
}

/// pkexec exits with 126 when the password prompt is dismissed and 127 when
/// permission is refused; the elevated step exits with 1 on failure. `None`
/// means it did not exit normally.
#[cfg(any(target_os = "linux", test))]
fn pkexec_result(code: Option<u32>) -> anyhow::Result<()> {
    match code {
        Some(0) => Ok(()),
        Some(126 | 127) => bail!(
            "The update was not installed: administrator permission was not given. Reclaim was left open."
        ),
        other => bail!(
            "Package update did not complete (status {}). Reclaim was left open.",
            other.map_or_else(|| "interrupted".into(), |c| c.to_string())
        ),
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
        super::pkexec_result(Some(0)).unwrap();
        for code in [Some(1), Some(126), Some(127), None] {
            assert!(super::pkexec_result(code).is_err(), "{code:?}");
        }
    }
}
