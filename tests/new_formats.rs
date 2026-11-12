#![expect(
    clippy::unwrap_used,
    reason = "fixture helpers fail the test by panicking"
)]
use reclaim::{Event, Found, Validation, recover_reader, scan_reader};
use std::{
    io::{self, Cursor, Read, Seek, SeekFrom},
    sync::atomic::{AtomicBool, Ordering},
};
const FIXTURES: &[(&str, &[u8])] = &[
    ("exe", include_bytes!("fixtures/formats/pe32.exe")),
    ("exe", include_bytes!("fixtures/formats/certificate.exe")),
    ("dll", include_bytes!("fixtures/formats/pe64.dll")),
    ("elf", include_bytes!("fixtures/formats/elf32-le.elf")),
    ("elf", include_bytes!("fixtures/formats/elf32-be.elf")),
    ("elf", include_bytes!("fixtures/formats/elf64-le.elf")),
    ("elf", include_bytes!("fixtures/formats/elf64-be.elf")),
    ("msi", include_bytes!("fixtures/formats/cfb3.msi")),
    ("msp", include_bytes!("fixtures/formats/cfb3.msp")),
    ("mst", include_bytes!("fixtures/formats/cfb3.mst")),
    ("ole", include_bytes!("fixtures/formats/cfb3.ole")),
    ("msi", include_bytes!("fixtures/formats/cfb4.msi")),
    ("msp", include_bytes!("fixtures/formats/cfb4.msp")),
    ("mst", include_bytes!("fixtures/formats/cfb4.mst")),
    ("ole", include_bytes!("fixtures/formats/cfb4.ole")),
    ("7z", include_bytes!("fixtures/formats/empty.7z")),
    ("cab", include_bytes!("fixtures/formats/sample.cab")),
    ("deb", include_bytes!("fixtures/formats/sample.deb")),
    ("rpm", include_bytes!("fixtures/formats/sample.rpm")),
    ("dex", include_bytes!("fixtures/formats/sample.dex")),
    ("gz", include_bytes!("fixtures/formats/sample.gz")),
    ("pkg", include_bytes!("fixtures/formats/sample.pkg")),
    ("xar", include_bytes!("fixtures/formats/sample.xar")),
    ("qoi", include_bytes!("fixtures/formats/sample.qoi")),
    ("rar", include_bytes!("fixtures/formats/sample.rar")),
    ("tar", include_bytes!("fixtures/formats/sample.tar")),
];
fn collect(input: &mut (impl Read + Seek)) -> Vec<Found> {
    let mut found = Vec::new();
    scan_reader(input, &AtomicBool::new(false), |e| {
        if let Event::Found(f) = e {
            found.push(f)
        }
    })
    .unwrap();
    found
}
#[test]
fn all_added_formats_carve_across_blocks_and_export_exact_bytes() {
    let dir = tempfile::tempdir().unwrap();
    for (index, (kind, bytes)) in FIXTURES.iter().enumerate() {
        // TAR's marker is at +257; this also puts that marker across the boundary.
        let prefix = 1024 * 1024 - 260;
        let disk = [vec![0; prefix], bytes.to_vec(), vec![0; 320]].concat();
        let mut input = Cursor::new(&disk);
        let files = collect(&mut input);
        let matched: Vec<_> = files
            .iter()
            .filter(|f| f.offset == prefix as u64 && f.kind == *kind)
            .collect();
        assert_eq!(matched.len(), 1, "{kind}: {files:?}");
        let found = matched[0];
        assert_eq!(found.size, bytes.len() as u64, "{kind}");
        let expected = if matches!(
            *kind,
            "exe" | "dll" | "elf" | "msi" | "msp" | "mst" | "ole" | "rpm" | "dex" | "xar" | "pkg"
        ) {
            Validation::Unverified
        } else {
            Validation::StructureChecked
        };
        assert_eq!(found.validation, expected, "{kind}");
        let output = dir.path().join(format!("{index}.{kind}"));
        recover_reader(&mut input, &output, found).unwrap();
        assert_eq!(std::fs::read(output).unwrap(), *bytes, "{kind}");
    }
}
#[test]
fn truncated_candidates_and_corrupt_metadata_do_not_become_results() {
    for (kind, bytes) in FIXTURES {
        let files = collect(&mut Cursor::new(&bytes[..bytes.len() - 1]));
        assert!(
            !files.iter().any(|f| f.offset == 0 && f.kind == *kind),
            "truncated {kind}: {files:?}"
        );
    }
    let cases: &[(&str, usize)] = &[
        ("exe", 60),
        ("elf", 4),
        ("msi", 30),
        ("7z", 8),
        ("cab", 8),
        ("deb", 66),
        ("rpm", 108),
        ("dex", 32),
        ("gz", 3),
        ("pkg", 8),
        ("qoi", 4),
        ("rar", 8),
        ("tar", 148),
    ];
    for (kind, at) in cases {
        let (_, fixture) = FIXTURES.iter().find(|(k, _)| k == kind).unwrap();
        let mut bad = fixture.to_vec();
        bad[*at] ^= 0x80;
        let files = collect(&mut Cursor::new(bad));
        assert!(
            !files.iter().any(|f| f.offset == 0 && f.kind == *kind),
            "corrupt {kind}: {files:?}"
        );
    }
}
struct Short(Cursor<Vec<u8>>);
impl Read for Short {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let n = b.len().min(3);
        self.0.read(&mut b[..n])
    }
}
impl Seek for Short {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.0.seek(p)
    }
}
#[test]
fn short_device_reads_and_cancellation_are_supported() {
    for (kind, bytes) in FIXTURES {
        let files = collect(&mut Short(Cursor::new(bytes.to_vec())));
        assert!(
            files.iter().any(|f| f.offset == 0 && f.kind == *kind),
            "{kind}"
        );
    }
    let cancel = AtomicBool::new(false);
    let mut found = 0;
    scan_reader(&mut Cursor::new(FIXTURES[0].1), &cancel, |e| match e {
        Event::Checking { .. } => cancel.store(true, Ordering::Relaxed),
        Event::Found(_) => found += 1,
        _ => {}
    })
    .unwrap();
    assert_eq!(found, 0);
}
#[test]
fn altered_executables_are_rechecked_before_export() {
    let mut bytes = FIXTURES[0].1.to_vec();
    let found = collect(&mut Cursor::new(&bytes)).remove(0);
    bytes[60..64].fill(0xff);
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("bad.exe");
    assert!(recover_reader(&mut Cursor::new(bytes), &output, &found).is_err());
    assert!(!output.exists());
}

