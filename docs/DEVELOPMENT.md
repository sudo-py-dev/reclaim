# Developer guide

For a user-focused overview, see the [main README](../README.md).

A Rust desktop app for recovering file candidates from drives and raw disk images. The desktop searches unallocated clusters using a filesystem allocation map and opens source data read-only. The lower-level full-source signature API retains limited deleted NTFS resident-record recovery.

## Features

- **Native drive discovery:** Linux sysfs, Windows storage APIs, and macOS IOKit/Disk Arbitration. No `lsblk`, PowerShell, `diskutil`, or `plutil` subprocesses.
- **Unused-space default:** NTFS, FAT12/16/32, and exFAT volume maps; occupied clusters and filesystem metadata are excluded from signature carving. No full-source fallback.
- **Live scan results:** unused-byte progress, cancellation, and recoverable partial results.
- **Filtered recovery:** search by filename, extension, or source offset; filter by file category. Recovery saves only selected files matching the active filters.
- **Safe output naming:** original NTFS leaf names when available, generated names otherwise, and numeric suffixes to avoid replacing existing files.
- **Localization:** saved language preference, seven matching translation catalogs, mirrored Hebrew/Arabic layouts, and isolated mixed-direction filenames and paths.
- **Desktop interface:** Slint with light/dark appearance, SVG icons, animated controls and progress, a scrollable drive list, and an integrated recovery guide.

## Build and run

