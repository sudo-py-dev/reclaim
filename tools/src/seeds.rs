//! Seed corpora from checked-in fixtures and a synthetic deleted NTFS record.
use crate::{Result, copy, files};
use std::{fs, path::Path};

pub fn resident_record() -> Vec<u8> {
    let mut record = vec![0; 1024];
    record[..4].copy_from_slice(b"FILE");
    for (offset, value) in [(4, 48u16), (6, 3), (16, 7), (20, 64), (48, 0xBEEF)] {
        record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    record[28..32].copy_from_slice(&1024u32.to_le_bytes());
    let mut at = 64;
    fn attribute(record: &mut [u8], at: &mut usize, kind: u32, value: &[u8]) {
        let length = (24 + value.len()).div_ceil(8) * 8;
        record[*at..*at + 4].copy_from_slice(&kind.to_le_bytes());
        record[*at + 4..*at + 8].copy_from_slice(&(length as u32).to_le_bytes());
        record[*at + 16..*at + 20].copy_from_slice(&(value.len() as u32).to_le_bytes());
        record[*at + 20..*at + 22].copy_from_slice(&24u16.to_le_bytes());
        record[*at + 24..*at + 24 + value.len()].copy_from_slice(value);
        *at += length;
    }
    attribute(&mut record, &mut at, 0x10, &[0; 48]);
    let name: Vec<_> = "report\u{202e}.txt".encode_utf16().collect();
    let mut filename = vec![0; 66];
    filename[64] = name.len() as u8;
    filename[65] = 1;
    for character in name {
        filename.extend(character.to_le_bytes());
    }
    attribute(&mut record, &mut at, 0x30, &filename);
    attribute(&mut record, &mut at, 0x80, b"resident data");
    record[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    record[24..28].copy_from_slice(&((at + 4) as u32).to_le_bytes());
    for (index, tail) in [510, 1022].into_iter().enumerate() {
        let original = [record[tail], record[tail + 1]];
        record[50 + index * 2..52 + index * 2].copy_from_slice(&original);
        record[tail..tail + 2].copy_from_slice(&0xBEEFu16.to_le_bytes());
    }
    record
}
pub fn generate(repository: &Path) -> Result {
    let corpus = repository.join("fuzz/corpus/scan");
    let containers = repository.join("fuzz/containers/corpus/containers");
    fs::create_dir_all(&corpus)?;
    fs::create_dir_all(&containers)?;
    let fixtures = repository.join("tests/fixtures");
    for fixture in files(&fixtures)? {
        if fixture.extension().is_some_and(|ext| ext == "md")
            || fs::metadata(&fixture)?.len() > 65536
        {
            continue;
        }
        let name = fixture
            .strip_prefix(&fixtures)?
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("-");
        copy(&fixture, corpus.join(name))?;
        if matches!(
            fixture.extension().and_then(|ext| ext.to_str()),
            Some("sqlite" | "ogg" | "opus" | "ogv")
        ) {
            copy(
                &fixture,
                containers.join(fixture.file_name().ok_or("fixture has no name")?),
            )?;
        }
    }
    fs::write(corpus.join("ntfs-resident"), resident_record())?;
    fs::write(corpus.join("document.pdf"), b"%PDF-1.7\nexample\n%%EOF")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeds_exclude_large_files_and_preserve_fixup_records() -> Result {
        let temporary = tempfile::tempdir()?;
        fs::create_dir_all(temporary.path().join("tests/fixtures/nested"))?;
        fs::write(
            temporary.path().join("tests/fixtures/nested/sample.ogg"),
            b"OggS",
        )?;
        fs::write(
            temporary.path().join("tests/fixtures/large.sqlite"),
            vec![0; 65537],
        )?;
        generate(temporary.path())?;
        assert!(
            temporary
                .path()
                .join("fuzz/corpus/scan/nested-sample.ogg")
                .is_file()
        );
        assert!(
            temporary
                .path()
                .join("fuzz/containers/corpus/containers/sample.ogg")
                .is_file()
        );
        assert!(
            !temporary
                .path()
                .join("fuzz/corpus/scan/large.sqlite")
                .exists()
        );
        let record = resident_record();
        assert_eq!(&record[..4], b"FILE");
        for tail in [510, 1022] {
            assert_eq!(&record[tail..tail + 2], &record[48..50]);
        }
        assert!(record.windows(13).any(|window| window == b"resident data"));
        Ok(())
    }
}
