//! IOKit/Disk Arbitration discovery; no diskutil or plist conversion process.
use super::{Destination, Drive};
use core_foundation_sys::{
    base::{CFAllocatorRef, CFGetTypeID, CFRelease, CFTypeID, CFTypeRef},
    dictionary::{CFDictionaryGetValue, CFDictionaryRef, CFMutableDictionaryRef},
    number::{
        CFBooleanGetTypeID, CFBooleanGetValue, CFNumberGetTypeID, CFNumberGetValue,
        kCFNumberSInt64Type,
    },
    string::{
        CFStringCreateWithCString, CFStringGetCString, CFStringGetLength,
        CFStringGetMaximumSizeForEncoding, CFStringGetTypeID, CFStringRef, kCFStringEncodingUTF8,
    },
    url::{CFURLCopyFileSystemPath, CFURLGetTypeID, kCFURLPOSIXPathStyle},
    uuid::{CFUUIDBytes, CFUUIDCreateFromString, CFUUIDGetUUIDBytes, CFUUIDRef},
};
use std::{
    collections::{HashMap, HashSet},
    ffi::{CStr, CString},
    ptr,
};

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOCreatePlugInInterfaceForService(
        entry: u32,
        client: CFUUIDRef,
        interface_id: CFUUIDRef,
        plugin: *mut *mut *const ComInterface,
        score: *mut i32,
    ) -> i32;
    fn IODestroyPlugInInterface(plugin: *mut *const ComInterface) -> i32;
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
    fn IORegistryEntryGetParentIterator(
        entry: u32,
        plane: *const std::ffi::c_char,
        iterator: *mut u32,
    ) -> i32;
    fn IOObjectConformsTo(object: u32, class: *const std::ffi::c_char) -> libc::boolean_t;
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
    fn DADiskCopyIOMedia(disk: CFTypeRef) -> u32;
    fn DADiskCopyDescription(disk: CFTypeRef) -> CFDictionaryRef;
}