Install stable [Rust](https://www.rust-lang.org/tools/install) and your platform's native build tools, then run:

```sh
cargo run --release --locked
```

On Ubuntu, install the native GUI development dependencies first:

```sh
sudo apt-get update
sudo apt-get install libfontconfig-dev
```

To build an executable without launching the app:

```sh
cargo build --release --locked
```

The executable is written to `target/release/reclaim` (`reclaim.exe` on Windows).

## Recover files

1. Stop writing to the affected drive. Choose the drive/volume in Reclaim, or open a raw disk image.
2. Select **Scan Drive** or **Scan Image**. Results appear as scanning progresses. You can stop and recover the files found so far.
3. Filter the results, then select the files to recover. The recovery count reflects the selected files currently shown.
4. Select **Recover files…** and save to a **different physical drive**. Open the saved files to verify their contents.

Open **Settings & help** in the sidebar for appearance, language, updates, and recovery guidance.

### Drive permissions

- **Linux:** discovery works without elevated access. If opening a block device is denied, Reclaim can request a read-only descriptor through the UDisks2 system API and the normal polkit authentication prompt. This fallback requires UDisks2, a system bus, and a polkit authentication agent. Labels and known LUKS state use optional udev metadata. Unlock encrypted volumes before refreshing.
- **Windows:** discovery uses native storage metadata queries. Reading raw drives may require running Reclaim as administrator. Unlock BitLocker volumes first. Drive-letter volumes with multiple backing extents are omitted.
- **macOS:** discovery uses IOKit, Disk Arbitration, and the kernel mount table. Raw reads may require Full Disk Access and administrator privileges. Unlock encrypted volumes first.

Disk images use ordinary file permissions. Reclaim does not unmount drives, unlock encrypted volumes, change device permissions, or automatically elevate itself. Scanning and export reuse the opened source handle.

## Supported recovery

| Category | Formats |
| --- | --- |
| Pictures | JPEG, PNG, GIF, BMP, WebP, QOI |
| Icons | ICO, including supported PNG and DIB payloads |
| Documents | PDF, DOCX/XLSX/PPTX, DOCM/XLSM/PPTM, ODT/ODS/ODP, EPUB |
| Archives and applications | ZIP, 7z, RAR5, CAB, TAR, GZIP, XAR, JAR, APK |
| Programs and installers | EXE/DLL, ELF, MSI/MSP/MST, DEB, RPM, DEX, XAR-based PKG |
| Compound documents | OLE/CFB (generic `ole` unless an Installer root CLSID identifies MSI/MSP/MST) |
| Audio and video | WAV, AIFF/AIFC, known-length AU, single-segment RIFF AVI |
| NTFS metadata | Supported deleted FILE records with resident unnamed DATA; original Unicode leaf names and inline contents |

Images receive decoding/structural validation. Other results are identified as unverified candidates or checked NTFS records. ZIP-based document/application extensions are inferred from archive metadata.

See [file-format research and coverage](FILE_FORMAT_RESEARCH.md) for primary sources, checks, and unsupported variants. Header/structure checks do not establish installability or executable completeness.

### Limits

Reclaim is not a complete filesystem recovery suite. A signature match does not establish that a file was deleted or that all its contents are usable. Overwritten or TRIM-erased data cannot be reconstructed by the scanner.

- Most candidates are limited to 64 MiB each, with a maximum of 100,000 results. Classic ZIP archives with known local entry sizes can exceed 64 MiB (up to the classic ZIP offset range) using bounded-memory directory checks; large ZIP-based packages are reported as ZIP. ZIP entry payloads are not decompressed or CRC-verified. Captured NTFS resident data has a separate 64 MiB memory budget.
- Original folder reconstruction, restoring timestamps, fragmented-file reconstruction, and APFS/ext4 allocation maps are not implemented. NTFS runlists are read for the MFT/$Bitmap allocation map only, not general deleted-file reconstruction. Split NTFS attributes needing ATTRIBUTE_LIST extents and whole-disk MBR/GPT sources are rejected; select a formatted volume instead.
- `scan_unused_reader` is the desktop entry point. `scan_reader` remains a full-source API for tests and explicit lower-level use. Deleted resident NTFS recovery inside allocated MFT clusters is excluded from the desktop unused-space mode.
- ZIP64/multipart archives, ZIP archives over 64 MiB that require data descriptors, unknown-length AU streams, and multi-segment OpenDML AVI are unsupported.
- Mounted or changing sources can produce inconsistent results. For repeatable work, scan a raw disk image.
- Language preference persists; appearance choices and scan results are session-local.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

`cargo test` includes the project audit in `tests/audit` (security rules tagged
by CWE, translation coverage and dead keys, supply-chain policy). Run it alone
with `cargo test --locked --test audit`. See [the code and UI audit](CODE_UI_AUDIT.md)
for what it checks and how to accept a reviewed finding.

Tests cover signature boundaries, image validation, NTFS records, filtered exports, collision protection, sector-aligned reads, native Linux metadata fixtures, cancellation, and Slint page rendering in both themes.

Optional headless UI previews:

```sh
mkdir -p /tmp/reclaim-previews
RECLAIM_PREVIEW_DIR=/tmp/reclaim-previews cargo test --locked --bin reclaim slint_workspace
```

The previews are PNG images rendered from the actual Slint components using its software renderer. They do not test native window management, authentication prompts, or real-device recovery.

[Desktop CI](../.github/workflows/build.yml) runs locked tests (including the UI
and project audit) and Clippy with warnings as errors on Linux, Windows, and
macOS. Formatting runs once on Linux. Separate jobs check dependency advisories,
bans and sources, and install the website's locked dependencies, type-check it,
and build it. Checks run on pull requests, main, and weekly; newer commits cancel
older PR runs. Third-party actions are pinned to immutable revisions.

Tests compile the app and UI in the test profile; PR checks do not perform a
second optimized build, package installers, or upload executable artifacts.
The [release workflow](../.github/workflows/release.yml) calls the same validation
workflow before building optimized binaries and native installers for all four
shipping targets. Only releases upload artifacts, sign updates, and publish
GitHub downloads. Release runs are serialized per tag and artifacts expire after
seven days. macOS binaries use ad-hoc signing; Developer ID signing and
notarization are not configured. Native desktop and hardware testing remain
necessary; headless tests do not replace them.

## License and credits

Reclaim is licensed under [MIT](../LICENSE).

- [Slint](https://slint.dev) provides the interface; [rfd](https://github.com/PolyMeilex/rfd) provides native file dialogs.
- The bundled DejaVu font includes its [license](../assets/fonts/DejaVu-LICENSE.txt).
- Interface icons are project-local SVG assets in `ui/icons/`.
- Translation notes are in [locales/README.md](../locales/README.md).

## Appearance

Choose light or dark appearance under Settings & help. The custom surfaces and Slint standard widgets use the same color scheme. The choice applies for the current session.

## Result metadata

Each result has a file-type badge and an information action. The details dialog shows exact size, source offset, validation result, filename provenance, and deletion status. Checked deleted NTFS resident records retain their record/sequence identifiers and available `$STANDARD_INFORMATION` creation, last-write, record-change, and last-access timestamps. FILETIME values are converted to UTC; zero and out-of-range dates are treated as unavailable. No deletion timestamp is inferred. Signature-only candidates have unknown deletion status and no filesystem dates. Exported filesystem timestamps are not rewritten from this metadata.

## Automatic drive refresh

A background worker rechecks the native drive inventory about every two seconds. Only changed inventories (including mount/unlock metadata) wake the UI. Manual Refresh requests use the same worker. Bounded update/request channels prevent an update backlog; dropping the monitor disconnects the worker. Existing selection is retained while its device remains present; removed selections are cleared, and disk-image selections are unaffected. Transient discovery errors retain the last successful list. Inventory updates do not restart scans or discard their results/authorized handles.

## Repository tooling

`cargo xtask` builds the independent `tools/` crate, without the desktop UI.
It replaces the former scripting tools:

```sh
cargo xtask fixtures
cargo xtask seed-corpus
cargo xtask package linux --binary target/release/reclaim
```

`fixtures` regenerates the project-owned framing data in `tests/fixtures/formats`.
Compression output can differ between encoder versions while preserving the
same decoded data. `seed-corpus` copies small checked-in fixtures and creates
NTFS/PDF seeds for both fuzz harnesses. Installer packaging requires the native
platform's packaging tools listed in the README.

The release workflow also uses `cargo xtask validate-tag`, `bundle`, and
`checksums`; update signatures remain a separate, verified release step.
CI formats, tests, and lints the tooling crate on all desktop platforms.
