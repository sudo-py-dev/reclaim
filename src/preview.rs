use eframe::egui;
use reclaim::Found;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

pub fn supported(kind: &str) -> bool {
    matches!(
        kind,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico"
    )
}

// Independent position: preview reads must not move the scanner's file cursor.
struct Input {
    file: Arc<std::fs::File>,
    position: u64,
}
impl Input {
    fn new(file: Arc<std::fs::File>) -> std::io::Result<Self> {
        #[cfg(windows)]
        let file = {
            use std::os::windows::io::{AsRawHandle, FromRawHandle};
            use windows_sys::Win32::{
                Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE},
                Storage::FileSystem::{
                    FILE_FLAG_RANDOM_ACCESS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
                    ReOpenFile,
                },
            };
            // A duplicated Windows handle shares its cursor. ReOpenFile creates
            // an independent cursor on the same authorized object, without
            // resolving the source path again or requesting write access.
            // SAFETY: file keeps the input handle alive during ReOpenFile.
            let handle = unsafe {
                ReOpenFile(
                    file.as_raw_handle(),
                    GENERIC_READ,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    FILE_FLAG_RANDOM_ACCESS,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err(std::io::Error::last_os_error());
            }
            // SAFETY: ReOpenFile returned a new owned handle; transfer it once.
            Arc::new(unsafe { std::fs::File::from_raw_handle(handle) })
        };
        Ok(Self { file, position: 0 })
    }
}
impl Read for Input {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        #[cfg(unix)]
        let count = {
            use std::os::unix::fs::FileExt;
            self.file.read_at(buffer, self.position)?
        };
        #[cfg(windows)]
        let count = {
            use std::os::windows::fs::FileExt;
            // This is safe only on our independently reopened handle:
            // seek_read changes the Windows file cursor.
            self.file.seek_read(buffer, self.position)?
        };
        self.position += count as u64;
        Ok(count)
    }
}
impl Seek for Input {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let position = match from {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::Current(value) => i128::from(self.position) + i128::from(value),
            SeekFrom::End(value) => i128::from(self.file.metadata()?.len()) + i128::from(value),
        };
        self.position = u64::try_from(position)
            .map_err(|_| std::io::Error::other("Invalid preview position"))?;
        Ok(self.position)
    }
}

pub fn load(
    found: &Found,
    handle: Option<Arc<std::fs::File>>,
    alignment: usize,
) -> Result<egui::ColorImage, String> {
    if found.size > 32 * 1024 * 1024 {
        return Err(
            "This image is too large to preview. Recover it to view the full image.".into(),
        );
    }
    let bytes = if let Some(bytes) = &found.resident_data {
        std::borrow::Cow::Borrowed(bytes.as_ref())
    } else {
        let file = handle.ok_or("The source is no longer available.")?;
        let mut input = reclaim::SourceReader::new(
            Input::new(file).map_err(|e| e.to_string())?,
            alignment,
            None,
        )
        .map_err(|e| e.to_string())?;
        input
            .seek(SeekFrom::Start(found.offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![0; found.size as usize];
        input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        std::borrow::Cow::Owned(bytes)
    };
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| {
        "This image could not be previewed. Its contents may be incomplete or damaged.".to_string()
    })?;
    let image = if image.width() > 80 || image.height() > 80 {
        image.thumbnail(80, 80)
    } else {
        image
    }
    .to_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn candidate(bytes: &[u8]) -> Found {
        Found {
            offset: 0,
            size: bytes.len() as u64,
            kind: "png",
            selected: false,
            validation: reclaim::Validation::Unverified,
            original_name: None,
            resident_data: Some(Arc::from(bytes)),
            ntfs: None,
        }
    }

    #[test]
    fn decodes_resident_image_and_rejects_damaged_data() {
        let image = image::DynamicImage::new_rgba8(2, 3);
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let preview = load(&candidate(bytes.get_ref()), None, 1).unwrap();
        assert_eq!(preview.size, [2, 3]);
        assert!(load(&candidate(b"damaged"), None, 1).is_err());
        let mut large = candidate(b"");
        large.size = 33 * 1024 * 1024;
        assert!(load(&large, None, 1).unwrap_err().contains("too large"));
    }

    #[test]
    fn positional_reads_preserve_scanner_cursor() {
        // Anonymous temporary files deny sharing on Windows, which prevents
        // ReOpenFile. Use a regular source file with the scanner's open mode.
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.bin");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"0123456789").unwrap();
        drop(file);
        let mut file = crate::drives::open_source(&path).unwrap();
        file.seek(SeekFrom::Start(7)).unwrap();
        let file = Arc::new(file);
        let mut input = Input::new(file.clone()).unwrap();
        input.seek(SeekFrom::Start(2)).unwrap();
        let mut bytes = [0; 3];
        input.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"234");
        assert_eq!(file.try_clone().unwrap().stream_position().unwrap(), 7);
    }
}
