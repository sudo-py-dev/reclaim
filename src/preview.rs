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
        bytes.to_vec()
    } else {
        let file = handle.ok_or("The source is no longer available.")?;
        let mut input = reclaim::SourceReader::new(Input { file, position: 0 }, alignment, None)
            .map_err(|e| e.to_string())?;
        input
            .seek(SeekFrom::Start(found.offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![0; found.size as usize];
        input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        bytes
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
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"0123456789").unwrap();
        file.seek(SeekFrom::Start(7)).unwrap();
        let file = Arc::new(file);
        let mut input = Input {
            file: file.clone(),
            position: 2,
        };
        let mut bytes = [0; 3];
        input.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"234");
        assert_eq!(file.try_clone().unwrap().stream_position().unwrap(), 7);
    }
}
