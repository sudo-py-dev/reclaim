#![no_main]

use libfuzzer_sys::fuzz_target;
use reclaim::{Event, SourceReader};
use std::{io::Cursor, sync::atomic::AtomicBool};

fuzz_target!(|data: &[u8]| {
    // Bound each input while exercising both ordinary and aligned source reads.
    if data.len() > 64 * 1024 {
        return;
    }
    let cancel = AtomicBool::new(false);
    for alignment in [1, 512, 4096] {
        let mut source =
            SourceReader::new(Cursor::new(data), alignment, Some(data.len() as u64)).unwrap();
        reclaim::scan_reader(&mut source, &cancel, |event| {
            if let Event::Found(found) = event {
                assert!(found.offset <= data.len() as u64);
                if found.resident_data.is_none() {
                    assert!(found.offset + found.size <= data.len() as u64);
                }
                let name = found.recovery_name(1);
                assert_eq!(std::path::Path::new(&name).components().count(), 1);
                assert!(!name.contains(['/', '\\']));
            }
        })
        .unwrap();
    }
});
