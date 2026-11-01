use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

pub const AVAILABLE: bool = cfg!(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos"
));

#[derive(Clone, Debug)]
pub struct Drive {
    pub path: PathBuf,
    pub name: String,
    pub name_is_message: bool,
    pub size: u64,
    pub kind: String,
    pub location: String,
    pub mounted: bool,
    pub encrypted: bool,
    pub sector_size: usize,
}

impl Drive {
    pub fn display_name(&self) -> String {
        if self.name_is_message {
            reclaim::i18n::t(&self.name)
        } else {
            self.name.clone()
        }
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
pub use linux::discover;
#[cfg(target_os = "macos")]
pub use macos::discover;
#[cfg(target_os = "windows")]
pub use windows::discover;
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn discover() -> Result<Vec<Drive>, String> {
    Ok(Vec::new())
}

#[cfg(target_os = "linux")]
fn is_device(path: &Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    path.metadata()
        .is_ok_and(|m| m.file_type().is_block_device())
}

pub fn open_source(path: &Path) -> io::Result<File> {
    match File::open(path) {
        Ok(file) => Ok(file),
        #[cfg(target_os = "linux")]
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied && is_device(path) => {
            open_via_desktop(path)
        }
        Err(e) => {
            #[cfg(target_os = "windows")]
            if e.kind() == io::ErrorKind::PermissionDenied {
                return Err(io::Error::new(
                    e.kind(),
                    "Windows denied access. For physical drives, restart Reclaim with Run as administrator, or open a disk image. BitLocker volumes must be unlocked first.",
                ));
            }
            #[cfg(target_os = "macos")]
            if e.kind() == io::ErrorKind::PermissionDenied {
                return Err(io::Error::new(
                    e.kind(),
                    "macOS denied access. Check Full Disk Access in System Settings > Privacy & Security. Raw device access may also require administrator privileges. You can open a disk image instead.",
                ));
            }
            Err(e)
        }
    }
}

#[cfg(target_os = "linux")]
fn object_path(device: &Path) -> io::Result<String> {
    use std::os::unix::ffi::OsStrExt;
    let name = device
        .file_name()
        .ok_or_else(|| io::Error::other("Invalid device path"))?;
    let mut path = String::from("/org/freedesktop/UDisks2/block_devices/");
    for byte in name.as_bytes() {
        if byte.is_ascii_alphanumeric() {
            path.push(char::from(*byte));
        } else {
            use std::fmt::Write;
            let _ = write!(path, "_{byte:02x}");
        }
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn open_via_desktop(path: &Path) -> io::Result<File> {
    use zbus::{
        blocking::{Connection, Proxy},
        proxy::MethodFlags,
        zvariant::{OwnedFd, Value},
    };
    let path = std::fs::canonicalize(path)?;
    let request = || -> zbus::Result<File> {
        let connection = Connection::system()?;
        let object = object_path(&path).map_err(zbus::Error::from)?;
        let proxy = Proxy::new(
            &connection,
            "org.freedesktop.UDisks2",
            object.as_str(),
            "org.freedesktop.UDisks2.Block",
        )?;
        let options = std::collections::HashMap::<&str, Value<'_>>::new();
        let descriptor: Option<OwnedFd> = proxy.call_with_flags(
            "OpenDevice",
            MethodFlags::AllowInteractiveAuth.into(),
            &("r", options),
        )?;
        let descriptor = descriptor
            .ok_or_else(|| zbus::Error::Failure("No drive access was returned".into()))?;
        let owned: std::os::fd::OwnedFd = descriptor.into();
        Ok(File::from(owned))
    };
    request().map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, format!("Could not get read-only drive access: {e}. Allow the desktop authentication prompt and try again. If no prompt appears, check that UDisks2 and a polkit authentication agent are installed, or open a disk image.")))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    #[cfg(target_os = "linux")]
    fn udisks_paths_escape_mapper_names() {
        assert_eq!(
            object_path(Path::new("/dev/dm-0")).unwrap(),
            "/org/freedesktop/UDisks2/block_devices/dm_2d0"
        );
        assert_eq!(
            object_path(Path::new("/dev/nvme0n1p1")).unwrap(),
            "/org/freedesktop/UDisks2/block_devices/nvme0n1p1"
        );
    }
}
