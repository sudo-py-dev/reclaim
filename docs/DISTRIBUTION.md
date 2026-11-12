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

## Snap Store

The package name is `reclaim`, registered to the project's Snap Store publisher.
The source recipe is `snap/snapcraft.yaml`; it builds the Rust app in an isolated
Ubuntu 24.04 environment, uses strict confinement and the GNOME desktop runtime,
and packages the MIT application and bundled font licenses.

Build committed source without copying local credentials, caches or disk images:

```sh
packaging/snap/build.sh
```

If a local LXD build cannot obtain network access, use Canonical's remote builders
from a full Git checkout of the committed public source (shallow clones are not
supported). Authorize Snapcraft in Launchpad when prompted. Remote builds publish
the source snapshot to Launchpad. Keep build logs private:

```sh
umask 077
snapcraft remote-build --build-for=amd64 --launchpad-accept-public-upload
```

The recipe installs Ubuntu's Rustup package and invokes Rust 1.99.0 explicitly.
Toolchain files are installed only in the build environment; they are not shipped
in the application snap.

The result is `dist/reclaim_<version>_amd64.snap`. Upload and request release with:

```sh
snapcraft upload dist/reclaim_<version>_amd64.snap --release=stable
snapcraft upload-metadata dist/reclaim_<version>_amd64.snap
snapcraft status reclaim
```

The `block-devices` plug, including partition access, requires a Snap Store
permission declaration and snapd 2.71 or later. Physical drives are opened
read-only; UDisks2 supplies the normal desktop authentication prompt where needed.
Storage permissions are not requested for automatic connection. They must be
approved by store reviewers and explicitly connected by the user:

```sh
sudo snap connect reclaim:block-devices
sudo snap connect reclaim:udisks2
sudo snap connect reclaim:hardware-observe
sudo snap connect reclaim:mount-observe
sudo snap connect reclaim:removable-media
```

The last interface is for disk images or recovery folders on removable media.
Home-folder disk images can be used without raw-device permissions. The Snap
Store manages updates; Reclaim never tries to install a native `.deb` inside the
snap. A registered name or uploaded revision does not mean the package has been
approved or released. Check `snapcraft status reclaim` for the published channels.
