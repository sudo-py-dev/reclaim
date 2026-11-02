use crate::{CandidateEnd, LIMIT, Validation};
use image::{ImageFormat, ImageReader};
use std::{
    io::Cursor,
    sync::atomic::{AtomicBool, Ordering},
};

pub fn validate(kind: &str, bytes: &[u8], cancel: &AtomicBool) -> Option<Validation> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    if crate::formats::supported(kind) {
        return crate::formats::validate(kind, bytes, cancel);
    }
    if kind == "sqlite" {
        return matches!(crate::detection::sqlite_end(bytes), CandidateEnd::Complete(n) if n == bytes.len())
            .then_some(Validation::StructureChecked);
    }
    if matches!(kind, "ogg" | "opus" | "ogv") {
        return matches!(crate::detection::Ogg::default().push(bytes, cancel), CandidateEnd::Complete(n) if n == bytes.len())
            .then_some(Validation::StructureChecked);
    }
    let boundary_valid = match kind {
        "png" => matches!(png_end(bytes), CandidateEnd::Complete(n) if n == bytes.len()),
        "gif" => matches!(gif_end(bytes), CandidateEnd::Complete(n) if n == bytes.len()),
        "jpg" => matches!(jpeg_end(bytes), CandidateEnd::Complete(n) if n == bytes.len()),
        "bmp" => {
            bytes.len() >= 18
                && &bytes[..2] == b"BM"
                && u32::from_le_bytes(bytes[2..6].try_into().unwrap()) as usize == bytes.len()
        }
        "webp" => {
            bytes.len() >= 12
                && &bytes[..4] == b"RIFF"
                && &bytes[8..12] == b"WEBP"
                && u64::from(u32::from_le_bytes(bytes[4..8].try_into().unwrap())) + 8
                    == bytes.len() as u64
        }
        _ => true,
    };
    if !boundary_valid {
        return None;
    }
    let format = match kind {
        "ico" => {
            return matches!(ico_end(bytes, cancel), CandidateEnd::Complete(n) if n == bytes.len())
                .then_some(Validation::ImageDecoded);
        }
        "png" => ImageFormat::Png,
        "jpg" => ImageFormat::Jpeg,
        "gif" => ImageFormat::Gif,
        "bmp" => ImageFormat::Bmp,
        "webp" => ImageFormat::WebP,
        _ => return Some(Validation::Unverified),
    };
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    use image::{AnimationDecoder, ImageDecoder};
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(256 * 1024 * 1024);
    let valid = if format == ImageFormat::Gif {
        let Ok(mut decoder) = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)) else {
            return None;
        };
        if decoder.set_limits(limits).is_err() {
            return None;
        }
        valid_animation(decoder.dimensions(), decoder.into_frames(), cancel)
    } else if format == ImageFormat::WebP {
        let Ok(mut decoder) = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)) else {
            return None;
        };
        if decoder.set_limits(limits).is_err() {
            return None;
        }
        if decoder.has_animation() {
            valid_animation(decoder.dimensions(), decoder.into_frames(), cancel)
        } else {
            decode(bytes, format, 32768)
        }
    } else {
        decode(bytes, format, 32768)
    };
    valid.then_some(Validation::ImageDecoded)
}

fn valid_animation(dimensions: (u32, u32), frames: image::Frames<'_>, cancel: &AtomicBool) -> bool {
    if u64::from(dimensions.0) * u64::from(dimensions.1) * 4 > 256 * 1024 * 1024 {
        return false;
    }
    let mut decoded = 0u64;
    let mut count = 0usize;
    for frame in frames {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(frame) = frame else {
            return false;
        };
        count += 1;
        decoded += frame.buffer().as_raw().len() as u64;
        if count > 4096 || decoded > 256 * 1024 * 1024 {
            return false;
        }
    }
    count > 0
}

fn decode(bytes: &[u8], format: ImageFormat, dimension_limit: u32) -> bool {
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(dimension_limit);
    limits.max_image_height = Some(dimension_limit);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().is_ok()
}

