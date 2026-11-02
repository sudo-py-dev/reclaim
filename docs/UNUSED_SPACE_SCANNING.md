# Unused-space scanning

The desktop uses `scan_unused_reader` by default. It reads allocation metadata
from a supported formatted volume, builds free extents, and applies signatures
only to those extents. It never silently falls back to a full-source scan.

Supported maps:

- **FAT12/16/32:** BPB geometry and cluster-count-derived FAT type, reserved
  entries and table bounds. FAT32 active-table flags are respected. With mirrored
  FATs, a cluster is considered free only if every consulted copy marks it free.
  Reserved areas, FATs and fixed root directories are outside the search region.
- **exFAT:** boot-region checksum and geometry, active FAT/bitmap selection,
  root-directory and bitmap chains. Allocation comes from the bitmap, not the FAT.
  The root and bitmap storage must be marked allocated. Media-failure flags,
  loops, truncated tables and invalid extents fail the scan.
- **NTFS:** boot geometry and MFT record size; sector fixups; MFT record zero's
  nonresident DATA runlist; record six's resident or nonresident volume bitmap.
  Fragmented runlists within a base record are supported, including MFT-relative
  reads across runs. Compressed/sparse/encrypted/split allocation streams requiring
  ATTRIBUTE_LIST extensions are rejected. Metadata clusters must be allocated.

Every free extent has its own bounded reader. Candidate seeks and lookahead cannot
cross an occupied gap. Found offsets map back to original volume offsets for
export. The scanner's buffer shrinks for small extents to avoid allocating 1 MiB
for every isolated free cluster. Aggregate progress counts **unused bytes**, not
the physical offset currently being examined; 100% means all mapped free extents
were searched. A volume with no free clusters finishes without carving data.

The filesystem map is read at scan start, not an operating-system snapshot. Other
programs can change a mounted volume afterward. The existing advice to stop using
the drive, or scan an unchanged volume image, still applies. Sector-aligned source
caching can read adjacent bytes, but signatures are never searched in occupied
clusters. Allocation metadata itself must be read even though it is allocated.

Whole-disk MBR/GPT images, unsupported filesystems (including ext4/APFS), and
unreadable/damaged maps produce an actionable error. Select a supported formatted
volume/partition. Deleted NTFS resident records inside allocated MFT storage are
not searched in this mode. The explicit library `scan_reader` remains available
for full-source scans and is used by existing format tests.

Resource bounds: allocation bitmap at most 64 MiB; at most one million free
extents; bounded directory/runlist walks; existing candidate and resident-data
budgets. Cancellation is checked during mapping as well as signature scanning.

## Sources

- [Microsoft exFAT specification](https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification): volume geometry, boot checksum, allocation bitmap directory entries and cluster allocation rules.
- [Microsoft FAT on-disk structures](https://github.com/Microsoft/Windows-driver-samples/blob/main/filesys/fastfat/fat.h): BPB geometry and FAT12/16/32 interpretation, cross-checked with [libfsfat's format documentation](https://github.com/libyal/libfsfat/blob/main/documentation/File%20Allocation%20Table%20%28FAT%29%20format.asciidoc).
- [Microsoft MFT documentation](https://learn.microsoft.com/en-us/windows/win32/devnotes/master-file-table): MFT DATA and record-six volume bitmap, distinct from the MFT record-allocation BITMAP attribute.
- [libfsntfs format documentation](https://github.com/libyal/libfsntfs/blob/main/documentation/New%20Technologies%20File%20System%20%28NTFS%29.asciidoc): NTFS boot layout, record fixups, nonresident attributes and signed relative data runs.

## Validation

Portable tests cover occupied-file exclusion, exact free-file export offsets,
occupied-gap isolation, FAT disagreement, NTFS resident/nonresident bitmaps and
fragmented MFT streams, exFAT metadata protection, malformed geometry, fail-closed
errors, cancellation, and unused-byte progress. Manual read-only smoke scans also
completed on regular-file images created with `mkfs.fat` (FAT12, FAT16, FAT32),
`mkfs.exfat`, and `mkntfs`. No user drive was formatted or modified.
