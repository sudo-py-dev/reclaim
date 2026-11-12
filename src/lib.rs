#![forbid(unsafe_code)]

use crate::bytes::to_usize;
mod allocation;
mod bytes;
mod detection;
mod formats;
use detection::{CandidateEnd, LIMIT};
pub mod i18n;
mod ntfs;
mod resilient;
mod source;
pub mod updater;
mod validation;
mod zip;
pub use source::SourceReader;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

/// Formats currently recognized by the scanner, grouped for the interface.
pub const FILE_CATEGORIES: &[&str] = &[
    "All files",
    "Pictures",
    "Documents",
    "Archives",
    "Audio",
    "Video",
    "Applications",
    "Icons",
];
pub fn category_index(kind: &str) -> usize {
    match kind {
        "jpg" | "png" | "gif" | "bmp" | "webp" | "qoi" => 1,
        "ico" => 7,
        "pdf" | "docx" | "xlsx" | "pptx" | "docm" | "xlsm" | "pptm" | "odt" | "ods" | "odp"
        | "epub" | "ole" | "txt" | "csv" | "json" | "xml" | "md" | "html" | "log" | "sqlite" => 2,
        "zip" | "7z" | "cab" | "rar" | "tar" | "gz" | "xar" => 3,
        "wav" | "aiff" | "aifc" | "au" | "mp3" | "ogg" | "opus" | "mid" => 4,
        "avi" | "mp4" | "ogv" => 5,
        "jar" | "apk" | "exe" | "dll" | "elf" | "msi" | "msp" | "mst" | "deb" | "rpm" | "dex"
        | "pkg" | "swf" => 6,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Validation {
    ImageDecoded,
    MetadataChecked,
    StructureChecked,
    Unverified,
}
impl Validation {
    pub fn label(self) -> &'static str {
        match self {
            Self::ImageDecoded => "Image decoded",
            Self::MetadataChecked => "NTFS record checked",
            Self::StructureChecked => "Structure checked",
            Self::Unverified => "Unverified candidate",
        }
    }
}
/// Times captured from a checked deleted NTFS record, as UTC Unix seconds.
/// NTFS standard information does not supply a deletion timestamp.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NtfsMetadata {
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub record_changed: Option<i64>,
    pub accessed: Option<i64>,
    pub record_number: u32,
    pub sequence_number: u16,
}
#[derive(Clone, Debug)]
pub struct Found {
    pub offset: u64,
    pub size: u64,
    pub kind: &'static str,
    pub selected: bool,
    pub validation: Validation,
    pub original_name: Option<String>,
    pub resident_data: Option<std::sync::Arc<[u8]>>,
    pub ntfs: Option<NtfsMetadata>,
}
impl Found {
    /// A portable original filename when available, otherwise a generated name.
    /// The number is the position in the complete scan, independent of filtering.
    pub fn recovery_name(&self, number: usize) -> String {
        if let Some(name) = &self.original_name {
            return portable_name(name);
        }
        let label = match category_index(self.kind) {
            1 => "Image",
            2 => "Document",
            3 => "Archive",
            4 => "Audio",
            5 => "Video",
            6 => "Application",
            7 => "Icon",
            _ => "File",
        };
        portable_name(&format!("{label} {number:04}.{}", self.kind))
    }
}
fn portable_name(name: &str) -> String {
    let mut name: String = name
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    name = name.trim_end_matches([' ', '.']).to_owned();
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        name.insert(0, '_');
    }
    if name.is_empty() {
        name = "Recovered file".into();
    }
    if name.len() > 200 {
        let ext = name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_owned())
            .filter(|ext| ext.len() <= 20);
        let mut end = 170.min(name.len());
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        name.truncate(end);
        if let Some(ext) = ext {
            name.push('.');
            name.push_str(&ext);
        }
    }
    name
}

