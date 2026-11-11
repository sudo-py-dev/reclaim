use super::*;
#[derive(Clone)]
struct Stream {
    runs: Vec<(u64, u64)>,
    size: u64,
    initialized: u64,
}
enum Data {
    Resident(Vec<u8>),
    Nonresident(Stream),
}
fn fix_record(mut record: Vec<u8>, sector: usize) -> io::Result<Vec<u8>> {
    if record.len() < 48
        || &record[..4] != b"FILE"
        || u16at(&record, 22) & 1 == 0
        || record[32..40] != [0; 8]
    {
        return Err(invalid("NTFS allocation metadata record is invalid"));
    }
    let usa = u16at(&record, 4) as usize;
    let count = u16at(&record, 6) as usize;
    let first = u16at(&record, 20) as usize;
    let used = u32at(&record, 24) as usize;
    if count != record.len() / sector + 1
        || usa < 48
        || usa + count * 2 > first
        || first < 48
        || used > record.len()
        || used < first + 4
        || u32at(&record, 28) as usize != record.len()
    {
        return Err(invalid("NTFS metadata record geometry is invalid"));
    }
    let sequence = u16at(&record, usa);
    if sequence == 0 {
        return Err(invalid("NTFS metadata fixup is invalid"));
    }
    for index in 1..count {
        let tail = index * sector - 2;
        if u16at(&record, tail) != sequence {
            return Err(invalid("NTFS metadata sector fixup failed"));
        }
        let pair = [record[usa + index * 2], record[usa + index * 2 + 1]];
        record[tail..tail + 2].copy_from_slice(&pair);
    }
    Ok(record)
}
fn little(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .enumerate()
        .fold(0, |value, (i, byte)| value | (u64::from(*byte) << (i * 8)))
}
fn data(record: &[u8], clusters: u64, cluster_size: u64) -> io::Result<Data> {
    let mut at = u16at(record, 20) as usize;
    let used = u32at(record, 24) as usize;
    let mut found = None;
    let mut terminated = false;
    while at + 4 <= used {
        let kind = u32at(record, at);
        if kind == 0xffffffff {
            terminated = true;
            break;
        }
        if at + 16 > used {
            return Err(invalid("NTFS attribute is truncated"));
        }
        let len = u32at(record, at + 4) as usize;
        if len < 24 || !len.is_multiple_of(8) || at + len > used {
            return Err(invalid("NTFS attribute length is invalid"));
        }
        if kind == 0x20 {
            return Err(invalid(
                "Unused-space scanning cannot read split NTFS allocation attributes yet",
            ));
        }
        let attr = &record[at..at + len];
        if kind == 0x80 && attr[9] == 0 {
            if found.is_some() || u16at(attr, 12) != 0 {
                return Err(invalid(
                    "NTFS allocation stream is compressed, encrypted, sparse, or ambiguous",
                ));
            }
            found = Some(match attr[8] {
                0 => {
                    let size = u32at(attr, 16) as usize;
                    let offset = u16at(attr, 20) as usize;
                    if offset < 24
                        || offset.checked_add(size).is_none_or(|n| n > len)
                        || size > MAX_MAP
                    {
                        return Err(invalid("Invalid resident NTFS bitmap"));
                    }
                    Data::Resident(attr[offset..offset + size].to_vec())
                }
                1 => {
                    if len < 64 || u64at(attr, 16) != 0 {
                        return Err(invalid("Unsupported NTFS allocation extent"));
                    }
                    let mut p = u16at(attr, 32) as usize;
                    let last = u64at(attr, 24);
                    let size = u64at(attr, 48);
                    let initialized = u64at(attr, 56);
                    if p < 64 || p >= len || initialized > size {
                        return Err(invalid("Invalid NTFS stream lengths"));
                    }
                    let mut runs = Vec::new();
                    let mut lcn = 0i128;
                    let mut covered = 0u64;
                    let mut end = false;
                    while p < len {
                        let tag = attr[p];
                        p += 1;
                        if tag == 0 {
                            end = true;
                            break;
                        }
                        let n = (tag & 15) as usize;
                        let o = (tag >> 4) as usize;
                        if n == 0
                            || n > 8
                            || o == 0
                            || o > 8
                            || p + n + o > len
                            || runs.len() >= 65536
                        {
                            return Err(invalid("Invalid or sparse NTFS allocation runs"));
                        }
                        let length = little(&attr[p..p + n]);
                        p += n;
                        let raw = little(&attr[p..p + o]);
                        let negative = attr[p + o - 1] & 128 != 0;
                        let delta = if negative {
                            i128::from(raw) - (1i128 << (o * 8))
                        } else {
                            i128::from(raw)
                        };
                        p += o;
                        lcn += delta;
                        if length == 0
                            || lcn < 0
                            || lcn > i128::from(clusters)
                            || (lcn as u64)
                                .checked_add(length)
                                .is_none_or(|v| v > clusters)
                        {
                            return Err(invalid("NTFS allocation run leaves the volume"));
                        }
                        covered = covered
                            .checked_add(length)
                            .ok_or_else(|| invalid("Invalid NTFS run length"))?;
                        if covered > clusters {
                            return Err(invalid("NTFS allocation runs exceed the volume"));
                        }
                        runs.push((lcn as u64, length));
                    }
                    if !end
                        || runs.is_empty()
                        || last.checked_add(1) != Some(covered)
                        || product(covered, cluster_size)? < size
                    {
                        return Err(invalid("NTFS allocation runs do not cover their stream"));
                    }
                    Data::Nonresident(Stream {
                        runs,
                        size,
                        initialized,
                    })
                }
                _ => return Err(invalid("Invalid NTFS attribute storage type")),
            });
        }
        at += len;
    }
    if !terminated {
        return Err(invalid("NTFS metadata attributes have no terminator"));
    }
    found.ok_or_else(|| invalid("NTFS allocation stream is missing"))
}
fn stream_read(
    input: &mut (impl Read + Seek),
    stream: &Stream,
    offset: u64,
    len: usize,
    cluster_size: u64,
    total: u64,
    cancel: &AtomicBool,
) -> io::Result<Vec<u8>> {
    let end = offset
        .checked_add(len as u64)
        .ok_or_else(|| invalid("Invalid NTFS stream position"))?;
    if end > stream.size || end > stream.initialized || len > MAX_MAP {
        return Err(invalid("NTFS allocation metadata is not fully initialized"));
    }
    // audit-allow(DOS-ALLOC): len is checked against MAX_MAP (64 MiB) above.
    let mut bytes = Vec::with_capacity(len);
    let mut logical = 0;
    for (lcn, clusters) in &stream.runs {
        let run_bytes = product(*clusters, cluster_size)?;
        let run_end = logical + run_bytes;
        let from = offset.max(logical);
        let to = end.min(run_end);
        if from < to {
            bytes.extend(read_at(
                input,
                lcn * cluster_size + from - logical,
                (to - from) as usize,
                total,
                cancel,
            )?);
        }
        logical = run_end;
        if logical >= end {
            break;
        }
    }
    if bytes.len() != len {
        return Err(invalid("NTFS allocation metadata is incomplete"));
    }
    Ok(bytes)
}
pub(super) fn plan(
    input: &mut (impl Read + Seek),
    b: &[u8],
    total: u64,
    cancel: &AtomicBool,
) -> io::Result<Plan> {
    let sector = u16at(b, 11);
    let per = u64::from(b[13]);
    let sectors = u64at(b, 40);
    if !matches!(sector, 512 | 1024 | 2048 | 4096)
        || per == 0
        || !per.is_power_of_two()
        || per > 128
        || sectors == 0
        || product(sectors, sector)? > total
    {
        return Err(invalid("Invalid NTFS volume geometry"));
    }
    let cluster_size = sector * per;
    let clusters = sectors / per;
    if clusters == 0 || clusters.div_ceil(8) > MAX_MAP as u64 {
        return Err(invalid(
            "NTFS allocation bitmap exceeds the supported map size",
        ));
    }
    let size_code = b[64] as i8;
    let record_size = if size_code < 0 {
        1u64.checked_shl((-i16::from(size_code)) as u32)
            .ok_or_else(|| invalid("Invalid NTFS record size"))?
    } else {
        product(size_code as u64, cluster_size)?
    };
    if !(1024..=65536).contains(&record_size)
        || !record_size.is_multiple_of(sector)
        || !record_size.is_power_of_two()
    {
        return Err(invalid("Unsupported NTFS record geometry"));
    }
    let mft_at = product(u64at(b, 48), cluster_size)?;
    let record = fix_record(
        read_at(input, mft_at, record_size as usize, total, cancel)?,
        sector as usize,
    )?;
    let Data::Nonresident(mft) = data(&record, clusters, cluster_size)? else {
        return Err(invalid("NTFS MFT data is not a nonresident stream"));
    };
    let raw = stream_read(
        input,
        &mft,
        6 * record_size,
        record_size as usize,
        cluster_size,
        total,
        cancel,
    )?;
    let bitmap_record = fix_record(raw, sector as usize)?;
    // Verify the metadata record identity rather than trusting a physical guess.
    if u32at(&bitmap_record, 44) != 6 {
        return Err(invalid("NTFS volume bitmap record identity does not match"));
    }
    let bitmap_data = data(&bitmap_record, clusters, cluster_size)?;
    let needed = clusters.div_ceil(8) as usize;
    let (bitmap, bitmap_runs) = match bitmap_data {
        Data::Resident(bytes) => {
            if bytes.len() < needed {
                return Err(invalid("NTFS bitmap is truncated"));
            }
            (bytes, Vec::new())
        }
        Data::Nonresident(stream) => {
            let bytes = stream_read(input, &stream, 0, needed, cluster_size, total, cancel)?;
            (bytes, stream.runs)
        }
    };
    for (lcn, count) in mft.runs.iter().chain(bitmap_runs.iter()) {
        for cluster in *lcn..*lcn + *count {
            if cluster.is_multiple_of(8192) {
                check(cancel)?;
            }
            if bitmap[(cluster / 8) as usize] & (1 << (cluster % 8)) == 0 {
                return Err(invalid("NTFS metadata is incorrectly marked unused"));
            }
        }
    }
    if bitmap[0] & 1 == 0 {
        return Err(invalid("NTFS boot cluster is incorrectly marked unused"));
    }
    from_bitmap(&bitmap, clusters, 0, cluster_size, "NTFS", cancel)
}
