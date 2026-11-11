//! IOKit/Disk Arbitration discovery; no diskutil or plist conversion process.
use super::{Destination, Drive};
use core_foundation_sys::{base::*, dictionary::*, number::*, string::*, url::*};
use std::{
    collections::{HashMap, HashSet},
    ffi::{CStr, CString},
    ptr,
};

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const std::ffi::c_char) -> CFMutableDictionaryRef;
    fn IOServiceGetMatchingServices(
        port: u32,
        matching: CFDictionaryRef,
        iterator: *mut u32,
    ) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: CFStringRef,
        allocator: CFAllocatorRef,
        options: u32,
    ) -> CFTypeRef;
    fn IORegistryEntryGetParentEntry(
        entry: u32,
        plane: *const std::ffi::c_char,
        parent: *mut u32,
    ) -> i32;
    fn IORegistryEntryGetRegistryEntryID(entry: u32, id: *mut u64) -> i32;
}
#[link(name = "DiskArbitration", kind = "framework")]
unsafe extern "C" {
    fn DASessionCreate(allocator: CFAllocatorRef) -> CFTypeRef;
    fn DADiskCreateFromBSDName(
        allocator: CFAllocatorRef,
        session: CFTypeRef,
        name: *const std::ffi::c_char,
    ) -> CFTypeRef;
    fn DADiskCopyDescription(disk: CFTypeRef) -> CFDictionaryRef;
}

// Both wrappers own a +1 reference, released on every early-return path.
struct Object(u32);
impl Drop for Object {
    fn drop(&mut self) {
        unsafe {
            IOObjectRelease(self.0);
        }
    }
}
struct Cf(CFTypeRef);
impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CFRelease(self.0);
            }
        }
    }
}
fn cf_string(text: &str) -> Cf {
    let value = CString::new(text).expect("constant CF key has no NUL");
    Cf(
        unsafe { CFStringCreateWithCString(ptr::null(), value.as_ptr(), kCFStringEncodingUTF8) }
            .cast(),
    )
}
fn string(value: CFTypeRef) -> Option<String> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFStringGetTypeID() } {
        return None;
    }
    let value = value.cast();
    let length = unsafe {
        CFStringGetMaximumSizeForEncoding(CFStringGetLength(value), kCFStringEncodingUTF8)
    };
    if !(0..=1024 * 1024).contains(&length) {
        return None;
    }
    let mut bytes = vec![0i8; length as usize + 1];
    if unsafe {
        CFStringGetCString(
            value,
            bytes.as_mut_ptr(),
            bytes.len() as isize,
            kCFStringEncodingUTF8,
        )
    } == 0
    {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(bytes.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    )
}
fn boolean(value: CFTypeRef) -> bool {
    !value.is_null()
        && unsafe { CFGetTypeID(value) == CFBooleanGetTypeID() && CFBooleanGetValue(value.cast()) }
}
fn number(value: CFTypeRef) -> Option<u64> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFNumberGetTypeID() } {
        return None;
    }
    let mut number = 0i64;
    let ok = unsafe {
        CFNumberGetValue(
            value.cast(),
            kCFNumberSInt64Type,
            (&mut number as *mut i64).cast(),
        )
    };
    if ok { u64::try_from(number).ok() } else { None }
}
fn property(entry: u32, key: &str) -> Cf {
    let key = cf_string(key);
    Cf(unsafe { IORegistryEntryCreateCFProperty(entry, key.0.cast(), ptr::null(), 0) })
}
fn field(dictionary: CFDictionaryRef, key: &str) -> CFTypeRef {
    if dictionary.is_null() {
        return ptr::null();
    }
    let key = cf_string(key);
    unsafe { CFDictionaryGetValue(dictionary, key.0) }
}
fn valid_id(id: &str) -> bool {
    id.strip_prefix("disk").is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix
                .split('s')
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    })
}
fn registry_id(object: u32) -> Option<u64> {
    let mut id = 0;
    (unsafe { IORegistryEntryGetRegistryEntryID(object, &mut id) } == 0).then_some(id)
}
fn ancestors(object: u32) -> Vec<u64> {
    let mut ids = Vec::new();
    let mut current = object;
    let mut owned = None;
    for _ in 0..128 {
        let mut parent = 0;
        if unsafe { IORegistryEntryGetParentEntry(current, c"IOService".as_ptr(), &mut parent) }
            != 0
        {
            break;
        }
        let next = Object(parent);
        if let Some(id) = registry_id(parent) {
            ids.push(id);
        }
        current = parent;
        owned = Some(next);
    }
    drop(owned);
    ids
}

