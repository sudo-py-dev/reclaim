pub mod i18n;
mod ntfs;
mod source;
pub mod updater;
mod validation;
pub use source::SourceReader;
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

/// Formats currently recognized by the scanner, grouped for the interface.
pub const FORMAT_SUMMARY: &str = "Small NTFS resident files · JPEG · PNG · GIF · BMP · WebP · ICO · PDF · Office · OpenDocument · EPUB · ZIP · JAR · APK · WAV · AIFF · AU · AVI";
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
        "jpg" | "png" | "gif" | "bmp" | "webp" => 1,
        "ico" => 7,
        "pdf" | "docx" | "xlsx" | "pptx" | "docm" | "xlsm" | "pptm" | "odt" | "ods" | "odp"
        | "epub" | "txt" | "csv" | "json" | "xml" | "md" | "html" | "log" => 2,
        "zip" => 3,
        "wav" | "aiff" | "aifc" | "au" | "mp3" => 4,
        "avi" | "mp4" => 5,
        "jar" | "apk" | "exe" => 6,
        _ => 0,
    }
}

const LIMIT: usize = 64 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Validation {
    ImageDecoded,
    MetadataChecked,
    Unverified,
}
impl Validation {
    pub fn label(self) -> &'static str {
        match self {
            Self::ImageDecoded => "Image decoded",
            Self::MetadataChecked => "NTFS record checked",
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
        format!("{label} {number:04}.{}", self.kind)
    }
}
fn portable_name(name: &str) -> String {
    let mut name: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
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
    Progress(u64),
    Found(Found),
}
struct Format {
    head: &'static [u8],
    tail: &'static [u8],
    ext: &'static str,
}
const FORMATS: &[Format] = &[
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
];

