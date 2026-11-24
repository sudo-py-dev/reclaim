//! A report owns the read-only handle that its partition offsets refer to.
use crate::drives::{self, Drive};
use reclaim::{SourceReader, i18n, partition};
use std::{fmt::Write as _, fs::File, io, path::Path};

#[derive(Default)]
pub(crate) struct Report {
    pub overview: String,
    pub health: String,
    pub raw: String,
    pub layout: String,
    pub findings: Vec<(u64, String)>,
    pub viewer_path: std::path::PathBuf,
    pub sector: u64,
    pub choices: Vec<String>,
    regions: Vec<(u64, u64)>,
    disk: Option<File>,
    alignment: usize,
    pub size: u64,
}

pub(crate) fn field(label: &str, value: impl std::fmt::Display) -> String {
    format!("{} · {}", i18n::t(label), i18n::isolate(&value.to_string()))
}

pub(crate) fn partition_detail(offset: u64, len: u64, fs: &str) -> String {
    [
        field("Byte offset", offset),
        field("Length", crate::format_bytes(len)),
        field("Filesystem signature", i18n::t(fs)),
    ]
    .join("\n")
}

impl Report {
    pub fn reader(&self) -> io::Result<SourceReader<File>> {
        let disk = self
            .disk
            .as_ref()
            .ok_or_else(|| io::Error::other("Disk unavailable"))?;
        SourceReader::new(disk.try_clone()?, self.alignment, Some(self.size))
    }

    pub fn region_offset(&self, index: usize) -> Option<u64> {
        self.regions.get(index).map(|&(offset, _)| offset)
    }
    pub fn open_partition(&self, index: usize) -> io::Result<(SourceReader<File>, u64, u64)> {
        let &(offset, len) = self
            .regions
            .get(index)
            .ok_or_else(|| io::Error::other("Invalid partition"))?;
        Ok((self.reader()?, offset, len))
    }
    fn set_view_source(&mut self, path: &Path, disk: File, size: u64, alignment: usize) {
        self.disk = Some(disk);
        path.clone_into(&mut self.viewer_path);
        self.size = size;
        self.alignment = alignment;
        self.sector = if alignment >= 512 {
            alignment as u64
        } else {
            512
        };
    }
    fn inspect_disk(&mut self, path: &Path, drive: Option<&Drive>, disk: File) -> io::Result<()> {
        let size = if let Some(d) = drive {
            d.size
        } else {
            disk.metadata()?.len()
        };
        let alignment = drive.map_or(1, |d| d.sector_size);
        self.set_view_source(path, disk, size, alignment);
        let table = partition::inspect(
            &mut self.reader()?,
            size,
            (alignment >= 512).then_some(alignment),
        )?;
        self.sector = table.sector;
        self.layout = format!(
            "{}\n{}",
            path.display(),
            field("Logical sector size", format!("{} B", table.sector))
        );
        self.findings = table.notes;
        for p in table.partitions {
            let label = format!(
                "#{}  {}  ·  LBA {}–{}",
                p.slot,
                i18n::t(&p.kind),
                p.first,
                p.last
            );
            if let (Some(start), Some(end)) = (
                p.first.checked_mul(table.sector),
                p.last
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(table.sector)),
            ) && p.within_usable_bounds
                && start < end
                && end <= size
            {
                self.choices.push(label);
                self.regions.push((start, end - start));
            } else {
                write!(self.layout, "\n{label}").ok();
            }
        }
        Ok(())
    }
}

fn backing<'a>(mut drive: &'a Drive, inventory: &'a [Drive]) -> Option<&'a Drive> {
    for _ in 0..32 {
        if matches!(drive.kind.as_str(), "Entire drive" | "USB drive") {
            return Some(drive);
        }
        let parent = drive.parent.as_ref()?;
        drive = inventory.iter().find(|d| &d.path == parent)?;
    }
    None
}