pub enum Event {
    /// Absolute source byte range that could not be read (offset, length).
    Unreadable {
        offset: u64,
        bytes: u64,
    },
    /// Total unallocated bytes from the selected volume's filesystem map.
    UnusedSpace {
        bytes: u64,
        filesystem: &'static str,
    },
    Progress(u64),
    Checking {
        offset: u64,
        kind: &'static str,
        bytes: usize,
    },
    Found(Found),
}
struct Format {
    head: &'static [u8],
    tail: &'static [u8],
    ext: &'static str,
}
static FORMATS: &[Format] = &[
    Format {
        head: b"MThd",
        tail: b"",
        ext: "mid",
    },
    Format {
        head: b"FWS",
        tail: b"",
        ext: "swf",
    },
    Format {
        head: b"\x1f\x8b\x08",
        tail: b"",
        ext: "gz",
    },
    Format {
        head: b"xar!",
        tail: b"",
        ext: "xar",
    },
    Format {
        head: b"MZ",
        tail: b"",
        ext: "pe",
    },
    Format {
        head: b"\x7fELF",
        tail: b"",
        ext: "elf",
    },
    Format {
        head: b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1",
        tail: b"",
        ext: "cfb",
    },
    Format {
        head: b"!<arch>\n",
        tail: b"",
        ext: "deb",
    },
    Format {
        head: b"\xed\xab\xee\xdb",
        tail: b"",
        ext: "rpm",
    },
    Format {
        head: b"7z\xbc\xaf\x27\x1c",
        tail: b"",
        ext: "7z",
    },
    Format {
        head: b"MSCF",
        tail: b"",
        ext: "cab",
    },
    Format {
        head: b"Rar!\x1a\x07\x01\0",
        tail: b"",
        ext: "rar",
    },
    Format {
        head: b"qoif",
        tail: b"",
        ext: "qoi",
    },
    Format {
        head: b"dex\n",
        tail: b"",
        ext: "dex",
    },
    Format {
        head: b"FORM",
        tail: b"",
        ext: "aiff",
    },
    Format {
        head: b".snd",
        tail: b"",
        ext: "au",
    },
    Format {
        head: b"\0\0\x01\0",
        tail: b"",
        ext: "ico",
    },
    Format {
        head: b"RIFF",
        tail: b"",
        ext: "riff",
    },
    Format {
        head: b"BM",
        tail: b"",
        ext: "bmp",
    },
    Format {
        head: b"PK\x03\x04",
        tail: b"PK\x05\x06",
        ext: "zip",
    },
    Format {
        head: b"\xff\xd8\xff",
        tail: b"\xff\xd9",
        ext: "jpg",
    },
    Format {
        head: b"\x89PNG\r\n\x1a\n",
        tail: b"\0\0\0\0IEND\xaeB`\x82",
        ext: "png",
    },
    Format {
        head: b"%PDF-",
        tail: b"%%EOF",
        ext: "pdf",
    },
    Format {
        head: b"GIF89a",
        tail: b"\x00\x3b",
        ext: "gif",
    },
    Format {
        head: b"GIF87a",
        tail: b"\x00\x3b",
        ext: "gif",
    },
    Format {
        head: b"OggS",
        tail: b"",
        ext: "ogg",
    },
    Format {
        head: b"SQLite format 3\0",
        tail: b"",
        ext: "sqlite",
    },
];

// Constant-time first-byte dispatch; adding formats cannot stale manual ranges.
const fn signature_buckets() -> [u64; 256] {
    let mut buckets = [0u64; 256];
    let mut i = 0;
    while i < FORMATS.len() {
        buckets[FORMATS[i].head[0] as usize] |= 1u64 << i;
        i += 1;
    }
    buckets
}
const SIGNATURE_BUCKETS: [u64; 256] = signature_buckets();
static TAR_FORMAT: Format = Format {
    head: b"ustar",
    tail: b"",
    ext: "tar",
};
fn detect_format(bytes: &[u8]) -> Option<&'static Format> {
    let mut bucket = SIGNATURE_BUCKETS[*bytes.first()? as usize];
    while bucket != 0 {
        let index = bucket.trailing_zeros() as usize;
        bucket &= bucket - 1;
        let format = &FORMATS[index];
        if bytes.starts_with(format.head) && formats::plausible(format.ext, bytes) {
            return Some(format);
        }
    }
    if bytes.get(257..262) == Some(b"ustar".as_slice()) {
        Some(&TAR_FORMAT)
    } else {
        None
    }
}

