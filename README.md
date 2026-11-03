<div align="center">
  <img src="assets/icons/reclaim.png" alt="Reclaim app icon" width="88">
  <h1>Reclaim</h1>
  <p><strong>Find lost files. Review what’s found. Save what matters.</strong></p>
  <p>A desktop app for recovering photos, documents, and other files from unused space on drives and volume images.</p>

[Download Reclaim](https://github.com/sudo-py-dev/reclaim/releases/latest) · [Recovery guide](#recover-in-four-steps) · [Developer docs](docs/DEVELOPMENT.md)

[![Desktop builds](https://github.com/sudo-py-dev/reclaim/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/sudo-py-dev/reclaim/actions/workflows/build.yml)
[![Latest release](https://img.shields.io/github/v/release/sudo-py-dev/reclaim?color=3584e4)](https://github.com/sudo-py-dev/reclaim/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-9141ac)](LICENSE)

Linux · Windows · macOS &nbsp; / &nbsp; 7 languages &nbsp; / &nbsp; Early preview

</div>

![Reclaim’s drive selection screen in dark mode, with example drives](docs/screenshots/reclaim-drives.png)

## Start here

Get an installer from the **[latest release](https://github.com/sudo-py-dev/reclaim/releases/latest)**. Portable ZIP downloads are available too.

| Platform | Download | Install |
| --- | --- | --- |
| Windows · x86_64 | [Windows installer](https://github.com/sudo-py-dev/reclaim/releases/latest/download/reclaim-windows-x86_64-setup.exe) | Run the installer. Installs for your user with a Start menu shortcut and uninstaller. |
| Debian / Ubuntu · x86_64 | `.deb` from the [release downloads](https://github.com/sudo-py-dev/reclaim/releases/latest) | Run `sudo apt install ./reclaim_X.Y.Z_amd64.deb`, replacing `X.Y.Z` with the downloaded version. |
| macOS · Apple Silicon | [Apple Silicon installer](https://github.com/sudo-py-dev/reclaim/releases/latest/download/reclaim-macos-aarch64.pkg) | Open the package to install Reclaim in Applications. |
| macOS · Intel | [Intel installer](https://github.com/sudo-py-dev/reclaim/releases/latest/download/reclaim-macos-x86_64.pkg) | Open the package to install Reclaim in Applications. |

Ubuntu 22.04 / Debian 12 or newer are recommended. Windows includes the Microsoft C++ runtime. macOS packages are unsigned and the app is ad-hoc signed, not notarized; you may need to approve it in **Privacy & Security**. Windows and macOS still need further testing on real devices.

> **Protect the source.** Stop using the affected drive as soon as possible. Install Reclaim and save recovered files on a different physical drive to avoid overwriting data you want to recover.

## Recover in four steps

1. **Choose a drive or volume image.** Connected drives refresh automatically. Unlock encrypted volumes first; reading a drive may require administrator access.
2. **Start scanning.** Results appear as they are found. You can stop early and keep working with the results already collected.
3. **Review and select.** Search by name or file type, use category filters, and open file details to inspect size and recovery checks.
4. **Save somewhere else.** Choose **Recover files…** and a folder on another physical drive. Only checked files matching the current filters are saved. Open them to check their contents.

![Reclaim’s results screen with search, category filters, and selected files](docs/screenshots/reclaim-results.png)

*Screenshots are rendered from the app with sample data. They illustrate the interface, not recovery success.*

## Built for a careful recovery

**Read-only scanning.** Reclaim reads the allocation map for NTFS, FAT12/16/32, or exFAT, then searches free clusters. Allocated files are skipped. Unsupported or damaged maps produce an error rather than a scan of occupied space. Recovered files receive numbered names when needed to avoid replacing existing output files.

**Results you can inspect.** File-type icons, search, category filters, image previews for supported formats, and a details dialog help you review candidates before saving. A match is a possible recovery candidate; it does not prove that a file was deleted or is complete.

**A comfortable desktop interface.** Choose light, dark, or System appearance, choose English, Hebrew, Spanish, French, German, Arabic, or Russian, with right-to-left layouts for Hebrew and Arabic, and turn animations on or off. System appearance follows desktop changes while the app runs. A built-in recovery guide is available from the book icon in Settings.

### What it looks for

| Category | Formats and examples |
| --- | --- |
| Pictures and icons | JPEG, PNG, GIF, BMP, WebP, QOI, ICO |
| Documents and ebooks | PDF, Word, Excel, PowerPoint, OpenDocument, EPUB |
| Archives and app packages | ZIP, 7z, RAR5, CAB, TAR, GZIP, XAR, JAR, APK |
| Programs and installers | EXE/DLL, ELF, MSI/MSP/MST, DEB, RPM, DEX, flat XAR-based PKG |
| Audio and video | WAV, AIFF, AU, AVI, Ogg Vorbis, Opus, Theora |
| Databases | SQLite |

See the [format coverage report](docs/FILE_FORMAT_RESEARCH.md) for validation checks and unsupported variants.

### Know the limits

Reclaim is an **early preview**, and recovery is not guaranteed. Overwritten or TRIM-erased data cannot be reconstructed by the scanner. Files may be incomplete or damaged even when they appear in the results.

- **Use a formatted volume.** Whole-disk images containing MBR/GPT partition tables and APFS/ext4 allocation maps are not supported. For repeatable work, scan a saved volume image; a mounted source can change during scanning.
- **Expect generated filenames.** Original folders are not restored. The desktop unused-space scan does not search allocated NTFS MFT clusters for small resident deleted files or their original names.
- **Most files are limited to 64 MiB.** Some classic ZIP archives can be larger. Fragmented-file reconstruction and some format variants are unsupported; see the [detailed limits](docs/DEVELOPMENT.md#limits).
- **Deletion dates are unavailable.** File dates, when available, are never presented as deletion dates.
- **Save before closing.** Scan results are not kept after you quit.

## Updates and troubleshooting

Reclaim checks for stable updates on startup. Open **Settings → What’s new** for release notes. On Linux and Windows, **Update and restart** verifies the download’s SHA-256 checksum, installs the update, and reopens the app. Finish scanning and recovery first. Update checks contact GitHub and do not send scan data.

<details>
<summary>Installation, updates, and drive access</summary>

- **Portable builds:** extract the ZIP to a writable folder. On Linux and macOS, the executable may need `chmod +x reclaim-*`. Linux portable builds require glibc 2.35 or newer and X11/Wayland desktop libraries.
- **Linux updates:** installed Debian packages use PackageKit and may show the normal administrator authentication prompt. The package service must be available. Portable copies update in their writable folder.
- **Windows updates:** the verified per-user installer closes Reclaim, preserves the install location and uninstaller metadata, then starts the installed version.
- **macOS updates:** download verification is followed by **Open installer**. Quit Reclaim before upgrading.
- **Uninstall:** use the Windows uninstaller, run `sudo apt remove reclaim` on Debian/Ubuntu, or move Reclaim.app to Trash on macOS. The Windows desktop shortcut is optional.
- **Drive access:** Linux can request read-only access through UDisks2 and polkit. Windows raw drives may require **Run as administrator**. macOS may require Full Disk Access and administrator privileges. Unlock encrypted volumes before scanning.

See [platform permissions](docs/DEVELOPMENT.md#drive-permissions) for details.

</details>

<details>
<summary>Windows startup errors and crash reports</summary>

Windows uses DirectX 12, including the Microsoft software adapter when available, so a VM does not need an OpenGL driver just to open Reclaim. Startup errors, Rust panics, and unhandled Windows exceptions display a native error message.

Diagnostics stay on your computer at `%LOCALAPPDATA%\Reclaim\logs\reclaim.log`, with a fallback under `%TEMP%` if that location cannot be written. The previous log is retained when a log exceeds 1 MiB at startup. The crash guard cannot catch failures before Windows loads the executable or every fatal system failure.

</details>

## Build and contribute

Reclaim is written in Rust with [egui](https://github.com/emilk/egui). Install stable Rust and your platform’s native build tools. Linux also needs the [GUI development libraries](docs/DEVELOPMENT.md#build-and-run).

```sh
git clone https://github.com/sudo-py-dev/reclaim.git
cd reclaim
cargo run --release --locked
```

The [developer guide](docs/DEVELOPMENT.md) covers tests, native permissions, architecture details, and recovery limits. For focused reference, see the [code and UI audit](docs/CODE_UI_AUDIT.md), [format research](docs/FILE_FORMAT_RESEARCH.md), [translation notes](locales/README.md), and [screenshot instructions](docs/screenshots/README.md).

<details>
<summary>Publishing a release</summary>

Set the package version in `Cargo.toml`, refresh `Cargo.lock`, commit, and push a matching `vX.Y.Z` tag. The [release workflow](.github/workflows/release.yml) tests and builds Linux x86_64, Windows x86_64, and separate Apple Silicon/Intel macOS executables. Size-focused release settings enable full LTO, symbol stripping, and a single code generation unit. ZIP downloads include license notices; raw executables are also published for the updater, with SHA-256 sidecars for manual verification.

Windows builds statically link the Microsoft C++ runtime and verify the executable imports before packaging, so clean Windows installations do not need the Visual C++ Redistributable.

The workflow publishes a stable GitHub release with generated release notes after all four builds pass, making the downloads and in-app update available immediately. Rerunning it for the same tag replaces the release assets and publishes any existing draft. Prereleases and drafts are ignored by the updater. Release asset names form the updater contract and must stay consistent with `src/updater.rs`. Forks should change the repository constant there. Every release also includes a compressed Inno Setup installer, an xz-compressed Debian package, and architecture-specific macOS `.pkg` installers. Installer assets receive SHA-256 sidecars too. The macOS packages are unsigned until Developer ID signing and notarization credentials are configured; the workflow currently uses ad-hoc app signing.

To build a native installer locally after compiling the release executable, run `python3 scripts/package-release.py linux --binary target/release/reclaim` (or use `windows` / `macos` with the matching executable and `--arch aarch64` for Apple Silicon). The script requires `dpkg-deb` on Linux, Inno Setup 6 on Windows, or Apple command-line tools on macOS. Output goes to `dist/`.

File signatures and format-specific recovery limits are documented in [the format research report](docs/FILE_FORMAT_RESEARCH.md). New candidates generally retain the 64 MiB limit; executable overlays and installer payloads may be incomplete.

</details>

## License

[MIT](LICENSE). Bundled fonts and icons include their [DejaVu](assets/fonts/DejaVu-LICENSE.txt) and [Font Awesome](assets/fonts/FontAwesome-LICENSE.txt) license notices.
