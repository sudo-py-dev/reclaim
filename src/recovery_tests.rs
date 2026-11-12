use super::*;
use std::io::Write;

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires RECLAIM_TEST_DEVICE and RECLAIM_TEST_DESTINATION on that USB volume"]
fn live_usb_export_to_source_drive_is_blocked_before_any_write() {
    let source = PathBuf::from(std::env::var_os("RECLAIM_TEST_DEVICE").unwrap());
    let directory = PathBuf::from(std::env::var_os("RECLAIM_TEST_DESTINATION").unwrap());
    assert_eq!(
        drives::destination(&source, &directory),
        drives::Destination::SameDrive
    );
    let before = std::fs::read_dir(&directory).unwrap().count();
    let mut app = Reclaim::default();
    app.scanned_source = Some(source);
    app.results = vec![Found {
        offset: 0,
        size: 4,
        kind: "txt",
        selected: true,
        validation: reclaim::Validation::Unverified,
        original_name: Some("must-not-be-written.txt".into()),
        resident_data: Some(Arc::from(&b"data"[..])),
        ntfs: None,
    }];
    app.export(directory.clone());
    assert!(!app.busy);
    assert!(app.receiver.is_none());
    assert!(app.status.contains("different physical drive"));
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), before);
}

#[test]
fn recovery_exports_only_selected_files_matching_current_filters() {
    for (category, query, expected) in [
        (1, "", vec!["photo.png"]),
        (3, "", vec!["backup.zip"]),
        (0, "PHOTO", vec!["photo.png"]),
        (3, "photo", vec![]),
        (0, "", vec!["backup.zip", "photo.png"]),
        (2, "", vec![]),
    ] {
        let directory_temp = tempfile::tempdir().unwrap();
        let directory = directory_temp.path().join("filter-export");
        std::fs::create_dir_all(&directory).unwrap();
        let source = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let mut app = Reclaim::default();
        app.scanned_source = Some(source.clone());
        app.source_handle = Some(Arc::new(std::fs::File::open(source).unwrap()));
        app.category = category;
        app.filter = query.into();
        app.results = [
            ("png", "photo.png", true),
            ("zip", "backup.zip", true),
            ("pdf", "notes.pdf", false),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (kind, name, selected))| Found {
            offset: i as u64 * 4096,
            size: 4,
            kind,
            selected,
            validation: reclaim::Validation::MetadataChecked,
            original_name: Some(name.into()),
            resident_data: Some(Arc::from(&b"data"[..])),
            ntfs: None,
        })
        .collect();
        app.export(directory.clone());
        if expected.is_empty() {
            assert!(!app.busy);
            assert!(app.receiver.is_none());
        } else {
            let result = app
                .receiver
                .as_ref()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            assert!(matches!(result, Message::Export(Ok(count)) if count == expected.len()));
        }
        let mut actual: Vec<_> = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        actual.sort();
        assert_eq!(actual, expected, "category={category}, query={query}");
        for name in actual {
            assert_eq!(std::fs::read(directory.join(name)).unwrap(), b"data");
        }
    }
}

#[test]
fn stopping_scan_preserves_selected_partial_results() {
    let mut app = Reclaim::default();
    app.busy = true;
    app.cancel.store(true, Ordering::Relaxed);
    let (tx, rx) = mpsc::sync_channel(4);
    app.receiver = Some(rx);
    tx.send(Message::Scan(Event::Found(Found {
        offset: 0,
        size: 4,
        kind: "png",
        selected: true,
        validation: reclaim::Validation::MetadataChecked,
        original_name: Some("photo.png".into()),
        resident_data: Some(Arc::from(&b"data"[..])),
        ntfs: None,
    })))
    .unwrap();
    tx.send(Message::Done(Ok(()))).unwrap();
    app.poll();
    assert!(!app.busy);
    assert!(app.scan_stopped);
    assert_eq!(app.recovery_files().len(), 1);
    assert!(app.status.starts_with("Scan stopped."));
}

#[test]
fn automatic_drive_updates_preserve_selection_and_scan_results() {
    let drive = |name: &str| drives::Drive {
        path: format!("/dev/{name}").into(),
        name: name.into(),
        name_is_message: false,
        size: 64_000_000_000,
        kind: "USB drive".into(),
        location: "Not mounted".into(),
        mounted: false,
        encrypted: false,
        sector_size: 512,
    };
    let mut app = Reclaim::default();
    app.drives = vec![drive("usb1")];
    app.source = Some("/dev/usb1".into());
    app.scanned_source = app.source.clone();
    app.busy = true;
    app.scanned = 4096;
    app.apply_drive_update(Ok(vec![drive("usb1"), drive("usb2")]));
    assert_eq!(
        app.source.as_deref(),
        Some(std::path::Path::new("/dev/usb1"))
    );
    assert!(app.busy);
    assert_eq!(app.scanned, 4096);
    app.apply_drive_update(Err("temporary discovery error".into()));
    assert_eq!(app.drives.len(), 2);
    assert!(app.source.is_some());
    app.apply_drive_update(Ok(vec![drive("usb2")]));
    assert!(app.source.is_none());
    assert_eq!(
        app.scanned_source.as_deref(),
        Some(std::path::Path::new("/dev/usb1"))
    );
    assert!(app.busy); // The active reader owns its handle and reports completion/errors.
    assert!(app.drives_error.is_none());
    app.source = Some("/media/backup.img".into());
    app.apply_drive_update(Ok(vec![]));
    assert_eq!(
        app.source.as_deref(),
        Some(std::path::Path::new("/media/backup.img"))
    );
}