fn candidate_end(fmt: &Format, bytes: &[u8], from: usize, cancel: &AtomicBool) -> CandidateEnd {
    if let Some(end) = formats::end(fmt.ext, bytes, cancel) {
        return end;
    }
    match fmt.ext {
        "sqlite" => return detection::sqlite_end(bytes),
        "ico" => return validation::ico_end(bytes, cancel),
        "png" => return validation::png_end(bytes),
        "gif" => return validation::gif_end(bytes),
        "jpg" => return validation::jpeg_end(bytes),
        _ => {}
    }
    if fmt.tail.is_empty() {
        let size = match fmt.ext {
            "riff" | "aiff" => {
                if bytes.len() < 12 {
                    return CandidateEnd::More;
                }
                let kind = &bytes[8..12];
                let n = if fmt.ext == "riff" {
                    if !matches!(kind, b"WAVE" | b"WEBP" | b"AVI ") {
                        return CandidateEnd::Invalid;
                    }
                    bytes::u32_le(bytes, 4)
                } else {
                    if !matches!(kind, b"AIFF" | b"AIFC") {
                        return CandidateEnd::Invalid;
                    }
                    bytes::u32_be(bytes, 4)
                };
                if n < 4 {
                    return CandidateEnd::Invalid;
                }
                u64::from(n) + 8
            }
            "au" => {
                if bytes.len() < 24 {
                    return CandidateEnd::More;
                }
                let read = |i| bytes::u32_be(bytes, i);
                let (offset, data) = (read(4), read(8));
                // Unknown-length AU streams cannot be bounded reliably in a disk image.
                if offset < 24 || data == u32::MAX || data == 0 || read(16) == 0 || read(20) == 0 {
                    return CandidateEnd::Invalid;
                }
                u64::from(offset) + u64::from(data)
            }
            "bmp" => {
                if bytes.len() < 14 {
                    return CandidateEnd::More;
                }
                if bytes[6..10] != [0; 4] {
                    return CandidateEnd::Invalid;
                }
                let n = bytes::u32_le(bytes, 2);
                if n < 14 {
                    return CandidateEnd::Invalid;
                }
                if bytes.len() < 18 {
                    return CandidateEnd::More;
                }
                let dib = bytes::u32_le(bytes, 14);
                let pixel_offset = bytes::u32_le(bytes, 10);
                if !matches!(dib, 12 | 40 | 52 | 56 | 64 | 108 | 124)
                    || u64::from(dib) + 14 > u64::from(n)
                    || u64::from(pixel_offset) < u64::from(dib) + 14
                    || pixel_offset >= n
                {
                    return CandidateEnd::Invalid;
                }
                u64::from(n)
            }
            _ => return CandidateEnd::Invalid,
        };
        if size > LIMIT as u64 {
            return CandidateEnd::Invalid;
        }
        return if bytes.len() >= to_usize(size) {
            CandidateEnd::Complete(to_usize(size))
        } else {
            CandidateEnd::More
        };
    }
    // ZIP end-of-central-directory includes a fixed record and optional comment.
    // Search the full record overlap when the EOCD crosses a read boundary.
    let from = if fmt.ext == "zip" {
        from.saturating_sub(65557)
    } else {
        from
    };
    for (index, window) in bytes[from..].windows(fmt.tail.len()).enumerate() {
        if window != fmt.tail {
            continue;
        }
        let end = from + index + fmt.tail.len();
        if fmt.ext != "zip" {
            return CandidateEnd::Complete(end);
        }
        let start = end - 4;
        if bytes.len() < start + 22 {
            return CandidateEnd::More;
        }
        let comment = u16::from_le_bytes([bytes[start + 20], bytes[start + 21]]) as usize;
        let size = start + 22 + comment;
        if size > LIMIT {
            return CandidateEnd::Invalid;
        }
        if bytes.len() >= size && valid_zip(&bytes[..size], start) {
            return CandidateEnd::Complete(size);
        }
    }
    CandidateEnd::More
}

