use reclaim::{Event, Validation, recover_reader, scan_reader};
use std::{
    io::{Seek, SeekFrom, Write},
    sync::atomic::AtomicBool,
};

#[test]
fn large_wav_is_exported_and_truncated_or_changed_candidates_are_rejected() {
    let mut disk = tempfile::tempfile().unwrap();
    let payload = 65 * 1024 * 1024u32;
    let size = u64::from(payload) + 44;
    let offset = 512;
    disk.seek(SeekFrom::Start(offset)).unwrap();
    let mut header = Vec::new();
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(payload + 36).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    header.extend_from_slice(&8000u32.to_le_bytes());
    header.extend_from_slice(&8000u32.to_le_bytes());
    header.extend_from_slice(b"\x01\0\x08\0data");
    header.extend_from_slice(&payload.to_le_bytes());
    disk.write_all(&header).unwrap();

    let scan = |disk: &mut std::fs::File| {
        let mut results = Vec::new();
        scan_reader(disk, &AtomicBool::new(false), |event| {
            if let Event::Found(file) = event
                && file.kind == "wav"
            {
                results.push(file);
            }
        })
        .unwrap();
        results
    };
    disk.set_len(offset + size - 1).unwrap();
    assert!(scan(&mut disk).is_empty());
    disk.set_len(offset + size).unwrap();
    let files = scan(&mut disk);
    assert_eq!(files.len(), 1);
    let found = &files[0];
    assert_eq!(
        (found.offset, found.size, found.validation),
        (offset, size, Validation::Unverified)
    );

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("audio.wav");
    recover_reader(&mut disk, &output, found).unwrap();
    assert_eq!(std::fs::metadata(output).unwrap().len(), size);

    disk.seek(SeekFrom::Start(offset + 4)).unwrap();
    disk.write_all(&(payload + 34).to_le_bytes()).unwrap();
    let output = dir.path().join("changed.wav");
    assert!(recover_reader(&mut disk, &output, found).is_err());
    assert!(!output.exists());

    let mut fragmented = found.clone();
    fragmented.extents = Some(vec![(offset, 512), (offset + 512, size - 512)].into());
    let output = dir.path().join("changed-with-extents.wav");
    assert!(recover_reader(&mut disk, &output, &fragmented).is_err());
    assert!(!output.exists());
}

#[test]
fn large_tar_uses_entry_lengths_and_preserves_the_following_file() {
    let mut disk = tempfile::tempfile().unwrap();
    let payload = 65 * 1024 * 1024u64;
    let size = payload + 1536;
    let mut header = [0; 512];
    header[..5].copy_from_slice(b"a.bin");
    header[124..135].copy_from_slice(format!("{payload:011o}").as_bytes());
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    let checksum = header.iter().map(|&byte| u32::from(byte)).sum::<u32>();
    header[148..154].copy_from_slice(format!("{checksum:06o}").as_bytes());
    header[154] = 0;
    disk.write_all(&header).unwrap();
    disk.seek(SeekFrom::Start(size)).unwrap();
    disk.write_all(include_bytes!("fixtures/plain.zip"))
        .unwrap();

    let mut found = Vec::new();
    scan_reader(&mut disk, &AtomicBool::new(false), |event| {
        if let Event::Found(file) = event {
            found.push(file);
        }
    })
    .unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(
        (found[0].kind, found[0].size, found[0].validation),
        ("tar", size, Validation::StructureChecked)
    );
    assert_eq!(found[1].offset, size);

    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("archive.tar");
    recover_reader(&mut disk, &output, &found[0]).unwrap();
    assert_eq!(std::fs::metadata(output).unwrap().len(), size);

    disk.seek(SeekFrom::Start(size - 1)).unwrap();
    disk.write_all(b"x").unwrap();
    let output = dir.path().join("changed.tar");
    assert!(recover_reader(&mut disk, &output, &found[0]).is_err());
    assert!(!output.exists());
}

