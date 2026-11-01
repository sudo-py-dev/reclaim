use reclaim::{Found, Validation, recover_reader, recover_unique};
use std::io::{self, Cursor};

fn candidate() -> Found {
    Found {
        offset: 0,
        size: 4,
        kind: "bin",
        selected: true,
        validation: Validation::Unverified,
        original_name: None,
        resident_data: None,
        ntfs: None,
    }
}

#[test]
fn failed_exports_never_publish_partial_or_invalid_files() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("recovered");
    let mut found = candidate();
    assert_eq!(
        recover_reader(&mut Cursor::new(b"ab"), &destination, &found)
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
    found.validation = Validation::ImageDecoded;
    found.kind = "png";
    assert!(recover_reader(&mut Cursor::new(b"abcd"), &destination, &found).is_err());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn exports_preserve_symlink_targets_and_are_private() {
    use std::os::unix::{fs::PermissionsExt, fs::symlink};
    let directory = tempfile::tempdir().unwrap();
    let victim = directory.path().join("victim");
    let destination = directory.path().join("recovered");
    std::fs::write(&victim, b"keep").unwrap();
    symlink(&victim, &destination).unwrap();
    assert_eq!(
        recover_reader(&mut Cursor::new(b"data"), &destination, &candidate())
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
    assert!(destination.is_symlink());
    let output =
        recover_unique(&mut Cursor::new(b"data"), directory.path(), &candidate(), 1).unwrap();
    assert_eq!(
        std::fs::metadata(output).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn public_candidate_names_cannot_escape_export_directory_or_spoof_direction() {
    let directory = tempfile::tempdir().unwrap();
    let mut found = candidate();
    for name in [
        "../../outside",
        "..\\outside",
        "report\u{202e}exe.pdf",
        "\u{2067}hidden",
    ] {
        found.original_name = Some(name.into());
        let output =
            recover_unique(&mut Cursor::new(b"data"), directory.path(), &found, 1).unwrap();
        assert_eq!(output.parent(), Some(directory.path()));
        assert!(!output.to_string_lossy().contains(['\u{202e}', '\u{2067}']));
    }
    found.original_name = None;
    found.kind = "/../../outside";
    let output = recover_unique(&mut Cursor::new(b"data"), directory.path(), &found, 1).unwrap();
    assert_eq!(output.parent(), Some(directory.path()));
}

#[test]
fn source_reader_preserves_bytes_and_checks_seek_bounds() {
    use std::io::{Read, Seek, SeekFrom};
    let data: Vec<u8> = (0..2048).map(|n| (n % 251) as u8).collect();
    for alignment in [1, 512, 4096] {
        let mut source =
            reclaim::SourceReader::new(Cursor::new(&data), alignment, Some(data.len() as u64))
                .unwrap();
        source.seek(SeekFrom::Start(511)).unwrap();
        let mut buffer = [0; 7];
        source.read_exact(&mut buffer).unwrap();
        assert_eq!(buffer, data[511..518]);
        assert!(source.seek(SeekFrom::Current(-519)).is_err());
        assert_eq!(source.stream_position().unwrap(), 518);
        source.seek(SeekFrom::End(-7)).unwrap();
        source.read_exact(&mut buffer).unwrap();
        assert_eq!(buffer, data[2041..]);
        assert_eq!(source.read(&mut buffer).unwrap(), 0);
        source.seek(SeekFrom::Start(u64::MAX)).unwrap();
        assert!(source.seek(SeekFrom::Current(1)).is_err());
        assert_eq!(source.read(&mut buffer).unwrap(), 0);
    }
}