fn valid_zip(bytes: &[u8], eocd: usize) -> bool {
    if eocd + 22 > bytes.len() {
        return false;
    }
    let short = |p| u16::from_le_bytes([bytes[p], bytes[p + 1]]) as usize;
    let long = |p| bytes::u32_le(bytes, p) as usize;
    if eocd + 22 + short(eocd + 20) != bytes.len() {
        return false;
    }
    let count = short(eocd + 10);
    if count == 0
        || count == 65535
        || short(eocd + 4) != 0
        || short(eocd + 6) != 0
        || short(eocd + 8) != count
    {
        return false;
    }
    let mut cursor = long(eocd + 16);
    if cursor.checked_add(long(eocd + 12)) != Some(eocd) {
        return false;
    }
    for _ in 0..count {
        if cursor + 46 > eocd
            || &bytes[cursor..cursor + 4] != b"PK\x01\x02"
            || short(cursor + 34) != 0
        {
            return false;
        }
        let name_len = short(cursor + 28);
        let next = cursor + 46 + name_len + short(cursor + 30) + short(cursor + 32);
        if next > eocd {
            return false;
        }
        let local = long(cursor + 42);
        if local.checked_add(30).is_none_or(|end| end > eocd)
            || &bytes[local..local + 4] != b"PK\x03\x04"
        {
            return false;
        }
        let local_name_end = local + 30 + short(local + 26);
        let data = local_name_end + short(local + 28);
        if data > eocd
            || bytes[local + 30..local_name_end] != bytes[cursor + 46..cursor + 46 + name_len]
            || data
                .checked_add(long(cursor + 20))
                .is_none_or(|end| end > eocd)
        {
            return false;
        }
        cursor = next;
    }
    cursor == eocd
}

fn candidate_kind(fmt: &Format, bytes: &[u8]) -> &'static str {
    match fmt.ext {
        "riff" => match &bytes[8..12] {
            b"WEBP" => "webp",
            b"AVI " => "avi",
            _ => "wav",
        },
        "aiff" if &bytes[8..12] == b"AIFC" => "aifc",
        "zip" => zip_kind(bytes),
        _ => formats::kind(fmt.ext, bytes),
    }
}

// Only classify packages from a complete, single-disk classic ZIP directory.
// Names appearing in compressed payloads must not change the recovered extension.
fn zip_kind(bytes: &[u8]) -> &'static str {
    // The classic EOCD is 22 bytes plus at most a u16-sized comment.
    let search_start = bytes.len().saturating_sub(22 + u16::MAX as usize);
    let Some((relative, _)) = bytes[search_start..]
        .windows(4)
        .enumerate()
        .rfind(|(i, w)| *w == b"PK\x05\x06" && valid_zip(bytes, search_start + *i))
    else {
        return "zip";
    };
    let eocd = search_start + relative;
    // valid_zip already checked the complete directory and local name ranges.
    let u16_at = |p| u16::from_le_bytes([bytes[p], bytes[p + 1]]) as usize;
    let u32_at = |p| bytes::u32_le(bytes, p) as usize;
    let count = u16_at(eocd + 10);
    let mut cursor = u32_at(eocd + 16);
    const PACKAGE_NAMES: &[&[u8]] = &[
        b"[Content_Types].xml",
        b"word/document.xml",
        b"word/vbaProject.bin",
        b"xl/workbook.xml",
        b"xl/vbaProject.bin",
        b"ppt/presentation.xml",
        b"ppt/vbaProject.bin",
        b"META-INF/container.xml",
        b"AndroidManifest.xml",
        b"classes.dex",
        b"META-INF/MANIFEST.MF",
    ];
    let mut names = [false; PACKAGE_NAMES.len()];
    let mut mime = None;
    for _ in 0..count {
        let name_len = u16_at(cursor + 28);
        let next = cursor + 46 + name_len + u16_at(cursor + 30) + u16_at(cursor + 32);
        let name = &bytes[cursor + 46..cursor + 46 + name_len];
        if let Some(index) = PACKAGE_NAMES.iter().position(|&known| known == name) {
            names[index] = true;
        }
        if name == b"mimetype" {
            let local = u32_at(cursor + 42);
            if local.checked_add(30).is_some_and(|end| end <= eocd)
                && &bytes[local..local + 4] == b"PK\x03\x04"
                && u16_at(local + 8) == 0
                && u16_at(local + 6) & 1 == 0
            {
                let start = local + 30 + u16_at(local + 26) + u16_at(local + 28);
                let len = u32_at(cursor + 24);
                if start.checked_add(len).is_some_and(|end| end <= eocd) {
                    mime = Some(&bytes[start..start + len]);
                }
            }
        }
        cursor = next;
    }
    let has = |name: &[u8]| {
        PACKAGE_NAMES
            .iter()
            .position(|&known| known == name)
            .is_some_and(|index| names[index])
    };
    if has(b"[Content_Types].xml") {
        if has(b"word/document.xml") {
            return if has(b"word/vbaProject.bin") {
                "docm"
            } else {
                "docx"
            };
        }
        if has(b"xl/workbook.xml") {
            return if has(b"xl/vbaProject.bin") {
                "xlsm"
            } else {
                "xlsx"
            };
        }
        if has(b"ppt/presentation.xml") {
            return if has(b"ppt/vbaProject.bin") {
                "pptm"
            } else {
                "pptx"
            };
        }
    }
    match mime {
        Some(b"application/epub+zip") if has(b"META-INF/container.xml") => return "epub",
        Some(b"application/vnd.oasis.opendocument.text") => return "odt",
        Some(b"application/vnd.oasis.opendocument.spreadsheet") => return "ods",
        Some(b"application/vnd.oasis.opendocument.presentation") => return "odp",
        _ => {}
    }
    if has(b"AndroidManifest.xml") && has(b"classes.dex") {
        return "apk";
    }
    if has(b"META-INF/MANIFEST.MF") {
        return "jar";
    }
    "zip"
}

