//! Read-only, bounded inspection of primary MBR entries and both GPT copies.
use crate::bytes::{u16_le, u32_le, u64_le};
use std::io::{self, Read, Seek, SeekFrom};

#[derive(Debug, PartialEq, Eq)]
pub struct Partition {
    pub slot: usize,
    pub first: u64,
    pub last: u64,
    pub kind: String,
    /// Absolute byte offset of the entry that describes this partition.
    pub entry_offset: u64,
}

#[derive(Debug, Default)]
pub struct Layout {
    pub sector: u64,
    pub notes: Vec<(u64, String)>,
    pub partitions: Vec<Partition>,
}

fn read_at(input: &mut (impl Read + Seek), offset: u64, buffer: &mut [u8]) -> io::Result<()> {
    input.seek(SeekFrom::Start(offset))?;
    input.read_exact(buffer)
}

/// A boot or superblock signature, scoped to the selected volume.
pub fn filesystem(
    input: &mut (impl Read + Seek),
    offset: u64,
    size: u64,
) -> io::Result<&'static str> {
    let mut data = [0; 2048];
    let len = usize::try_from(size).unwrap_or(usize::MAX).min(data.len());
    read_at(
        input,
        offset,
        data.get_mut(..len)
            .ok_or_else(|| invalid("invalid probe length"))?,
    )?;
    Ok(match () {
        () if data.starts_with(b"LUKS\xba\xbe") => "LUKS",
        () if data.get(3..11) == Some(b"-FVE-FS-") => "BitLocker",
        () if data.get(3..11) == Some(b"NTFS    ") => "NTFS",
        () if data.get(3..11) == Some(b"EXFAT   ") => "exFAT",
        () if data.get(82..90) == Some(b"FAT32   ") => "FAT32",
        () if data.get(54..62) == Some(b"FAT16   ") => "FAT16",
        () if data.get(54..62) == Some(b"FAT12   ") => "FAT12",
        () if u16_le(&data, 1080) == 0xef53 => "ext2/3/4",
        () if matches!(data.get(1024..1026), Some(b"H+" | b"HX")) => "HFS+",
        () if data.get(32..36) == Some(b"NXSB") => "APFS",
        () if data.starts_with(b"XFSB") => "XFS",
        () => "Unrecognized",
    })
}

