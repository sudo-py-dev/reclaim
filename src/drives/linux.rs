//! Native Linux discovery. Sizes/topology come from sysfs, mounts from the
//! current namespace, and optional udev data supplies labels/filesystem types.
// Hardened module: no unchecked indexing or arithmetic in shipped code (CWE-125, CWE-190).
#![cfg_attr(
    not(test),
    deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)
)]
use super::{Destination, Drive};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub fn discover() -> Result<Vec<Drive>, String> {
    // The sandbox exposes disk images through the file portal, not host drives.
    if Path::new("/.flatpak-info").exists() {
        return Ok(Vec::new());
    }
    discover_at(
        Path::new("/sys/class/block"),
        Path::new("/dev"),
        Path::new("/proc/self/mountinfo"),
        Path::new("/run/udev/data"),
    )
    .map_err(|error| format!("Could not list drives: {error}. You can still open a disk image."))
}

/// Kernel and udev text files are small; the cap keeps a misbehaving
/// pseudo-file from exhausting memory (CWE-400).
const MAX_SYSTEM_FILE: u64 = 8 * 1024 * 1024;
fn read_system_file(path: impl AsRef<Path>) -> std::io::Result<String> {
    let mut text = String::new();
    fs::File::open(path)?
        .take(MAX_SYSTEM_FILE)
        .read_to_string(&mut text)?;
    Ok(text)
}

fn read(path: impl AsRef<Path>) -> String {
    read_system_file(path).unwrap_or_default().trim().to_owned()
}
fn number(path: impl AsRef<Path>) -> Option<u64> {
    read(path).parse().ok()
}

