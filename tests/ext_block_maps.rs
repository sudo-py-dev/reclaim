//! Deleted files on ext2 come back whole from the block maps the kernel
//! leaves behind, even when they were stored in many pieces.
use reclaim::{Event, Found, FoundReader, scan_unused_reader};
use std::{
    io::{Cursor, Read},
    sync::atomic::AtomicBool,
};

/// The document the fixture stored across freed gaps, then deleted. It is
/// longer than one block map covers, so the second map level is needed too.
#[cfg(test)]
fn document() -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n".to_vec();
    for line in 0..7000 {
        bytes.extend(format!("line {line:05} of the deleted ext2 block map test\n").bytes());
    }
    bytes.extend(b"%%EOF\n");
    bytes
}

#[cfg(test)]
fn fixture() -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/deleted-blockmap-ext2.img.gz",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}

#[cfg(test)]
fn scan(image: &[u8]) -> Vec<Found> {
    let mut found = Vec::new();
    scan_unused_reader(
        &mut Cursor::new(image),
        image.len() as u64,
        &AtomicBool::new(false),
        |event| {
            if let Event::Found(file) = event {
                found.push(file);
            }
        },
    )
    .unwrap();
    found
}

#[test]
fn fragmented_deleted_file_returns_whole_from_ext2_block_maps() {
    let image = fixture();
    let expected = document();
    let mut input = Cursor::new(&image);
    let whole = scan(&image).into_iter().find(|file| {
        let mut recovered = Vec::new();
        FoundReader::new(&mut input, file)
            .unwrap()
            .read_to_end(&mut recovered)
            .unwrap();
        recovered == expected
    });
    let file = whole.expect("the deleted document did not come back whole");
    assert!(
        file.extents
            .as_ref()
            .is_some_and(|extents| extents.len() > 2),
        "the fixture document should be stored in many pieces"
    );
}

#[test]
fn fragmented_tar_returns_whole_from_ext2_block_maps() {
    let mut image = fixture();
    let pdf_size = document().len();
    let pdf = scan(&image)
        .into_iter()
        .find(|file| file.kind == "pdf" && file.size == pdf_size as u64)
        .unwrap();
    let block = 512;
    let payload = (pdf_size / block - 3) * block;
    let mut archive = vec![b'A'; block + payload];
    archive[..block].copy_from_slice(&include_bytes!("fixtures/formats/sample.tar")[..block]);
    archive[124..136].copy_from_slice(format!("{payload:011o}\0").as_bytes());
    archive[148..156].fill(b' ');
    let checksum: usize = archive[..block].iter().map(|&byte| usize::from(byte)).sum();
    archive[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
    archive.resize(archive.len() + 2 * block, 0);

    // Replace the fixture document's bytes while retaining its fragmented layout.
    let mut written = 0;
    for &(start, size) in pdf.extents.as_deref().unwrap() {
        let start = usize::try_from(start).unwrap();
        let count = usize::try_from(size).unwrap().min(archive.len() - written);
        image[start..start + count].copy_from_slice(&archive[written..written + count]);
        written += count;
    }
    assert_eq!(written, archive.len());

    let tar = scan(&image)
        .into_iter()
        .find(|file| file.kind == "tar" && file.offset == pdf.offset)
        .unwrap();
    assert!(
        tar.extents
            .as_ref()
            .is_some_and(|extents| extents.len() > 2)
    );
    let mut recovered = Vec::new();
    FoundReader::new(&mut Cursor::new(&image), &tar)
        .unwrap()
        .read_to_end(&mut recovered)
        .unwrap();
    assert_eq!(recovered, archive);
}