#[test]
fn scan_feedback_tracks_checks_and_preserves_the_stop_request() {
    let mut app = Reclaim::default();
    app.busy = true;
    let (tx, rx) = mpsc::sync_channel(8);
    app.receiver = Some(rx);
    tx.send(Message::Scan(Event::Checking {
        offset: 4096,
        kind: "png",
        bytes: 8192,
    }))
    .unwrap();
    app.poll();
    assert_eq!(app.scanned, 4096);
    let (_, kind, bytes, _) = app.scan_activity.unwrap();
    assert_eq!((kind, bytes), ("png", 8192));
    assert!(app.last_scan_update.is_some());
    tx.send(Message::Scan(Event::Progress(8192))).unwrap();
    app.poll();
    assert!(app.scan_activity.is_none());
    app.stop_operation();
    let stopping = app.status.clone();
    tx.send(Message::Scan(Event::Checking {
        offset: 8192,
        kind: "jpg",
        bytes: 16384,
    }))
    .unwrap();
    app.poll();
    assert_eq!(app.status, stopping);
    tx.send(Message::Done(Ok(()))).unwrap();
    app.poll();
    assert!(!app.busy);
    assert!(app.scan_activity.is_none());
    assert!(app.scan_stopped);
}

#[test]
fn unused_space_progress_does_not_jump_to_the_physical_disk_offset() {
    let mut app = Reclaim::default();
    app.busy = true;
    let (tx, rx) = mpsc::sync_channel(8);
    app.receiver = Some(rx);
    tx.send(Message::Scan(Event::UnusedSpace {
        bytes: 4096,
        filesystem: "NTFS",
    }))
    .unwrap();
    tx.send(Message::Scan(Event::Progress(512))).unwrap();
    tx.send(Message::Scan(Event::Checking {
        offset: 50 * 1024 * 1024,
        kind: "zip",
        bytes: 1024,
    }))
    .unwrap();
    app.poll();
    assert!(app.unused_scan && app.allocation_ready);
    assert_eq!(app.total, 4096);
    assert_eq!(app.scanned, 512);
    let since = app.scan_activity.unwrap().3;
    tx.send(Message::Scan(Event::Progress(512))).unwrap();
    tx.send(Message::Scan(Event::Checking {
        offset: 50 * 1024 * 1024,
        kind: "zip",
        bytes: 2048,
    }))
    .unwrap();
    app.poll();
    assert_eq!(app.scan_activity.unwrap().3, since);
}

#[test]
fn scan_total_stays_unknown_until_unused_space_is_mapped() {
    let source = tempfile::NamedTempFile::new().unwrap();
    source.as_file().set_len(8192).unwrap();
    let mut app = Reclaim::default();
    app.source = Some(source.path().to_owned());
    app.start();
    assert_eq!(app.source_size, Some(8192));
    assert_eq!(app.total, 0);
    assert!(!app.allocation_ready);
    app.stop_operation();
}

#[test]
fn recovery_reads_offsets_beyond_the_unused_byte_total() {
    let mut source = tempfile::tempfile().unwrap();
    source.write_all(&[0; 4096]).unwrap();
    source.write_all(b"data").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut app = Reclaim::default();
    app.scanned_source = Some("test-volume".into());
    app.source_handle = Some(Arc::new(source));
    app.source_size = Some(4100);
    app.total = 512; // Free space is smaller than the physical file offset.
    app.results = vec![Found {
        offset: 4096,
        size: 4,
        kind: "txt",
        selected: true,
        validation: reclaim::Validation::Unverified,
        original_name: Some("recovered.txt".into()),
        resident_data: None,
        ntfs: None,
    }];
    app.export(directory.path().to_owned());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.busy && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::yield_now();
    }
    assert!(!app.busy, "export did not finish");
    assert_eq!(
        std::fs::read(directory.path().join("recovered.txt")).unwrap(),
        b"data"
    );
}

#[test]
fn disconnected_workers_stop_busy_state_without_overwriting_completed_results() {
    for exporting in [false, true] {
        let mut app = Reclaim::default();
        app.busy = true;
        app.exporting = exporting;
        let (tx, rx) = mpsc::channel();
        app.receiver = Some(rx);
        drop(tx);
        app.poll();
        assert!(!app.busy);
        assert!(!app.exporting);
        assert!(app.receiver.is_none());
        assert_eq!(app.scan_error.is_some(), !exporting);
        let status = app.status.clone();
        app.poll();
        assert_eq!(app.status, status);
    }
}

#[test]
fn skipped_ranges_are_merged_and_completion_reports_incomplete_coverage() {
    let mut app = Reclaim::default();
    app.busy = true;
    let (tx, rx) = mpsc::channel();
    app.receiver = Some(rx);
    for offset in [4096, 8192, 20480] {
        tx.send(Message::Scan(Event::Unreadable {
            offset,
            bytes: 4096,
        }))
        .unwrap();
    }
    tx.send(Message::Done(Ok(()))).unwrap();
    app.poll();
    assert_eq!(app.unreadable, vec![(4096, 8192), (20480, 4096)]);
    assert!(!app.busy);
    assert!(app.scan_error.is_none());
    assert!(app.status.contains("unreadable areas"));
}

#[test]
fn source_access_failure_offers_elevation_only_when_requested_by_the_worker() {
    for denied in [false, true] {
        let mut app = Reclaim::default();
        app.busy = true;
        let (tx, rx) = mpsc::channel();
        app.receiver = Some(rx);
        if denied {
            tx.send(Message::SourceAccessDenied).unwrap();
        }
        tx.send(Message::Done(Err("source could not be opened".into())))
            .unwrap();
        drop(tx);
        app.poll();
        assert!(!app.busy);
        assert!(app.scan_error.is_some());
        assert_eq!(app.needs_administrator, denied);
        assert!(app.results.is_empty());
    }
}
