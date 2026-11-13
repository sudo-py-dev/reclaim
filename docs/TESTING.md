# Testing

Every check Reclaim has, how to run it, and what each one proves. The last
part records the tests on a real USB drive and their results.

## Before every change

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --target x86_64-pc-windows-gnu --all-targets -- -D warnings
```

`cargo test` runs every suite below, including the project audit and the
headless UI renders. On 2026-11-12 it ran 183 tests with 4 opt-in tests
ignored; all passed on Linux, and both Clippy runs passed. CI runs the same
tests and Clippy on Linux, Windows and macOS (see [CI](#ci)).

## Automated tests

| Suite | What it checks |
| --- | --- |
| `src/` unit tests (library) | Signature boundaries, image and archive validation, NTFS records, sector-aligned and resilient reads, unreadable-sector skipping, allocation maps for FAT12/16/32, exFAT, NTFS, ext2/3/4 and HFS+/HFSX, BLOCK_UNINIT, unknown ext features, ReFS refusal, ext2 block-map parsing against hostile data, cancellation and progress |
| `src/` unit tests (app) | Drive discovery parsing on Linux, Windows and macOS, drive hot-plug refresh, recovery and export rules, update metadata and signatures, translations, native process hardening, diagnostics |
| `src/slint_ui_tests.rs` | Renders every page headlessly and runs the layout checks below; keyboard controls, modal dialogs, filtering, export and window commands |
| `tests/security.rs` | USB disconnect after a partial read, competing exports with the same name, invalid candidate ranges, existing files and symbolic links. A failed export must leave no partial or staging file; a successful one must not touch existing data |
| `tests/image_detection.rs`, `container_detection.rs`, `new_formats.rs`, `expanded_formats.rs` | Each supported file type is found, sized and named correctly, and damaged copies are rejected |
| `tests/large_zip.rs`, `zip_scan_work.rs` | Large archives are classified with bounded work |
| `tests/ntfs_resident.rs` | Small files stored inside NTFS records |
| `tests/ext_journal.rs` | A split, deleted file comes back whole and with its name from the ext3 and ext4 journal (kernel-written fixtures) |
| `tests/ext_block_maps.rs` | A split, deleted 322 KB file comes back whole on ext2 from the freed block maps, including the second map level (kernel-written fixture) |
| `tests/audit` | The project audit (next section) |

The fixtures are described in [tests/fixtures/README.md](../tests/fixtures/README.md).

## Project audit

`tests/audit` parses the source and fails the build on any of these, each rule
tagged by CWE:

- `unsafe` outside the listed platform files or without a `SAFETY` comment; no
  `static mut`, `transmute`, leaks or unreviewed uninitialised memory.
- Programs started by anything other than the two native launchers.
- Plain HTTP, TLS bypasses, weak hashes, committed secrets, non-private
  temporary files.
- Reads and allocations sized by scanned data without an explicit bound, and a
  ratchet (`tests/audit/baseline.toml`) on unchecked indexing.
- Debug leftovers, silenced warnings, orphan files, em dashes in user text.
- An interface message missing from a catalog, dead catalog keys, mismatched
  placeholders, or `ui/app.slint` defaults out of step with their translations.
- A test fixture, such as a disk image, holding the local user path of whoever
  made it (`/home/<user>`, `/run/media/<user>`, `C:\Users\<user>`), also
  inside gzip.
- An informal "you" in the German, French or Russian catalog (all three address
  the user formally), or a Latin term ending in `+` or `#` ("HFS+") in Hebrew
  or Arabic without the left-to-right mark that keeps the symbol in place.
- Inexact dependency versions, stale advisory exceptions, or a fuzz lockfile
  that differs from production.

A finding can be accepted only next to its code with
`// audit-allow(RULE): reason`; an acceptance that no longer matches fails too.
Run it alone with `cargo test --locked --test audit`. `Cargo.toml` adds the
compiler half: no `unwrap`, `expect`, `panic`, stray prints or undocumented
`unsafe` in shipped code. CI also runs `cargo-deny` against the RustSec
advisory database.

## Interface

`layout_is_clear_and_steady_on_every_page_language_and_size` checks every page,
with no source, during a scan with long file names, and with the update notice
open, in English, Hebrew, Arabic and German, at 900x640 and 1160x780. Every
button, checkbox, dropdown and text field must lie inside the window and must
not overlap another control; controls in a scroll view are judged by their
visible part. The "Select all" caption and the found and selected counts never
elide; with counts of 250,000 both must still lie inside the list column. The scan headline must keep its size and position whether it says
"scanning" or names a slow check. A streaming test adds 2,000 results and checks
the list always equals a full rebuild.

Headless previews (PNG files from the real Slint components, software
rendered) cover all three pages in all seven languages, light and dark, and
English, Hebrew and Arabic at the compact size:

```sh
mkdir -p /tmp/reclaim-previews
RECLAIM_PREVIEW_DIR=/tmp/reclaim-previews cargo test --locked --bin reclaim slint_workspace
```

The screenshots in `docs/screenshots/` come from these renders. They do not
test the real compositor, native dialogs, screen readers, authentication
prompts or Windows and macOS hardware.

## Fuzzing

Three cargo-fuzz targets cover the scanner, the source reader and the
unused-space maps. Setup and commands are in [fuzz/README.md](../fuzz/README.md).

## Opt-in hardware tests

Two Linux tests only read the selected device or check that exporting to the
same drive is refused. They never format, delete or write test files. Check the
device name first:

