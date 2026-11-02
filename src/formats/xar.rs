use super::{CandidateEnd, le16, need, span};
use quick_xml::events::Event;
use std::{
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
};
struct Record {
    name: Vec<u8>,
    offset: Option<u64>,
    length: Option<u64>,
}
pub(super) fn inspect(b: &[u8], cancel: &AtomicBool) -> Result<(usize, bool), CandidateEnd> {
    need(b, 28)?;
    let header = u16::from_be_bytes(b[4..6].try_into().unwrap()) as usize;
    let compressed = u64::from_be_bytes(b[8..16].try_into().unwrap());
    let unpacked = u64::from_be_bytes(b[16..24].try_into().unwrap());
    if !b.starts_with(b"xar!")
        || header < 28
        || le16(b, 6) != 0x100
        || unpacked == 0
        || unpacked > 4 * 1024 * 1024
        || compressed == 0
        || compressed > 4 * 1024 * 1024
    {
        return Err(CandidateEnd::Invalid);
    }
    let heap = span(header as u64, compressed)?;
    need(b, heap)?;
    let mut decoder = flate2::read::ZlibDecoder::new(&b[header..heap]);
    let mut xml = Vec::new();
    decoder
        .by_ref()
        .take(unpacked + 1)
        .read_to_end(&mut xml)
        .map_err(|_| CandidateEnd::Invalid)?;
    if xml.len() as u64 != unpacked || decoder.total_in() != compressed {
        return Err(CandidateEnd::Invalid);
    }
    let mut reader = quick_xml::Reader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut stack: Vec<Record> = Vec::new();
    let mut extent = 0;
    let mut package = false;
    let mut root = false;
    let mut toc = false;
    for _ in 0..1_000_000 {
        if cancel.load(Ordering::Relaxed) {
            return Err(CandidateEnd::Invalid);
        }
        match reader.read_event().map_err(|_| CandidateEnd::Invalid)? {
            Event::Start(element) => {
                if stack.len() >= 64 {
                    return Err(CandidateEnd::Invalid);
                }
                let name = element.name().as_ref().to_vec();
                if stack.is_empty() {
                    if root || name != b"xar" {
                        return Err(CandidateEnd::Invalid);
                    }
                    root = true;
                }
                if name == b"toc" && stack.len() == 1 {
                    toc = true;
                }
                stack.push(Record {
                    name,
                    offset: None,
                    length: None,
                });
            }
            Event::Text(text) => {
                if stack.len() >= 2 {
                    let tag = &stack.last().unwrap().name;
                    let parent = stack.len() - 2;
                    if (matches!(tag.as_slice(), b"offset" | b"length")
                        || (tag == b"size"
                            && matches!(
                                stack[parent].name.as_slice(),
                                b"checksum" | b"signature" | b"x-signature"
                            )))
                        && matches!(
                            stack[parent].name.as_slice(),
                            b"data" | b"ea" | b"checksum" | b"signature" | b"x-signature"
                        )
                    {
                        let value = std::str::from_utf8(text.as_ref())
                            .ok()
                            .and_then(|s| s.parse::<u64>().ok())
                            .ok_or(CandidateEnd::Invalid)?;
                        if tag == b"offset" {
                            if stack[parent].offset.replace(value).is_some() {
                                return Err(CandidateEnd::Invalid);
                            }
                        } else if stack[parent].length.replace(value).is_some() {
                            return Err(CandidateEnd::Invalid);
                        }
                    } else if tag == b"name"
                        && matches!(&text[..], b"PackageInfo" | b"Distribution")
                    {
                        package = true;
                    }
                }
            }
            Event::End(_) => {
                let record = stack.pop().ok_or(CandidateEnd::Invalid)?;
                if let Some(offset) = record.offset {
                    let length = record.length.ok_or(CandidateEnd::Invalid)?;
                    extent = extent.max(span(offset, length)?);
                } else if record.length.is_some() {
                    return Err(CandidateEnd::Invalid);
                }
            }
            Event::DocType(_) | Event::GeneralRef(_) | Event::CData(_) => {
                return Err(CandidateEnd::Invalid);
            }
            Event::Eof => {
                if !root || !toc || !stack.is_empty() || extent == 0 {
                    return Err(CandidateEnd::Invalid);
                }
                let end = span(heap as u64, extent as u64)?;
                need(b, end)?;
                return Ok((end, package));
            }
            _ => {}
        }
    }
    Err(CandidateEnd::Invalid)
}