// mountinfo encodes whitespace/backslashes as octal; udev/by-label use hex.
fn unescape(value: &str, octal: bool) -> String {
    let mut rest = value.as_bytes();
    let mut output = Vec::with_capacity(rest.len());
    while let Some((&first, tail)) = rest.split_first() {
        let decoded = match tail {
            [a, b, c, ..] if octal && first == b'\\' => octal_byte([*a, *b, *c]),
            [b'x', a, b, ..] if !octal && first == b'\\' => std::str::from_utf8(&[*a, *b])
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok()),
            _ => None,
        };
        // Both escape forms are four bytes: the backslash and three more.
        match (decoded, tail) {
            (Some(byte), [_, _, _, after @ ..]) => {
                output.push(byte);
                rest = after;
            }
            _ => {
                output.push(first);
                rest = tail;
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

/// Three octal digits as one byte; `None` for other digits or values over 255.
fn octal_byte(digits: [u8; 3]) -> Option<u8> {
    if !digits.iter().all(|b| (b'0'..=b'7').contains(b)) {
        return None;
    }
    u8::from_str_radix(std::str::from_utf8(&digits).ok()?, 8).ok()
}

struct Node {
    name: String,
    sys: PathBuf,
    dev: String,
    parents: Vec<String>,
    partition: bool,
}

fn discover_at(sys: &Path, dev: &Path, mounts: &Path, udev: &Path) -> std::io::Result<Vec<Drive>> {
    // Missing namespace metadata must not silently mark active drives unmounted.
    let mount_text = read_system_file(mounts)?;
    let mut mounted = HashMap::<String, String>::new();
    for line in mount_text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if let [_, _, device, _, point, _, _, ..] = fields.as_slice()
            && fields.contains(&"-")
        {
            let point = unescape(point, true);
            mounted
                .entry((*device).into())
                .and_modify(|old| {
                    if point == "/" {
                        *old = point.clone();
                    }
                })
                .or_insert(point);
        }
    }
    let mut nodes = Vec::new();
    for entry in fs::read_dir(sys)? {
        let Ok(entry) = entry else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if ["loop", "ram", "zram", "sr"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            continue;
        }
        let path = entry.path();
        let partition = path.join("partition").exists();
        let mut parents = Vec::new();
        if partition
            && let Ok(real) = fs::canonicalize(&path)
            && let Some(parent) = real.parent().and_then(Path::file_name)
        {
            parents.push(parent.to_string_lossy().into_owned());
        }
        if let Ok(slaves) = fs::read_dir(path.join("slaves")) {
            parents.extend(
                slaves
                    .flatten()
                    .map(|s| s.file_name().to_string_lossy().into_owned()),
            );
        }
        nodes.push(Node {
            name,
            dev: read(path.join("dev")),
            sys: path,
            parents,
            partition,
        });
    }
    nodes.sort_by(|a, b| a.name.cmp(&b.name));
    let mut active: HashSet<_> = nodes
        .iter()
        .filter(|n| mounted.contains_key(&n.dev))
        .map(|n| n.name.clone())
        .collect();
    // Propagate mounted state through partitions, dm-crypt, LVM, and RAID.
    for _ in 0..nodes.len() {
        let mut changed = false;
        for node in &nodes {
            if active.contains(&node.name) {
                for parent in &node.parents {
                    changed |= active.insert(parent.clone());
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut labels = HashMap::new();
    if let Ok(entries) = fs::read_dir(dev.join("disk/by-label")) {
        for entry in entries.flatten() {
            if let Ok(target) = fs::canonicalize(entry.path()) {
                labels.insert(
                    target,
                    unescape(&entry.file_name().to_string_lossy(), false),
                );
            }
        }
    }
    let mut drives = Vec::new();
    for node in &nodes {
        // The kernel ABI reports size in 512-byte units even for 4K devices.
        let Some(size) = number(node.sys.join("size"))
            .and_then(|n| n.checked_mul(512))
            .filter(|n| *n > 0)
        else {
            continue;
        };
        let path = dev.join(&node.name);
        let properties: HashMap<_, _> = read(udev.join(format!("b{}", node.dev)))
            .lines()
            .filter_map(|line| {
                line.strip_prefix("E:")
                    .and_then(|s| s.split_once('='))
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
            })
            .collect();
        let get = |key: &str| properties.get(key).map(String::as_str).unwrap_or("");
        let label = labels
            .get(&fs::canonicalize(&path).unwrap_or_else(|_| path.clone()))
            .cloned()
            .unwrap_or_else(|| unescape(get("ID_FS_LABEL_ENC"), false));
        let dm_uuid = read(node.sys.join("dm/uuid"));
        let unlocked = dm_uuid.starts_with("CRYPT-");
        let encrypted = get("ID_FS_TYPE") == "crypto_LUKS";
        let usb = get("ID_BUS") == "usb"
            || fs::canonicalize(&node.sys).ok().is_some_and(|p| {
                p.components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with("usb"))
            });
        let model = read(node.sys.join("device/model"));
        let mount = mounted.get(&node.dev);
        let name = if !label.is_empty() {
            label.clone()
        } else if !model.is_empty() {
            model.clone()
        } else if mount.is_some_and(|m| m == "/") {
            "Linux system".into()
        } else if let Some(m) =
            mount.filter(|m| m.starts_with("/media/") || m.starts_with("/run/media/"))
        {
            Path::new(m)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        } else if !node.partition && !node.name.starts_with("dm-") && !node.name.starts_with("md") {
            if usb { "USB drive" } else { "Internal drive" }.into()
        } else {
            "Volume".into()
        };
        let sector_size = number(node.sys.join("queue/logical_block_size"))
            .or_else(|| {
                node.parents
                    .iter()
                    .find_map(|parent| number(sys.join(parent).join("queue/logical_block_size")))
            })
            .filter(|n| n.is_power_of_two() && (512..=65536).contains(n))
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(512);
        let in_use = active.contains(&node.name);
        let whole =
            !node.partition && !node.name.starts_with("dm-") && !node.name.starts_with("md");
        drives.push(Drive {
            path,
            name_is_message: label.is_empty()
                && model.is_empty()
                && !mount.is_some_and(|m| m.starts_with("/media/") || m.starts_with("/run/media/")),
            name,
            size,
            kind: if unlocked {
                "Unlocked volume"
            } else if whole && usb {
                "USB drive"
            } else if whole {
                "Entire drive"
            } else {
                "Volume"
            }
            .into(),
            location: if encrypted {
                "Encrypted: unlock in Disks, then refresh".into()
            } else if let Some(m) = mount {
                format!("Mounted at {m}")
            } else if in_use {
                "Contains mounted volumes".into()
            } else {
                "Not mounted".into()
            },
            mounted: in_use,
            encrypted,
            sector_size,
        });
    }
    // Prefer named volumes over their parent disk. The volume retains its
    // filesystem label and is the source the user selected before deduplication.
    // Keep whole disks when no valid partition is available (e.g. lost tables).
    let paths: HashSet<_> = drives.iter().map(|d| d.path.clone()).collect();
    let represented_parents: HashSet<_> = nodes
        .iter()
        .filter(|node| node.partition && paths.contains(&dev.join(&node.name)))
        .flat_map(|node| node.parents.iter().map(|parent| dev.join(parent)))
        .collect();
    drives.retain(|drive| !represented_parents.contains(&drive.path));

    Ok(drives)
}

/// Compares device numbers rather than paths, so symlinks, bind mounts and
/// dm-crypt/LVM/RAID layers all resolve to the physical disks underneath.
pub fn destination(source: &Path, folder: &Path) -> Destination {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let Ok(source) = fs::metadata(source) else {
        return Destination::Unknown;
    };
    // Writing next to a disk image never changes the image's own bytes.
    if !source.file_type().is_block_device() {
        return Destination::Separate;
    }
    let Ok(folder) = fs::metadata(folder) else {
        return Destination::Unknown;
    };
    let number = |dev| format!("{}:{}", libc::major(dev), libc::minor(dev));
    destination_at(
        Path::new("/sys/dev/block"),
        &number(source.rdev()),
        &number(folder.dev()),
    )
}

fn destination_at(numbers: &Path, source: &str, folder: &str) -> Destination {
    // Anonymous filesystem numbers (notably multi-device Btrfs) cannot be
    // resolved safely from mountinfo: its source names only one member.
    let (Some(source), Some(folder)) = (
        whole_disks(&numbers.join(source)),
        whole_disks(&numbers.join(folder)),
    ) else {
        return Destination::Unknown;
    };
    if source.is_disjoint(&folder) {
        Destination::Separate
    } else {
        Destination::SameDrive
    }
}

/// Resolve every physical disk, or refuse to certify separation. A partial
/// set is unsafe: an unresolved RAID/device-mapper member may be the source.
fn whole_disks(node: &Path) -> Option<HashSet<PathBuf>> {
    fn visit(node: &Path, disks: &mut HashSet<PathBuf>, depth: usize) -> Option<()> {
        if depth > 16 {
            return None;
        }
        let mut real = fs::canonicalize(node).ok()?;
        if real.join("partition").try_exists().ok()? {
            real = real.parent()?.to_path_buf();
        }
        let layers = fs::read_dir(real.join("slaves")).ok()?;
        let mut leaf = true;
        for layer in layers {
            leaf = false;
            visit(&layer.ok()?.path(), disks, depth.saturating_add(1))?;
        }
        if leaf {
            disks.insert(real);
        }
        Some(())
    }
    let mut disks = HashSet::new();
    visit(node, &mut disks, 0)?;
    (!disks.is_empty()).then_some(disks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    struct Fixture(tempfile::TempDir);
    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().to_path_buf();
            fs::create_dir_all(root.join("sys")).unwrap();
            fs::create_dir_all(root.join("dev/disk/by-label")).unwrap();
            fs::create_dir_all(root.join("udev")).unwrap();
            fs::write(
                root.join("mountinfo"),
                "1 0 0:1 / / rw - rootfs rootfs rw\n",
            )
            .unwrap();
            Self(directory)
        }
        fn write(&self, path: &str, value: &str) {
            let path = self.0.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value).unwrap();
        }
        fn disk(&self, name: &str, number: &str, sectors: &str) {
            self.write(&format!("devices/{name}/size"), sectors);
            self.write(&format!("devices/{name}/dev"), number);
            self.write(&format!("devices/{name}/queue/logical_block_size"), "4096");
            symlink(
                self.0.path().join(format!("devices/{name}")),
                self.0.path().join(format!("sys/{name}")),
            )
            .unwrap();
            self.write(&format!("dev/{name}"), "");
        }
        fn discover(&self) -> std::io::Result<Vec<Drive>> {
            discover_at(
                &self.0.path().join("sys"),
                &self.0.path().join("dev"),
                &self.0.path().join("mountinfo"),
                &self.0.path().join("udev"),
            )
        }
    }

    #[test]
    fn recovery_folders_on_the_scanned_physical_disk_are_detected() {
        let f = Fixture::new();
        fs::create_dir_all(f.0.path().join("numbers")).unwrap();
        let node = |name: &str, number: &str| {
            f.write(&format!("devices/{name}/dev"), number);
            if name.contains('/') {
                f.write(&format!("devices/{name}/partition"), "1");
            } else {
                fs::create_dir_all(f.0.path().join(format!("devices/{name}/slaves"))).unwrap();
            }
            let short = name.rsplit('/').next().unwrap();
            symlink(
                f.0.path().join(format!("devices/{name}")),
                f.0.path().join(format!("sys/{short}")),
            )
            .unwrap();
            symlink(
                f.0.path().join(format!("devices/{name}")),
                f.0.path().join(format!("numbers/{number}")),
            )
            .unwrap();
        };
        node("sda", "8:0");
        node("sda/sda1", "8:1");
        node("sda/sda2", "8:2");
        node("sdb", "8:16");
        node("sdb/sdb1", "8:17");
        // An encrypted volume opened from the second partition of sda.
        node("dm-90", "253:90");
        fs::create_dir_all(f.0.path().join("devices/dm-90/slaves")).unwrap();
        symlink(
            f.0.path().join("devices/sda/sda2"),
            f.0.path().join("devices/dm-90/slaves/sda2"),
        )
        .unwrap();
        f.write(
            "mountinfo",
            "40 1 0:42 / /home rw - btrfs /dev/dm-90 rw\n41 1 0:43 / /mnt/sdb rw - btrfs /dev/sdb1 rw\n",
        );
        let check = |source, folder| destination_at(&f.0.path().join("numbers"), source, folder);
        assert_eq!(check("8:0", "8:1"), Destination::SameDrive);
        assert_eq!(check("8:1", "8:2"), Destination::SameDrive);
        assert_eq!(check("8:1", "253:90"), Destination::SameDrive);
        assert_eq!(check("8:17", "0:42"), Destination::Unknown);
        assert_eq!(check("8:2", "0:42"), Destination::Unknown);
        assert_eq!(check("8:16", "0:43"), Destination::Unknown);
        assert_eq!(check("8:17", "8:1"), Destination::Separate);
        assert_eq!(check("8:1", "0:99"), Destination::Unknown);

        // A missing member must invalidate the entire topology, even if a
        // healthy member was already resolved to a different physical disk.
        symlink(
            f.0.path().join("missing-device"),
            f.0.path().join("devices/dm-90/slaves/missing"),
        )
        .unwrap();
        assert_eq!(check("8:17", "253:90"), Destination::Unknown);
        fs::remove_file(f.0.path().join("devices/dm-90/slaves/missing")).unwrap();
        // Missing topology metadata must not be interpreted as a physical leaf.
        fs::remove_dir(f.0.path().join("devices/sdb/slaves")).unwrap();
        assert_eq!(check("8:17", "8:1"), Destination::Unknown);
        // A cyclic layer graph is incomplete, not a separate physical drive.
        symlink(
            f.0.path().join("devices/dm-90"),
            f.0.path().join("devices/dm-90/slaves/cycle"),
        )
        .unwrap();
        assert_eq!(check("8:1", "253:90"), Destination::Unknown);
    }

    #[test]
    fn disk_images_are_never_overwritten_by_recovery() {
        let folder = tempfile::tempdir().unwrap();
        let image = folder.path().join("image");
        fs::write(&image, b"image").unwrap();
        let result = destination(&image, folder.path());
        assert_eq!(result, Destination::Separate);
        assert_eq!(
            destination(Path::new("/nonexistent/reclaim-source"), Path::new("/")),
            Destination::Unknown
        );
    }

    #[test]
    fn a_usb_disk_appears_once_using_its_named_scannable_volume() {
        let f = Fixture::new();
        f.disk("sdb", "8:16", "120000");
        f.write("devices/sdb/device/model", "SanDisk 3.2Gen1");
        f.write("udev/b8:16", "E:ID_BUS=usb\n");
        f.write("devices/sdb/sdb1/size", "119000");
        f.write("devices/sdb/sdb1/dev", "8:17");
        f.write("devices/sdb/sdb1/partition", "1");
        symlink(
            f.0.path().join("devices/sdb/sdb1"),
            f.0.path().join("sys/sdb1"),
        )
        .unwrap();
        f.write("dev/sdb1", "");
        f.write(
            "mountinfo",
            "30 1 8:17 / /run/media/user/HBCD_PE_X64 rw - vfat /dev/sdb1 rw\n",
        );
        f.write("udev/b8:17", "E:ID_FS_LABEL_ENC=HBCD_PE_X64\n");
        let rows = f.discover().unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].path.ends_with("sdb1"));
        assert_eq!(rows[0].size, 119000 * 512);
        assert_eq!(rows[0].name, "HBCD_PE_X64");
        assert!(rows[0].mounted);
    }

    #[test]
    fn sizes_sector_alignment_labels_and_layered_mounts() {
        let f = Fixture::new();
        f.disk("nvme0n1", "259:0", "10000");
        f.write("devices/nvme0n1/device/model", "  Test SSD  ");
        f.write("devices/nvme0n1/nvme0n1p1/size", "9000");
        f.write("devices/nvme0n1/nvme0n1p1/dev", "259:1");
        f.write("devices/nvme0n1/nvme0n1p1/partition", "1");
        symlink(
            f.0.path().join("devices/nvme0n1/nvme0n1p1"),
            f.0.path().join("sys/nvme0n1p1"),
        )
        .unwrap();
        f.write("dev/nvme0n1p1", "");
        f.disk("dm-0", "253:0", "8000");
        f.write("devices/dm-0/dm/uuid", "CRYPT-LUKS2-test");
        f.write("devices/dm-0/slaves/nvme0n1p1", "");
        f.write(
            "mountinfo",
            "30 1 253:0 / /media/My\\040Photos rw - ext4 /dev/mapper/photos rw\n",
        );
        symlink(
            f.0.path().join("dev/dm-0"),
            f.0.path().join("dev/disk/by-label/תמונות\\x20חדשות"),
        )
        .unwrap();
        f.write("udev/b259:1", "E:ID_FS_TYPE=crypto_LUKS\n");
        let rows = f.discover().unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.mounted));
        assert!(!rows.iter().any(|row| row.path.ends_with("nvme0n1")));
        let partition = rows
            .iter()
            .find(|row| row.path.ends_with("nvme0n1p1"))
            .unwrap();
        assert!(partition.encrypted);
        assert_eq!(partition.size, 9000 * 512); // sysfs units, not 4096-byte sectors
        assert_eq!(partition.sector_size, 4096);
        let unlocked = rows.iter().find(|row| row.path.ends_with("dm-0")).unwrap();
        assert_eq!(unlocked.name, "תמונות חדשות");
        assert!(!unlocked.encrypted);
        assert_eq!(unlocked.kind, "Unlocked volume");
        assert_eq!(unlocked.location, "Mounted at /media/My Photos");
    }

    #[test]
    fn optional_metadata_is_not_a_dependency_and_invalid_sizes_are_omitted() {
        let f = Fixture::new();
        f.disk("sda", "8:0", "2048");
        f.disk("loop0", "7:0", "1000");
        f.disk("zram0", "252:0", "1000");
        f.disk("sr0", "11:0", "1000");
        f.disk("sdb", "8:16", "0");
        f.disk("sdc", "8:32", "18446744073709551615");
        f.disk("sdd", "8:48", "invalid");
        f.write("devices/sda/queue/logical_block_size", "123");
        fs::remove_dir_all(f.0.path().join("udev")).unwrap();
        fs::remove_dir_all(f.0.path().join("dev/disk")).unwrap();
        let rows = f.discover().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].size, 1048576);
        assert_eq!(rows[0].sector_size, 512);
        assert!(!rows[0].mounted);
        fs::remove_file(f.0.path().join("mountinfo")).unwrap();
        assert!(f.discover().is_err());
    }
    #[test]
    #[ignore = "reads the host kernel inventory; requires Linux sysfs/procfs"]
    fn live_native_discovery_smoke() {
        let rows = discover().expect("native kernel inventory");
        for row in &rows {
            assert!(row.path.starts_with("/dev"));
            assert!(row.size > 0);
            assert!(row.sector_size.is_power_of_two());
            assert!(
                !row.path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("loop")
            );
        }
        eprintln!("Native discovery: {} disk/volume entries", rows.len());
    }
    #[test]
    fn escaped_names_keep_unicode_and_malformed_sequences() {
        assert_eq!(
            unescape(r"/media/תמונות\040חדשות\134", true),
            "/media/תמונות חדשות\\"
        );
        assert_eq!(unescape(r"Photos\x20\xd7\x90", false), "Photos א");
        assert_eq!(unescape(r"bad\xZZ", false), r"bad\xZZ");
        assert_eq!(unescape(r"bad\777", true), r"bad\777");
    }
}