#[test]
fn varied_header_mutations_never_panic_or_report_out_of_bounds_files() {
    let mut state = 0x3141592653589793u64;
    for (_, fixture) in FIXTURES {
        for _ in 0..48 {
            let mut bytes = fixture.to_vec();
            for _ in 0..4 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let at = usize::try_from(state % u64::try_from(bytes.len()).unwrap()).unwrap();
                // Take the low byte of the high half on purpose (pseudo-random flip).
                bytes[at] ^= (state >> 32).to_le_bytes()[0] | 1;
            }
            let found = collect(&mut Cursor::new(&bytes));
            for file in found {
                assert!(file.size > 0);
                assert!(file.offset + file.size <= bytes.len() as u64);
            }
        }
    }
}

#[test]
fn gzip_crc_and_xar_xml_are_checked_and_archive_members_do_not_consume_neighbours() {
    let gz = include_bytes!("fixtures/formats/sample.gz");
    let mut bad = gz.to_vec();
    let crc = bad.len() - 8;
    bad[crc] ^= 1;
    assert!(collect(&mut Cursor::new(bad)).is_empty());
    let bytes = [gz.as_slice(), gz.as_slice()].concat();
    let found = collect(&mut Cursor::new(bytes));
    assert_eq!(
        found.iter().map(|f| (f.offset, f.size)).collect::<Vec<_>>(),
        vec![(0, gz.len() as u64), (gz.len() as u64, gz.len() as u64)]
    );
    let plain = b'X';
    let xml = b"<!DOCTYPE xar [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><xar><toc/></xar>";
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(xml).unwrap();
    let toc = encoder.finish().unwrap();
    let mut xar = b"xar!".to_vec();
    xar.extend_from_slice(&28u16.to_be_bytes());
    xar.extend_from_slice(&1u16.to_be_bytes());
    xar.extend_from_slice(&(toc.len() as u64).to_be_bytes());
    xar.extend_from_slice(&(xml.len() as u64).to_be_bytes());
    xar.extend_from_slice(&0u32.to_be_bytes());
    xar.extend_from_slice(&toc);
    xar.push(plain);
    assert!(
        !collect(&mut Cursor::new(xar))
            .iter()
            .any(|f| matches!(f.kind, "xar" | "pkg"))
    );
}

#[test]
fn common_mz_noise_is_rejected_before_candidate_disk_reads() {
    struct Counted {
        input: Cursor<Vec<u8>>,
        read: usize,
    }
    impl Read for Counted {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            let n = self.input.read(b)?;
            self.read += n;
            Ok(n)
        }
    }
    impl Seek for Counted {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            self.input.seek(p)
        }
    }
    let mut bytes = vec![0xe7; 2 * 1024 * 1024];
    for at in (0..bytes.len() - 64).step_by(512) {
        bytes[at..at + 2].copy_from_slice(b"MZ");
    }
    let length = bytes.len();
    let mut input = Counted {
        input: Cursor::new(bytes),
        read: 0,
    };
    assert!(collect(&mut input).is_empty());
    assert!(
        input.read <= length + 1024,
        "MZ noise triggered excessive lookahead: {}",
        input.read
    );
}
