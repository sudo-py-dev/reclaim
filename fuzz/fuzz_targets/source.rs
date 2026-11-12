#![no_main]
use libfuzzer_sys::fuzz_target;
use reclaim::SourceReader;
use std::io::{Cursor, Read, Seek, SeekFrom};

fuzz_target!(|data: &[u8]| {
    if data.len() > 65536 {
        return;
    }
    for alignment in [1, 512, 4096, 65536] {
        let mut source =
            SourceReader::new(Cursor::new(data), alignment, Some(data.len() as u64)).unwrap();
        let mut reference = Cursor::new(data);
        let mut actual = [0u8; 257];
        let mut expected = [0u8; 257];
        for command in data.chunks(9).take(128) {
            let mut encoded = [0u8; 8];
            for (to, from) in encoded.iter_mut().zip(command.iter().skip(1)) {
                *to = *from;
            }
            let raw = u64::from_le_bytes(encoded);
            let seek = match command[0] % 4 {
                0 => SeekFrom::Start(raw),
                1 => SeekFrom::Start(raw % (data.len() as u64 + 1)),
                2 => SeekFrom::Current(i64::from_le_bytes(encoded)),
                _ => SeekFrom::End(i64::from_le_bytes(encoded)),
            };
            let a = source.seek(seek);
            let b = reference.seek(seek);
            assert_eq!(a.is_ok(), b.is_ok());
            if let (Ok(a), Ok(b)) = (a, b) {
                assert_eq!(a, b);
            }
            assert_eq!(source.stream_position().unwrap(), reference.position());
            let count = usize::from(command[0]);
            let a = source.read(&mut actual[..count]).unwrap();
            let b = reference.read(&mut expected[..count]).unwrap();
            assert_eq!(a, b);
            assert_eq!(&actual[..a], &expected[..b]);
        }
    }
});
