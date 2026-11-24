//! Read-only windows on a disk and an optional comparison image.
use reclaim::i18n;
use std::{
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
    sync::Arc,
};

pub const WINDOW: u64 = 512;

#[derive(Debug)]
pub struct Image {
    pub path: PathBuf,
    file: File,
    size: u64,
}
impl Image {
    pub fn open(path: PathBuf) -> io::Result<Arc<Self>> {
        let mut options = OpenOptions::new();
        options.read(true);
        // A named pipe selected in the image picker must never wait for a writer.
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = options.open(&path)?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(io::Error::other(i18n::t("Choose a raw disk image")));
        }
        Ok(Arc::new(Self {
            path,
            file,
            size: meta.len(),
        }))
    }
}

pub fn address(text: &str, sector: u64) -> Option<u64> {
    let text = text.trim();
    let (number, multiplier) = match text.split_once(char::is_whitespace) {
        Some((prefix, value)) if prefix.eq_ignore_ascii_case("LBA") => (value.trim(), sector),
        _ => (text, 1),
    };
    let value = if let Some(hex) = number
        .strip_prefix("0x")
        .or_else(|| number.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()?
    } else {
        number.parse::<u64>().ok()?
    };
    value.checked_mul(multiplier)
}

#[derive(Default)]
pub struct Viewer {
    pub offset: u64,
    pub image: Option<Arc<Image>>,
}

pub struct Page {
    pub viewer: Viewer,
    pub dump: String,
    pub last: u64,
    pub more: bool,
}
fn read_at(input: &mut (impl Read + Seek), offset: u64, size: u64) -> io::Result<Vec<u8>> {
    let count = usize::try_from(size.saturating_sub(offset).min(WINDOW)).unwrap_or(0);
    let mut block = [0; 512];
    let bytes = block
        .get_mut(..count)
        .ok_or_else(|| io::Error::other("Invalid read buffer length"))?;
    if !bytes.is_empty() {
        input.seek(SeekFrom::Start(offset))?;
        input.read_exact(bytes)?;
    }
    Ok(bytes.to_vec())
}

fn column(bytes: &[u8], count: usize, width: usize) -> String {
    let mut hex = String::new();
    let mut ascii = String::new();
    for i in 0..width {
        if let Some(&byte) = bytes.get(i) {
            write!(hex, "{byte:02X} ").ok();
            ascii.push(if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '.'
            });
        } else if i < count {
            hex.push_str("-- ");
            ascii.push(' ');
        } else {
            hex.push_str("   ");
            ascii.push(' ');
        }
    }
    format!("{hex} |{ascii}|")
}

fn dump(offset: u64, source: &[u8], comparison: Option<&[u8]>) -> String {
    let width = if comparison.is_some() { 8 } else { 16 };
    let length = source.len().max(comparison.map_or(0, <[u8]>::len));
    let address_width =
        if u32::try_from(offset.saturating_add(length.saturating_sub(1) as u64)).is_ok() {
            8
        } else {
            16
        };
    let mut out = String::new();
    for start in (0..length).step_by(width) {
        let end = (start + width).min(length);
        let count = end - start;
        let left = source.get(start..).unwrap_or_default();
        write!(
            out,
            "{:0address_width$X}  {}",
            offset + start as u64,
            column(left, count, width)
        )
        .ok();
        if let Some(other) = comparison {
            let right = other.get(start..).unwrap_or_default();
            write!(out, "    {}", column(right, count, width)).ok();
            if (0..count).any(|i| left.get(i) != right.get(i)) {
                // The marker line sits immediately below the different bytes of the second image.
                write!(
                    out,
                    "\n{:padding$}",
                    "",
                    padding = address_width + 2 + width * 4 + 7
                )
                .ok();
                for i in 0..count {
                    out.push_str(if left.get(i) == right.get(i) {
                        "   "
                    } else {
                        "^^ "
                    });
                }
            }
        }
        out.push('\n');
    }
    out
}

pub fn read(
    mut reader: impl Read + Seek,
    size: u64,
    offset: u64,
    image: Option<Arc<Image>>,
) -> io::Result<Page> {
    let end = size.max(image.as_ref().map_or(0, |i| i.size));
    if offset >= end {
        return Err(io::Error::other(i18n::t(
            "Address outside the image or drive.",
        )));
    }
    let left = read_at(&mut reader, offset, size)?;
    let right = image
        .as_ref()
        .map(|i| read_at(&mut i.file.try_clone()?, offset, i.size))
        .transpose()?;
    let last = offset + end.saturating_sub(offset).min(WINDOW) - 1;
    Ok(Page {
        dump: dump(offset, &left, right.as_deref()),
        viewer: Viewer { offset, image },
        last,
        more: last < end - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parses_large_offsets_and_uses_source_logical_sector_size() {
        assert_eq!(address("0x100000000", 512), Some(1 << 32));
        assert_eq!(address("  lba 0x10 ", 4096), Some(65536));
        for bad in ["LBA 18446744073709551615", "-1", "x42", "LBA"] {
            assert_eq!(address(bad, 512), None);
        }
    }

    #[test]
    fn comparison_covers_both_files_and_marks_only_different_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("second.img");
        std::fs::write(&path, b"axcd!").unwrap();
        let image = Image::open(path).unwrap();
        let page = read(Cursor::new(b"abcd"), 4, 0, Some(Arc::clone(&image))).unwrap();
        let mut lines = page.dump.lines();
        let row = lines.next().unwrap();
        let markers = lines.next().unwrap();
        assert!(row.contains("61 62 63 64 --") && row.contains("61 78 63 64 21"));
        assert_eq!(markers.find("^^"), row.find("78"));
        assert_eq!(markers.rfind("^^"), row.find("21"));
        let tail = read(Cursor::new(b"abcd"), 4, 4, Some(image)).unwrap();
        assert_eq!(tail.last, 4);
        assert!(!tail.more);
        assert!(read(Cursor::new(b"abcd"), 4, 4, None).is_err());
    }
}