pub fn png_end(bytes: &[u8]) -> CandidateEnd {
    use CandidateEnd::*;
    let mut p = 8usize;
    let mut data_seen = false;
    loop {
        if bytes.len() < p + 8 {
            return More;
        }
        let len = u32::from_be_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
        let Some(end) = p
            .checked_add(12)
            .and_then(|p| p.checked_add(len))
            .filter(|n| *n <= LIMIT)
        else {
            return Invalid;
        };
        let kind = &bytes[p + 4..p + 8];
        if !kind.iter().all(u8::is_ascii_alphabetic)
            || (p == 8 && (kind != b"IHDR" || len != 13))
            || (p != 8 && kind == b"IHDR")
        {
            return Invalid;
        }
        if bytes.len() < end {
            return More;
        }
        let expected = u32::from_be_bytes(bytes[end - 4..end].try_into().unwrap());
        if crc32fast::hash(&bytes[p + 4..end - 4]) != expected {
            return Invalid;
        }
        if kind == b"IDAT" {
            data_seen = true;
        }
        if kind == b"IEND" {
            return if len == 0 && data_seen {
                Complete(end)
            } else {
                Invalid
            };
        }
        p = end;
    }
}

pub fn gif_end(bytes: &[u8]) -> CandidateEnd {
    use CandidateEnd::*;
    if bytes.len() < 13 {
        return More;
    }
    if bytes[6..8] == [0, 0] || bytes[8..10] == [0, 0] {
        return Invalid;
    }
    let mut p = 13
        + if bytes[10] & 0x80 != 0 {
            3 * (1 << ((bytes[10] & 7) + 1))
        } else {
            0
        };
    let mut image_seen = false;
    loop {
        if bytes.len() <= p {
            return More;
        }
        match bytes[p] {
            0x3b => return if image_seen { Complete(p + 1) } else { Invalid },
            0x21 => {
                if bytes.len() < p + 2 {
                    return More;
                }
                p += 2;
            }
            0x2c => {
                if bytes.len() < p + 10 {
                    return More;
                }
                if bytes[p + 5..p + 7] == [0, 0] || bytes[p + 7..p + 9] == [0, 0] {
                    return Invalid;
                }
                let packed = bytes[p + 9];
                p += 10
                    + if packed & 0x80 != 0 {
                        3 * (1 << ((packed & 7) + 1))
                    } else {
                        0
                    };
                if bytes.len() <= p {
                    return More;
                }
                if !(2..=8).contains(&bytes[p]) {
                    return Invalid;
                }
                p += 1;
                image_seen = true;
            }
            _ => return Invalid,
        }
        loop {
            if bytes.len() <= p {
                return More;
            }
            let count = bytes[p] as usize;
            p += 1;
            if count == 0 {
                break;
            }
            p += count;
            if p > bytes.len() {
                return More;
            }
        }
    }
}

pub fn jpeg_end(bytes: &[u8]) -> CandidateEnd {
    use CandidateEnd::*;
    let mut p = 2;
    let mut entropy = false;
    let mut scan_seen = false;
    loop {
        if entropy {
            while p < bytes.len() {
                if bytes[p] != 0xff {
                    p += 1;
                    continue;
                }
                if bytes.len() <= p + 1 {
                    return More;
                }
                match bytes[p + 1] {
                    0x00 | 0xd0..=0xd7 => p += 2,
                    0xff => p += 1,
                    _ => {
                        entropy = false;
                        break;
                    }
                }
            }
        }
        if bytes.len() <= p + 1 {
            return More;
        }
        if bytes[p] != 0xff {
            return Invalid;
        }
        while p < bytes.len() && bytes[p] == 0xff {
            p += 1;
        }
        if bytes.len() <= p {
            return More;
        }
        let marker = bytes[p];
        p += 1;
        match marker {
            0xd9 => return if scan_seen { Complete(p) } else { Invalid },
            0x01 | 0xd0..=0xd7 => continue,
            0x00 | 0xd8 => return Invalid,
            _ => {}
        }
        if bytes.len() < p + 2 {
            return More;
        }
        let len = u16::from_be_bytes([bytes[p], bytes[p + 1]]) as usize;
        if len < 2 {
            return Invalid;
        }
        p += len;
        if p > LIMIT {
            return Invalid;
        }
        if p > bytes.len() {
            return More;
        }
        if marker == 0xda {
            entropy = true;
            scan_seen = true;
        }
    }
}

