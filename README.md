# Reclaim

A Rust desktop app for recovering file candidates from drives and raw disk images. Reclaim combines signature scanning with limited recovery of deleted NTFS resident files, and opens source data read-only.

## Features

- **Native drive discovery:** Linux sysfs, Windows storage APIs, and macOS IOKit/Disk Arbitration. No `lsblk`, PowerShell, `diskutil`, or `plutil` subprocesses.
- **Live scan results:** byte progress, throughput, cancellation, and recoverable partial results.
- **Filtered recovery:** search by filename, extension, or source offset; filter by file category. Recovery saves only selected files matching the active filters.
- **Safe output naming:** original NTFS leaf names when available, generated names otherwise, and numeric suffixes to avoid replacing existing files.
- **English and Hebrew:** immediate language switching, a saved language preference, and RTL layouts.
- **Desktop interface:** light/dark/system appearance, bundled icon fonts, optional animations, a scrollable drive list, and a four-step recovery guide.

## Build and run

Install stable [Rust](https://www.rust-lang.org/tools/install) and your platform's native build tools, then run:

```sh
cargo run --release --locked
```

On Ubuntu, install the native GUI development dependencies first:

```sh
sudo apt-get update
sudo apt-get install libx11-dev libxi-dev libxrandr-dev libxcursor-dev libwayland-dev libxkbcommon-dev libgl1-mesa-dev
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

Settings opens directly from the gear button. Choose appearance, language, or animations there. The book button in Settings opens the recovery guide, including details about filenames and recovery limits.

### Drive permissions

- **Linux:** discovery works without elevated access. If opening a block device is denied, Reclaim can request a read-only descriptor through the UDisks2 system API and the normal polkit authentication prompt. This fallback requires UDisks2, a system bus, and a polkit authentication agent. Labels and known LUKS state use optional udev metadata. Unlock encrypted volumes before refreshing.
- **Windows:** discovery uses native storage metadata queries. Reading raw drives may require running Reclaim as administrator. Unlock BitLocker volumes first. Drive-letter volumes with multiple backing extents are omitted.
- **macOS:** discovery uses IOKit, Disk Arbitration, and the kernel mount table. Raw reads may require Full Disk Access and administrator privileges. Unlock encrypted volumes first.

Disk images use ordinary file permissions. Reclaim does not unmount drives, unlock encrypted volumes, change device permissions, or automatically elevate itself. Scanning and export reuse the opened source handle.

## Supported recovery

| Category | Formats |
| --- | --- |
| Pictures | JPEG, PNG, GIF, BMP, WebP |
| Icons | ICO, including supported PNG and DIB payloads |
| Documents | PDF, DOCX/XLSX/PPTX, DOCM/XLSM/PPTM, ODT/ODS/ODP, EPUB |
| Archives and applications | ZIP, JAR, APK |
| Audio and video | WAV, AIFF/AIFC, known-length AU, single-segment RIFF AVI |
| NTFS metadata | Supported deleted FILE records with resident unnamed DATA; original Unicode leaf names and inline contents |

Images receive decoding/structural validation. Other results are identified as unverified candidates or checked NTFS records. ZIP-based document/application extensions are inferred from archive metadata.

### Limits

Reclaim is not a complete filesystem recovery suite. A signature match does not establish that a file was deleted or that all its contents are usable. Overwritten or TRIM-erased data cannot be reconstructed by the scanner.

- Candidates are limited to 64 MiB each and 100,000 results; captured NTFS resident data has a separate 64 MiB memory budget.
- Original folders, timestamps, nonresident NTFS cluster runs, general fragmented-file reconstruction, and FAT/exFAT/APFS/ext4 metadata recovery are not implemented.
- ZIP64/multipart archives, unknown-length AU streams, and multi-segment OpenDML AVI are unsupported.
- Mounted or changing sources can produce inconsistent results. For repeatable work, scan a raw disk image.
- Language preference persists; appearance/animation choices and scan results are session-local.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Tests cover signature boundaries, image validation, NTFS records, filtered exports, collision protection, sector-aligned reads, native Linux metadata fixtures, cancellation, RTL rendering, keyboard controls, and responsive layouts.

Optional headless UI previews:

```sh
mkdir -p /tmp/reclaim-previews
RECLAIM_PREVIEW_DIR=/tmp/reclaim-previews cargo test --locked --bin reclaim pages_render
```

The previews are PPM images rendered from egui meshes. They do not test native window management, authentication prompts, or real-device recovery.

[Desktop CI](.github/workflows/build.yml) runs tests, formatting, Clippy, and release builds on Linux, Windows, and macOS. It also packages a universal Intel/Apple Silicon macOS app with ad-hoc signing. Developer ID signing and notarization are not configured. Windows/macOS cross-target checks have passed locally; hardware and native desktop testing remain necessary.

## License and credits

Reclaim is licensed under [MIT](LICENSE).

- [eframe/egui](https://github.com/emilk/egui) provides the interface; [rfd](https://github.com/PolyMeilex/rfd) provides native file dialogs.
- The bundled DejaVu font includes its [license](assets/fonts/DejaVu-LICENSE.txt).
- Font Awesome Free 6.7.2 Solid supplies the icons. Its fonts are licensed under SIL OFL 1.1; the [upstream license](assets/fonts/FontAwesome-LICENSE.txt) is bundled with the app.
- Translation notes are in [locales/README.md](locales/README.md).
