#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
# Build only committed source. Exclude local caches, credentials and disk images
# by exporting the Git tree rather than copying the working directory.
git -C "$root" diff --quiet HEAD || { printf 'Commit source changes before building the publishing package.\n' >&2; exit 1; }
mkdir -p "$root/dist"
work="$(mktemp -d "$root/dist/snap-source.XXXXXX")"
trap 'rm -rf -- "$work"' EXIT
git -C "$root" archive HEAD | tar -x -C "$work"
cd "$work"
snapcraft pack --use-lxd --output "$root/dist"
