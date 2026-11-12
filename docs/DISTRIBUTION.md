# Distribution packaging

## WinGet

The initial package is `sudo-py-dev.Reclaim` version `0.2.3`, submitted for review at
https://github.com/microsoft/winget-pkgs/pull/478554. Publication depends on Microsoft's review and checks; opening the PR does not make the package available immediately.

The submission files are in `packaging/winget/manifests/s/sudo-py-dev/Reclaim/0.2.3/`.
The installer is the existing upstream per-user x64 Inno Setup release. Its SHA-256
was independently downloaded and verified, and the manifests pass Microsoft's
1.28.0 JSON schemas. Windows installation testing must be completed by WinGet CI
or on a Windows machine. The account owner must personally handle any required
Contributor License Agreement acceptance.

Once accepted:

```powershell
winget install --id sudo-py-dev.Reclaim --exact
```

For subsequent releases, create a new version directory, update every version and
the version-specific download URL, hash the actual installer, and submit an update
PR to `microsoft/winget-pkgs`. Do not point installer URLs at `latest`.

## Standalone Flatpak

The Flatpak application ID is `io.github.sudo_py_dev.reclaim`, corresponding to
`sudo-py-dev/reclaim` on GitHub. The existing native package IDs are preserved.
The Flatpak edition supports recovery from raw disk images through the desktop
file chooser. Direct physical-drive access requires the native package. No home,
host filesystem, raw device, network, system bus, or host process execution
permissions are granted. Sources and recovery folders are selected through the
desktop portals. Reclaim disables its installer-based updater inside Flatpak.

Prerequisites: Rust via rustup, Python 3.11+, Flatpak, the Freedesktop Platform and SDK 25.08,
and the Rust dependencies in the Cargo cache.

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Sdk//25.08 org.freedesktop.Platform//25.08
cargo fetch --locked --target x86_64-unknown-linux-gnu
packaging/flatpak/build.sh
flatpak install --user ./dist/reclaim-linux-x86_64.flatpak
flatpak run io.github.sudo_py_dev.reclaim
```

The script compiles the source inside the SDK, offline, using the installed Rust
toolchain. This avoids linking against a newer host glibc than the Flatpak runtime.
It exports a local OSTree repository under `dist/flatpak-repo` and a standalone
bundle under `dist/`. Dependencies may be fetched before the build; the packaged
application has no network permission.

The release workflow builds the bundle, adds checksums and release signatures,
and uploads it alongside native installers for future version tags. Update a
standalone installation by installing the newer downloaded bundle. The local
OSTree build repository is unsigned; hosted repository distribution should add
GPG signing before advertising it as an update source.

## Flathub submission boundary

This repository's standalone Flatpak packaging was prepared with OpenAI Codex.
It is not a Flathub submission and must not be represented as human-authored.

Flathub's current [generative AI policy](https://docs.flathub.org/docs/for-app-authors/requirements#generative-ai-policy)
prohibits AI-authored or AI-assisted Flathub manifests and AI agents creating
submission PRs, commit messages, descriptions, or review replies. A human
maintainer must independently author compliant packaging and submit it. Disclose
known AI-generated application code and documentation, including the update-screen
and standalone Flatpak work from this session, with an honest assessment of extent.

A future Flathub submission also needs an assessment of eligibility as a recovery
utility, an appropriate sandbox-supported scope, a source build with offline
Cargo dependencies, and the standard build, metadata, permission, and runtime
checks. Do not submit this standalone build script as a Flathub manifest.
