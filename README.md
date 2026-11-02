# Reclaim

**Find lost files. Review what’s found. Save what matters.**

Reclaim helps you look for recoverable photos, documents, and other files on a drive or a saved copy of a drive. A simple interface guides you from choosing a drive to saving the files you want to keep.

**Version 0.1.8 · Early preview · English & Hebrew**

![Reclaim 0.1.0 — choose a drive to scan](docs/screenshots/reclaim-drives.png)

*Current interface, shown with example drives. Dark mode and English are selected.*

## What you can do

- **Scan unused space** — look for recoverable files in clusters marked free on NTFS, FAT12/16/32, and exFAT volumes. Existing allocated files are skipped.
- **Watch files appear** — review results while the scan runs, or stop early and work with what it has found.
- **Understand each result** — recognize file types by their icons and open file details for size, recovery checks, and original dates when available.
- **Find the files you want** — search by name or file type, and narrow the list to photos, documents, archives, and more.
- **Recover only your selection** — save the checked files that match your current filters.
- **Work from a volume copy** — open an image of a supported formatted volume instead of scanning the original. Whole-disk images containing partition tables are not yet supported for unused-space scans.
- **Make it comfortable** — choose light, dark, or System mode, switch between English and Hebrew, and turn animations on or off. System mode follows your desktop’s appearance changes without restarting the app.

Reclaim reads the filesystem allocation map first, then searches only its free clusters. The map describes allocation when read; a mounted volume can change during scanning. For repeatable recovery, stop using the source and scan a saved volume image.

Reclaim scans your source in **read-only mode**. When saving recovered files, it gives duplicate names a new number rather than replacing existing files.

## Recover files in four steps

1. **Choose your drive.** Select the drive that held your files. Newly connected or removed drives update automatically within about two seconds. Refresh is also available.
2. **Start scanning.** Choose **Scan Drive**. You may be asked for permission to read the drive.
3. **Review the results.** Use search and file-type filters, then check the files you want to recover.
4. **Save somewhere else.** Choose **Recover files…** and pick a folder on a **different physical drive**. Open the saved files to check them.

> Stop using the affected drive as soon as possible. Saving or installing things on it can overwrite files you are trying to recover.

![Reclaim 0.1.0 — search, filter, and select files to recover](docs/screenshots/reclaim-results.png)

*Example results with file-type icons. Finding a file does not guarantee that it can be recovered successfully.*

## Files Reclaim looks for

| File type | Examples |
| --- | --- |
| Photos and pictures | JPG, PNG, GIF, BMP, WebP, QOI |
| Documents | PDF, Word, Excel, PowerPoint, OpenDocument, EPUB |
| Archives and app packages | ZIP, 7z, RAR5, CAB, TAR, GZIP, XAR, JAR, APK |
| Programs and installers | EXE/DLL, ELF, MSI/MSP/MST, DEB, RPM, DEX, flat XAR-based PKG |
| Audio and video | WAV, AIFF, AU, AVI, Ogg Vorbis, Opus, Theora |
| Databases | SQLite |
| Icons | ICO |

Unused-space results usually receive generated names such as `Image 0001.png`. This mode does not search the allocated NTFS master file table for small resident deleted files or their original names. Unsupported or damaged allocation maps produce an error; Reclaim never silently switches to searching occupied space.

Use the information icon beside a result to inspect its details. **Deletion dates are not recovered**; file dates are never presented as the time of deletion.

## Getting started

