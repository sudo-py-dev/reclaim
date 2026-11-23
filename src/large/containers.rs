//! Boundary checks for finite EBML segments and table-free RF64/BW64 WAVE files.
use super::read_at;
use crate::bytes;
use std::{
    io::{self, Read, Seek},
    sync::atomic::AtomicBool,
};

const MAX_EBML_HEADER: usize = 4096;

struct Element {
    id: u32,
    data: u64,
    end: u64,
}

// RFC 8794 §§4–6. Keep the marker in IDs and strip it from sizes.
fn vint(raw: &[u8], max: usize, id: bool) -> Option<(usize, u64)> {
    let first = *raw.first()?;
    let width = first.leading_zeros() as usize + 1;
    if width > max {
        return None;
    }
    let (first, rest) = raw.get(..width)?.split_first()?;
    let marker = 0x80 >> (width - 1);
    let value = if id { *first } else { *first & (marker - 1) };
    let value = rest.iter().fold(u64::from(value), |value, &byte| {
        (value << 8) | u64::from(byte)
    });
    (id || value != (1u64 << (7 * width)) - 1).then_some((width, value))
}

fn element(
    input: &mut (impl Read + Seek),
    at: u64,
    limit: u64,
    cancel: &AtomicBool,
) -> io::Result<Option<Element>> {
    let mut header = [0; 12];
    let length = bytes::to_usize(limit.saturating_sub(at).min(header.len() as u64));
    let Some(header) = header.get_mut(..length) else {
        return Ok(None);
    };
    if !read_at(input, at, header, cancel)? {
        return Ok(None);
    }
    let Some((id_width, id)) = vint(header, 4, true) else {
        return Ok(None);
    };
    let Some((size_width, size)) = header.get(id_width..).and_then(|raw| vint(raw, 8, false))
    else {
        return Ok(None);
    };
    let Some(data) = at.checked_add((id_width + size_width) as u64) else {
        return Ok(None);
    };
    let Some(end) = data.checked_add(size).filter(|end| *end <= limit) else {
        return Ok(None);
    };
    Ok(Some(Element {
        id: u32::try_from(id).unwrap_or_default(),
        data,
        end,
    }))
}

pub(super) fn matroska(
    input: &mut (impl Read + Seek),
    start: u64,
    cancel: &AtomicBool,
) -> io::Result<Option<(u64, &'static str)>> {
    let Some(header) = element(input, start, u64::MAX, cancel)? else {
        return Ok(None);
    };
    if header.id != 0x1a45_dfa3 || header.end - header.data > MAX_EBML_HEADER as u64 {
        return Ok(None);
    }
    let mut at = header.data;
    let mut kind = None;
    while at < header.end {
        let Some(child) = element(input, at, header.end, cancel)? else {
            return Ok(None);
        };
        if child.id == 0x4282 {
            if kind.is_some() {
                return Ok(None);
            }
            let mut name = [0; MAX_EBML_HEADER];
            let Some(name) = name.get_mut(..bytes::to_usize(child.end - child.data)) else {
                return Ok(None);
            };
            if !read_at(input, child.data, name, cancel)? {
                return Ok(None);
            }
            kind = match name.split(|&byte| byte == 0).next() {
                Some(b"matroska") => Some("mkv"),
                Some(b"webm") => Some("webm"),
                _ => return Ok(None),
            };
        }
        at = child.end;
    }
    let Some(kind) = kind else {
        return Ok(None);
    };
    let Some(segment) = element(input, at, u64::MAX, cancel)? else {
        return Ok(None);
    };
    if segment.id != 0x1853_8067 {
        return Ok(None);
    }

    // The Segment and each direct child must have finite sizes.
    // Share the element budget across all inspected levels.
    at = segment.data;
    let mut seen = 0;
    let mut budget = 1_000_000;
    while at < segment.end {
        if budget == 0 {
            return Ok(None);
        }
        budget -= 1;
        let Some(child) = element(input, at, segment.end, cancel)? else {
            return Ok(None);
        };
        let bit = match child.id {
            0x1549_a966 => 1, // Info
            0x1654_ae6b => 2, // Tracks
            0x1f43_b675 => 4, // Cluster
            _ => 0,
        };
        if bit != 0 {
            let mut nested_at = child.data;
            if nested_at == child.end {
                return Ok(None);
            }
            while nested_at < child.end {
                if budget == 0 {
                    return Ok(None);
                }
                budget -= 1;
                let Some(nested) = element(input, nested_at, child.end, cancel)? else {
                    return Ok(None);
                };
                nested_at = nested.end;
            }
            seen |= bit;
        }
        at = child.end;
    }
    Ok((seen == 7).then_some((at - start, kind)))
}

pub(super) fn wave64(
    input: &mut (impl Read + Seek),
    start: u64,
    cancel: &AtomicBool,
) -> io::Result<Option<(u64, &'static str)>> {
    let mut header = [0; 48];
    if !read_at(input, start, &mut header, cancel)?
        || header.get(4..16) != Some(b"\xff\xff\xff\xffWAVEds64")
        || bytes::u32_le(&header, 16) != 28
        || bytes::u32_le(&header, 44) != 0
    {
        return Ok(None);
    }

    let kind = match header.get(..4) {
        Some(b"RF64") => "rf64",
        Some(b"BW64") => "bw64",
        _ => return Ok(None),
    };

    // EBU Tech 3306 and ITU-R BS.2088. ds64 replaces the RIFF and data size sentinels.
    let size = bytes::u64_le(&header, 20);
    let data_size = bytes::u64_le(&header, 28);
    let Some(end) = start.checked_add(8).and_then(|at| at.checked_add(size)) else {
        return Ok(None);
    };
    let Some(mut at) = start.checked_add(48) else {
        return Ok(None);
    };
    let mut format_seen = false;
    let mut data_seen = false;
    let mut chunk = [0; 8];
    for _ in 0..100_000 {
        if at == end {
            return Ok((format_seen && data_seen).then_some((end - start, kind)));
        }
        let Some(body) = at.checked_add(8).filter(|body| *body <= end) else {
            break;
        };
        if !read_at(input, at, &mut chunk, cancel)? {
            break;
        }
        let length = bytes::u32_le(&chunk, 4);
        let length = match (chunk.get(..4), length) {
            (Some(b"data"), u32::MAX) if format_seen && !data_seen && data_size != 0 => {
                data_seen = true;
                data_size
            }
            (Some(b"data"), _) | (_, u32::MAX) => break,
            (Some(b"fmt "), length) if length >= 16 && !format_seen => {
                format_seen = true;
                u64::from(length)
            }
            (Some(b"fmt "), _) => break,
            (_, length) => u64::from(length),
        };
        let Some(next) = length
            .checked_next_multiple_of(2)
            .and_then(|padded| body.checked_add(padded))
        else {
            break;
        };
        at = next;
    }
    Ok(None)
}
