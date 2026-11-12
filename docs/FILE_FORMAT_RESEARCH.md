# File-signature research and recovery coverage

Researched 2026-11-02 using publisher specifications, upstream source, Debian's
manuals, and Microsoft Open Specifications. This document separates recognizing
a format from establishing a recoverable byte range and validating its contents.
A header match alone is not treated as a complete file.

## Added formats

These additions expose 20 extensions across 15 format families. Each candidate
has a bounded extent probe, truncation checks, category mapping, portable naming,
and export-time format rechecking. Source reads remain read-only. Programs and
installers are never executed to identify or test them.

| Family / extensions | Signature | Boundary and checks | Result label / limitations |
| --- | --- | --- | --- |
| Standard MIDI: `mid` | `MThd` | Formats 0–2, declared header and track lengths, valid division, bounded four-byte variable integers, channel/running-status events, meta/SysEx lengths, exact End-of-Track. | **Structure checked**. At most 4096 tracks and one million events per track, 64 MiB total. Unknown chunks, RIFF-wrapped MIDI and MIDI 2.0 clips are unsupported. Meta/SysEx payload semantics are not interpreted. |
| Uncompressed Flash: `swf` | `FWS` | Versions 1–19, declared total length, variable RECT extent, frame header, bounded short/long tag extents, required first FileAttributes for version 8+, ShowFrame count and exact final End tag. | **Unverified candidate**. Tag framing only; nested payloads, scripts and rectangle coordinates are not semantically validated or executed. Compressed CWS/ZWS are unsupported. 64 MiB maximum. |
| Windows PE: `exe`, `dll` | `4D 5A`, then `50 45 00 00` at the DOS header's PE pointer | PE32/PE32+ optional header; 1–96 sections; aligned, nonoverlapping raw ranges; referenced symbol/string table; certificate-table extent and record lengths. DLL flag selects extension. | **Unverified candidate**. Extent inferred from referenced structures, not an exact EOF. Unreferenced overlays, some debug data, and self-extracting installer payloads may be missing. Old DOS/NE/LE files are not PE and are rejected. Driver/native images retain `exe`; no guessed `sys` extension. |
| Linux / Unix ELF: `elf` | `7F 45 4C 46` | 32/64-bit, little/big endian; executable/shared/relocatable/core types; program and section tables; all referenced file-backed ranges. `SHT_NOBITS` does not add file bytes. | **Unverified candidate**. Inferred minimum extent; no trailing-data guarantee. Extended numbering is not implemented. ET_DYN remains `elf` because it can be a PIE executable or shared library. |
| Compound storage / Windows Installer: `ole`, `msi`, `msp`, `mst` | `D0 CF 11 E0 A1 B1 1A E1` | CFB v3/v4 header and sector geometry; DIFAT/FAT bounds and sector markers; directory-chain loops; root entry and root CLSID. Extent through last allocated sector. | **Unverified candidate**. Mini-stream contents, storage trees, installer database tables, payloads, and digital signatures are not fully validated. Unallocated trailing sectors can be omitted. Other compound documents use `ole`, rather than guessing old Office extensions. |
| Debian package: `deb` | `21 3C 61 72 63 68 3E 0A` (`!<arch>\n`) | ar member framing and lengths; `debian-binary` 2.x; supported `control.tar*` then `data.tar*`; underscore extension members; even-byte member alignment. | **Structure checked**. Stops after required data member; optional trailing members can be omitted. Compressed member contents and package installability are not verified. Generic ar/thin archives are not claimed as DEB. |
| RPM package: `rpm` | `ED AB EE DB` | Legacy v3 lead used by RPM v4 packages; signature/main-header framing and bounded index/store ranges; signature SIZE/LONGSIZE supplies header+payload extent. | **Unverified candidate**. Requires advertised package length. Package hashes, signatures, payload and installability are not verified. RPM v6 format changes are not claimed. |
| 7-Zip: `7z` | `37 7A BC AF 27 1C` | Start-header CRC; checked next-header offset/length; next-header CRC and header/encoded-header type. | **Structure checked**. Does not unpack or verify data streams. Requires nonempty next header; split/truncated sets and headerless empty variants are not supported. |
| Cabinet: `cab` | `4D 53 43 46` (`MSCF`) | Cabinet length/version/reserved fields; optional reserved areas; folder/file tables; names, folder references, data-block bounds and sizes. | **Structure checked**. Single-cabinet form only; no previous/next cabinet links or split files. Compression is not decoded and CFDATA checksums are not verified. |
| RAR5: `rar` | `52 61 72 21 1A 07 01 00` | Header CRC32, bounded variable integers, main/file/service records, packed-data lengths, end marker. | **Structure checked**. RAR5 only; RAR4, multi-volume and encrypted-header archives are not implemented. File payloads are not decompressed or CRC-verified. |
| TAR: `tar` | `ustar` at offset 257 | Candidate starts 257 bytes before marker; every member header has a valid checksum and supported type; octal data lengths; two zero end blocks. Marker survives scan-block boundaries. | **Structure checked**. USTAR/PAX and compatible GNU long-name records. Old V7 without marker, GNU sparse/base-256 sizes, concatenated archives and optional tape-blocking padding are not reconstructed. |
| GZIP: `gz` | `1F 8B 08` | Optional-header fields and header CRC; incremental raw-DEFLATE decoding; trailer CRC32 and uncompressed size. | **Structure checked**. One member per result; concatenated members appear separately. Output is streamed into a fixed scratch buffer and limited to 256 MiB; compressed candidate limit is 64 MiB. |
| Apple XAR / flat package: `xar`, `pkg` | `78 61 72 21` (`xar!`) | Header sizes/version; bounded zlib TOC decoding; XML structure and heap extents from data, extended-attribute, checksum and signature records. `PackageInfo`/`Distribution` metadata identifies a PKG candidate. | **Unverified candidate**. No payload/signature verification; TOC metadata can be misleading. TOC compressed/uncompressed lengths capped at 4 MiB, nesting at 64. DTDs/entity references are rejected. Not DMG, old bundle-style PKG, or every XAR variant. |
| Android DEX: `dex` | `dex\n` + supported version + NUL | Versions 035, 037–040; declared file/header lengths and endian tag; table/map ranges; full Adler-32 check. | **Unverified candidate**. Does not verify SHA-1, instructions or all map entries. Reverse-endian, compact DEX and experimental 041 containers are not implemented. |
| QOI image: `qoi` | `71 6F 69 66` (`qoif`) | Dimensions/channels/colorspace; opcode byte lengths, exact pixel/run count, eight-byte end marker. | **Structure checked**. Pixel framing, not an image-decoder validation. At most 16,384 per dimension and 64 million pixels. Existing thumbnail codecs are unchanged. |

