#![no_main]

#[path = "../../../src/bytes.rs"]
#[expect(
    dead_code,
    reason = "the lightweight probes use only part of the production byte helpers"
)]
mod bytes;

// Compile the exact production probes, including their shared bounds and result
// type, without pulling the desktop stack into the sanitizer build.
#[path = "../../../src/detection.rs"]
mod detection;
use detection::CandidateEnd;
use libfuzzer_sys::fuzz_target;
use std::sync::atomic::AtomicBool;

fuzz_target!(|data: &[u8]| {
    if data.len() > 64 * 1024 {
        return;
    }
    for length in [0, 16, 99, 100, 107, 108, data.len()] {
        if length <= data.len() {
            if let CandidateEnd::Complete(end) = detection::sqlite_end(&data[..length]) {
                assert!((1..=length).contains(&end));
            }
        }
    }
    let mut ogg = detection::Ogg::default();
    let cancel = AtomicBool::new(false);
    let chunk = usize::from(data.first().copied().unwrap_or(0)) + 1;
    let mut length = 0;
    while length < data.len() {
        length = (length + chunk).min(data.len());
        match ogg.push(&data[..length], &cancel) {
            CandidateEnd::More => (),
            CandidateEnd::Invalid => break,
            CandidateEnd::Complete(end) => {
                assert!((1..=length).contains(&end));
                assert!(matches!(ogg.kind(), "ogg" | "opus" | "ogv"));
                break;
            }
        }
    }
});