// Interrupted reads must not abort a scan or discard an in-flight candidate.
fn read_chunk(input: &mut impl Read, bytes: &mut [u8], cancel: &AtomicBool) -> io::Result<usize> {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(0);
        }
        match input.read(bytes) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

/// Bounded-memory signature carving. Candidates are not proof of valid/deleted files.
pub fn scan(path: &Path, cancel: &AtomicBool, event: impl FnMut(Event)) -> io::Result<()> {
    scan_reader(&mut File::open(path)?, cancel, event)
}

/// Search only free allocation clusters in a supported formatted volume.
/// Filesystem metadata is read to establish the map. Unsupported maps fail
/// without falling back to occupied-space carving.
pub fn scan_unused_reader(
    input: &mut (impl Read + Seek),
    total: u64,
    cancel: &AtomicBool,
    event: impl FnMut(Event),
) -> io::Result<()> {
    allocation::scan(input, total, cancel, event)
}

pub fn scan_reader(
    file: &mut (impl Read + Seek),
    cancel: &AtomicBool,
    event: impl FnMut(Event),
) -> io::Result<()> {
    scan_reader_with_buffer(file, cancel, event, 1024 * 1024)
}

pub(crate) fn scan_reader_with_buffer(
    file: &mut (impl Read + Seek),
    cancel: &AtomicBool,
    event: impl FnMut(Event),
    buffer_size: usize,
) -> io::Result<()> {
    scan_reader_with_filter(file, cancel, event, buffer_size, |_, _| true)
}