“Structure checked” describes framing only unless the row explicitly states a
payload check. It does not mean trusted, signed, installable, or safe to run.
Executable/container candidates labeled unverified are not used to skip embedded
files during the main signature search.

## Primary references

- [MIDI Association Standard MIDI Files specification](https://midi.org/standard-midi-files-specification), cross-checked against the [upstream Midly SMF reader](https://github.com/kovaxis/midly/blob/master/src/smf.rs): header/track layout and formats. Added 2026-11-12.
- [Adobe SWF specification v19, publisher document mirrored by Open Flash](https://open-flash.github.io/mirrors/swf-spec-19.pdf), chapters 1–4: file length, RECT extent, record headers, FileAttributes, ShowFrame and End. Added 2026-11-12.

- [Microsoft PE/COFF specification](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format): DOS/PE headers, raw section extents, data directories and certificate-file offsets.
- [Microsoft CFB specification](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b), [CFB header](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/05060311-bfce-4b12-874d-71fd4ce63aea), [DIFAT](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/0afa4e43-b18f-432a-9917-4f276eca7a73): sector allocation and storage layout.
- [Microsoft's Installer file-type identifiers](https://devblogs.microsoft.com/setup/identifying-windows-installer-file-types/): root CLSIDs `000C1084` (MSI), `000C1086` (MSP), `000C1082` (MST), with common `0000-0000-C000-000000000046` suffix. Header CLSID is reserved; classification uses the root storage CLSID.
- [ELF gABI header](https://gabi.xinuos.com/elf/02-eheader.html), [program headers](https://gabi.xinuos.com/elf/07-pheader.html), [section headers](https://gabi.xinuos.com/elf/03-sheader.html): class/endianness, offsets, counts, file-backed section and segment sizes.
- [Debian deb(5)](https://manpages.debian.org/bookworm/dpkg-dev/deb.5.en.html): ar layout, mandatory member order, extensions, compression names and padding.
- [RPM v4 format](https://rpm-software-management.github.io/rpm/manual/format_v4.html) and [header format](https://rpm-software-management.github.io/rpm/manual/format_header.html): lead, signature, header/store framing and package-length tags. Lead major 3 is distinct from RPM's package-format version.
- [7-Zip's upstream format specification](https://github.com/ip7z/7zip/blob/main/DOC/7zFormat.txt): signature/start/next headers and CRCs.
- [Microsoft MS-CAB specification](https://download.microsoft.com/download/4/D/A/4DA14F27-B4EF-4170-A6E6-5B1EF85B1BAA/%5BMS-CAB%5D.pdf), independently cross-checked against [Chromium's cabinet writer](https://chromium.googlesource.com/chromium/src/+/fd8e1c8da7222a1ac2391d2e7504bca3e3df0453/chrome/tools/build/win/makecab.py): cabinet header and table/data framing.
- [RARLab RAR5 technical note](https://www.rarlab.com/technote.htm): signatures, CRCs, variable integers, block flags, data areas and terminator.
- [GNU TAR format documentation](https://www.gnu.org/software/tar/manual/html_node/Standard.html): header checksum, 512-byte members and end blocks.
- [IETF RFC 1952](https://www.rfc-editor.org/rfc/rfc1952): GZIP flags, optional fields, compressed stream and CRC/ISIZE trailer.
- [Apple XAR header](https://github.com/apple-oss-distributions/xar/blob/main/xar/include/xar.h.in), [archive reader/writer](https://github.com/apple-oss-distributions/xar/blob/main/xar/lib/archive.c), [heap data access](https://github.com/apple-oss-distributions/xar/blob/main/xar/lib/data.c): on-disk header, compressed XML TOC and heap-relative ranges.
- [Android's DEX specification](https://source.android.com/docs/core/runtime/dex-format): version-specific headers, file length, tables, map and checksum.
- [QOI publisher's specification](https://qoiformat.org/qoi-specification.pdf): header, chunks, pixel runs and end marker.

## Why not copy a huge magic-number list?

A signature catalogue can identify a possible format while saying nothing about
its physical end, fragmented layout, compressed stream termination, or integrity.
Adding signatures alone would produce arbitrary-size or truncated recoveries and
more false lookahead reads, the scan slowdown addressed in the ZIP fixes.

Formats considered for later work include RAR4, ZIP64, XZ/BZIP2/Zstandard,
Mach-O/universal binaries, MP4/MOV/HEIF, TIFF, ISO/UDF/DMG, and old Office-specific
CFB classification. They need additional extent/decoder/filesystem work or more
variant coverage; this implementation does not advertise those as supported.
Plain text, scripts, and arbitrary BIN/COM data usually lack a unique recoverable
signature/ending. Small resident NTFS files can still retain their original names
and content independently of signature carving.

## Verification and resources

- `cargo xtask fixtures` creates project-owned framing fixtures,
  including PE32/PE32+, certificate extents, four ELF class/byte-order combinations,
  CFB v3/v4 Installer CLSIDs, genuine USTAR/GZIP/DEB members, and small archive/image
  encodings. Minimal MSI/RPM/DEX fixtures exercise format metadata; they are not
  installable packages or working applications.
- `tests/new_formats.rs` verifies cross-block carving, exact exports, truncation,
  mutated headers, short reads, cancellation, checksum rejection, neighbouring
  GZIP members, XAR DTD rejection, and bounded work for common MZ noise.
- Probes retain the existing 64 MiB candidate limit; the earlier bounded-memory
  large-classic-ZIP path is separate. Loops, table counts, inflated output and XML
  depth have limits. These bounds do not establish a wall-clock deadline for a
  blocked drive read.
- Header buckets are built from the signature table at compile time. Most bytes
  need no signature comparisons. Impossible MZ pointers are rejected from the
  streaming scan buffer before extra I/O. GZIP uses a persistent decoder so each
  chunk is processed once during discovery.
