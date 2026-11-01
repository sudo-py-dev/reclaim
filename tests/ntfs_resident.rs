use reclaim::{Event, Found, Validation, recover_reader, recover_unique, scan_reader};
use std::{
    io::{self, Cursor, Read, Seek, SeekFrom},
    sync::atomic::AtomicBool,
};
fn put16(bytes: &mut [u8], at: usize, n: u16) {
    bytes[at..at + 2].copy_from_slice(&n.to_le_bytes());
}
fn put32(bytes: &mut [u8], at: usize, n: u32) {
    bytes[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn attribute(record: &mut [u8], at: &mut usize, kind: u32, value: &[u8]) -> usize {
    let start = *at;
    let length = (24 + value.len()).next_multiple_of(8);
    put32(record, start, kind);
    put32(record, start + 4, length as u32);
    put32(record, start + 16, value.len() as u32);
    put16(record, start + 20, 24);
    record[start + 24..start + 24 + value.len()].copy_from_slice(value);
    *at += length;
    start
}
fn record(name: &str, contents: &[u8], flags: u16, external: bool) -> Vec<u8> {
    let mut bytes = vec![0; 1024];
    bytes[..4].copy_from_slice(b"FILE");
    put16(&mut bytes, 4, 48);
    put16(&mut bytes, 6, 3);
    put16(&mut bytes, 16, 7);
    put16(&mut bytes, 20, 64);
    put16(&mut bytes, 22, flags);
    put32(&mut bytes, 28, 1024);
    put32(&mut bytes, 44, 64);
    let mut at = 64;
    attribute(&mut bytes, &mut at, 0x10, &[0; 48]);
    let utf16: Vec<_> = name.encode_utf16().collect();
    let mut filename = vec![0; 66 + utf16.len() * 2];
    filename[64] = utf16.len() as u8;
    filename[65] = 1;
    for (i, c) in utf16.iter().enumerate() {
        put16(&mut filename, 66 + i * 2, *c);
    }
    attribute(&mut bytes, &mut at, 0x30, &filename);
    let data_at = attribute(&mut bytes, &mut at, 0x80, contents);
    if external {
        bytes[data_at + 8] = 1;
    }
    put32(&mut bytes, at, u32::MAX);
    put32(&mut bytes, 24, (at + 4) as u32);
    put16(&mut bytes, 48, 0xbeef);
    for (i, tail) in [510, 1022].into_iter().enumerate() {
        let original = [bytes[tail], bytes[tail + 1]];
        bytes[50 + i * 2..52 + i * 2].copy_from_slice(&original);
        put16(&mut bytes, tail, 0xbeef);
    }
    bytes
}
fn scan(bytes: &[u8]) -> Vec<Found> {
    let mut found = vec![];
    scan_reader(&mut Cursor::new(bytes), &AtomicBool::new(false), |e| {
        if let Event::Found(f) = e {
            found.push(f);
        }
    })
    .unwrap();
    found
}
struct Unreadable;
impl Read for Unreadable {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("Resident exports must not read the raw source");
    }
}
impl Seek for Unreadable {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        panic!("Resident exports must not seek the raw source");
    }
}

#[test]
fn original_unicode_name_and_sector_fixups_recover_exactly() {
    let contents = b"resident content crossing sector tails\n".repeat(12);
    let raw = record("r\u{e9}sum\u{e9}.txt", &contents, 0, false);
    let mut image = vec![0; 1024 * 1024];
    image.extend_from_slice(&raw);
    let found = scan(&image);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].original_name.as_deref(),
        Some("r\u{e9}sum\u{e9}.txt")
    );
    assert_eq!(found[0].recovery_name(123), "r\u{e9}sum\u{e9}.txt");
    assert_eq!(found[0].validation, Validation::MetadataChecked);
    assert_eq!(found[0].resident_data.as_deref(), Some(contents.as_slice()));
    let dir = std::env::temp_dir().join(format!("reclaim-ntfs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let one = recover_unique(&mut Unreadable, &dir, &found[0], 1).unwrap();
    let two = recover_unique(&mut Unreadable, &dir, &found[0], 1).unwrap();
    assert_eq!(
        two.file_name().unwrap().to_string_lossy(),
        "r\u{e9}sum\u{e9} (2).txt"
    );
    assert_eq!(std::fs::read(&one).unwrap(), contents);
    assert_eq!(std::fs::read(&two).unwrap(), contents);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejects_active_directory_external_torn_truncated_and_path_like_records() {
    for raw in [
        record("active.txt", b"data", 1, false),
        record("folder", b"data", 2, false),
        record("external.txt", b"data", 0, true),
        record("../escape.txt", b"data", 0, false),
    ] {
        assert!(scan(&raw).is_empty());
    }
    let mut torn = record("torn.txt", b"data", 0, false);
    torn[510] ^= 1;
    assert!(scan(&torn).is_empty());
    let valid = record("short.txt", b"data", 0, false);
    assert!(scan(&valid[..900]).is_empty());
}

#[test]
fn resident_images_are_validated_and_dont_become_duplicate_signature_results() {
    let png = include_bytes!("fixtures/real.png");
    let found = scan(&record("photo.png", png, 0, false));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].validation, Validation::ImageDecoded);
    assert_eq!(found[0].recovery_name(1), "photo.png");
    let mut bad = png.to_vec();
    bad[29] ^= 1;
    assert!(scan(&record("broken.png", &bad, 0, false)).is_empty());
}

#[test]
fn empty_files_and_portable_original_names() {
    let found = scan(&record("CON.txt", b"", 0, false));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].recovery_name(1), "_CON.txt");
    let path =
        std::env::temp_dir().join(format!("reclaim-empty-resident-{}.txt", std::process::id()));
    recover_reader(&mut Unreadable, &path, &found[0]).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
    std::fs::remove_file(path).unwrap();
}