enum CandidateEnd {
    More,
    Invalid,
    Complete(usize),
}
fn candidate_end(fmt: &Format, bytes: &[u8], from: usize, cancel: &AtomicBool) -> CandidateEnd {
    match fmt.ext {
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
                    u32::from_le_bytes(bytes[4..8].try_into().unwrap())
                } else {
                    if !matches!(kind, b"AIFF" | b"AIFC") {
                        return CandidateEnd::Invalid;
                    }
                    u32::from_be_bytes(bytes[4..8].try_into().unwrap())
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
                let read = |i| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
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
                let n = u32::from_le_bytes(bytes[2..6].try_into().unwrap());
                if n < 14 {
                    return CandidateEnd::Invalid;
                }
                if bytes.len() < 18 {
                    return CandidateEnd::More;
                }
                let dib = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
                let pixel_offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
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
        return if bytes.len() >= size as usize {
            CandidateEnd::Complete(size as usize)
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
    let long = |p| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
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
        _ => fmt.ext,
    }
}

// Only classify packages from a complete, single-disk classic ZIP directory.
// Names appearing in compressed payloads must not change the recovered extension.
fn zip_kind(bytes: &[u8]) -> &'static str {
    let Some((eocd, _)) = bytes
        .windows(4)
        .enumerate()
        .rfind(|(i, w)| *w == b"PK\x05\x06" && valid_zip(bytes, *i))
    else {
        return "zip";
    };
    if bytes.len() < eocd + 22 {
        return "zip";
    }
    let u16_at = |p| u16::from_le_bytes([bytes[p], bytes[p + 1]]) as usize;
    let u32_at = |p| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()) as usize;
    if u16_at(eocd + 4) != 0 || u16_at(eocd + 6) != 0 || u16_at(eocd + 8) != u16_at(eocd + 10) {
        return "zip";
    }
    let count = u16_at(eocd + 10);
    let size = u32_at(eocd + 12);
    let mut cursor = u32_at(eocd + 16);
    if cursor.checked_add(size) != Some(eocd) {
        return "zip";
    }
    let mut names = Vec::new();
    let mut mime = None;
    for _ in 0..count {
        if cursor + 46 > eocd || &bytes[cursor..cursor + 4] != b"PK\x01\x02" {
            return "zip";
        }
        let name_len = u16_at(cursor + 28);
        let next = cursor + 46 + name_len + u16_at(cursor + 30) + u16_at(cursor + 32);
        if next > eocd {
            return "zip";
        }
        let name = &bytes[cursor + 46..cursor + 46 + name_len];
        names.push(name);
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
    if cursor != eocd {
        return "zip";
    }
    let has = |name: &[u8]| names.contains(&name);
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

/// Bounded-memory signature carving. Candidates are not proof of valid/deleted files.
pub fn scan(path: &Path, cancel: &AtomicBool, event: impl FnMut(Event)) -> io::Result<()> {
    scan_reader(&mut File::open(path)?, cancel, event)
}

/// Scan an already-authorized, read-only source without reopening its path.
pub fn scan_reader(
    file: &mut (impl Read + Seek),
    cancel: &AtomicBool,
    mut event: impl FnMut(Event),
) -> io::Result<()> {
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = vec![0; 1024 * 1024];
    let mut carry = Vec::new();
    let mut base = 0u64;
    let mut found_count = 0usize;
    let mut skip_until = 0u64;
    let mut resident_bytes = 0usize;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let n = file.read(&mut buffer)?;
        carry.extend_from_slice(&buffer[..n]);
        let mut i = 0;
        while (n == 0 && i < carry.len()) || i + 8 <= carry.len() {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            if base + (i as u64) < skip_until {
                i = (skip_until - base).min(carry.len() as u64) as usize;
                continue;
            }
            let offset = base + i as u64;
            if offset.is_multiple_of(512) && carry[i..].starts_with(b"FILE") {
                let position = file.stream_position()?;
                file.seek(SeekFrom::Start(offset))?;
                let result = ntfs::read_record(file, offset, cancel);
                file.seek(SeekFrom::Start(position))?;
                if let Some((found, record_size)) = result? {
                    skip_until = offset + record_size as u64;
                    resident_bytes += found.size as usize;
                    found_count += 1;
                    if resident_bytes > LIMIT || found_count > 100_000 {
                        return Err(io::Error::other(
                            "Recovery result limit reached. Available results can still be exported.",
                        ));
                    }
                    event(Event::Found(found));
                }
            }
            if let Some(fmt) = FORMATS.iter().find(|f| carry[i..].starts_with(f.head)) {
                // Read candidate independently so the main streaming buffer stays bounded.
                let offset = base + i as u64;
                let position = file.stream_position()?;
                file.seek(SeekFrom::Start(offset))?;
                let mut candidate = Vec::new();
                let mut end = None;
                let mut kind = fmt.ext;
                let mut checked = Validation::Unverified;
                let mut chunk = vec![0u8; 8192];
                while candidate.len() < LIMIT && !cancel.load(Ordering::Relaxed) {
                    let remaining = (LIMIT - candidate.len()).min(chunk.len());
                    let count = file.read(&mut chunk[..remaining])?;
                    if count == 0 {
                        break;
                    }
                    let from = candidate.len().saturating_sub(fmt.tail.len());
                    candidate.extend_from_slice(&chunk[..count]);
                    match candidate_end(fmt, &candidate, from, cancel) {
                        CandidateEnd::Complete(size) => {
                            kind = candidate_kind(fmt, &candidate[..size]);
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
                if let Some(size) = end {
                    if checked == Validation::ImageDecoded {
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
        || found.size > LIMIT as u64
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
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = (|| {
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
        output.sync_all()?;
        if found.validation == Validation::ImageDecoded {
            let bytes = std::fs::read(destination)?;
            if validation::validate(found.kind, &bytes, &AtomicBool::new(false))
                != Some(Validation::ImageDecoded)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Recovered image failed validation. The source may have changed since scanning.",
                ));
            }
        }
        Ok(())
    })();
    drop(output);
    if result.is_err() {
        let _ = std::fs::remove_file(destination);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carve_boundary_and_recover_without_overwrite() {
        let dir = std::env::temp_dir().join(format!("reclaim-test-{}", std::process::id()));
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
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn all_supported_signatures() {
        let path = std::env::temp_dir().join(format!("reclaim-formats-{}", std::process::id()));
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
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cancellation_and_incomplete_candidates() {
        let p = std::env::temp_dir().join(format!("reclaim-incomplete-{}", std::process::id()));
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
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn length_based_formats_and_zip_comment() {
        let path = std::env::temp_dir().join(format!("reclaim-lengths-{}", std::process::id()));
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
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn tiny_candidate_at_end_and_failed_export_cleanup() {
        let dir = std::env::temp_dir().join(format!("reclaim-end-{}", std::process::id()));
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
        std::fs::remove_dir_all(dir).unwrap();
    }
}
