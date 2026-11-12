#!/usr/bin/env bash
# Standalone Flatpak bundle; this is not a Flathub submission manifest.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
app_id=io.github.sudo_py_dev.reclaim
runtime_version=25.08
mkdir -p "$root/dist"
work="$(mktemp -d "$root/dist/flatpak-build.XXXXXX")"
trap 'rm -rf -- "$work"' EXIT
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
toolchain="$(rustc --print sysroot)"
arch="$(flatpak --default-arch)"
# Compile inside the SDK so the executable targets the runtime's glibc,
# rather than the potentially newer host glibc. Cargo dependencies must already
# be cached (cargo fetch --locked); the build itself has no network access.
flatpak build-init "$work/app" "$app_id" org.freedesktop.Sdk org.freedesktop.Platform "$runtime_version"
flatpak build --unshare=network \
    --filesystem="$root" --filesystem="$cargo_home" --filesystem="$toolchain:ro" \
    --env=PATH="$toolchain/bin:/usr/bin:/bin" --env=CARGO_HOME="$cargo_home" \
    --env=CARGO_TARGET_DIR="$root/target/flatpak" \
    --build-dir="$root" "$work/app" \
    "$toolchain/bin/cargo" build --offline --locked --release --bin reclaim
install -Dm755 "$root/target/flatpak/release/reclaim" "$work/app/files/bin/reclaim"
install -Dm644 "$root/packaging/flatpak/$app_id.desktop" "$work/app/files/share/applications/$app_id.desktop"
install -Dm644 "$root/packaging/flatpak/$app_id.metainfo.xml" "$work/app/files/share/metainfo/$app_id.metainfo.xml"
python3 - "$root/Cargo.toml" "$work/app/files/share/metainfo/$app_id.metainfo.xml" <<'PY'
import datetime
import pathlib
import sys
import tomllib
import xml.etree.ElementTree as ET

version = tomllib.loads(pathlib.Path(sys.argv[1]).read_text())["package"]["version"]
metadata = ET.parse(sys.argv[2])
release = metadata.find("releases/release")
if release is None:
    raise SystemExit("AppStream metadata is missing its release entry")
release.set("version", version)
release.set("date", datetime.date.today().isoformat())
metadata.write(sys.argv[2], encoding="utf-8", xml_declaration=True)
PY
install -Dm644 "$root/assets/icons/reclaim.png" "$work/app/files/share/icons/hicolor/256x256/apps/$app_id.png"
install -Dm644 "$root/LICENSE" "$work/app/files/share/licenses/$app_id/LICENSE"
install -Dm644 "$root/assets/fonts/DejaVu-LICENSE.txt" "$work/app/files/share/licenses/$app_id/DejaVu-LICENSE.txt"
# File chooser and output-folder access use xdg-desktop-portal. No host home,
# raw devices, network, host process spawning, or system bus access is granted.
flatpak build-finish --command=reclaim --share=ipc --socket=wayland \
    --socket=fallback-x11 "$work/app"
flatpak build-export "$root/dist/flatpak-repo" "$work/app" stable
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    "$root/dist/flatpak-repo" "$root/dist/reclaim-linux-$arch.flatpak" "$app_id" stable
printf 'Created %s\n' "$root/dist/reclaim-linux-$arch.flatpak"
