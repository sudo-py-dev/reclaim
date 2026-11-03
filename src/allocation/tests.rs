use super::*;
use std::io::Cursor;
fn put16(b: &mut [u8], at: usize, n: u16) {
    b[at..at + 2].copy_from_slice(&n.to_le_bytes());
}
fn put32(b: &mut [u8], at: usize, n: u32) {
    b[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn put64(b: &mut [u8], at: usize, n: u64) {
    b[at..at + 8].copy_from_slice(&n.to_le_bytes());
}
fn fat12_entry(b: &mut [u8], table: usize, cluster: usize, value: u16) {
    let at = table + cluster + cluster / 2;
    let previous = u16::from_le_bytes(b[at..at + 2].try_into().unwrap());
    put16(
        b,
        at,
        if cluster.is_multiple_of(2) {
            previous & 0xf000 | value & 0xfff
        } else {
            previous & 15 | (value & 0xfff) << 4
        },
    );
}
fn fat12() -> (Vec<u8>, usize) {
    let mut b = vec![0; 2 * 1024 * 1024];
    b[0..3].copy_from_slice(b"\xeb\x3c\x90");
    put16(&mut b, 11, 512);
    b[13] = 1;
    put16(&mut b, 14, 1);
    b[16] = 2;
    put16(&mut b, 17, 16);
    put16(&mut b, 19, 4096);
    b[21] = 0xf8;
    put16(&mut b, 22, 12);
    b[510..512].copy_from_slice(b"\x55\xaa");
    for table in [512, 13 * 512] {
        b[table..table + 12 * 512].fill(0xff);
        fat12_entry(&mut b, table, 0, 0xff8);
        for cluster in [3, 4, 6, 7] {
            fat12_entry(&mut b, table, cluster, 0);
        }
    }
    (b, 26 * 512)
}
fn record(number: u32, attribute: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 1024];
    b[..4].copy_from_slice(b"FILE");
    put16(&mut b, 4, 48);
    put16(&mut b, 6, 3);
    put16(&mut b, 16, 1);
    put16(&mut b, 20, 56);
    put16(&mut b, 22, 1);
    put32(&mut b, 24, (56 + attribute.len() + 4) as u32);
    put32(&mut b, 28, 1024);
    put32(&mut b, 44, number);
    b[56..56 + attribute.len()].copy_from_slice(attribute);
    put32(&mut b, 56 + attribute.len(), u32::MAX);
    put16(&mut b, 48, 0xbeef);
    for (i, tail) in [510, 1022].into_iter().enumerate() {
        let pair = [b[tail], b[tail + 1]];
        b[50 + i * 2..52 + i * 2].copy_from_slice(&pair);
        put16(&mut b, tail, 0xbeef);
    }
    b
}
fn nonresident(runs: &[u8], clusters: u64, size: u64) -> Vec<u8> {
    let mut b = vec![0; (64 + runs.len()).next_multiple_of(8)];
    let len = b.len();
    put32(&mut b, 0, 0x80);
    put32(&mut b, 4, len as u32);
    b[8] = 1;
    put64(&mut b, 24, clusters - 1);
    put16(&mut b, 32, 64);
    put64(&mut b, 40, clusters * 512);
    put64(&mut b, 48, size);
    put64(&mut b, 56, size);
    b[64..64 + runs.len()].copy_from_slice(runs);
    b
}
fn ntfs(fragmented: bool, resident: bool) -> Vec<u8> {
    let mut b = vec![0; 1024 * 1024];
    b[3..11].copy_from_slice(b"NTFS    ");
    put16(&mut b, 11, 512);
    b[13] = 1;
    put64(&mut b, 40, 2048);
    put64(&mut b, 48, 8);
    b[64] = (-10i8) as u8;
    b[510..512].copy_from_slice(b"\x55\xaa");
    let runs = if fragmented {
        vec![0x11, 8, 8, 0x11, 24, 24, 0]
    } else {
        vec![0x11, 32, 8, 0]
    };
    let mft = record(0, &nonresident(&runs, 32, 16384));
    b[4096..5120].copy_from_slice(&mft);
    let mut bitmap = vec![0xff; 256];
    for cluster in 100..104 {
        bitmap[cluster / 8] &= !(1 << (cluster % 8));
    }
    let attr = if resident {
        let mut attr = vec![0; 280];
        put32(&mut attr, 0, 0x80);
        put32(&mut attr, 4, 280);
        put32(&mut attr, 16, 256);
        put16(&mut attr, 20, 24);
        attr[24..].copy_from_slice(&bitmap);
        attr
    } else {
        b[80 * 512..80 * 512 + 256].copy_from_slice(&bitmap);
        nonresident(&[0x11, 1, 80, 0], 1, 256)
    };
    let at = if fragmented {
        36 * 512
    } else {
        8 * 512 + 6 * 1024
    };
    b[at..at + 1024].copy_from_slice(&record(6, &attr));
    b
}
fn exfat() -> Vec<u8> {
    let mut b = vec![0; 1024 * 1024];
    b[3..11].copy_from_slice(b"EXFAT   ");
    put64(&mut b, 72, 2048);
    put32(&mut b, 80, 24);
    put32(&mut b, 84, 16);
    put32(&mut b, 88, 64);
    put32(&mut b, 92, 1984);
    put32(&mut b, 96, 2);
    put16(&mut b, 104, 0x100);
    b[108] = 9;
    b[110] = 1;
    b[510..512].copy_from_slice(b"\x55\xaa");
    put32(&mut b, 24 * 512 + 8, u32::MAX);
    put32(&mut b, 24 * 512 + 12, u32::MAX);
    let root = 64 * 512;
    b[root] = 0x81;
    put32(&mut b, root + 20, 3);
    put64(&mut b, root + 24, 248);
    b[65 * 512..65 * 512 + 248].fill(0xff);
    for cluster in 100..104 {
        let bit = cluster - 2;
        b[65 * 512 + bit / 8] &= !(1 << (bit % 8));
    }
    let mut sum = 0u32;
    for (i, byte) in b[..11 * 512].iter().enumerate() {
        if !matches!(i, 106 | 107 | 112) {
            sum = sum.rotate_right(1).wrapping_add(u32::from(*byte));
        }
    }
    for at in (11 * 512..12 * 512).step_by(4) {
        put32(&mut b, at, sum);
    }
    b
}
fn run(b: &[u8]) -> (Vec<crate::Found>, u64, u64) {
    let mut files = Vec::new();
    let mut free = 0;
    let mut progress = 0;
    scan(
        &mut Cursor::new(b),
        b.len() as u64,
        &AtomicBool::new(false),
        |event| match event {
            crate::Event::Found(f) => files.push(f),
            crate::Event::UnusedSpace { bytes, .. } => free = bytes,
            crate::Event::Progress(n) => {
                assert!(n >= progress);
                assert!(n <= free);
                progress = n;
            }
            _ => {}
        },
    )
    .unwrap();
    (files, free, progress)
}
#[test]
fn occupied_files_are_not_read_or_found_and_free_offsets_export_correctly() {
    let (mut b, heap) = fat12();
    let occupied = b"%PDF-1.7\noccupied\n%%EOF";
    let free = b"%PDF-1.7\ndeleted\n%%EOF";
    b[heap..heap + occupied.len()].copy_from_slice(occupied);
    b[heap + 512..heap + 512 + free.len()].copy_from_slice(free);
    struct Audit {
        input: Cursor<Vec<u8>>,
        forbidden: (u64, u64),
    }
    impl Read for Audit {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let at = self.input.position();
            let n = self.input.read(bytes)?;
            assert!(
                at + n as u64 <= self.forbidden.0 || at >= self.forbidden.1,
                "occupied file was read for carving"
            );
            Ok(n)
        }
    }
    impl Seek for Audit {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            self.input.seek(from)
        }
    }
    let mut input = Audit {
        input: Cursor::new(b.clone()),
        forbidden: (heap as u64, heap as u64 + 512),
    };
    let mut files = Vec::new();
    scan(&mut input, b.len() as u64, &AtomicBool::new(false), |e| {
        if let crate::Event::Found(f) = e {
            files.push(f)
        }
    })
    .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].offset, (heap + 512) as u64);
    assert_eq!(files[0].size, free.len() as u64);
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("free.pdf");
    crate::recover_reader(&mut Cursor::new(&b), &output, &files[0]).unwrap();
    assert_eq!(std::fs::read(output).unwrap(), free);
    let (_, unused, done) = run(&b);
    assert_eq!(unused, 2048);
    assert_eq!(done, unused);
}
#[test]
fn a_candidate_cannot_bridge_an_occupied_cluster() {
    let (mut b, heap) = fat12();
    b[heap + 512..heap + 512 + 9].copy_from_slice(b"%PDF-1.7\n");
    b[heap + 4 * 512..heap + 4 * 512 + 5].copy_from_slice(b"%%EOF");
    assert!(run(&b).0.is_empty());
}
#[test]
fn mirrored_fat_disagreement_is_treated_as_occupied() {
    let (mut b, heap) = fat12();
    fat12_entry(&mut b, 13 * 512, 3, 0xfff);
    let map = plan(
        &mut Cursor::new(&b),
        b.len() as u64,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(map.unused, 1536);
    assert!(
        !map.ranges
            .iter()
            .any(|(start, _)| *start == (heap + 512) as u64)
    );
}
#[test]
fn ntfs_resident_nonresident_and_fragmented_mft_maps_exclude_occupied_data() {
    for fragmented in [false, true] {
        for resident in [false, true] {
            let mut b = ntfs(fragmented, resident);
            let pdf = b"%PDF-1.7\nfree NTFS\n%%EOF";
            b[100 * 512..100 * 512 + pdf.len()].copy_from_slice(pdf);
            b[120 * 512..120 * 512 + pdf.len()].copy_from_slice(pdf);
            let (files, unused, done) = run(&b);
            assert_eq!(unused, 2048);
            assert_eq!(done, 2048);
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].offset, 100 * 512);
        }
    }
}
#[test]
fn exfat_bitmap_is_authoritative_and_metadata_cannot_be_marked_free() {
    let mut b = exfat();
    let pdf = b"%PDF-1.7\nfree exFAT\n%%EOF";
    let at = (64 + 100 - 2) * 512;
    b[at..at + pdf.len()].copy_from_slice(pdf);
    b[at + 10 * 512..at + 10 * 512 + pdf.len()].copy_from_slice(pdf);
    let (files, unused, done) = run(&b);
    assert_eq!((unused, done), (2048, 2048));
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].offset, at as u64);
    b[65 * 512] &= !1;
    assert!(
        plan(
            &mut Cursor::new(&b),
            b.len() as u64,
            &AtomicBool::new(false)
        )
        .is_err()
    );
}
#[test]
fn unsupported_and_corrupt_maps_never_fall_back_to_full_disk_scanning() {
    let b = b"%PDF-1.7\nshould not scan\n%%EOF".repeat(40);
    let mut events = 0;
    assert!(
        scan(
            &mut Cursor::new(&b),
            b.len() as u64,
            &AtomicBool::new(false),
            |_| events += 1
        )
        .is_err()
    );
    assert_eq!(events, 0);
    let mut b = ntfs(false, false);
    b[80 * 512 + 1] = 0;
    assert!(
        scan(
            &mut Cursor::new(&b),
            b.len() as u64,
            &AtomicBool::new(false),
            |_| events += 1
        )
        .is_err()
    );
    assert_eq!(events, 0);
    let cancel = AtomicBool::new(true);
    assert!(
        scan(&mut Cursor::new(&b), b.len() as u64, &cancel, |_| events +=
            1)
        .is_ok()
    );
    assert_eq!(events, 0);
}
#[test]
fn malformed_boot_geometry_does_not_panic() {
    for mut b in [fat12().0, ntfs(false, false), exfat()] {
        for at in [
            11, 13, 14, 16, 19, 22, 40, 48, 64, 72, 80, 84, 88, 92, 96, 108, 109,
        ] {
            let old = b[at];
            b[at] = 255;
            let _ = plan(
                &mut Cursor::new(&b),
                b.len() as u64,
                &AtomicBool::new(false),
            );
            b[at] = old;
        }
    }
}