// Both wrappers own a +1 reference, released on every early-return path.
struct Object(u32);
impl Drop for Object {
    fn drop(&mut self) {
        // SAFETY: self owns exactly one IOKit reference and releases it once.
        unsafe {
            IOObjectRelease(self.0);
        }
    }
}
struct Cf(CFTypeRef);
impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 is a non-null CF object owned by this wrapper.
            unsafe {
                CFRelease(self.0);
            }
        }
    }
}
/// A `CFString` for a registry or dictionary key; null if the key contains NUL.
fn cf_string(text: &str) -> Cf {
    let Ok(value) = CString::new(text) else {
        return Cf(ptr::null());
    };
    Cf(
        // SAFETY: value is a NUL-terminated UTF-8 C string that outlives the call.
        unsafe { CFStringCreateWithCString(ptr::null(), value.as_ptr(), kCFStringEncodingUTF8) }
            .cast(),
    )
}
/// Whether `value` is a non-null CF object of the type `type_id` names.
fn is_type(value: CFTypeRef, type_id: unsafe extern "C" fn() -> CFTypeID) -> bool {
    if value.is_null() {
        return false;
    }
    // SAFETY: the CF `*GetTypeID` functions take no arguments and return a constant.
    let expected = unsafe { type_id() };
    // SAFETY: value is a non-null CF object borrowed for the duration of the call.
    let actual = unsafe { CFGetTypeID(value) };
    actual == expected
}
fn string(value: CFTypeRef) -> Option<String> {
    if !is_type(value, CFStringGetTypeID) {
        return None;
    }
    let value = value.cast();
    // SAFETY: value was checked above to be a CFString.
    let characters = unsafe { CFStringGetLength(value) };
    // SAFETY: a plain size calculation on a character count; no pointers.
    let length = unsafe { CFStringGetMaximumSizeForEncoding(characters, kCFStringEncodingUTF8) };
    if !(0..=1024 * 1024).contains(&length) {
        return None;
    }
    let size = usize::try_from(length).ok()?.saturating_add(1);
    // audit-allow(DOS-ALLOC): length is checked to be at most 1 MiB just above.
    let mut bytes = vec![0i8; size];
    // SAFETY: bytes is writable for exactly bytes.len() bytes, the length passed.
    if unsafe {
        CFStringGetCString(
            value,
            bytes.as_mut_ptr(),
            isize::try_from(bytes.len()).unwrap_or(isize::MAX),
            kCFStringEncodingUTF8,
        )
    } == 0
    {
        return None;
    }
    Some(
        // SAFETY: CFStringGetCString succeeded, so bytes holds a NUL-terminated string.
        unsafe { CStr::from_ptr(bytes.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    )
}
fn boolean(value: CFTypeRef) -> bool {
    is_type(value, CFBooleanGetTypeID)
        // SAFETY: value was checked to be a CFBoolean just before it is read.
        && unsafe { CFBooleanGetValue(value.cast()) }
}
fn number(value: CFTypeRef) -> Option<u64> {
    if !is_type(value, CFNumberGetTypeID) {
        return None;
    }
    let mut number = 0i64;
    // SAFETY: value is a CFNumber (checked above) and number is a valid i64 out-pointer.
    let ok =
        unsafe { CFNumberGetValue(value.cast(), kCFNumberSInt64Type, (&raw mut number).cast()) };
    if ok { u64::try_from(number).ok() } else { None }
}
fn property(entry: u32, key: &str) -> Cf {
    let key = cf_string(key);
    if key.0.is_null() {
        return Cf(ptr::null());
    }
    // SAFETY: entry is a live registry entry and key a valid CFString; Cf owns the result.
    Cf(unsafe { IORegistryEntryCreateCFProperty(entry, key.0.cast(), ptr::null(), 0) })
}
fn field(dictionary: CFDictionaryRef, key: &str) -> CFTypeRef {
    if dictionary.is_null() {
        return ptr::null();
    }
    let key = cf_string(key);
    if key.0.is_null() {
        return ptr::null();
    }
    // SAFETY: dictionary is non-null and key a valid CFString; the value is borrowed, not retained.
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
    // SAFETY: object is a live registry entry and id a valid out-pointer.
    (unsafe { IORegistryEntryGetRegistryEntryID(object, &raw mut id) } == 0).then_some(id)
}
fn bsd_name(object: u32) -> Option<String> {
    // audit-allow(I18N-MISSING): an IOKit registry property name.
    string(property(object, "BSD Name").0).filter(|id| valid_id(id))
}

/// The APFS container scheme has one `IOMedia` parent per physical store.
fn apfs_backing_disk(scheme: u32) -> Option<std::path::PathBuf> {
    let mut iterator = 0;
    // SAFETY: scheme is live, the plane is static, and iterator is an out-pointer.
    if unsafe { IORegistryEntryGetParentIterator(scheme, c"IOService".as_ptr(), &raw mut iterator) }
        != 0
    {
        return None;
    }
    let iterator = Object(iterator);
    let mut backing = None;
    for _ in 0..128 {
        // SAFETY: iterator owns a live parent iterator.
        let parent = unsafe { IOIteratorNext(iterator.0) };
        if parent == 0 {
            return backing.map(Into::into);
        }
        let parent = Object(parent);
        let path = format!("/dev/{}", bsd_name(parent.0)?);
        let disk = format!("/dev/{}", whole_disk(&path)?);
        if backing.as_ref().is_some_and(|other| other != &disk) {
            return None;
        }
        backing = Some(disk);
    }
    None
}

enum Backing {
    Ordinary,
    Apfs(Option<std::path::PathBuf>),
}

fn ancestors(object: u32, inspect_backing: bool) -> (Vec<u64>, Backing) {
    let mut ids = Vec::new();
    let mut current = object;
    let mut owned = None;
    let mut backing = Backing::Ordinary;
    for _ in 0..128 {
        let mut parent = 0;
        // SAFETY: current is a live entry, the plane name is a static C string, parent is an out-pointer.
        if unsafe { IORegistryEntryGetParentEntry(current, c"IOService".as_ptr(), &raw mut parent) }
            != 0
        {
            break;
        }
        let next = Object(parent);
        if inspect_backing && matches!(backing, Backing::Ordinary)
            // SAFETY: parent is live and the class name is static.
            && unsafe { IOObjectConformsTo(parent, c"AppleAPFSContainerScheme".as_ptr()) } != 0
        {
            backing = Backing::Apfs(apfs_backing_disk(parent));
        }
        if let Some(id) = registry_id(parent) {
            ids.push(id);
        }
        current = parent;
        owned = Some(next);
    }
    drop(owned);
    (ids, backing)
}

pub fn discover() -> Result<Vec<Drive>, String> {
    discover_with_topology(false)
}

fn discover_with_topology(inspect_backing: bool) -> Result<Vec<Drive>, String> {
    // Own the mount snapshot; getmntinfo's process-global buffer is unsafe
    // to retain while other desktop libraries may query mounts concurrently.
    let mut capacity = 64usize;
    let mount_entries = loop {
        let mut entries: Vec<libc::statfs> = (0..capacity)
            // audit-allow(SEC-UNINIT): output buffers that getfsstat fills in.
            // SAFETY: statfs is plain old data, for which all zero bytes is a valid value.
            .map(|_| unsafe { std::mem::zeroed() })
            .collect();
        // SAFETY: entries is writable for its full byte size, which is the size passed.
        let count = unsafe {
            libc::getfsstat(
                entries.as_mut_ptr(),
                // At most 65,536 entries of a few KiB, far below i32::MAX; a
                // larger buffer would be reported as smaller, which is safe.
                i32::try_from(size_of_val(entries.as_slice())).unwrap_or(i32::MAX),
                libc::MNT_NOWAIT,
            )
        };
        if count < 0 {
            return Err(format!(
                "Could not inspect Mac drives: {}",
                std::io::Error::last_os_error()
            ));
        }
        // `count` is not negative here, checked just above.
        let count = usize::try_from(count).unwrap_or(0);
        if count < entries.len() {
            entries.truncate(count);
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
            // SAFETY: the kernel fills f_mntfromname with a NUL-terminated name.
            let from = unsafe { CStr::from_ptr(mount.f_mntfromname.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: the kernel fills f_mntonname with a NUL-terminated name.
            let to = unsafe { CStr::from_ptr(mount.f_mntonname.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            (from, to)
        })
        .collect();
    // SAFETY: the class name is a static C string.
    let matching = unsafe { IOServiceMatching(c"IOMedia".as_ptr()) };
    if matching.is_null() {
        return Err("macOS could not inspect drives. Open a raw disk image instead.".into());
    }
    let mut iterator = 0;
    // IOKit consumes the matching dictionary even on failure.
    // SAFETY: matching is a fresh dictionary that IOKit consumes; iterator is an out-pointer.
    if unsafe { IOServiceGetMatchingServices(0, matching, &raw mut iterator) } != 0 {
        return Err("macOS could not inspect drives. Open a raw disk image instead.".into());
    }
    let iterator = Object(iterator);
    // SAFETY: a null allocator selects the default allocator.
    let session = Cf(unsafe { DASessionCreate(ptr::null()) });
    let mut rows = Vec::new();
    let mut topology = Vec::new();
    let mut seen = HashSet::new();
    loop {
        // SAFETY: iterator owns a live IOKit iterator.
        let entry = unsafe { IOIteratorNext(iterator.0) };
        if entry == 0 {
            break;
        }
        let entry = Object(entry);
        let Some(id) = bsd_name(entry.0) else {
            continue;
        };
        let Some(size) = number(property(entry.0, "Size").0).filter(|n| *n > 0) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let Ok(bsd_name) = CString::new(id.clone()) else {
            continue;
        };
        let disk = Cf(if session.0.is_null() {
            ptr::null()
        } else {
            // SAFETY: session is non-null and bsd_name is a NUL-terminated C string.
            unsafe { DADiskCreateFromBSDName(ptr::null(), session.0, bsd_name.as_ptr()) }
        });
        let description = Cf(if disk.0.is_null() {
            ptr::null()
        } else {
            // SAFETY: disk is a non-null DADisk owned by Cf.
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
        if mount.is_none() && is_type(url, CFURLGetTypeID) {
            let value =
                // SAFETY: url was checked to be a CFURL; the copied path is owned by Cf.
                Cf(unsafe { CFURLCopyFileSystemPath(url.cast(), kCFURLPOSIXPathStyle) }.cast());
            mount = string(value.0);
        }
        let in_use = mount.is_some() || boolean(property(entry.0, "Open").0);
        let (ancestors, backing) = ancestors(entry.0, inspect_backing);
        topology.push((registry_id(entry.0), ancestors));
        let physical_whole = whole && matches!(backing, Backing::Ordinary);
        let parent = match backing {
            Backing::Apfs(physical) => physical,
            Backing::Ordinary if inspect_backing && !whole => {
                whole_disk(&path).map(|id| format!("/dev/{id}").into())
            }
            Backing::Ordinary => None,
        };
        // The volume macOS runs from; read before `mount` moves into the location.
        let system = mount.as_deref() == Some("/");
        rows.push(Drive {
            path: path.into(),
            name,
            name_is_message: false,
            size,
            kind: if physical_whole {
                "Entire drive"
            } else {
                "Volume"
            }
            .into(),
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
            // audit-allow(I18N-MISSING): IOKit property key.
            sector_size: number(property(entry.0, "Preferred Block Size").0)
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| n.is_power_of_two() && (512..=65536).contains(n))
                .unwrap_or(1),
            phone: false,
            unlocked: false,
            parent,
            system,
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
    let digits = id
        .strip_prefix("disk")?
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    id.get(.."disk".len() + digits)
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
    // audit-allow(SEC-UNINIT): an output structure that statfs fills in.
    // SAFETY: statfs is plain old data, for which all zero bytes is a valid value.
    let mut mount: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: folder is NUL-terminated and mount is a writable statfs.
    if unsafe { libc::statfs(folder.as_ptr(), &raw mut mount) } != 0 {
        return Destination::Unknown;
    }
    // SAFETY: the kernel fills f_mntfromname with a NUL-terminated name.
    let from = unsafe { CStr::from_ptr(mount.f_mntfromname.as_ptr()) }.to_string_lossy();
    // APFS volumes sit in a synthesized container whose physical store has a
    // different number, so differing identifiers do not prove separation.
    if whole_disk(&from) == Some(source) {
        Destination::SameDrive
    } else {
        Destination::Unknown
    }
}

pub(crate) fn inventory(_: bool) -> Result<Vec<Drive>, String> {
    discover_with_topology(true)
}

// Prefixes of the IOKit C interfaces in IOCFPlugIn.h and NVMeSMARTLibExternal.h.
// QueryInterface selects the exact NVMe interface before its method is called.
#[repr(C)]
struct ComInterface {
    reserved: *mut std::ffi::c_void,
    query:
        unsafe extern "C" fn(*mut std::ffi::c_void, CFUUIDBytes, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "C" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "C" fn(*mut std::ffi::c_void) -> u32,
}

#[repr(C)]
struct NvmeInterface {
    base: ComInterface,
    version: u16,
    revision: u16,
    read_data: unsafe extern "C" fn(*mut std::ffi::c_void, *mut [u8; 512]) -> i32,
}

fn uuid(text: &str) -> Option<Cf> {
    let string = cf_string(text);
    if string.0.is_null() {
        return None;
    }
    // SAFETY: the CFString is live and the null allocator selects the default.
    let value = Cf(unsafe { CFUUIDCreateFromString(ptr::null(), string.0.cast()) }.cast());
    (!value.0.is_null()).then_some(value)
}

fn nvme_log(entry: u32) -> Option<[u8; 512]> {
    let client = uuid("AA0FA6F9-C2D6-457F-B10B-59A13253292F")?;
    let plugin_id = uuid("C244E858-109C-11D4-91D4-0050E4C6426F")?;
    let interface_id = uuid("CCD1DB19-FD9A-4DAF-BF95-12454B230AB6")?;
    let mut plugin = ptr::null_mut();
    let mut score = 0;
    // SAFETY: entry belongs to the live registry walk, both UUIDs are live,
    // and the two output pointers refer to initialized local storage.
    let status = unsafe {
        IOCreatePlugInInterfaceForService(
            entry,
            client.0.cast(),
            plugin_id.0.cast(),
            &raw mut plugin,
            &raw mut score,
        )
    };
    if status != 0 || plugin.is_null() {
        return None;
    }
    // SAFETY: this is the Core Foundation UUID created above.
    let id = unsafe { CFUUIDGetUUIDBytes(interface_id.0.cast()) };
    let mut interface = ptr::null_mut();
    // SAFETY: successful creation returns a live IOCFPlugInInterface** whose
    // first table members are the COM prefix. QueryInterface fills the output.
    let plugin_table = unsafe { *plugin };
    // SAFETY: the live plugin owns its COM table until destruction below.
    let query = unsafe { (*plugin_table).query };
    // SAFETY: plugin is live, id identifies IONVMeSMARTInterface, and interface is writable.
    let status = unsafe { query(plugin.cast(), id, &raw mut interface) };
    let result = if status == 0 && !interface.is_null() {
        let handle = interface.cast::<*const NvmeInterface>();
        // SAFETY: a successful query for this UUID supplies that exact vtable,
        // which remains owned by this function until Release below.
        let table = unsafe { *handle };
        // SAFETY: the queried interface owns a live NVMe method table.
        let methods = unsafe { &*table };
        let mut page = [0; 512];
        // SAFETY: SMARTReadData writes one complete 512-byte NVMe health page.
        let read = unsafe { (methods.read_data)(interface, &raw mut page) };
        // SAFETY: balance the single successful QueryInterface before plugin destruction.
        unsafe { (methods.base.release)(interface) };
        (read == 0).then_some(page)
    } else {
        None
    };
    // SAFETY: plugin is the owned interface from IOCreatePlugInInterfaceForService.
    unsafe { IODestroyPlugInInterface(plugin) };
    result
}

pub(crate) fn health(path: &std::path::Path) -> Option<(String, String)> {
    let name = CString::new(path.file_name()?.to_str()?).ok()?;
    // SAFETY: the default allocator is selected with a null pointer.
    let session = Cf(unsafe { DASessionCreate(ptr::null()) });
    if session.0.is_null() {
        return None;
    }
    // SAFETY: the session is live and name is NUL-terminated.
    let disk = Cf(unsafe { DADiskCreateFromBSDName(ptr::null(), session.0, name.as_ptr()) });
    if disk.0.is_null() {
        return None;
    }
    // SAFETY: disk is live; the copy returns an owned IOMedia entry.
    let media = unsafe { DADiskCopyIOMedia(disk.0) };
    if media == 0 {
        return None;
    }
    let mut entry = Object(media);
    for _ in 0..64 {
        // audit-allow(I18N-MISSING): IOKit property key.
        if boolean(property(entry.0, "NVMe SMART Capable").0)
            && let Some(page) = nvme_log(entry.0)
        {
            return super::nvme_health(&page);
        }
        // audit-allow(I18N-MISSING): IOKit property key.
        if let Some(status) = string(property(entry.0, "SMART Status").0) {
            return Some((
                format!("SMART · {status}"),
                format!("IOKit SMART Status={status}"),
            ));
        }
        let mut parent = 0;
        // SAFETY: entry owns a live registry entry; parent is a writable output.
        if unsafe { IORegistryEntryGetParentEntry(entry.0, c"IOService".as_ptr(), &raw mut parent) }
            != 0
        {
            break;
        }
        entry = Object(parent);
    }
    None
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
        for device in ["/dev/disk", "/dev/diské", "/dev/disk3sé", "/dev/disk٣"] {
            assert_eq!(whole_disk(device), None);
        }
    }
}
