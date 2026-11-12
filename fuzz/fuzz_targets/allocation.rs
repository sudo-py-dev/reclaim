#![no_main]
use libfuzzer_sys::fuzz_target;
use reclaim::{Event, SourceReader};
use std::{io::Cursor, sync::atomic::AtomicBool};

fuzz_target!(|data: &[u8]| {
    if data.len() > 65536 {
        return;
    }
    for alignment in [1, 512, 4096] {
        let mut source =
            SourceReader::new(Cursor::new(data), alignment, Some(data.len() as u64)).unwrap();
        let mut total = 0;
        let _result = reclaim::scan_unused_reader(
            &mut source,
            data.len() as u64,
            &AtomicBool::new(false),
            |event| match event {
                Event::UnusedSpace { bytes, .. } => {
                    assert!(bytes <= data.len() as u64);
                    total = bytes;
                }
                Event::Progress(bytes) => assert!(bytes <= total),
                Event::Found(found) => {
                    assert!(found.resident_data.is_none());
                    assert!(
                        found
                            .offset
                            .checked_add(found.size)
                            .is_some_and(|end| end <= data.len() as u64)
                    );
                }
                _ => (),
            },
        );
        // Malformed allocation maps are expected to return errors. Panics,
        // sanitizer violations, out-of-range candidates and hangs are bugs.
    }
});
