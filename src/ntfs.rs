//! Limited NTFS 3-style record recovery. Only deleted base records with one
//! unnamed resident $DATA value are accepted. No cluster runs are guessed.
use crate::{Found, NtfsMetadata, Validation};
use std::{
    io::{self, Read},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_RECORD: usize = 16 * 1024;
fn short(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

// FILETIME is 100-nanosecond intervals since 1601-01-01 UTC. Zero means
// unavailable here; reject values beyond year 9999 rather than invent a date.
fn filetime(bytes: &[u8], at: usize) -> Option<i64> {
    let ticks = u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?);
    if ticks == 0 {
        return None;
    }
    let seconds = i64::try_from(ticks / 10_000_000)
        .ok()?
        .checked_sub(11_644_473_600)?;
    (seconds <= 253_402_300_799).then_some(seconds)
}

pub fn read_record(
    input: &mut impl Read,
    offset: u64,
    cancel: &AtomicBool,
) -> io::Result<Option<(Found, usize)>> {
    let mut header = [0u8; 48];
    if let Err(e) = input.read_exact(&mut header) {
        return if e.kind() == io::ErrorKind::UnexpectedEof {
            Ok(None)
        } else {
            Err(e)
        };
    }
    let size = long(&header, 28) as usize;
    if &header[..4] != b"FILE" || !size.is_power_of_two() || !(1024..=MAX_RECORD).contains(&size) {
        return Ok(None);
    }
    let mut record = vec![0; size];
    record[..48].copy_from_slice(&header);
    if let Err(e) = input.read_exact(&mut record[48..]) {
        return if e.kind() == io::ErrorKind::UnexpectedEof {
            Ok(None)
        } else {
            Err(e)
        };
    }
    Ok(parse_record(record, offset, cancel).map(|found| (found, size)))
}

fn parse_record(mut record: Vec<u8>, offset: u64, cancel: &AtomicBool) -> Option<Found> {
    if cancel.load(Ordering::Relaxed)
        || short(&record, 16) == 0
        || short(&record, 22) != 0
        || record[32..40] != [0; 8]
    {
        return None;
    }
    let used = long(&record, 24) as usize;
    let first = short(&record, 20) as usize;
    let usa = short(&record, 4) as usize;
    let count = short(&record, 6) as usize;
    if count < 2 || !record.len().is_multiple_of(count - 1) {
        return None;
    }
    let sector = record.len() / (count - 1);
    if !matches!(sector, 512 | 4096)
        || usa < 48
        || usa + count * 2 > first
        || first < 48
        || !first.is_multiple_of(8)
        || used > record.len()
        || used < first + 4
    {
        return None;
    }
    let sequence = short(&record, usa);
    if sequence == 0 {
        return None;
    }
    // Reject torn records before restoring the original sector-tail bytes.
    for index in 1..count {
        let tail = index * sector - 2;
        if short(&record, tail) != sequence {
            return None;
        }
        let original = [record[usa + index * 2], record[usa + index * 2 + 1]];
        record[tail..tail + 2].copy_from_slice(&original);
    }
    let mut at = first;
    let mut name: Option<(u8, String)> = None;
    let mut data = None;
    let mut terminated = false;
    let mut standard_info = false;
    let mut metadata = NtfsMetadata {
        record_number: long(&record, 44),
        sequence_number: short(&record, 16),
        ..Default::default()
    };
    while at + 4 <= used {
        let kind = long(&record, at);
        if kind == u32::MAX {
            terminated = true;
            break;
        }
        if at + 16 > used {
            return None;
        }
        let length = long(&record, at + 4) as usize;
        if length < 24
            || !length.is_multiple_of(8)
            || at.checked_add(length)? > used
            || !matches!(record[at + 8], 0 | 1)
        {
            return None;
        }
        let attr = &record[at..at + length];
        // Attribute lists and external data require full run-list reconstruction.
        if kind == 0x20 {
            return None;
        }
        if !matches!(
            kind,
            0x10 | 0x30
                | 0x40
                | 0x50
                | 0x60
                | 0x70
                | 0x80
                | 0x90
                | 0xa0
                | 0xb0
                | 0xc0
                | 0xd0
                | 0xe0
                | 0xf0
                | 0x100
        ) {
            return None;
        }
        if record[at + 8] == 0 && matches!(kind, 0x10 | 0x30 | 0x80) {
            if short(attr, 12) != 0 {
                return None;
            }
            let value_at = short(attr, 20) as usize;
            let value_len = long(attr, 16) as usize;
            if value_at < 24 || value_at.checked_add(value_len)? > length {
                return None;
            }
            let value = &attr[value_at..value_at + value_len];
            match kind {
                0x10 => {
                    if value.len() < 48 {
                        return None;
                    }
                    if standard_info {
                        return None;
                    }
                    standard_info = true;
                    metadata.created = filetime(value, 0);
                    metadata.modified = filetime(value, 8);
                    metadata.record_changed = filetime(value, 16);
                    metadata.accessed = filetime(value, 24);
                }
                0x30 => {
                    if attr[9] != 0 || value.len() < 66 {
                        return None;
                    }
                    let characters = value[64] as usize;
                    let namespace = value[65];
                    if characters == 0 || namespace > 3 || value.len() != 66 + characters * 2 {
                        return None;
                    }
                    let utf16: Vec<_> = value[66..]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|b| u16::from_le_bytes(*b))
                        .collect();
                    let candidate = String::from_utf16(&utf16).ok()?;
                    if candidate
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '/' | '\\'))
                        || matches!(candidate.as_str(), "." | "..")
                    {
                        return None;
                    }
                    let rank = match namespace {
                        1 | 3 => 3,
                        0 => 2,
                        _ => 1,
                    };
                    if name.as_ref().is_none_or(|(old, _)| rank > *old) {
                        name = Some((rank, candidate));
                    }
                }
                0x80 if attr[9] == 0 => {
                    if data.is_some() {
                        return None;
                    }
                    data = Some(Arc::<[u8]>::from(value));
                }
                _ => {}
            }
        } else if kind == 0x80 && attr[9] == 0 {
            return None;
        }
        at += length;
    }
    if !terminated || !standard_info {
        return None;
    }
    let (_, name) = name?;
    let data = data?;
    let kind = extension_kind(&name);
    let validation = match crate::validation::validate(kind, &data, cancel)? {
        Validation::ImageDecoded => Validation::ImageDecoded,
        _ => Validation::MetadataChecked,
    };
    Some(Found {
        offset,
        size: data.len() as u64,
        kind,
        selected: true,
        validation,
        original_name: Some(name),
        resident_data: Some(data),
        ntfs: Some(metadata),
    })
}

fn extension_kind(name: &str) -> &'static str {
    match name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "jpg",
        Some("png") => "png",
        Some("gif") => "gif",
        Some("bmp") => "bmp",
        Some("webp") => "webp",
        Some("ico") => "ico",
        Some("pdf") => "pdf",
        Some("zip") => "zip",
        Some("docx") => "docx",
        Some("xlsx") => "xlsx",
        Some("pptx") => "pptx",
        Some("docm") => "docm",
        Some("xlsm") => "xlsm",
        Some("pptm") => "pptm",
        Some("odt") => "odt",
        Some("ods") => "ods",
        Some("odp") => "odp",
        Some("epub") => "epub",
        Some("txt") => "txt",
        Some("csv") => "csv",
        Some("json") => "json",
        Some("xml") => "xml",
        Some("md") => "md",
        Some("html" | "htm") => "html",
        Some("log") => "log",
        Some("wav") => "wav",
        Some("aiff" | "aif") => "aiff",
        Some("au") => "au",
        Some("avi") => "avi",
        Some("mp3") => "mp3",
        Some("mp4") => "mp4",
        Some("jar") => "jar",
        Some("apk") => "apk",
        Some("exe") => "exe",
        _ => "bin",
    }
}