pub fn discover() -> Result<Vec<Drive>, String> {
    // Own the mount snapshot; getmntinfo's process-global buffer is unsafe
    // to retain while other desktop libraries may query mounts concurrently.
    let mut capacity = 64usize;
    let mount_entries = loop {
        let mut entries: Vec<libc::statfs> = (0..capacity)
            .map(|_| unsafe { std::mem::zeroed() })
            .collect();
        let count = unsafe {
            libc::getfsstat(
                entries.as_mut_ptr(),
                std::mem::size_of_val(entries.as_slice()) as i32,
                libc::MNT_NOWAIT,
            )
        };
        if count < 0 {
            return Err(format!(
                "Could not inspect Mac drives: {}",
                std::io::Error::last_os_error()
            ));
        }
        if (count as usize) < entries.len() {
            entries.truncate(count as usize);
            break entries;
        }
        capacity *= 2;
        if capacity > 65536 {
            return Err("Mount table changed too quickly. Try Refresh.".into());
        }
    };
    let mounts: HashMap<String, String> = mount_entries
        .iter()
        .map(|mount| {
            let from = unsafe { CStr::from_ptr(mount.f_mntfromname.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            let to = unsafe { CStr::from_ptr(mount.f_mntonname.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            (from, to)
        })
        .collect();
    let matching = unsafe { IOServiceMatching(c"IOMedia".as_ptr()) };
    if matching.is_null() {
        return Err("macOS could not inspect drives. Open a raw disk image instead.".into());
    }
    let mut iterator = 0;
    // IOKit consumes the matching dictionary even on failure.
    if unsafe { IOServiceGetMatchingServices(0, matching, &mut iterator) } != 0 {
        return Err("macOS could not inspect drives. Open a raw disk image instead.".into());
    }
    let iterator = Object(iterator);
    let session = Cf(unsafe { DASessionCreate(ptr::null()) });
    let mut rows = Vec::new();
    let mut topology = Vec::new();
    let mut seen = HashSet::new();
    loop {
        let entry = unsafe { IOIteratorNext(iterator.0) };
        if entry == 0 {
            break;
        }
        let entry = Object(entry);
        let Some(id) = string(property(entry.0, "BSD Name").0).filter(|id| valid_id(id)) else {
            continue;
        };
        let Some(size) = number(property(entry.0, "Size").0).filter(|n| *n > 0) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let bsd_name = CString::new(id.clone()).unwrap();
        let disk = Cf(if session.0.is_null() {
            ptr::null()
        } else {
            unsafe { DADiskCreateFromBSDName(ptr::null(), session.0, bsd_name.as_ptr()) }
        });
        let description = Cf(if disk.0.is_null() {
            ptr::null()
        } else {
            unsafe { DADiskCopyDescription(disk.0) }.cast()
        });
        let dictionary = description.0.cast();
        let name = string(field(dictionary, "DAVolumeName"))
            .or_else(|| string(field(dictionary, "DAMediaName")))
            .unwrap_or_else(|| id.clone());
        let whole = boolean(property(entry.0, "Whole").0);
        let locked =
            boolean(field(dictionary, "DAVolumeLocked")) || boolean(property(entry.0, "Locked").0);
        let path = format!("/dev/{id}");
        let mut mount = mounts.get(&path).cloned();
        if mount.is_none() {
            // APFS snapshots can mount diskNsMsK while their media is diskNsM.
            mount = mounts
                .iter()
                .find(|(source, _)| {
                    source.strip_prefix(&path).is_some_and(|tail| {
                        tail.strip_prefix('s')
                            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                    })
                })
                .map(|(_, point)| point.clone());
        }
        let url = field(dictionary, "DAVolumePath");
        if mount.is_none() && !url.is_null() && unsafe { CFGetTypeID(url) == CFURLGetTypeID() } {
            let value =
                Cf(unsafe { CFURLCopyFileSystemPath(url.cast(), kCFURLPOSIXPathStyle) }.cast());
            mount = string(value.0);
        }
        let in_use = mount.is_some() || boolean(property(entry.0, "Open").0);
        topology.push((registry_id(entry.0), ancestors(entry.0)));
        rows.push(Drive {
            path: path.into(),
            name,
            name_is_message: false,
            size,
            kind: if whole { "Entire drive" } else { "Volume" }.into(),
            location: if locked {
                "Locked: unlock in Disk Utility first".into()
            } else if let Some(point) = mount {
                format!("Mounted at {point}")
            } else if in_use {
                "Contains mounted volumes".into()
            } else {
                "Not mounted".into()
            },
            mounted: in_use,
            encrypted: locked,
            sector_size: 1,
        });
    }
    let active_parents: HashSet<_> = rows
        .iter()
        .zip(&topology)
        .filter(|(row, _)| row.mounted)
        .flat_map(|(_, (_, parents))| parents.iter().copied())
        .collect();
    for (row, (id, _)) in rows.iter_mut().zip(topology) {
        if !row.mounted && id.is_some_and(|id| active_parents.contains(&id)) {
            row.mounted = true;
            row.location = "Contains mounted volumes".into();
        }
    }
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(rows)
}

/// The whole-disk identifier (`disk3`) of a device path such as `/dev/rdisk3s1`.
fn whole_disk(device: &str) -> Option<&str> {
    let id = device.strip_prefix("/dev/")?;
    let id = id.strip_prefix('r').unwrap_or(id);
    if !valid_id(id) {
        return None;
    }
    let digits = id["disk".len()..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    Some(&id[.."disk".len() + digits])
}

pub fn destination(source: &std::path::Path, folder: &std::path::Path) -> Destination {
    use std::os::unix::ffi::OsStrExt;
    let source = source.to_string_lossy();
    // Writing next to a disk image never changes the image's own bytes.
    if !source.starts_with("/dev/") {
        return Destination::Separate;
    }
    let (Some(source), Ok(folder)) = (
        whole_disk(&source),
        CString::new(folder.as_os_str().as_bytes()),
    ) else {
        return Destination::Unknown;
    };
    let mut mount: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: folder is NUL-terminated and mount is a writable statfs.
    if unsafe { libc::statfs(folder.as_ptr(), &mut mount) } != 0 {
        return Destination::Unknown;
    }
    let from = unsafe { CStr::from_ptr(mount.f_mntfromname.as_ptr()) }.to_string_lossy();
    // APFS volumes sit in a synthesized container whose physical store has a
    // different number, so differing identifiers do not prove separation.
    if whole_disk(&from) == Some(source) {
        Destination::SameDrive
    } else {
        Destination::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_bsd_disk_identifiers_are_accepted() {
        for id in ["disk0", "disk3s1", "disk3s1s1"] {
            assert!(valid_id(id));
        }
        for id in ["../../disk0", "disks1", "disk1ss2", "disk1s"] {
            assert!(!valid_id(id));
        }
    }
    #[test]
    fn slices_and_raw_devices_resolve_to_their_whole_disk() {
        assert_eq!(whole_disk("/dev/disk12"), Some("disk12"));
        assert_eq!(whole_disk("/dev/rdisk3s1"), Some("disk3"));
        assert_eq!(whole_disk("/dev/disk3s1s1"), Some("disk3"));
        assert_eq!(whole_disk("/dev/sda"), None);
        assert_eq!(whole_disk("disk3"), None);
    }
}