pub(crate) fn check(path: &Path, selected: Option<&Drive>) -> Report {
    let mut report = Report::default();
    let inventory = selected
        .and_then(|_| drives::inventory(true).ok())
        .unwrap_or_default();
    let selected = inventory.iter().find(|d| d.path == path).or(selected);
    let size = selected.map_or_else(|| path.metadata().map_or(0, |m| m.len()), |d| d.size);
    let physical = selected.and_then(|d| backing(d, &inventory));
    report.overview = format!(
        "{}\n{}",
        path.display(),
        field(
            "Capacity",
            format!("{} ({size} B)", crate::format_bytes(size))
        )
    );
    if let Some(d) = selected {
        write!(
            report.overview,
            "\n{} · {}\n{}",
            d.display_name(),
            i18n::t(&d.location),
            field(
                "Encryption",
                i18n::t(if d.unlocked || d.kind == "Unlocked volume" {
                    "Unlocked"
                } else if d.encrypted {
                    "Encrypted"
                } else {
                    "Unknown or unreported"
                })
            )
        )
        .ok();
        if let Some(disk) = physical {
            write!(
                report.overview,
                "\n{}",
                field(
                    "Backing disk",
                    format!("{} ({})", disk.path.display(), disk.display_name())
                )
            )
            .ok();
            for volume in inventory.iter().filter(|v| {
                v.path != disk.path && backing(v, &inventory).is_some_and(|b| b.path == disk.path)
            }) {
                write!(
                    report.overview,
                    "\n  {} · {} · {}",
                    volume.path.display(),
                    volume.display_name(),
                    i18n::t(&volume.location)
                )
                .ok();
            }
        }
        if let Some((health, raw)) = drives::health(&physical.unwrap_or(d).path) {
            report.health = health.lines().map(i18n::t).collect::<Vec<_>>().join("\n");
            report.raw = raw;
        } else {
            report.health = i18n::t(
                "Unavailable. The device, adapter, or system driver did not expose health data.",
            );
        }
    } else {
        report.health = i18n::t("Disk images contain no live device health readings.");
    }
    let opened = drives::open_source(path);
    let probe = (|| {
        if size == 0 {
            return Err(io::Error::other("Empty or unknown source size"));
        }
        let mut reader = SourceReader::new(
            opened
                .as_ref()
                .map_err(|e| io::Error::new(e.kind(), e.to_string()))?
                .try_clone()?,
            selected.map_or(1, |d| d.sector_size),
            Some(size),
        )?;
        partition::filesystem(&mut reader, 0, size)
    })();
    match probe {
        Ok(fs) => write!(
            report.overview,
            "\n{}\n{}",
            field("Read access", i18n::t("Source header readable")),
            field("Filesystem signature", i18n::t(fs))
        )
        .ok(),
        Err(e) => write!(report.overview, "\n{}", field("Read check incomplete", e)).ok(),
    };
    // A readable volume remains inspectable when its physical disk cannot be opened.
    if selected.is_some()
        && physical.is_none_or(|d| d.path != path)
        && let Ok(file) = &opened
        && let Ok(file) = file.try_clone()
    {
        report.set_view_source(path, file, size, selected.map_or(1, |d| d.sector_size));
    }
    if selected.is_some() && physical.is_none() {
        report.layout = i18n::t(
            "Backing disk unavailable. Open a whole-disk image to inspect its partition table.",
        );
    } else {
        let disk_path = physical.map_or(path, |d| &d.path);
        let disk = if disk_path == path {
            opened
        } else {
            drives::open_source(disk_path)
        };
        if let Err(e) = disk.and_then(|disk| report.inspect_disk(disk_path, physical, disk)) {
            report.layout = i18n::t(&format!("Partition check incomplete · {e}"));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_report_opens_partition_at_its_actual_offset() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("disk.img");
        let mut data = vec![0; 16384];
        data[510..512].copy_from_slice(&[0x55, 0xaa]);
        data[450] = 7;
        data[454] = 8;
        data[458] = 8;
        data[4099..4107].copy_from_slice(b"NTFS    ");
        std::fs::write(&path, &data).unwrap();
        let report = check(&path, None);
        assert!(report.health.contains("images"));
        let (mut reader, start, size) = report.open_partition(0).unwrap();
        assert_eq!(start, 4096);
        assert_eq!(
            partition::filesystem(&mut reader, start, size).unwrap(),
            "NTFS"
        );
        assert_eq!(std::fs::read(path).unwrap(), data);
    }
}
