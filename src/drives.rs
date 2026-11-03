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

#[derive(Clone, Debug, PartialEq, Eq)]
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

/// Whether saving into a folder could overwrite the scanned source's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    /// The folder is stored on a physical drive the source lives on.
    SameDrive,
    /// The folder cannot share storage with the source: another drive, or the
    /// source is a disk image whose contents ordinary writes never change.
    Separate,
    /// The backing drives could not be determined; the user must decide.
    Unknown,
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn destination(source: &Path, _folder: &Path) -> Destination {
    if source.is_file() {
        Destination::Separate
    } else {
        Destination::Unknown
    }
}

mod monitor;
pub use monitor::{DriveMonitor, watch};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
pub use linux::{destination, discover};
#[cfg(target_os = "macos")]
pub use macos::{destination, discover};
#[cfg(target_os = "windows")]
pub use windows::{destination, discover};
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
    #[ignore = "requires RECLAIM_TEST_DEVICE and read-only desktop authorization"]
    fn live_named_volume_can_read_data() {
        use std::io::Read;
        let path =
            PathBuf::from(std::env::var_os("RECLAIM_TEST_DEVICE").expect("explicit test device"));
        let drive = discover()
            .unwrap()
            .into_iter()
            .find(|drive| drive.path == path)
            .expect("named volume must be offered as a scan source");
        let handle = open_source(&path).expect("read-only volume authorization");
        let limit = drive.size.min(16 * 1024 * 1024);
        let mut source =
            reclaim::SourceReader::new(handle, drive.sector_size, Some(limit)).unwrap();
        let count = source
            .read(&mut [0; 4096])
            .expect("read first sector window");
        assert_eq!(count, 4096);
        eprintln!(
            "Read {count} bytes from {} ({})",
            drive.name,
            path.display()
        );
        let mut scanned = 0;
        let mut candidates = 0;
        reclaim::scan_reader(
            &mut source,
            &std::sync::atomic::AtomicBool::new(false),
            |event| match event {
                reclaim::Event::Progress(bytes) => scanned = bytes,
                reclaim::Event::Found(_) => candidates += 1,
                reclaim::Event::Checking { .. } | reclaim::Event::UnusedSpace { .. } => {}
            },
        )
        .unwrap();
        assert_eq!(scanned, limit);
        eprintln!("Bounded scan read {scanned} bytes and found {candidates} candidates");
    }

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
