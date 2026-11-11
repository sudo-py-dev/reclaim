//! Project-owned, inert framing examples. These are parser data, not runnable programs.
use crate::Result;
use flate2::{Compression, GzBuilder, write::ZlibEncoder};
use sha1::{Digest, Sha1};
use std::{fs, io::Write, path::Path};

fn u16(b: &mut [u8], at: usize, value: u16) {
    b[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn u32(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn gzip(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}
fn zlib(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}
fn tar(name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut header = vec![0; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    header[124..136].copy_from_slice(format!("{:011o}\0", bytes.len()).as_bytes());
    header[136..148].copy_from_slice(b"00000000000\0");
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..265].copy_from_slice(b"ustar\x0000");
    let checksum: u32 = header.iter().map(|b| u32::from(*b)).sum();
    header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
    header.extend(bytes);
    header.resize(512 + bytes.len().div_ceil(512) * 512 + 1024, 0);
    header
}
fn ar_member(name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut result = format!(
        "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
        format!("{name}/"),
        0,
        0,
        0,
        "100644",
        bytes.len()
    )
    .into_bytes();
    assert_eq!(result.len(), 60);
    result.extend(bytes);
    if !bytes.len().is_multiple_of(2) {
        result.push(b'\n');
    }
    result
}
fn rar_block(body: &[u8], data: &[u8]) -> Vec<u8> {
    let mut header = vec![body.len() as u8];
    header.extend(body);
    let mut result = crc32fast::hash(&header).to_le_bytes().to_vec();
    result.extend(header);
    result.extend(data);
    result
}
fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in bytes {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
pub fn generate(output: &Path) -> Result {
    fs::create_dir_all(output)?;
    let put = |name: &str, bytes: &[u8]| -> Result {
        fs::write(output.join(name), bytes)?;
        Ok(())
    };
    for (wide, extension, flags, certificate) in [
        (false, "exe", 2, false),
        (true, "dll", 0x2002, false),
        (true, "exe", 2, true),
    ] {
        let mut b = vec![0; if certificate { 1040 } else { 1024 }];
        b[..2].copy_from_slice(b"MZ");
        u32(&mut b, 60, 128);
        b[128..132].copy_from_slice(b"PE\0\0");
        u16(&mut b, 132, if wide { 0x8664 } else { 0x14c });
        u16(&mut b, 134, 1);
        let optional = if wide { 240 } else { 224 };
        u16(&mut b, 148, optional);
        u16(&mut b, 150, flags);
        let o = 152;
        u16(&mut b, o, if wide { 0x20b } else { 0x10b });
        for (at, value) in [(32, 4096), (36, 512), (56, 8192), (60, 512)] {
            u32(&mut b, o + at, value);
        }
        u16(&mut b, o + 68, 3);
        u32(&mut b, o + if wide { 108 } else { 92 }, 16);
        let p = o + usize::from(optional);
        b[p..p + 8].copy_from_slice(b".text\0\0\0");
        for (at, value) in [(8, 1), (12, 4096), (16, 512), (20, 512), (36, 0x60000020)] {
            u32(&mut b, p + at, value);
        }
        b[512] = 0xc3;
        if certificate {
            u32(&mut b, o + 144, 1024);
            u32(&mut b, o + 148, 16);
            u32(&mut b, 1024, 16);
            u16(&mut b, 1028, 0x200);
            u16(&mut b, 1030, 2);
        }
        put(
            &format!(
                "{}.{}",
                if certificate {
                    "certificate"
                } else if wide {
                    "pe64"
                } else {
                    "pe32"
                },
                extension
            ),
            &b,
        )?;
    }
    for wide in [false, true] {
        for little in [false, true] {
            let (eh, ph) = if wide { (64, 56) } else { (52, 32) };
            let mut b = vec![0; 128];
            b[..7].copy_from_slice(&[
                0x7f,
                b'E',
                b'L',
                b'F',
                if wide { 2 } else { 1 },
                if little { 1 } else { 2 },
                1,
            ]);
            let mut write = |at: usize, width: usize, n: u64| {
                let bytes = if little {
                    n.to_le_bytes()
                } else {
                    n.to_be_bytes()
                };
                b[at..at + width].copy_from_slice(if little {
                    &bytes[..width]
                } else {
                    &bytes[8 - width..]
                });
            };
            write(16, 2, 2);
            write(18, 2, if wide { 62 } else { 3 });
            write(20, 4, 1);
            write(if wide { 32 } else { 28 }, if wide { 8 } else { 4 }, eh);
            let h = if wide { 52 } else { 40 };
            write(h, 2, eh);
            write(h + 2, 2, ph);
            write(h + 4, 2, 1);
            write(eh as usize, 4, 1);
            write(
                eh as usize + if wide { 32 } else { 16 },
                if wide { 8 } else { 4 },
                128,
            );
            write(
                eh as usize + if wide { 40 } else { 20 },
                if wide { 8 } else { 4 },
                128,
            );
            put(
                &format!(
                    "elf{}-{}.elf",
                    if wide { 64 } else { 32 },
                    if little { "le" } else { "be" }
                ),
                &b,
            )?;
        }
    }
    for (major, shift) in [(3, 9), (4, 12)] {
        for (extension, clsid) in [
            ("msi", 0xc1084),
            ("msp", 0xc1086),
            ("mst", 0xc1082),
            ("ole", 0),
        ] {
            let sector = 1 << shift;
            let mut b = vec![0; sector * 3];
            b[..8].copy_from_slice(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
            for (at, value) in [(24, 0x3e), (26, major), (28, 0xfffe), (30, shift), (32, 6)] {
                u16(&mut b, at, value);
            }
            for (at, value) in [
                (40, if major == 4 { 1 } else { 0 }),
                (44, 1),
                (48, 1),
                (56, 4096),
                (60, 0xfffffffe),
                (68, 0xfffffffe),
            ] {
                u32(&mut b, at, value);
            }
            b[76..512].fill(0xff);
            u32(&mut b, 76, 0);
            b[sector..sector * 2].fill(0xff);
            u32(&mut b, sector, 0xfffffffd);
            u32(&mut b, sector + 4, 0xfffffffe);
            let at = sector * 2;
            let name: Vec<_> = "Root Entry\0"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            b[at..at + name.len()].copy_from_slice(&name);
            u16(&mut b, at + 64, name.len() as u16);
            b[at + 66] = 5;
            b[at + 67] = 1;
            for offset in [68, 72, 76] {
                u32(&mut b, at + offset, u32::MAX);
            }
            u32(&mut b, at + 116, 0xfffffffe);
            if clsid != 0 {
                u32(&mut b, at + 80, clsid);
                b[at + 88] = 0xc0;
                b[at + 95] = 0x46;
            }
            put(&format!("cfb{major}.{extension}"), &b)?;
        }
    }
    let header = b"\x01\0";
    let mut start = Vec::new();
    start.extend(0u64.to_le_bytes());
    start.extend((header.len() as u64).to_le_bytes());
    start.extend(crc32fast::hash(header).to_le_bytes());
    let mut seven = b"7z\xbc\xaf\x27\x1c\0\x04".to_vec();
    seven.extend(crc32fast::hash(&start).to_le_bytes());
    seven.extend(start);
    seven.extend(header);
    put("empty.7z", &seven)?;
    let payload = b"Reclaim format recovery fixture\n";
    put("sample.tar", &tar("payload.txt", payload))?;
    put("sample.gz", &gzip(payload)?)?;
    let control=b"Package: reclaim-fixture\nVersion: 1.0\nArchitecture: all\nMaintainer: Reclaim contributors\nDescription: Project-owned recovery fixture\n";
    let mut deb = b"!<arch>\n".to_vec();
    for (name, data) in [
        ("debian-binary", b"2.0\n".to_vec()),
        ("control.tar.gz", gzip(&tar("control", control))?),
        (
            "data.tar.gz",
            gzip(&tar("usr/share/reclaim-fixture.txt", payload))?,
        ),
    ] {
        deb.extend(ar_member(name, &data));
    }
    put("sample.deb", &deb)?;
    let mut main = b"\x8e\xad\xe8\x01\0\0\0\0".to_vec();
    for n in [1u32, 4, 1000, 4, 0, 1, 0] {
        main.extend(n.to_be_bytes());
    }
    let mut raw = b"070701".to_vec();
    raw.extend(payload);
    let packed = gzip(&raw)?;
    let mut signature = b"\x8e\xad\xe8\x01\0\0\0\0".to_vec();
    for n in [1u32, 4, 1000, 4, 0, 1, (main.len() + packed.len()) as u32] {
        signature.extend(n.to_be_bytes());
    }
    let mut rpm = vec![0; 96];
    rpm[..6].copy_from_slice(b"\xed\xab\xee\xdb\x03\0");
    rpm[78..80].copy_from_slice(&5u16.to_be_bytes());
    rpm.extend(&signature);
    rpm.resize(rpm.len() + ((8 - signature.len() % 8) % 8), 0);
    rpm.extend(main);
    rpm.extend(packed);
    put("sample.rpm", &rpm)?;
    let name = b"payload.txt\0";
    let data_at = 36 + 8 + 16 + name.len();
    let total = data_at + 8 + payload.len();
    let mut cab = vec![0; total];
    cab[..4].copy_from_slice(b"MSCF");
    u32(&mut cab, 8, total as u32);
    u32(&mut cab, 16, 44);
    cab[24..26].copy_from_slice(b"\x03\x01");
    u16(&mut cab, 26, 1);
    u16(&mut cab, 28, 1);
    u32(&mut cab, 36, data_at as u32);
    u16(&mut cab, 40, 1);
    u32(&mut cab, 44, payload.len() as u32);
    cab[60..60 + name.len()].copy_from_slice(name);
    u16(&mut cab, data_at + 4, payload.len() as u16);
    u16(&mut cab, data_at + 6, payload.len() as u16);
    cab[data_at + 8..].copy_from_slice(payload);
    put("sample.cab", &cab)?;
    let mut rar = b"Rar!\x1a\x07\x01\0".to_vec();
    rar.extend(rar_block(b"\x01\0\0", &[]));
    let mut body = vec![
        2,
        2,
        payload.len() as u8,
        0,
        payload.len() as u8,
        0,
        0,
        1,
        1,
    ];
    body.push(b'a');
    rar.extend(rar_block(&body, payload));
    rar.extend(rar_block(b"\x05\0\0", &[]));
    put("sample.rar", &rar)?;
    let mut qoi = b"qoif".to_vec();
    qoi.extend(1u32.to_be_bytes());
    qoi.extend(1u32.to_be_bytes());
    qoi.extend(b"\x04\0\xff\x10\x20\x30\xff\0\0\0\0\0\0\0\x01");
    put("sample.qoi", &qoi)?;
    let mut dex = vec![0; 128];
    dex[..8].copy_from_slice(b"dex\n039\0");
    for (at, n) in [
        (32, 128),
        (36, 112),
        (40, 0x12345678),
        (52, 112),
        (104, 16),
        (108, 112),
        (112, 1),
        (120, 1),
        (124, 112),
    ] {
        u32(&mut dex, at, n);
    }
    u16(&mut dex, 116, 0x1000);
    // DEX specifies SHA-1 as a file-format checksum, not a security primitive.
    let digest = Sha1::digest(&dex[32..]);
    dex[12..32].copy_from_slice(&digest);
    let checksum = adler32(&dex[12..]);
    u32(&mut dex, 8, checksum);
    put("sample.dex", &dex)?;
    for (extension, name) in [("xar", "payload.txt"), ("pkg", "PackageInfo")] {
        let xml = format!(
            "<?xml version=\"1.0\"?><xar><toc><file id=\"1\"><name>{name}</name><type>file</type><data><length>{}</length><offset>0</offset><size>{}</size><encoding style=\"application/octet-stream\"/></data></file></toc></xar>",
            payload.len(),
            payload.len()
        );
        let toc = zlib(xml.as_bytes())?;
        let mut xar = vec![0; 28];
        xar[..4].copy_from_slice(b"xar!");
        xar[4..6].copy_from_slice(&28u16.to_be_bytes());
        xar[6..8].copy_from_slice(&1u16.to_be_bytes());
        xar[8..16].copy_from_slice(&(toc.len() as u64).to_be_bytes());
        xar[16..24].copy_from_slice(&(xml.len() as u64).to_be_bytes());
        xar.extend(toc);
        xar.extend(payload);
        put(&format!("sample.{extension}"), &xar)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixtures_are_deterministic_and_match_fixed_framing() -> Result {
        let a = tempfile::tempdir()?;
        let b = tempfile::tempdir()?;
        generate(a.path())?;
        generate(b.path())?;
        let generated = crate::files(a.path())?;
        assert_eq!(generated.len(), 26);
        for file in generated {
            let name = file.file_name().ok_or("fixture has no name")?;
            assert_eq!(fs::read(&file)?, fs::read(b.path().join(name))?);
            if !matches!(
                file.extension().and_then(|s| s.to_str()),
                Some("gz" | "deb" | "rpm" | "xar" | "pkg")
            ) {
                assert_eq!(
                    fs::read(&file)?,
                    fs::read(crate::root().join("tests/fixtures/formats").join(name))?,
                    "fixture {name:?}"
                );
            }
        }
        Ok(())
    }
}
