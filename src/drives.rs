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
    /// A phone or camera connected for file transfer (MTP/PTP). It is listed
    /// so the user can see why it cannot be scanned; it is never opened.
    pub phone: bool,
    /// An encrypted container that is open: its contents are listed under it as
    /// their own entry, which is what can be scanned. The container stays ciphertext.
    pub unlocked: bool,
    /// The listed entry this one was opened from or sits in (dm-crypt, LVM, RAID),
    /// so the list can nest it there. `None` for top-level entries.
    pub parent: Option<PathBuf>,
    /// The running operating system's volume. Scanning it while in use can
    /// overwrite the very files the user wants back.
    pub system: bool,
}

impl Drive {
    /// Whether this entry can be selected and scanned.
    pub fn scannable(&self) -> bool {
        !self.encrypted && !self.phone
    }
    pub fn display_name(&self) -> String {
        if self.name_is_message {
            reclaim::i18n::t(&self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Orders `drives` so each entry follows the one it sits in, depth first,
/// keeping the original order among siblings. A parent that is not listed, or a
/// cycle, leaves the entry at the top level.
pub fn nest(drives: Vec<Drive>) -> Vec<Drive> {
    fn place(index: usize, parents: &[Option<usize>], placed: &mut [bool], order: &mut Vec<usize>) {
        if placed.get(index).copied().unwrap_or(true) {
            return;
        }
        if let Some(slot) = placed.get_mut(index) {
            *slot = true;
        }
        order.push(index);
        for (child, parent) in parents.iter().enumerate() {
            if *parent == Some(index) {
                place(child, parents, placed, order);
            }
        }
    }
    let paths: Vec<PathBuf> = drives.iter().map(|d| d.path.clone()).collect();
    let parent_of = |drive: &Drive| {
        drive
            .parent
            .as_ref()
            .and_then(|parent| paths.iter().position(|p| p == parent))
    };
    let parents: Vec<Option<usize>> = drives.iter().map(parent_of).collect();
    let mut order = Vec::with_capacity(drives.len());
    let mut placed = vec![false; drives.len()];
    for index in 0..drives.len() {
        if parents.get(index).copied().flatten().is_none() {
            place(index, &parents, &mut placed, &mut order);
        }
    }
    // Whatever is left sits in a cycle; it is shown at the top level, never dropped.
    for index in 0..drives.len() {
        place(index, &parents, &mut placed, &mut order);
    }
    let mut slots: Vec<Option<Drive>> = drives.into_iter().map(Some).collect();
    order
        .into_iter()
        .filter_map(|index| slots.get_mut(index).and_then(Option::take))
        .collect()
}

/// How many listed entries `drive` sits inside, for indenting the list.
/// An entry caught in a parent cycle is shown at the top level, as [`nest`] places it.
pub fn depth(drives: &[Drive], drive: &Drive) -> usize {
    let mut seen = vec![&drive.path];
    let mut current = drive;
    while let Some(parent) = current.parent.as_ref()
        && let Some(next) = drives.iter().find(|d| &d.path == parent)
    {
        if seen.contains(&&next.path) {
            return 0;
        }
        seen.push(&next.path);
        current = next;
    }
    seen.len().saturating_sub(1)
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
    #[cfg(windows)]
    let opened = windows::open_source(path);
    #[cfg(not(windows))]
    let opened = File::open(path);
    match opened {
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
                    "macOS denied access. Reading a whole drive requires administrator rights, and macOS may ask you to allow access to removable volumes or Full Disk Access in System Settings > Privacy & Security. You can open a disk image instead.",
                ));
            }
            Err(e)
        }
    }
}

#[cfg(target_os = "linux")]
fn object_path(device: &Path) -> io::Result<String> {
    use std::fmt::Write as _;
    use std::os::unix::ffi::OsStrExt;
    let name = device
        .file_name()
        .ok_or_else(|| io::Error::other("Invalid device path"))?;
    let mut path = String::from("/org/freedesktop/UDisks2/block_devices/");
    for byte in name.as_bytes() {
        if byte.is_ascii_alphanumeric() {
            path.push(char::from(*byte));
        } else {
            // Writing to a String cannot fail.
            write!(path, "_{byte:02x}").ok();
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
                reclaim::Event::Checking { .. }
                | reclaim::Event::UnusedSpace { .. }
                | reclaim::Event::Unreadable { .. } => {}
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

#[cfg(test)]
mod nest_tests {
    use super::*;

    fn entry(path: &str, parent: Option<&str>) -> Drive {
        Drive {
            path: path.into(),
            name: path.into(),
            name_is_message: false,
            size: 1,
            kind: "Volume".into(),
            location: "Not mounted".into(),
            mounted: false,
            encrypted: false,
            sector_size: 512,
            phone: false,
            unlocked: false,
            parent: parent.map(Into::into),
            system: false,
        }
    }

    fn paths(drives: &[Drive]) -> Vec<&str> {
        drives.iter().map(|d| d.path.to_str().unwrap()).collect()
    }

    #[test]
    fn layered_volumes_follow_what_they_were_opened_from() {
        let nested = nest(vec![
            entry("p1", None),
            entry("dm-1", Some("dm-0")),
            entry("dm-0", Some("p3")),
            entry("p2", None),
            entry("p3", None),
            entry("orphan", Some("gone")),
        ]);
        assert_eq!(paths(&nested), ["p1", "p2", "p3", "dm-0", "dm-1", "orphan"]);
        let depths: Vec<_> = nested.iter().map(|d| depth(&nested, d)).collect();
        assert_eq!(depths, [0, 0, 0, 1, 2, 0]);
    }

    #[test]
    fn a_parent_cycle_keeps_every_entry_and_terminates() {
        let nested = nest(vec![
            entry("a", Some("b")),
            entry("b", Some("a")),
            entry("c", None),
        ]);
        assert_eq!(nested.len(), 3);
        assert!(nested.iter().all(|d| depth(&nested, d) == 0));
        // An entry hanging off a cycle is still top level, not indented by the loop.
        let tail = nest(vec![
            entry("a", Some("b")),
            entry("b", Some("a")),
            entry("t", Some("a")),
        ]);
        assert!(tail.iter().all(|d| depth(&tail, d) == 0));
    }
}