pub(crate) fn scan_reader_with_filter(
    file: &mut (impl Read + Seek),
    cancel: &AtomicBool,
    mut event: impl FnMut(Event),
    buffer_size: usize,
    mut readable: impl FnMut(u64, u64) -> bool,
) -> io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0; buffer_size.clamp(262, 1024 * 1024)];
    let mut carry = Vec::new();
    let signature_length = FORMATS
        .iter()
        .map(|f| f.head.len())
        .max()
        .unwrap_or(1)
        .max(262);
    let mut base = 0u64;
    let mut found_count = 0usize;
    let mut skip_until = 0u64;
    let mut resident_bytes = 0usize;
    let mut last_progress = std::time::Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let n = read_chunk(file, &mut buffer, cancel)?;
        carry.extend_from_slice(&buffer[..n]);
        let mut i = 0;
        while (n == 0 && i < carry.len()) || i + signature_length <= carry.len() {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            if base + (i as u64) < skip_until {
                i = to_usize((skip_until - base).min(carry.len() as u64));
                continue;
            }
            // Empty disk regions are common. Only ICO begins with zero bytes;
            // retain the last two zeros so an ICO spanning the run survives.
            if carry[i..].starts_with(&[0; 16]) {
                let zeros = carry[i..]
                    .iter()
                    .position(|b| *b != 0)
                    .unwrap_or(carry.len() - i);
                i += zeros - 2;
                continue;
            }
            let offset = base + i as u64;
            if i.is_multiple_of(64 * 1024)
                && last_progress.elapsed() >= std::time::Duration::from_millis(250)
            {
                event(Event::Progress(offset));
                last_progress = std::time::Instant::now();
            }
            if offset.is_multiple_of(512) && carry[i..].starts_with(b"FILE") {
                let position = file.stream_position()?;
                file.seek(SeekFrom::Start(offset))?;
                let result = ntfs::read_record(file, offset, cancel);
                file.seek(SeekFrom::Start(position))?;
                if let Some((found, record_size)) = result?
                    && readable(offset, record_size as u64)
                {
                    skip_until = offset + record_size as u64;
                    resident_bytes += to_usize(found.size);
                    found_count += 1;
                    if resident_bytes > LIMIT || found_count > 100_000 {
                        return Err(io::Error::other(
                            "Recovery result limit reached. Available results can still be exported.",
                        ));
                    }
                    event(Event::Found(found));
                }
            }
            if let Some(fmt) = detect_format(&carry[i..]) {
                // Read candidate independently so the main streaming buffer stays bounded.
                let offset = base + i as u64;
                let position = file.stream_position()?;
                file.seek(SeekFrom::Start(offset))?;
                let mut candidate = Vec::new();
                let mut ogg = detection::Ogg::default();
                let mut gzip = formats::Gzip::default();
                let mut end = None;
                let mut kind = fmt.ext;
                let mut checked = Validation::Unverified;
                let mut chunk = vec![0u8; 8192];
                let mut rejected_zip = false;
                let mut candidate_limit = LIMIT;
                if fmt.ext == "zip" {
                    event(Event::Checking {
                        offset,
                        kind: "zip",
                        bytes: 0,
                    });
                    match zip::inspect(file, offset, cancel)? {
                        zip::Inspection::Complete(size) if size > LIMIT as u64 => {
                            end = Some(to_usize(size));
                            checked = Validation::StructureChecked;
                        }
                        zip::Inspection::Complete(size) => candidate_limit = to_usize(size),
                        zip::Inspection::Invalid => rejected_zip = true,
                        zip::Inspection::Unsupported => {}
                    }
                    file.seek(SeekFrom::Start(offset))?;
                }
                while !rejected_zip
                    && end.is_none()
                    && candidate.len() < candidate_limit
                    && !cancel.load(Ordering::Relaxed)
                {
                    if last_progress.elapsed() >= std::time::Duration::from_millis(250) {
                        event(Event::Checking {
                            offset,
                            kind: fmt.ext,
                            bytes: candidate.len(),
                        });
                        last_progress = std::time::Instant::now();
                    }
                    let remaining = (candidate_limit - candidate.len()).min(chunk.len());
                    let count = read_chunk(file, &mut chunk[..remaining], cancel)?;
                    if count == 0 {
                        break;
                    }
                    let from = candidate.len().saturating_sub(fmt.tail.len());
                    candidate.extend_from_slice(&chunk[..count]);
                    let boundary = if fmt.ext == "ogg" {
                        ogg.push(&candidate, cancel)
                    } else if fmt.ext == "gz" {
                        gzip.push(&candidate, cancel)
                    } else {
                        candidate_end(fmt, &candidate, from, cancel)
                    };
                    match boundary {
                        CandidateEnd::Complete(size) => {
                            kind = if fmt.ext == "ogg" {
                                ogg.kind()
                            } else {
                                candidate_kind(fmt, &candidate[..size])
                            };
                            event(Event::Checking {
                                offset,
                                kind,
                                bytes: size,
                            });
                            if let Some(validation) =
                                validation::validate(kind, &candidate[..size], cancel)
                            {
                                checked = validation;
                                end = Some(size);
                            }
                            break;
                        }
                        CandidateEnd::Invalid => break,
                        CandidateEnd::More => {
                            if chunk.len() < 1024 * 1024 {
                                chunk.resize(1024 * 1024, 0);
                            }
                        }
                    }
                }
                file.seek(SeekFrom::Start(position))?;
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                if let Some(size) = end
                    && readable(offset, size as u64)
                {
                    if matches!(
                        checked,
                        Validation::ImageDecoded | Validation::StructureChecked
                    ) || fmt.ext == "zip"
                    {
                        skip_until = offset + size as u64;
                    }
                    found_count += 1;
                    if found_count > 100_000 {
                        return Err(io::Error::other(
                            "Candidate limit reached (100,000). Recover available results or scan a smaller image.",
                        ));
                    }
                    event(Event::Found(Found {
                        offset,
                        size: size as u64,
                        kind,
                        selected: true,
                        validation: checked,
                        original_name: None,
                        resident_data: None,
                        ntfs: None,
                    }));
                }
            }
            i += 1;
        }
        carry.drain(..i);
        base += i as u64;
        event(Event::Progress(base));
        if n == 0 {
            break;
        }
    }
    Ok(())
}

/// Never overwrite an existing output; open the source only for reading.
pub fn recover(source: &Path, destination: &Path, found: &Found) -> io::Result<()> {
    recover_reader(&mut File::open(source)?, destination, found)
}

