//! Bounded-memory walk for large, classic ZIP archives with known entry sizes.
use std::{
    collections::HashSet,
    io::{self, Read, Seek, SeekFrom},
    sync::atomic::{AtomicBool, Ordering},
};
const MAX_END: u64 = u32::MAX as u64 + 65557;
fn short(bytes: &[u8], at: usize) -> u64 {
    crate::bytes::u16_le(bytes, at) as u64
}
fn long(bytes: &[u8], at: usize) -> u64 {
    crate::bytes::u32_le(bytes, at) as u64
}
fn read<const N: usize>(
    input: &mut (impl Read + Seek),
    start: u64,
    at: u64,
) -> io::Result<[u8; N]> {
    if at > MAX_END {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
    }
    input.seek(SeekFrom::Start(
        start
            .checked_add(at)
            .ok_or_else(|| io::Error::other("Invalid ZIP range"))?,
    ))?;
    let mut bytes = [0; N];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}
pub(crate) enum Inspection {
    Complete(u64),
    Invalid,
    Unsupported,
}

pub(crate) fn inspect(
    input: &mut (impl Read + Seek),
    start: u64,
    cancel: &AtomicBool,
) -> io::Result<Inspection> {
    match walk(input, start, cancel) {
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(Inspection::Invalid),
        result => result,
    }
}

pub(crate) fn large_end(
    input: &mut (impl Read + Seek),
    start: u64,
    cancel: &AtomicBool,
) -> io::Result<Option<u64>> {
    Ok(match inspect(input, start, cancel)? {
        Inspection::Complete(size) if size > crate::LIMIT as u64 => Some(size),
        _ => None,
    })
}
fn walk(input: &mut (impl Read + Seek), start: u64, cancel: &AtomicBool) -> io::Result<Inspection> {
    let mut at = 0;
    let mut locals = HashSet::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Inspection::Invalid);
        }
        let signature = read::<4>(input, start, at)?;
        if &signature == b"PK\x01\x02" {
            break;
        }
        if &signature != b"PK\x03\x04" || locals.len() >= 65534 {
            return Ok(Inspection::Invalid);
        }
        let header = read::<30>(input, start, at)?;
        // ZIP64 and data descriptors need a different traversal. The ordinary
        // bounded carver still handles small archives containing descriptors.
        if short(&header, 26) == 0 || short(&header, 4) > 63 {
            return Ok(Inspection::Invalid);
        }
        if short(&header, 6) & 8 != 0 || long(&header, 18) == u32::MAX as u64 {
            return Ok(Inspection::Unsupported);
        }
        locals.insert(at);
        at += 30 + short(&header, 26) + short(&header, 28) + long(&header, 18);
    }
    if locals.is_empty() {
        return Ok(Inspection::Invalid);
    }
    let directory = at;
    let count = locals.len();
    for _ in 0..count {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Inspection::Invalid);
        }
        let header = read::<46>(input, start, at)?;
        if &header[..4] != b"PK\x01\x02" || short(&header, 34) != 0 {
            return Ok(Inspection::Invalid);
        }
        let offset = long(&header, 42);
        if !locals.remove(&offset) {
            return Ok(Inspection::Invalid);
        }
        let local = read::<30>(input, start, offset)?;
        let name_len = short(&header, 28) as usize;
        if short(&local, 26) as usize != name_len
            || header[8..16] != local[6..14]
            || header[16..28] != local[14..26]
        {
            return Ok(Inspection::Invalid);
        }
        let data_end = offset + 30 + name_len as u64 + short(&local, 28) + long(&header, 20);
        if data_end > directory {
            return Ok(Inspection::Invalid);
        }
        // audit-allow(DOS-ALLOC): name_len comes from a 16-bit field, at most 64 KiB.
        let mut name = vec![0; name_len];
        input.read_exact(&mut name)?;
        input.seek(SeekFrom::Start(start + at + 46))?;
        // audit-allow(DOS-ALLOC): name_len comes from a 16-bit field, at most 64 KiB.
        let mut central_name = vec![0; name_len];
        input.read_exact(&mut central_name)?;
        if name != central_name {
            return Ok(Inspection::Invalid);
        }
        at += 46 + name_len as u64 + short(&header, 30) + short(&header, 32);
        if at - directory > crate::LIMIT as u64 {
            return Ok(Inspection::Invalid);
        }
    }
    let end = read::<22>(input, start, at)?;
    if &end[..4] != b"PK\x05\x06"
        || short(&end, 4) != 0
        || short(&end, 6) != 0
        || short(&end, 8) != count as u64
        || short(&end, 10) != count as u64
        || long(&end, 12) != at - directory
        || long(&end, 16) != directory
    {
        return Ok(Inspection::Invalid);
    }
    let size = at + 22 + short(&end, 20);
    read::<1>(input, start, size - 1)?;
    Ok(Inspection::Complete(size))
}