```sh
RECLAIM_TEST_DEVICE=/dev/sdX1 cargo test --locked --bin reclaim live_named_volume_can_read_data -- --ignored --nocapture
RECLAIM_TEST_DEVICE=/dev/sdX1 RECLAIM_TEST_DESTINATION=/media/user/USB cargo test --locked --bin reclaim live_usb_export_to_source_drive_is_blocked_before_any_write -- --ignored
```

The read test scans only the first 16 MiB and needs read permission or the
desktop's read-only UDisks prompt. Do not unplug a drive during a real export
or test on a failing drive; disconnects are covered by the simulated tests.

## Filesystem images made with real tools

Read-only scans were run on images made with `mkfs.fat` (FAT12, FAT16, FAT32),
`mkfs.exfat` and `mkntfs`, and on `mkfs.ext2` (1 KiB blocks), `mkfs.ext3`
(2 KiB) and `mkfs.ext4` (4 KiB, 64bit, flex_bg, metadata_csum) with a file
written and deleted through `debugfs`. The deleted file was found and the kept
file was not, and free bytes matched `dumpe2fs` exactly on ext2 and ext3. HFS+
was only tested with images built by the unit tests.

## Real USB drive

### Setup

A 57 GB SanDisk USB 3.2 stick, used only for testing with its owner's
permission, was split into one partition per filesystem: FAT12 (4 MB), FAT16
(256 MB), and 1 GB each of FAT32, exFAT, NTFS, ext2, ext3 and ext4. Linux 7.0
formatted and wrote them through udisks (FAT and exFAT by the kernel drivers,
NTFS by `ntfs3`, ext by the ext4 driver).

The test set was 48 real files, 49 MB in total: JPEG, PNG, WebP, PDF, Ogg,
WAV, ZIP and GZIP, from 40 KB to 12 MB. FAT12 got 30 small files (2.7 MB). The
files went into nested folders (`Photos/2024 Trip`, `Documents` and the root)
and were then deleted.

Each partition was copied once, read-only, to an image file, and every scan ran
on the copy. A file counts only if it came back byte for byte (SHA-256 match);
"damaged" means the start matches but not the whole file.

### Rounds

1. **Empty drive:** the test files written to a fresh partition and deleted.
2. **Used drive:** 300 other files kept on the drive around the deleted ones.
3. **Full drive:** the partition filled with alternating gap files and kept
   files (192 KiB and 256 KiB; 48 KiB and 16 KiB on FAT12), the rest filled,
   then the gaps deleted. The test files were written into the gaps, so
   anything larger than a gap was split, and then deleted. This is the hardest
   case: the drive has to place files in the leftover holes.

### Results

Files recovered whole, out of 48 (FAT12 out of 30):

| Round | FAT12 | FAT16 | FAT32 | exFAT | NTFS | ext2 | ext3 | ext4 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Empty drive | 30 | 48 | 48 | 48 | 48 | 46 | 47 | 48 |
| Used drive | not run | 48 | 48 | 48 | 48 | 46 | 47 | 47 |
| Full drive | 30 | 48 | 47 | 48 | 48 | 35 | 47 | 47 |

FAT12 also returned every original file name. The used round was not run on
FAT12 because 4 MB does not hold the test files and 300 others.

### What the rounds changed

The first runs found real gaps, and each was fixed and measured again:

| Filesystem, round | Before | After | Fix |
| --- | --- | --- | --- |
| FAT32, full | 17 | 47 | FAT32 erases a deleted file's chain, but fills gaps in order. Free space is now carved as one stream with used space skipped, so a split file joins up; formats with internal checks reject wrong joins. A bug where skipping a bad sector could also skip the next healthy free area was fixed on the way |
| ext4, full | 30 | 47 | The journal often keeps a copy of a deleted file's inode and directory entry; files are rebuilt from it, with names, only when every block is still free |
| ext3, used | 8 | 47 | Same journal reader |
| ext2, empty and used | 8 | 46 | Freed indirect blocks keep the list of where a file's pieces are; carving now reads each file's pieces in order |

### Remaining misses

- **One WAV, everywhere:** `cp` stored its silent stretches as holes, which
  real camera and phone files do not have. A test artifact.
- **One PDF on FAT32 and ext2:** it has text after its final `%%EOF`, which
  carving cannot see.
- **ext2, full drive (11 more):** when a file's first 12 blocks were split
  themselves, their locations are erased with the file. Hundreds of stale maps
  from the filler files fit the same gaps, so guessing the start would join
  the wrong data; it is left to carving.

### Repeating it

The scoring harness was a temporary ignored test, not kept in the repository.
To repeat: format a stick you can erase, write the test files into each
partition in the three layouts above, copy each partition to an image with the
desktop's read-only authorization, run `scan_unused_reader` on the image, and
compare SHA-256 hashes of what `FoundReader` returns against the originals.

## CI

[Desktop CI](../.github/workflows/build.yml) runs the locked tests (including the
audit and UI) and Clippy with warnings as errors on Linux, Windows and macOS;
formatting once on Linux; `cargo-deny` for advisories, bans and sources; and a
type-check and build of the website. It runs on pull requests, on `main` and
weekly, with actions pinned to commit hashes. The
[release workflow](../.github/workflows/release.yml) runs the same checks before
building installers. Snap packages are built and smoke-tested in their own
workflow, which fails if the window does not appear.

## Not covered

Native window management, file dialogs, screen readers and authentication
prompts are not tested automatically. Windows and macOS hardware were not
tested from this Linux host; CI builds and tests them but does not scan real
drives. HFS+ was not tested on a real drive. Mounted volumes can change during
a scan; scanning an unchanged image is the reliable path.
