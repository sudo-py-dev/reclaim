//! Seek-based probes for formats with reliable boundaries independent of payload size.
use crate::{LIMIT, Validation, bytes, declared_size, detection, formats, read_chunk};
use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::atomic::AtomicBool,
};

mod containers;

pub(crate) struct Candidate {
    pub size: u64,
    pub kind: &'static str,
    pub validation: Validation,
}

pub(crate) fn supports(kind: &str) -> bool {
    seek_only(kind) || matches!(kind, "riff" | "aiff" | "au" | "sqlite" | "tar" | "7z")
}

pub(crate) fn supports_result(kind: &str) -> bool {
    kind != "ebml"
        && (seek_only(kind)
            || matches!(
                kind,
                "wav" | "avi" | "aiff" | "aifc" | "au" | "sqlite" | "tar" | "7z"
            ))
}

pub(crate) fn seek_only(kind: &str) -> bool {
    matches!(
        kind,
        "ogg" | "opus" | "ogv" | "ebml" | "mkv" | "webm" | "rf64" | "bw64"
    )
}

pub(crate) fn inspect(
    input: &mut (impl Read + Seek),
    start: u64,
    kind: &str,
    cancel: &AtomicBool,
) -> io::Result<Option<Candidate>> {
    let (size, kind, validation) = match kind {
        "riff" | "wav" | "avi" | "aiff" | "aifc" | "au" => {
            let mut header = [0; 24];
            if !read_at(input, start, &mut header, cancel)? {
                return Ok(None);
            }
            let (base, kind) = match (header.get(..4), header.get(8..12)) {
                (Some(b"RIFF"), Some(b"WAVE")) => ("riff", "wav"),
                (Some(b"RIFF"), Some(b"AVI ")) => ("riff", "avi"),
                (Some(b"FORM"), Some(b"AIFF")) => ("aiff", "aiff"),
                (Some(b"FORM"), Some(b"AIFC")) => ("aiff", "aifc"),
                (Some(b".snd"), _) => ("au", "au"),
                _ => return Ok(None),
            };
            let Ok(size) = declared_size(base, &header) else {
                return Ok(None);
            };
            (size, kind, Validation::Unverified)
        }
        "sqlite" => {
            let mut header = vec![0; 65536];
            if !read_at(input, start, &mut header, cancel)? {
                return Ok(None);
            }
            let Ok(size) = detection::sqlite_size(&header) else {
                return Ok(None);
            };
            (size, "sqlite", Validation::StructureChecked)
        }
        "ogg" | "opus" | "ogv" => {
            return ogg(input, start, cancel);
        }
        "ebml" | "mkv" | "webm" => {
            let Some((size, kind)) = containers::matroska(input, start, cancel)? else {
                return Ok(None);
            };
            (size, kind, Validation::StructureChecked)
        }
        "rf64" | "bw64" => {
            let Some((size, kind)) = containers::wave64(input, start, cancel)? else {
                return Ok(None);
            };
            (size, kind, Validation::StructureChecked)
        }
        "tar" => {
            let Some(size) = tar(input, start, cancel)? else {
                return Ok(None);
            };
            (size, "tar", Validation::StructureChecked)
        }
        "7z" => {
            let Some(size) = seven(input, start, cancel)? else {
                return Ok(None);
            };
            (size, "7z", Validation::StructureChecked)
        }
        _ => return Ok(None),
    };
    let Some(last) = start.checked_add(size).and_then(|end| end.checked_sub(1)) else {
        return Ok(None);
    };
    if (!seek_only(kind) && size <= LIMIT as u64) || !read_at(input, last, &mut [0], cancel)? {
        return Ok(None);
    }
    Ok(Some(Candidate {
        size,
        kind,
        validation,
    }))
}