#[test]
fn matroska_and_webm_carve_only_finite_segments_with_media() {
    for (doc_type, kind) in [
        (b"matroska".as_slice(), "mkv"),
        (b"webm".as_slice(), "webm"),
        (b"matroska\0".as_slice(), "mkv"),
        (b"webm\0ignored".as_slice(), "webm"),
    ] {
        let mut data = vec![
            0x1a,
            0x45,
            0xdf,
            0xa3,
            0x83 + u8::try_from(doc_type.len()).unwrap(),
        ];
        data.extend_from_slice(&[0x42, 0x82, 0x80 + u8::try_from(doc_type.len()).unwrap()]);
        data.extend_from_slice(doc_type);
        data.extend_from_slice(&[0x18, 0x53, 0x80, 0x67]);
        let segment_size = data.len();
        data.push(0x80);
        // Info with TimestampScale, Tracks with an Opus TrackEntry, and a
        // Cluster with a Timestamp and SimpleBlock.
        let elements = [
            (0x1549_a966u32, b"\x2a\xd7\xb1\x83\x0f\x42\x40".as_slice()),
            (
                0x1654_ae6b,
                b"\xae\x92\xd7\x81\x01\x73\xc5\x81\x01\x83\x81\x02\x86\x86A_OPUS",
            ),
            (
                0x1f43_b675,
                b"\xe7\x81\x00\xa3\x87\x81\x00\x00\x80\xf8\xff\xfe",
            ),
        ];
        for (id, payload) in elements {
            data.extend_from_slice(&id.to_be_bytes());
            data.push(0x80 + u8::try_from(payload.len()).unwrap());
            data.extend_from_slice(payload);
        }
        data[segment_size] += u8::try_from(data.len() - segment_size - 1).unwrap();

        let scan = |bytes: &[u8]| {
            let mut candidates = Vec::new();
            scan_reader(
                &mut std::io::Cursor::new(bytes),
                &AtomicBool::new(false),
                |event| {
                    if let Event::Found(file) = event
                        && file.kind == kind
                    {
                        candidates.push(file);
                    }
                },
            )
            .unwrap();
            candidates
        };
        let files = scan(&data);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].size, data.len() as u64);
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join(kind);
        recover_reader(&mut std::io::Cursor::new(&data), &output, &files[0]).unwrap();
        assert_eq!(std::fs::read(output).unwrap(), data);
        assert!(scan(&data[..data.len() - 1]).is_empty());
        let mut malformed = data.clone();
        malformed.truncate(segment_size + 1);
        malformed[segment_size] = 0x92;
        for (id, _) in elements {
            malformed.extend_from_slice(&id.to_be_bytes());
            malformed.extend_from_slice(&[0x81, 0]);
        }
        assert!(scan(&malformed).is_empty());

        let cluster_size = data.len() - elements[2].1.len() - 1;
        let original_size = data[cluster_size];
        data[cluster_size] = 0xff;
        assert!(scan(&data).is_empty());
        data[cluster_size] = original_size;
        data[segment_size] = 0xff;
        assert!(scan(&data).is_empty());
    }
}

#[test]
fn rf64_and_bw64_use_64_bit_lengths_above_four_gib() {
    use std::sync::atomic::Ordering;

    let payload = 1u64 << 32;
    let size = payload + 80;
    for (signature, kind) in [(b"RF64", "rf64"), (b"BW64", "bw64")] {
        let mut disk = tempfile::tempfile().unwrap();
        disk.write_all(signature).unwrap();
        disk.write_all(b"\xff\xff\xff\xffWAVEds64\x1c\0\0\0")
            .unwrap();
        disk.write_all(&(size - 8).to_le_bytes()).unwrap();
        disk.write_all(&payload.to_le_bytes()).unwrap();
        disk.write_all(&[0; 12]).unwrap();
        disk.write_all(
            b"fmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x40\x1f\0\0\x01\0\x08\0data\xff\xff\xff\xff",
        )
        .unwrap();
        disk.set_len(size).unwrap();

        let cancel = AtomicBool::new(false);
        let mut found = None;
        scan_reader(&mut disk, &cancel, |event| {
            if let Event::Found(file) = event {
                found = Some(file);
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        let found = found.unwrap();
        assert_eq!((found.kind, found.size), (kind, size));
    }
}
