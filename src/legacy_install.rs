//! One-time cleanup of the copy that Reclaim's old Inno Setup installer left
//! behind (0.2.4 and earlier), so Windows lists Reclaim once after the move
//! to Velopack. Delete this file once those versions are no longer in use.
// Hardened module: no unchecked indexing or arithmetic in shipped code (CWE-125, CWE-190).
#![cfg_attr(
    not(test),
    deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)
)]
#[cfg(any(windows, test))]
use std::path::{Path, PathBuf};

/// The old installer's per-user uninstall entry (its `AppId` plus `_is1`).
#[cfg(windows)]
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{0EC19857-83F3-4B19-9239-A900C15E1661}_is1";

/// The old uninstaller in `folder`, recognised only by the marker file the
/// old installer wrote next to it. Anything else is left alone.
#[cfg(any(windows, test))]
fn uninstaller(folder: &Path) -> Option<PathBuf> {
    let uninstaller = folder.join("unins000.exe");
    (folder.is_absolute() && folder.join("installed-by-inno").is_file() && uninstaller.is_file())
        .then_some(uninstaller)
}

/// The folder the user chose in the old setup comes first, then its default.
#[cfg(any(windows, test))]
fn find_uninstaller(
    registered: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
) -> Option<PathBuf> {
    registered
        .into_iter()
        .chain(local_app_data.map(|p| p.join("Programs").join("Reclaim")))
        .find_map(|folder| uninstaller(&folder))
}

/// Starts the old uninstaller silently. It is not awaited: Velopack gives a
/// hook 30 seconds, and the uninstaller removes only the old folder, its
/// shortcuts and its uninstall entry. Settings live elsewhere and stay.
#[cfg(windows)]
pub fn remove() {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    let Some(uninstaller) = find_uninstaller(registered_folder(), local_app_data) else {
        return;
    };
    let arguments = ["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"].map(str::to_owned);
    if let Err(error) = crate::native_process::spawn(&uninstaller, &arguments) {
        log::warn!("Could not remove the earlier installation: {error}");
    }
}

#[cfg(not(windows))]
pub fn remove() {}

/// `InstallLocation` from the old uninstall entry, read into a bounded buffer.
#[cfg(windows)]
fn registered_folder() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let key: Vec<u16> = UNINSTALL_KEY.encode_utf16().chain(Some(0)).collect();
    let value: Vec<u16> = "InstallLocation".encode_utf16().chain(Some(0)).collect();
    let mut buffer = [0u16; 1024];
    let mut bytes = u32::try_from(size_of_val(&buffer)).ok()?;
    // SAFETY: both names are NUL-terminated and alive for the call; `bytes`
    // holds the buffer's size, which the call never writes past.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &raw mut bytes,
        )
    };
    if status != 0 {
        return None;
    }
    let units = usize::try_from(bytes).ok()? / 2;
    let text = buffer.get(..units)?;
    let end = text
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(text.len());
    let folder = PathBuf::from(std::ffi::OsString::from_wide(text.get(..end)?));
    Some(folder).filter(|p| p.is_absolute())
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn finds_the_old_copy_only_by_its_marker() {
        let local = tempfile::tempdir().unwrap();
        let folder = local.path().join("Programs/Reclaim");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("unins000.exe"), b"uninstaller").unwrap();
        assert_eq!(
            super::find_uninstaller(None, Some(local.path().into())),
            None
        );
        fs::write(folder.join("installed-by-inno"), b"marker").unwrap();
        assert_eq!(
            super::find_uninstaller(None, Some(local.path().into())),
            Some(folder.join("unins000.exe"))
        );
    }

    #[test]
    fn prefers_the_folder_chosen_in_the_old_setup() {
        let chosen = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        for folder in [
            chosen.path().to_owned(),
            local.path().join("Programs/Reclaim"),
        ] {
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("unins000.exe"), b"uninstaller").unwrap();
            fs::write(folder.join("installed-by-inno"), b"marker").unwrap();
        }
        assert_eq!(
            super::find_uninstaller(Some(chosen.path().into()), Some(local.path().into())),
            Some(chosen.path().join("unins000.exe"))
        );
        // A relative or unmarked registry value falls back to the default.
        assert_eq!(
            super::find_uninstaller(Some("Reclaim".into()), Some(local.path().into())),
            Some(local.path().join("Programs/Reclaim/unins000.exe"))
        );
    }
}