pub fn ico_end(bytes: &[u8], cancel: &AtomicBool) -> CandidateEnd {
    use CandidateEnd::*;
    if bytes.len() >= 4 && &bytes[..4] != b"\0\0\x01\0" {
        return Invalid;
    }
    if bytes.len() < 6 {
        return More;
    }
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    if count == 0 || count > 256 {
        return Invalid;
    }
    let directory = 6 + count * 16;
    if bytes.len() < directory {
        return More;
    }
    let mut ranges = Vec::with_capacity(count);
    for entry in bytes[6..directory].as_chunks::<16>().0 {
        let planes = u16::from_le_bytes(entry[4..6].try_into().unwrap());
        let bits = u16::from_le_bytes(entry[6..8].try_into().unwrap());
        let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        let Some(end) = offset.checked_add(size).filter(|n| *n <= LIMIT) else {
            return Invalid;
        };
        if !matches!(entry[3], 0 | 255)
            || planes > 1
            || !matches!(bits, 0 | 1 | 4 | 8 | 16 | 24 | 32)
            || size < 16
            || offset < directory
        {
            return Invalid;
        }
        if bytes.len() >= offset + 16 {
            let payload = &bytes[offset..];
            if !payload.starts_with(b"\x89PNG\r\n\x1a\n")
                && !matches!(
                    u32::from_le_bytes(payload[..4].try_into().unwrap()),
                    12 | 40 | 108 | 124
                )
            {
                return Invalid;
            }
        }
        ranges.push((offset, end));
    }
    let mut sorted = ranges.clone();
    sorted.sort_unstable();
    if sorted
        .windows(2)
        .any(|pair| pair[1].0 < pair[0].1 && pair[0] != pair[1])
    {
        return Invalid;
    }
    let end = sorted.last().unwrap().1;
    if bytes.len() < end {
        return More;
    }
    for (entry, (offset, end)) in bytes[6..directory].as_chunks::<16>().0.iter().zip(ranges) {
        if cancel.load(Ordering::Relaxed) {
            return Invalid;
        }
        let payload = &bytes[offset..end];
        let dimension = |n: u8| if n == 0 { 256 } else { u32::from(n) };
        let (width, height) = (dimension(entry[0]), dimension(entry[1]));
        if payload.starts_with(b"\x89PNG\r\n\x1a\n") {
            if !matches!(png_end(payload),Complete(n) if n == payload.len())
                || payload.len() < 24
                || u32::from_be_bytes(payload[16..20].try_into().unwrap()) != width
                || u32::from_be_bytes(payload[20..24].try_into().unwrap()) != height
            {
                return Invalid;
            }
        } else {
            let header = u32::from_le_bytes(payload[..4].try_into().unwrap());
            let (w, h) = if header == 12 {
                (
                    u32::from(u16::from_le_bytes(payload[4..6].try_into().unwrap())),
                    u32::from(u16::from_le_bytes(payload[6..8].try_into().unwrap())),
                )
            } else {
                (
                    u32::from_le_bytes(payload[4..8].try_into().unwrap()),
                    u32::from_le_bytes(payload[8..12].try_into().unwrap()),
                )
            };
            if w != width || h != height * 2 {
                return Invalid;
            }
            let (planes_at, bits_at) = if header == 12 { (8, 10) } else { (12, 14) };
            let planes = u16::from_le_bytes(payload[planes_at..planes_at + 2].try_into().unwrap());
            let bits = u16::from_le_bytes(payload[bits_at..bits_at + 2].try_into().unwrap());
            let declared_bits = u16::from_le_bytes(entry[6..8].try_into().unwrap());
            if planes != 1 || (declared_bits != 0 && declared_bits != bits) {
                return Invalid;
            }
        }
        let mut frame = Vec::with_capacity(22 + payload.len());
        frame.extend_from_slice(b"\0\0\x01\0\x01\0");
        frame.extend_from_slice(entry);
        frame[18..22].copy_from_slice(&22u32.to_le_bytes());
        frame.extend_from_slice(payload);
        if !decode(&frame, ImageFormat::Ico, 256) {
            return Invalid;
        }
    }
    Complete(end)
}
