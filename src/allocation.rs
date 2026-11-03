//! Read-only, fail-closed allocation maps for formatted volume sources.
//! The map is a start-of-scan view, not a snapshot of a changing mounted volume.
use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::atomic::{AtomicBool, Ordering},
};
mod fat;
mod ntfs;
const MAX_MAP: usize = 64 * 1024 * 1024;
const MAX_RANGES: usize = 1_000_000;
#[derive(Debug)]
pub(crate) struct Plan {
    pub ranges: Vec<(u64, u64)>,
    pub unused: u64,
    pub filesystem: &'static str,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "Unused-space scanning needs a supported formatted volume (NTFS, FAT12/16/32, or exFAT). Select the volume/partition rather than a whole-disk image. Occupied space was not scanned.",
    )
}
fn check(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "Scan stopped"))
    } else {
        Ok(())
    }
}
fn u16at(b: &[u8], p: usize) -> u64 {
    u16::from_le_bytes(b[p..p + 2].try_into().unwrap()) as u64
}
fn u32at(b: &[u8], p: usize) -> u64 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap()) as u64
}
fn u64at(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(b[p..p + 8].try_into().unwrap())
}
fn product(a: u64, b: u64) -> io::Result<u64> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("Invalid filesystem geometry"))
}
fn read_at(
    input: &mut (impl Read + Seek),
    at: u64,
    len: usize,
    total: u64,
    cancel: &AtomicBool,
) -> io::Result<Vec<u8>> {
    if len > MAX_MAP || at.checked_add(len as u64).is_none_or(|end| end > total) {
        return Err(invalid(
            "Filesystem metadata extends beyond the selected volume",
        ));
    }
    input.seek(SeekFrom::Start(at))?;
    let mut bytes = vec![0; len];
    for chunk in bytes.chunks_mut(65536) {
        check(cancel)?;
        input.read_exact(chunk)?;
    }
    Ok(bytes)
}
impl Plan {
    fn new(filesystem: &'static str) -> Self {
        Self {
            ranges: Vec::new(),
            unused: 0,
            filesystem,
        }
    }
    fn add(&mut self, start: u64, len: u64) -> io::Result<()> {
        if len == 0 {
            return Ok(());
        }
        self.unused = self
            .unused
            .checked_add(len)
            .ok_or_else(|| invalid("Invalid unused-space size"))?;
        if let Some((previous, size)) = self.ranges.last_mut()
            && previous.checked_add(*size) == Some(start)
        {
            *size += len;
            return Ok(());
        }
        if self.ranges.len() >= MAX_RANGES {
            return Err(invalid("Unused space is too fragmented to map safely"));
        }
        self.ranges.push((start, len));
        Ok(())
    }
}
fn from_bitmap(
    bitmap: &[u8],
    clusters: u64,
    start: u64,
    cluster_size: u64,
    filesystem: &'static str,
    cancel: &AtomicBool,
) -> io::Result<Plan> {
    if clusters.div_ceil(8) > bitmap.len() as u64 {
        return Err(invalid("Allocation bitmap is truncated"));
    }
    let mut plan = Plan::new(filesystem);
    for cluster in 0..clusters {
        if cluster.is_multiple_of(8192) {
            check(cancel)?;
        }
        if bitmap[(cluster / 8) as usize] & (1 << (cluster % 8)) == 0 {
            plan.add(start + cluster * cluster_size, cluster_size)?;
        }
    }
    Ok(plan)
}
pub(crate) fn plan(
    input: &mut (impl Read + Seek),
    total: u64,
    cancel: &AtomicBool,
) -> io::Result<Plan> {
    let boot = read_at(input, 0, 512, total, cancel)?;
    if &boot[510..512] != b"\x55\xaa" {
        return Err(unsupported());
    }
    if &boot[3..11] == b"NTFS    " {
        ntfs::plan(input, &boot, total, cancel)
    } else if &boot[3..11] == b"EXFAT   " {
        fat::exfat(input, &boot, total, cancel)
    } else if matches!(u16at(&boot, 11), 512 | 1024 | 2048 | 4096) && boot[13].is_power_of_two() {
        fat::classic(input, &boot, total, cancel)
    } else {
        Err(unsupported())
    }
}

/// A free extent is an independent address space. Reads/seeks cannot cross an
/// allocated gap, including candidate lookahead by the ZIP/executable probes.
struct Extent<'a, R> {
    input: &'a mut R,
    start: u64,
    length: u64,
    position: u64,
}
impl<R: Read + Seek> Read for Extent<'_, R> {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let length = b.len().min(
            self.length
                .saturating_sub(self.position)
                .min(usize::MAX as u64) as usize,
        );
        if length == 0 {
            return Ok(0);
        }
        self.input
            .seek(SeekFrom::Start(self.start + self.position))?;
        let count = self.input.read(&mut b[..length])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "The drive stopped returning unused-space data",
            ));
        }
        self.position += count as u64;
        Ok(count)
    }
}
impl<R: Read + Seek> Seek for Extent<'_, R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let next = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.length) + i128::from(n),
        };
        self.position = u64::try_from(next).map_err(|_| invalid("Invalid candidate position"))?;
        Ok(self.position)
    }
}
pub(crate) fn scan(
    input: &mut (impl Read + Seek),
    total: u64,
    cancel: &AtomicBool,
    mut event: impl FnMut(crate::Event),
) -> io::Result<()> {
    let plan = match plan(input, total, cancel) {
        Err(e) if e.kind() == io::ErrorKind::Interrupted && cancel.load(Ordering::Relaxed) => {
            return Ok(());
        }
        result => result?,
    };
    event(crate::Event::UnusedSpace {
        bytes: plan.unused,
        filesystem: plan.filesystem,
    });
    let mut completed = 0;
    let mut found_count = 0;
    let mut resident_bytes = 0usize;
    for (start, length) in plan.ranges {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut extent = Extent {
            input,
            start,
            length,
            position: 0,
        };
        let mut limited = false;
        crate::resilient::scan(
            &mut extent,
            cancel,
            |e| match e {
                crate::Event::Found(mut file) => {
                    found_count += 1;
                    resident_bytes += file.resident_data.as_ref().map_or(0, |data| data.len());
                    if found_count > 100_000 || resident_bytes > MAX_MAP {
                        limited = true;
                        return;
                    }
                    file.offset += start;
                    event(crate::Event::Found(file));
                }
                crate::Event::Progress(bytes) => {
                    event(crate::Event::Progress(completed + bytes.min(length)))
                }
                crate::Event::Checking {
                    offset,
                    kind,
                    bytes,
                } => {
                    event(crate::Event::Progress(completed + offset.min(length)));
                    event(crate::Event::Checking {
                        offset: start + offset,
                        kind,
                        bytes,
                    });
                }
                crate::Event::Unreadable { offset, bytes } => {
                    event(crate::Event::Unreadable {
                        offset: start + offset,
                        bytes,
                    });
                }
                crate::Event::UnusedSpace { .. } => unreachable!(),
            },
            length.min(1024 * 1024) as usize,
        )?;
        if limited {
            return Err(invalid(
                "Candidate limit reached (100,000). Recover available results.",
            ));
        }
        completed += length;
        event(crate::Event::Progress(completed));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