Download an installer from [GitHub Releases](https://github.com/sudo-py-dev/reclaim/releases):

- **Windows:** run `reclaim-windows-x86_64-setup.exe`. Inno Setup installs for your user, adds a Start menu shortcut, and provides an uninstaller. A desktop shortcut is optional. The Microsoft C++ runtime is bundled into the executable; no Visual C++ Redistributable installation is required.
- **Debian / Ubuntu:** download `reclaim_X.Y.Z_amd64.deb` and run `sudo apt install ./reclaim_X.Y.Z_amd64.deb`. This installs the app launcher and required libraries. Remove it with `sudo apt remove reclaim`. Ubuntu 22.04 / Debian 12 or newer are recommended.
- **macOS:** open `reclaim-macos-aarch64.pkg` for Apple Silicon or `reclaim-macos-x86_64.pkg` for Intel. The installer places **Reclaim.app** in Applications. These installers are unsigned and the app is ad-hoc signed, not notarized; macOS may require explicit approval in Privacy & Security. Quit Reclaim before upgrading; move the app to Trash to remove it.

Portable ZIPs remain available. Extract one to a writable folder and run Reclaim. On Linux and macOS, you may need `chmod +x reclaim-*` before the first launch. Linux portable builds require glibc 2.35 or newer and X11/Wayland desktop libraries.

Reclaim checks for stable updates on startup. In Settings, **What’s new** shows the release notes. On Linux and Windows, **Update and restart** downloads to a private app-managed cache, verifies the release SHA-256 checksum, installs, and reopens Reclaim automatically. No Save dialog or manual installer launch is required. Scanning and recovery must finish before updating. System-installed Debian copies use PackageKit and may require the normal administrator authentication prompt; the package service must be available. Portable Linux copies update in their writable folder. Installed Windows copies use a verified private helper and the per-user Inno Setup installer in silent mode, preserving the install location and uninstaller metadata. If downloading or installation fails, the error is shown; Windows attempts to reopen the existing app after an installer failure. macOS retains the verified installer download and **Open installer** flow. Checks require access to GitHub and do not send scan data.

You can also [build from source](docs/DEVELOPMENT.md#build-and-run).

The project targets **Linux, Windows, and macOS**. Windows and macOS still need further testing on real devices. Your computer may ask for administrator access or permission to read a drive. Encrypted drives must be unlocked before scanning.

## A few things to know

- **Recovery is not guaranteed.** Files may be incomplete, damaged, or already overwritten. Always check the recovered files.
- **Some files may still exist on the drive.** Reclaim’s results are possible recovery candidates, not a list of confirmed deleted files.
- **Files up to about 64 MB each are supported.** Some file variants are unsupported; the [detailed limits](docs/DEVELOPMENT.md#limits) explain more.
- **Original folders are not restored.** Many recovered files will have new names.
- **Save your results before closing.** Scan results are not kept after you quit the app.

The built-in **Recovery guide** explains the process. Open Settings with the gear button, then select the book icon. Settings also contains the language dropdown, appearance choices, and animation switch.

## For developers

Build instructions, platform permissions, supported formats, tests, and technical limits are in the [developer guide](docs/DEVELOPMENT.md). [Desktop builds](.github/workflows/build.yml) are configured for Linux, Windows, and macOS.

The screenshots above are rendered from the current application with sample data. Instructions for refreshing them are in the [screenshot notes](docs/screenshots/README.md).

## License

Reclaim is free and open source under the [MIT license](LICENSE).

Built with [egui](https://github.com/emilk/egui). Bundled fonts and icons include their [DejaVu](assets/fonts/DejaVu-LICENSE.txt) and [Font Awesome](assets/fonts/FontAwesome-LICENSE.txt) licenses.

## Startup errors and crash reports

Windows uses DirectX 12 rendering, including the Microsoft software adapter when
available, so a VM does not need an OpenGL driver just to open Reclaim. Startup
errors, Rust panics, and unhandled Windows exceptions display a native error
message. Diagnostics are saved to `%LOCALAPPDATA%\Reclaim\logs\reclaim.log`
(with a fallback under `%TEMP%` if the user-data folder cannot be written).
The previous log is retained when a log exceeds 1 MiB at startup. Reports stay
on your computer. A crash guard cannot intercept errors before Windows loads
the executable or every fatal system failure.

## Publishing a release

Set the package version in `Cargo.toml`, refresh `Cargo.lock`, commit, and push a matching `vX.Y.Z` tag. The [release workflow](.github/workflows/release.yml) tests and builds Linux x86_64, Windows x86_64, and separate Apple Silicon/Intel macOS executables. Size-focused release settings enable full LTO, symbol stripping, and a single code generation unit. ZIP downloads include license notices; raw executables are also published for the updater, with SHA-256 sidecars for manual verification.

Windows builds statically link the Microsoft C++ runtime and verify the executable imports before packaging, so clean Windows installations do not need the Visual C++ Redistributable.

The workflow publishes a stable GitHub release with generated release notes after all four builds pass, making the downloads and in-app update available immediately. Rerunning it for the same tag replaces the release assets and publishes any existing draft. Prereleases and drafts are ignored by the updater. Release asset names form the updater contract and must stay consistent with `src/updater.rs`. Forks should change the repository constant there. Every release also includes a compressed Inno Setup installer, an xz-compressed Debian package, and architecture-specific macOS `.pkg` installers. Installer assets receive SHA-256 sidecars too. The macOS packages are unsigned until Developer ID signing and notarization credentials are configured; the workflow currently uses ad-hoc app signing.

To build a native installer locally after compiling the release executable, run `python3 scripts/package-release.py linux --binary target/release/reclaim` (or use `windows` / `macos` with the matching executable and `--arch aarch64` for Apple Silicon). The script requires `dpkg-deb` on Linux, Inno Setup 6 on Windows, or Apple command-line tools on macOS. Output goes to `dist/`.

File signatures and format-specific recovery limits are documented in [the format research report](docs/FILE_FORMAT_RESEARCH.md). New candidates generally retain the 64 MiB limit; executable overlays and installer payloads may be incomplete.
