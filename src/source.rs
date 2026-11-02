use std::io::{self, Read, Seek, SeekFrom};

/// Adapts byte-level carving to devices that require sector-aligned reads.
/// The underlying handle is read-only; seeks here are logical, not raw seeks.
pub struct SourceReader<R> {
    inner: R,
    position: u64,
    size: Option<u64>,
    alignment: usize,
    cache: Vec<u8>,
    cache_start: usize,
    window_size: usize,
    cached_at: Option<u64>,
    cached_len: usize,
}
impl<R: Read + Seek> SourceReader<R> {
    pub fn new(inner: R, alignment: usize, size: Option<u64>) -> io::Result<Self> {
        if !alignment.is_power_of_two() || alignment > 65536 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid source sector size",
            ));
        }
        let window_size = if alignment == 1 { 0 } else { 1024 * 1024 };
        let cache = vec![0; window_size + alignment];
        let cache_start = (alignment - (cache.as_ptr() as usize % alignment)) % alignment;
        Ok(Self {
            inner,
            position: 0,
            size,
            alignment,
            cache,
            cache_start,
            window_size,
            cached_at: None,
            cached_len: 0,
        })
    }
}
impl<R: Read + Seek> Read for SourceReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() || self.size.is_some_and(|n| self.position >= n) {
            return Ok(0);
        }
        if self.alignment == 1 {
            let mut count = bytes.len();
            if let Some(size) = self.size {
                count = count.min((size - self.position).min(usize::MAX as u64) as usize);
            }
            self.inner.seek(SeekFrom::Start(self.position))?;
            let count = self.inner.read(&mut bytes[..count])?;
            if count == 0 && self.size.is_some_and(|size| self.position < size) {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "The drive stopped returning data before its reported size. Reconnect it and scan again.",
                ));
            }
            self.position += count as u64;
            self.cached_at = None;
            return Ok(count);
        }
        let aligned = self.position / self.window_size as u64 * self.window_size as u64;
        if self.cached_at != Some(aligned) {
            self.inner.seek(SeekFrom::Start(aligned))?;
            let required = self
                .size
                .map(|n| (n - aligned).min(self.window_size as u64) as usize)
                .unwrap_or(self.window_size);
            self.cached_len = 0;
            while self.cached_len < required {
                match self.inner.read(
                    &mut self.cache
                        [self.cache_start + self.cached_len..self.cache_start + required],
                ) {
                    Ok(0) if self.size.is_some() && self.cached_len < required => {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "The drive stopped returning data before its reported size. Reconnect it and scan again.",
                        ));
                    }
                    Ok(0) => break,
                    Ok(n) => self.cached_len += n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
            self.cached_at = Some(aligned);
        }
        let start = (self.position - aligned) as usize;
        let count = bytes.len().min(self.cached_len.saturating_sub(start));
        if count == 0 {
            return Ok(0);
        }
        bytes[..count].copy_from_slice(
            &self.cache[self.cache_start + start..self.cache_start + start + count],
        );
        self.position += count as u64;
        Ok(count)
    }
}
impl<R: Read + Seek> Seek for SourceReader<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(n) => self.position.checked_add_signed(n).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Invalid source offset")
            })?,
            SeekFrom::End(n) => {
                let size = if let Some(size) = self.size {
                    size
                } else {
                    self.inner.seek(SeekFrom::End(0))?
                };
                size.checked_add_signed(n).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "Invalid source offset")
                })?
            }
        };
        self.position = position;
        Ok(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_unreadable_nonempty_device_is_an_error_not_an_empty_scan() {
        for alignment in [1, 512, 4096] {
            let mut reader = SourceReader::new(
                std::io::Cursor::new(Vec::<u8>::new()),
                alignment,
                Some(4096),
            )
            .unwrap();
            assert_eq!(
                reader.read(&mut [0; 128]).unwrap_err().kind(),
                io::ErrorKind::UnexpectedEof
            );
        }
        let mut empty =
            SourceReader::new(std::io::Cursor::new(Vec::<u8>::new()), 1, Some(0)).unwrap();
        assert_eq!(empty.read(&mut [0; 128]).unwrap(), 0);
    }

    struct SectorDevice(std::io::Cursor<Vec<u8>>);
    impl Read for SectorDevice {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            assert!(self.0.position().is_multiple_of(4096));
            assert!(bytes.len().is_multiple_of(4096));
            assert!((bytes.as_ptr() as usize).is_multiple_of(4096));
            self.0.read(bytes)
        }
    }
    impl Seek for SectorDevice {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            self.0.seek(from)
        }
    }
    #[test]
    fn carve_and_export_unaligned_candidate_on_a_sector_device() {
        let mut bytes = vec![0; 12288];
        let content = b"%PDF-1.7\nexample\n%%EOF";
        bytes[4093..4093 + content.len()].copy_from_slice(content);
        let mut reader =
            SourceReader::new(SectorDevice(std::io::Cursor::new(bytes)), 4096, Some(12288))
                .unwrap();
        let mut found = Vec::new();
        crate::scan_reader(
            &mut reader,
            &std::sync::atomic::AtomicBool::new(false),
            |e| {
                if let crate::Event::Found(f) = e {
                    found.push(f);
                }
            },
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].offset, 4093);
        let out = std::env::temp_dir().join(format!("reclaim-aligned-{}.pdf", std::process::id()));
        crate::recover_reader(&mut reader, &out, &found[0]).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), content);
        std::fs::remove_file(out).unwrap();
        assert!(reader.seek(SeekFrom::Current(-100000)).is_err());
    }
}