#[derive(Debug, PartialEq, Eq)]
struct Gpt {
    guid: [u8; 16],
    first: u64,
    last: u64,
    count: usize,
    stride: usize,
    crc: u32,
    entries: Vec<Partition>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn gpt(
    input: &mut (impl Read + Seek),
    sector: usize,
    blocks: u64,
    backup: bool,
) -> io::Result<(Gpt, u64)> {
    let end = blocks - 1;
    let (at, other) = if backup { (end, 1) } else { (1, end) };
    // audit-allow(DOS-ALLOC): logical sectors are validated to be at most 64 KiB.
    let mut header = vec![0; sector];
    read_at(input, at * sector as u64, &mut header)?;
    if !header.starts_with(b"EFI PART") {
        return Err(invalid("missing signature"));
    }
    let len = u32_le(&header, 12) as usize;
    let checksum = u32_le(&header, 16);
    header
        .get_mut(16..20)
        .ok_or_else(|| invalid("short header"))?
        .fill(0);
    if !(92..=sector).contains(&len)
        || crc32fast::hash(header.get(..len).ok_or_else(|| invalid("short header"))?) != checksum
    {
        return Err(invalid("invalid header length or checksum"));
    }
    let first = u64_le(&header, 40);
    let last = u64_le(&header, 48);
    if u64_le(&header, 24) != at
        || u64_le(&header, 32) != other
        || first < 2
        || last >= end
        || first > last
    {
        return Err(invalid(
            "header locations or usable bounds disagree with disk size",
        ));
    }
    let start = u64_le(&header, 72);
    let count = u32_le(&header, 80) as usize;
    let stride = u32_le(&header, 84) as usize;
    if count == 0 || count > 4096 || stride < 128 || !stride.is_power_of_two() {
        return Err(invalid("invalid or unsupported partition entry dimensions"));
    }
    let bytes = count
        .checked_mul(stride)
        .filter(|n| *n <= 4 * 1024 * 1024)
        .ok_or_else(|| invalid("entry array exceeds inspection limit"))?;
    let limit = start
        .checked_add(bytes.div_ceil(sector) as u64)
        .ok_or_else(|| invalid("entry array overflow"))?;
    if (!backup && (start < 2 || limit > first)) || (backup && (start <= last || limit > end)) {
        return Err(invalid("entry array outside metadata area"));
    }
    // audit-allow(DOS-ALLOC): validated against the 4 MiB limit above.
    let mut table = vec![0; bytes];
    read_at(input, start * sector as u64, &mut table)?;
    let crc = u32_le(&header, 88);
    if crc32fast::hash(&table) != crc {
        return Err(invalid("partition array checksum mismatch"));
    }
    let entries = table
        .chunks_exact(stride)
        .enumerate()
        .filter(|(_, e)| e.get(..16).is_some_and(|id| id != [0; 16]))
        .map(|(index, e)| {
            let name: Vec<_> = e
                .get(56..128)
                .unwrap_or_default()
                .as_chunks::<2>()
                .0
                .iter()
                .copied()
                .map(u16::from_le_bytes)
                .take_while(|v| *v != 0)
                .collect();
            let kind = String::from_utf16_lossy(&name)
                .chars()
                .filter(|c| c.is_alphanumeric() || " ._-".contains(*c))
                .collect();
            Partition {
                slot: index + 1,
                entry_offset: (index * stride) as u64,
                first: u64_le(e, 32),
                last: u64_le(e, 40),
                kind,
            }
        })
        .collect();
    Ok((
        Gpt {
            guid: header
                .get(56..72)
                .ok_or_else(|| invalid("short disk GUID"))?
                .try_into()
                .map_err(|_length| invalid("invalid disk GUID"))?,
            first,
            last,
            count,
            stride,
            crc,
            entries,
        },
        start * sector as u64,
    ))
}

pub fn inspect(
    input: &mut (impl Read + Seek),
    size: u64,
    known_sector: Option<usize>,
) -> io::Result<Layout> {
    let mut mbr = [0; 512];
    read_at(input, 0, &mut mbr)?;
    let mut sector = known_sector.unwrap_or(512);
    if !(512..=65536).contains(&sector) || !sector.is_power_of_two() {
        return Err(invalid("unknown logical sector size"));
    }
    let slots = mbr.get(446..510).unwrap_or_default();
    let protective = slots
        .as_chunks::<16>()
        .0
        .iter()
        .any(|e| e.get(4).copied().unwrap_or(0) == 0xee);
    let mut magic = [0; 8];
    let mut is_gpt = protective;
    for width in [sector, 4096] {
        if known_sector.is_some() && width != sector {
            continue;
        }
        let blocks = size / width as u64;
        if blocks >= 3
            && [1, blocks - 1].iter().any(|n| {
                read_at(input, n * width as u64, &mut magic).is_ok() && &magic == b"EFI PART"
            })
        {
            sector = width;
            is_gpt = true;
            break;
        }
    }
    let blocks = size / sector as u64;
    let mut report = Layout {
        sector: sector as u64,
        ..Layout::default()
    };
    let mut bounds = (1, blocks.saturating_sub(1));
    if is_gpt && blocks >= 3 {
        if !protective || u16_le(&mbr, 510) != 0xaa55 {
            report.notes.push((0, "Missing protective MBR".into()));
        }
        let primary = gpt(input, sector, blocks, false);
        let backup = gpt(input, sector, blocks, true);
        for (at, name, copy) in [
            (1, "Primary GPT", &primary),
            (blocks - 1, "Backup GPT", &backup),
        ] {
            report.notes.push((
                at * sector as u64,
                match copy {
                    Ok(_) => format!(
                        "{} · {}",
                        crate::i18n::t(name),
                        crate::i18n::t("Header and array checksums verified")
                    ),
                    Err(e) => format!("{} · {e}", crate::i18n::t(name)),
                },
            ));
        }
        if let (Ok((a, _)), Ok((b, _))) = (&primary, &backup)
            && a != b
        {
            report.notes.push((
                sector as u64,
                "GPT copies disagree. Showing primary entries.".into(),
            ));
        }
        if let Ok((copy, table_offset)) = primary.or(backup) {
            bounds = (copy.first, copy.last);
            report.partitions = copy.entries;
            for entry in &mut report.partitions {
                entry.entry_offset += table_offset;
            }
        }
    } else if u16_le(&mbr, 510) == 0xaa55 && filesystem(input, 0, size)? == "Unrecognized" {
        report.notes.push((0, "MBR · primary entries".into()));
        for (i, e) in slots
            .as_chunks::<16>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, e)| e.get(4).copied().unwrap_or(0) != 0)
        {
            let first = u64::from(u32_le(e.as_slice(), 8));
            let length = u32_le(e.as_slice(), 12);
            if length == 0 {
                report.notes.push((
                    446 + i as u64 * 16,
                    format!("Entry {} has zero length", i + 1),
                ));
                continue;
            }
            let kind = match e.get(4).copied().unwrap_or(0) {
                5 | 15 | 0x85 => "Extended container (logical entries uninspected)".into(),
                t => format!("MBR type 0x{t:02x}"),
            };
            report.partitions.push(Partition {
                slot: i + 1,
                entry_offset: 446 + i as u64 * 16,
                first,
                last: first + u64::from(length) - 1,
                kind,
            });
        }
    } else {
        report.notes.push((
            0,
            "No recognized disk partition table. A volume image may contain only a filesystem."
                .into(),
        ));
    }
    report.partitions.sort_by_key(|p| p.first);
    let mut previous_end = None;
    for p in &report.partitions {
        let usable = bounds.0..=bounds.1;
        if p.first > p.last || !usable.contains(&p.first) || !usable.contains(&p.last) {
            report.notes.push((
                p.entry_offset,
                format!("Entry {} exceeds usable bounds or is reversed", p.slot),
            ));
        }
        if previous_end.is_some_and(|end| p.first <= end) {
            report.notes.push((
                p.entry_offset,
                format!("Entry {} overlaps a previous entry", p.slot),
            ));
        }
        previous_end = Some(previous_end.unwrap_or(0).max(p.last));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture(sector: usize) -> Vec<u8> {
        let mut disk = vec![0; 128 * sector];
        disk[450] = 0xee;
        disk[510..512].copy_from_slice(&[0x55, 0xaa]);
        let mut entry = [0u8; 128];
        entry[0] = 1;
        entry[32..40].copy_from_slice(&40u64.to_le_bytes());
        entry[40..48].copy_from_slice(&60u64.to_le_bytes());
        for (current, alternate, array) in [(1usize, 127u64, 2usize), (127, 1, 126)] {
            let mut h = [0; 92];
            h[..8].copy_from_slice(b"EFI PART");
            h[8..12].copy_from_slice(&0x10000u32.to_le_bytes());
            h[12..16].copy_from_slice(&92u32.to_le_bytes());
            for (pos, value) in [
                (24, current as u64),
                (32, alternate),
                (40, 34),
                (48, 94),
                (72, array as u64),
            ] {
                h[pos..pos + 8].copy_from_slice(&value.to_le_bytes());
            }
            h[80..84].copy_from_slice(&1u32.to_le_bytes());
            h[84..88].copy_from_slice(&128u32.to_le_bytes());
            h[88..92].copy_from_slice(&crc32fast::hash(&entry).to_le_bytes());
            let crc = crc32fast::hash(&h);
            h[16..20].copy_from_slice(&crc.to_le_bytes());
            disk[current * sector..current * sector + 92].copy_from_slice(&h);
            disk[array * sector..array * sector + 128].copy_from_slice(&entry);
        }
        disk
    }
    fn layout(disk: Vec<u8>) -> Layout {
        let size = disk.len() as u64;
        inspect(&mut Cursor::new(disk), size, None).unwrap()
    }
    #[test]
    fn both_gpt_copies_and_4k_backup_survive_independent_checks() {
        for sector in [512, 4096] {
            let mut disk = fixture(sector);
            let good = layout(disk.clone());
            assert_eq!(good.sector, sector as u64);
            assert_eq!(good.partitions[0].last, 60);
            assert!(good.notes.iter().all(|(_, s)| s.contains("verified")));
            disk[sector] = 0;
            let backup = layout(disk);
            assert_eq!(backup.sector, sector as u64);
            assert_eq!(backup.partitions[0].first, good.partitions[0].first);
            assert_eq!(backup.partitions[0].entry_offset, 126 * sector as u64);
            assert_eq!(good.partitions[0].entry_offset, 2 * sector as u64);
            assert!(
                backup
                    .notes
                    .iter()
                    .any(|(_, s)| s.contains("missing signature"))
            );
        }
    }
    #[test]
    fn checksum_damage_and_valid_but_conflicting_copies_are_distinct() {
        let mut disk = fixture(512);
        let table = 126 * 512;
        disk[table + 32] = 42;
        assert!(
            layout(disk.clone())
                .notes
                .iter()
                .any(|(_, s)| s.contains("array checksum mismatch"))
        );
        let array_crc = crc32fast::hash(&disk[table..table + 128]);
        let h = &mut disk[127 * 512..127 * 512 + 92];
        h[88..92].copy_from_slice(&array_crc.to_le_bytes());
        h[16..20].fill(0);
        let crc = crc32fast::hash(h);
        h[16..20].copy_from_slice(&crc.to_le_bytes());
        assert!(
            layout(disk)
                .notes
                .iter()
                .any(|(_, s)| s.contains("disagree"))
        );
    }
    #[test]
    fn mbr_overlaps_and_out_of_bounds_entries_are_reported() {
        let mut disk = vec![0; 512 * 8];
        disk[510..512].copy_from_slice(&[0x55, 0xaa]);
        for (at, start, len) in [(446, 1u32, 4u32), (462, 4, 12)] {
            disk[at + 4] = 0x83;
            disk[at + 8..at + 12].copy_from_slice(&start.to_le_bytes());
            disk[at + 12..at + 16].copy_from_slice(&len.to_le_bytes());
        }
        let result = layout(disk);
        assert_eq!(result.partitions.len(), 2);
        assert!(result.notes.iter().any(|(_, s)| s.contains("overlaps")));
        assert!(result.notes.iter().any(|(_, s)| s.contains("bounds")));
    }
}