fn read_at(
    input: &mut (impl Read + Seek),
    offset: u64,
    buffer: &mut [u8],
    cancel: &AtomicBool,
) -> io::Result<bool> {
    // Out-of-range seeks may fail on devices that reject seeks beyond EOF.
    match input.seek(SeekFrom::Start(offset)) {
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => return Ok(false),
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    read_full(input, buffer, cancel)
}

fn read_full(
    input: &mut impl Read,
    mut buffer: &mut [u8],
    cancel: &AtomicBool,
) -> io::Result<bool> {
    while !buffer.is_empty() {
        let count = read_chunk(input, buffer, cancel)?;
        if count == 0 {
            return Ok(false);
        }
        let Some(remaining) = buffer.get_mut(count..) else {
            return Err(io::Error::other("Reader returned an invalid byte count"));
        };
        buffer = remaining;
    }
    Ok(true)
}

fn tar(input: &mut (impl Read + Seek), start: u64, cancel: &AtomicBool) -> io::Result<Option<u64>> {
    let mut offset = start;
    let mut header = [0; formats::TAR_HEADER_SIZE];
    for entries in 0..100_000 {
        if !read_at(input, offset, &mut header, cancel)? {
            break;
        }
        if header == [0; formats::TAR_HEADER_SIZE] {
            if entries > 0
                && let Some(end) = offset.checked_add(1024)
                && end - start > LIMIT as u64
                && read_at(input, end - 512, &mut header, cancel)?
                && header == [0; formats::TAR_HEADER_SIZE]
            {
                return Ok(Some(end - start));
            }
            break;
        }
        let Some(size) = formats::tar_entry_size(&header) else {
            break;
        };
        let Some(next) = (size as u64)
            .checked_next_multiple_of(512)
            .and_then(|padded| padded.checked_add(512))
            .and_then(|span| offset.checked_add(span))
        else {
            break;
        };
        offset = next;
    }
    Ok(None)
}

fn seven(
    input: &mut (impl Read + Seek),
    start: u64,
    cancel: &AtomicBool,
) -> io::Result<Option<u64>> {
    let mut head = [0; 32];
    if !read_at(input, start, &mut head, cancel)? || !formats::seven_header_valid(&head) {
        return Ok(None);
    }
    let length = bytes::u64_le(&head, 20);
    let Some((next, end)) = bytes::u64_le(&head, 12)
        .checked_add(32)
        .and_then(|next| next.checked_add(length).map(|end| (next, end)))
    else {
        return Ok(None);
    };
    if end <= LIMIT as u64 || length == 0 || start.checked_add(end).is_none() {
        return Ok(None);
    }
    let mut crc = crc32fast::Hasher::new();
    let mut chunk = vec![0; 32768];
    let mut at = next;
    while at < end {
        let count = bytes::to_usize((end - at).min(chunk.len() as u64));
        let Some(block) = chunk.get_mut(..count) else {
            return Ok(None);
        };
        if !read_at(input, start + at, block, cancel)?
            || (at == next && !matches!(block.first(), Some(1 | 0x17)))
        {
            return Ok(None);
        }
        crc.update(block);
        at += count as u64;
    }
    Ok((crc.finalize() == bytes::u32_le(&head, 28)).then_some(end))
}

fn ogg(
    input: &mut (impl Read + Seek),
    start: u64,
    cancel: &AtomicBool,
) -> io::Result<Option<Candidate>> {
    let mut ogg = detection::Ogg::default();
    // One maximum-size page: fixed header, lacing table and packet data.
    let mut page = vec![0; 27 + 255 + 255 * 255];
    input.seek(SeekFrom::Start(start))?;
    let mut offset = start;
    loop {
        let Some((header, body)) = page.split_at_mut_checked(27) else {
            return Ok(None);
        };
        if !read_full(input, header, cancel)? || !header.starts_with(b"OggS") {
            return Ok(None);
        }
        let Some(&segments) = header.last() else {
            return Ok(None);
        };
        let Some((lacing, payload)) = body.split_at_mut_checked(usize::from(segments)) else {
            return Ok(None);
        };
        if !read_full(input, lacing, cancel)? {
            return Ok(None);
        }
        let size = lacing.iter().map(|&n| usize::from(n)).sum::<usize>();
        let end = header.len() + lacing.len() + size;
        let Some((next, payload)) = offset.checked_add(end as u64).zip(payload.get_mut(..size))
        else {
            return Ok(None);
        };
        if !read_full(input, payload, cancel)? {
            return Ok(None);
        }
        let Some(current_page) = page.get(..end) else {
            return Ok(None);
        };
        match ogg.push_page(current_page, cancel) {
            crate::CandidateEnd::Invalid => return Ok(None),
            crate::CandidateEnd::More => offset = next,
            crate::CandidateEnd::Complete(_) => {
                return Ok(Some(Candidate {
                    size: next - start,
                    kind: ogg.kind(),
                    validation: Validation::StructureChecked,
                }));
            }
        }
    }
}