#[test]
fn media_faults_skip_free_data_but_never_fabricate_an_allocation_map() {
    struct Fault {
        input: Cursor<Vec<u8>>,
        bad: std::ops::Range<u64>,
    }
    impl Read for Fault {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let at = self.input.position();
            if at < self.bad.end && self.bad.start < at + bytes.len() as u64 {
                return Err(io::Error::from_raw_os_error(if cfg!(windows) {
                    1117
                } else {
                    5
                }));
            }
            self.input.read(bytes)
        }
    }
    impl Seek for Fault {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            self.input.seek(from)
        }
    }
    let (mut bytes, start) = fat12();
    let pdf = b"%PDF-1.7\nhello\n%%EOF";
    // Two free extents: damage the first, retain the file in the second.
    let bad_start = start + 512;
    let good_start = start + 4 * 512;
    bytes[good_start..good_start + pdf.len()].copy_from_slice(pdf);
    let total = bytes.len() as u64;
    let cancel = AtomicBool::new(false);
    let mut fault = Fault {
        input: Cursor::new(bytes.clone()),
        bad: bad_start as u64..(bad_start + 512) as u64,
    };
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    scan(&mut fault, total, &cancel, |e| match e {
        crate::Event::Found(f) => found.push(f.offset),
        crate::Event::Unreadable { offset, bytes } => skipped.push((offset, bytes)),
        _ => {}
    })
    .unwrap();
    assert_eq!(found, vec![good_start as u64]);
    assert!(!skipped.is_empty());
    let mut fault = Fault {
        input: Cursor::new(bytes),
        bad: 512..1024,
    };
    let mut mapped = false;
    assert!(
        scan(&mut fault, total, &cancel, |e| {
            if matches!(e, crate::Event::UnusedSpace { .. }) {
                mapped = true;
            }
        })
        .is_err()
    );
    assert!(!mapped);
}
