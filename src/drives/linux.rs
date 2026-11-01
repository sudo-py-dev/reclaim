//! Native Linux discovery. Sizes/topology come from sysfs, mounts from the
//! current namespace, and optional udev data supplies labels/filesystem types.
use super::Drive;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

pub fn discover() -> Result<Vec<Drive>, String> {
    discover_at(
        Path::new("/sys/class/block"),
        Path::new("/dev"),
        Path::new("/proc/self/mountinfo"),
        Path::new("/run/udev/data"),
    )
    .map_err(|error| format!("Could not list drives: {error}. You can still open a disk image."))
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_owned()
}
fn number(path: impl AsRef<Path>) -> Option<u64> {
    read(path).parse().ok()
}

// mountinfo encodes whitespace/backslashes as octal; udev/by-label use hex.
fn unescape(value: &str, octal: bool) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let escape = if octal && bytes[i] == b'\\' && i + 3 < bytes.len() {
            let digits = &bytes[i + 1..i + 4];
            if digits.iter().all(|b| (b'0'..=b'7').contains(b)) {
                u16::from_str_radix(std::str::from_utf8(digits).unwrap(), 8)
                    .ok()
                    .filter(|n| *n <= 255)
                    .map(|n| (n as u8, 4))
            } else {
                None
            }
        } else if !octal
            && bytes[i] == b'\\'
            && bytes.get(i + 1) == Some(&b'x')
            && i + 3 < bytes.len()
        {
            std::str::from_utf8(&bytes[i + 2..i + 4])
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .map(|n| (n, 4))
        } else {
            None
        };
        if let Some((byte, count)) = escape {
            output.push(byte);
            i += count;
        } else {
            output.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&output).into_owned()
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
    let mount_text = fs::read_to_string(mounts)?;
    let mut mounted = HashMap::<String, String>::new();
    for line in mount_text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() >= 7 && fields.contains(&"-") {
            let point = unescape(fields[4], true);
            mounted
                .entry(fields[2].into())
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
            format!("Volume {}", node.name)
        };
        let sector_size = number(node.sys.join("queue/logical_block_size"))
            .or_else(|| {
                node.parents
                    .iter()
                    .find_map(|parent| number(sys.join(parent).join("queue/logical_block_size")))
            })
            .filter(|n| n.is_power_of_two() && (512..=65536).contains(n))
            .unwrap_or(512) as usize;
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
                "Encrypted — unlock in Disks, then refresh".into()
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
    Ok(drives)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "reclaim-native-disks-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("sys")).unwrap();
            fs::create_dir_all(root.join("dev/disk/by-label")).unwrap();
            fs::create_dir_all(root.join("udev")).unwrap();
            fs::write(
                root.join("mountinfo"),
                "1 0 0:1 / / rw - rootfs rootfs rw\n",
            )
            .unwrap();
            Self(root)
        }
        fn write(&self, path: &str, value: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value).unwrap();
        }
        fn disk(&self, name: &str, number: &str, sectors: &str) {
            self.write(&format!("devices/{name}/size"), sectors);
            self.write(&format!("devices/{name}/dev"), number);
            self.write(&format!("devices/{name}/queue/logical_block_size"), "4096");
            symlink(
                self.0.join(format!("devices/{name}")),
                self.0.join(format!("sys/{name}")),
            )
            .unwrap();
            self.write(&format!("dev/{name}"), "");
        }
        fn discover(&self) -> std::io::Result<Vec<Drive>> {
            discover_at(
                &self.0.join("sys"),
                &self.0.join("dev"),
                &self.0.join("mountinfo"),
                &self.0.join("udev"),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
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
            f.0.join("devices/nvme0n1/nvme0n1p1"),
            f.0.join("sys/nvme0n1p1"),
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
            f.0.join("dev/dm-0"),
            f.0.join("dev/disk/by-label/תמונות\\x20חדשות"),
        )
        .unwrap();
        f.write("udev/b259:1", "E:ID_FS_TYPE=crypto_LUKS\n");
        let rows = f.discover().unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row.mounted));
        let whole = rows
            .iter()
            .find(|row| row.path.ends_with("nvme0n1"))
            .unwrap();
        assert_eq!(whole.size, 10000 * 512); // not 10000 * 4096
        assert_eq!(whole.name, "Test SSD");
        let partition = rows
            .iter()
            .find(|row| row.path.ends_with("nvme0n1p1"))
            .unwrap();
        assert!(partition.encrypted);
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
        fs::remove_dir_all(f.0.join("udev")).unwrap();
        fs::remove_dir_all(f.0.join("dev/disk")).unwrap();
        let rows = f.discover().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].size, 1048576);
        assert_eq!(rows[0].sector_size, 512);
        assert!(!rows[0].mounted);
        fs::remove_file(f.0.join("mountinfo")).unwrap();
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
