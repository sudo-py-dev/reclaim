use super::{CandidateEnd, le16, le32, need};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Default)]
pub(crate) struct Gzip {
    decoder: Option<flate2::Decompress>,
    consumed: usize,
    finished: bool,
    crc: crc32fast::Hasher,
}
impl Gzip {
    pub(crate) fn push(&mut self, b: &[u8], cancel: &AtomicBool) -> CandidateEnd {
        match self.parse(b, cancel) {
            Ok(size) => CandidateEnd::Complete(size),
            Err(state) => state,
        }
    }
    fn parse(&mut self, b: &[u8], cancel: &AtomicBool) -> Result<usize, CandidateEnd> {
        if self.decoder.is_none() {
            need(b, 10)?;
            if !b.starts_with(b"\x1f\x8b\x08") || b[3] & 0xe0 != 0 {
                return Err(CandidateEnd::Invalid);
            }
            let flags = b[3];
            let mut at = 10;
            if flags & 4 != 0 {
                need(b, at + 2)?;
                at += 2 + le16(b, at);
                need(b, at)?;
            }
            for mask in [8, 16] {
                if flags & mask != 0 {
                    if at >= 65536 {
                        return Err(CandidateEnd::Invalid);
                    }
                    let available = b.len().min(65536);
                    let count = b[at..available].iter().position(|c| *c == 0).ok_or(
                        if available == 65536 {
                            CandidateEnd::Invalid
                        } else {
                            CandidateEnd::More
                        },
                    )?;
                    at += count + 1;
                }
            }
            if flags & 2 != 0 {
                need(b, at + 2)?;
                if le16(b, at) != (crc32fast::hash(&b[..at]) & 0xffff) as usize {
                    return Err(CandidateEnd::Invalid);
                }
                at += 2;
            }
            self.consumed = at;
            self.decoder = Some(flate2::Decompress::new(false));
        }
        let decoder = self.decoder.as_mut().unwrap();
        let mut output = [0; 32768];
        while !self.finished {
            if cancel.load(Ordering::Relaxed) {
                return Err(CandidateEnd::Invalid);
            }
            let before_in = decoder.total_in();
            let before_out = decoder.total_out();
            let status = decoder
                .decompress(
                    &b[self.consumed..],
                    &mut output,
                    flate2::FlushDecompress::None,
                )
                .map_err(|_| CandidateEnd::Invalid)?;
            let consumed = (decoder.total_in() - before_in) as usize;
            let produced = (decoder.total_out() - before_out) as usize;
            self.consumed += consumed;
            self.crc.update(&output[..produced]);
            if decoder.total_out() > 256 * 1024 * 1024 {
                return Err(CandidateEnd::Invalid);
            }
            if status == flate2::Status::StreamEnd {
                self.finished = true;
                break;
            }
            if consumed == 0 && produced == 0 {
                return Err(CandidateEnd::More);
            }
        }
        need(b, self.consumed + 8)?;
        if self.crc.clone().finalize() != le32(b, self.consumed) as u32
            || decoder.total_out() as u32 != le32(b, self.consumed + 4) as u32
        {
            return Err(CandidateEnd::Invalid);
        }
        Ok(self.consumed + 8)
    }
}