/// Export without overwriting: add a numeric suffix when a name already exists.
pub fn recover_unique(
    input: &mut (impl Read + Seek),
    directory: &Path,
    found: &Found,
    number: usize,
) -> io::Result<std::path::PathBuf> {
    let name = found.recovery_name(number);
    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let extension = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();
    for attempt in 0..10_000 {
        let name = if attempt == 0 {
            name.clone()
        } else {
            format!("{stem} ({}){extension}", attempt + 1)
        };
        let destination = directory.join(name);
        match recover_reader(input, &destination, found) {
            Ok(()) => return Ok(destination),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Too many files with the same recovered name",
    ))
}

/// Recover through the same authorized source handle used for scanning.
pub fn recover_reader(
    input: &mut (impl Read + Seek),
    destination: &Path,
    found: &Found,
) -> io::Result<()> {
    if (found.size == 0 && found.resident_data.is_none())
        || (found.size > LIMIT as u64
            && !(found.kind == "zip" && found.size <= u32::MAX as u64 + 65557))
        || found
            .resident_data
            .as_ref()
            .is_some_and(|bytes| bytes.len() as u64 != found.size)
        || found.offset.checked_add(found.size).is_none()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Candidate range is invalid",
        ));
    }
    if found.resident_data.is_none() {
        input.seek(SeekFrom::Start(found.offset))?;
    }
    // Keep incomplete or unvalidated data private. Publish without replacing an
    // existing entry, including a symlink, only after all checks succeed.
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut output = tempfile::NamedTempFile::new_in(parent)?;
    let copied = if let Some(bytes) = &found.resident_data {
        io::copy(&mut bytes.as_ref(), &mut output)?
    } else {
        io::copy(&mut input.take(found.size), &mut output)?
    };
    if copied != found.size {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Source changed or candidate is truncated",
        ));
    }
    if found.kind == "zip" && found.size > LIMIT as u64 {
        if zip::large_end(output.as_file_mut(), 0, &AtomicBool::new(false))? != Some(found.size) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Recovered ZIP failed directory validation",
            ));
        }
    } else if matches!(
        found.validation,
        Validation::ImageDecoded | Validation::StructureChecked
    ) || formats::supported(found.kind)
    {
        output.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        output
            .by_ref()
            .take(found.size.saturating_add(1))
            .read_to_end(&mut bytes)?;
        let checked = validation::validate(found.kind, &bytes, &AtomicBool::new(false));
        let resident_metadata = found.validation == Validation::MetadataChecked
            && found.resident_data.is_some()
            && found.ntfs.is_some();
        if checked.is_none() || (!resident_metadata && checked != Some(found.validation)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                if found.validation == Validation::ImageDecoded {
                    "Recovered image failed validation. The source may have changed since scanning."
                } else {
                    "Recovered file failed format checks. The source may have changed since scanning."
                },
            ));
        }
    }
    output.as_file().sync_all()?;
    output.persist_noclobber(destination).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zip_classification_handles_maximum_comment_and_false_footer() {
        let mut bytes = include_bytes!("../tests/fixtures/package.docx").to_vec();
        let eocd = bytes
            .windows(4)
            .enumerate()
            .rfind(|(i, w)| *w == b"PK\x05\x06" && valid_zip(&bytes, *i))
            .unwrap()
            .0;
        assert_eq!(zip_kind(&bytes), "docx");
        bytes.truncate(eocd + 22);
        bytes[eocd + 20..eocd + 22].copy_from_slice(&u16::MAX.to_le_bytes());
        bytes.resize(bytes.len() + u16::MAX as usize, 0);
        // A later signature inside the comment must not hide the real footer.
        let fake = bytes.len() - 22;
        bytes[fake..fake + 4].copy_from_slice(b"PK\x05\x06");
        assert_eq!(zip_kind(&bytes), "docx");
        assert_eq!(zip_kind(b"PK\x05\x06"), "zip");
    }

    #[test]
    fn signature_dispatch_covers_every_format_and_partial_header() {
        for format in FORMATS {
            let selected = detect_format(format.head).unwrap();
            assert!(std::ptr::eq(selected, format));
            for length in 0..format.head.len() {
                assert!(detect_format(&format.head[..length]).is_none());
            }
        }
    }

    #[test]
    fn carve_boundary_and_recover_without_overwrite() {
        let dir_temp = tempfile::tempdir().unwrap();
        let dir = dir_temp.path().join("test");
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("image.bin");
        let out = dir.join("out.pdf");
        let mut bytes = vec![0; 1024 * 1024 - 3];
        let offset = bytes.len();
        bytes.extend_from_slice(b"%PDF-1.7\nhello\n%%EOF");
        bytes.extend_from_slice(&[0; 20]);
        std::fs::write(&source, &bytes).unwrap();
        let mut results = vec![];
        scan(&source, &AtomicBool::new(false), |e| {
            if let Event::Found(f) = e {
                results.push(f)
            }
        })
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].offset, offset as u64);
        recover(&source, &out, &results[0]).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"%PDF-1.7\nhello\n%%EOF");
        assert!(recover(&source, &out, &results[0]).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
    }
    #[test]
    fn all_supported_signatures() {
        let path_temp = tempfile::tempdir().unwrap();
        let path = path_temp.path().join("formats");
        let mut bytes = Vec::new();
        for content in [
            include_bytes!("../tests/fixtures/real.jpg").as_slice(),
            include_bytes!("../tests/fixtures/real.png").as_slice(),
            b"%PDF-1.7\nexample body\n%%EOF".as_slice(),
            include_bytes!("../tests/fixtures/real.gif").as_slice(),
            include_bytes!("../tests/fixtures/real89.gif").as_slice(),
        ] {
            bytes.extend_from_slice(&[0; 32]);
            bytes.extend_from_slice(content);
        }
        bytes.extend_from_slice(&[0; 32]);
        std::fs::write(&path, bytes).unwrap();
        let mut kinds = vec![];
        scan(&path, &AtomicBool::new(false), |e| {
            if let Event::Found(f) = e {
                kinds.push(f.kind);
            }
        })
        .unwrap();
        assert_eq!(kinds, vec!["jpg", "png", "pdf", "gif", "gif"]);
    }
    #[test]
    fn cancellation_and_incomplete_candidates() {
        let p_temp = tempfile::tempdir().unwrap();
        let p = p_temp.path().join("incomplete");
        std::fs::write(&p, b"%PDF-1.7 unfinished").unwrap();
        for cancelled in [false, true] {
            let mut count = 0;
            scan(&p, &AtomicBool::new(cancelled), |e| {
                if matches!(e, Event::Found(_)) {
                    count += 1
                }
            })
            .unwrap();
            assert_eq!(count, 0);
        }
    }
    #[test]
    fn length_based_formats_and_zip_comment() {
        let path_temp = tempfile::tempdir().unwrap();
        let path = path_temp.path().join("lengths");
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt data!");
        let bmp = include_bytes!("../tests/fixtures/real.bmp").to_vec();
        let zip = include_bytes!("../tests/fixtures/plain.zip").to_vec();
        let data = [wav.clone(), bmp.clone(), zip.clone()].concat();
        std::fs::write(&path, &data).unwrap();
        let mut found = vec![];
        scan(&path, &AtomicBool::new(false), |e| {
            if let Event::Found(f) = e {
                found.push(f);
            }
        })
        .unwrap();
        assert_eq!(
            found.iter().map(|f| (f.kind, f.size)).collect::<Vec<_>>(),
            vec![
                ("wav", 24),
                ("bmp", bmp.len() as u64),
                ("zip", zip.len() as u64)
            ]
        );
    }
    #[test]
    fn tiny_candidate_at_end_and_failed_export_cleanup() {
        let dir_temp = tempfile::tempdir().unwrap();
        let dir = dir_temp.path().join("end");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("source");
        let output = dir.join("output");
        std::fs::write(&path, b"padding\xff\xd8\xff\xff\xd9").unwrap();
        let mut found = vec![];
        scan(&path, &AtomicBool::new(false), |e| {
            if let Event::Found(f) = e {
                found.push(f);
            }
        })
        .unwrap();
        assert!(
            found.is_empty(),
            "A JPEG marker without image data must be rejected"
        );
        std::fs::write(&path, b"padding%PDF-1.0\n%%EOF").unwrap();
        scan(&path, &AtomicBool::new(false), |e| {
            if let Event::Found(f) = e {
                found.push(f);
            }
        })
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].size, 14);
        found[0].size = 100;
        assert!(recover(&path, &output, &found[0]).is_err());
        assert!(!output.exists());
    }
}
