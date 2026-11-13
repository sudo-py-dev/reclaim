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
- **ext2/ext3/ext4:** superblock geometry and feature flags, the group descriptor
  table (32-byte or 64-bit descriptors, flex_bg placement) and each group's block
  bitmap. Groups flagged BLOCK_UNINIT under group checksums never held data and
  are skipped. Compression, external journal devices, META_BG and bigalloc are
  rejected. The superblock, descriptor table and bitmap blocks must be allocated.
  Descriptor checksums are not verified. A mounted journaled volume may have
  allocations still in its journal, so its bitmap can lag by a few blocks.
- **HFS+ and HFSX:** volume header geometry and the allocation file (one bit per
  block, most significant bit first) from the eight extents in the header. An
  allocation file that needs the extents overflow file is rejected. The volume
  header, its backup and the allocation file must be allocated. HFS wrapped in a
  classic HFS volume is not recognized.
- **ReFS:** recognized by its boot sector and refused with its own message. Its
  free-space structures are not publicly documented, so no map is guessed.

## Deleted files from filesystem records

Before carving, the map step also reads the records a filesystem keeps for
deleted files, and reports those files first:

- **NTFS:** deleted MFT records with an unnamed, uncompressed, fully
  initialized `$DATA` stream. The record's cluster runs give the exact size and
  every fragment, so fragmented files come back whole, with their original name
  when the record still has one. Linux's `ntfs3` driver writes records in the
  older NTFS 3.0 layout (update array at 42) and removes the name on delete;
  those records are accepted and their type is taken from the file's first
  bytes.
- **FAT12/16/32:** deleted directory entries, with the long name when its
  checksum matches the short name (the short name's lost first letter is
  restored from the long name). The FAT chain of a deleted file is cleared, so
  the data is assumed to be stored in one piece.
- **exFAT:** deleted entry sets. Files flagged as stored in one piece come back
  exactly; others follow their FAT chain when it is intact and unused.

A record is used only when every cluster it names is still free, so its data
cannot belong to a newer file. Its space is then removed from carving only if
the file's first bytes match its type (MP4, MOV, HEIC and Matroska headers are
recognised here too); otherwise the file is listed as unverified and its space
is still carved. Walks are bounded: 32 directory levels, 20,000 directories,
128 MiB of directory data and 50,000 files per volume.

- **ext3/ext4 journal:** Linux erases a deleted file's block list and name.
  The journal often still holds older copies of the inode-table and directory
  blocks from when the file existed. The newest copy of each inode that is free
  now, with an extent tree (ext4) or block map (ext3) whose blocks are all
  still unused, gives the file's exact size and every piece; names come from
  journaled directory blocks. Block maps with holes (sparse files) are skipped.

- **ext2/ext3 block maps:** past its first 12 blocks, an ext2 or ext3 file
  lists its blocks in separate indirect blocks. Linux clears the list in the
  inode on delete, but the freed indirect blocks keep their contents. Free
  space is read once for blocks that hold a list of free block numbers; the 12
  free blocks before the first listed block are taken as the file's start when
  they sit together and open with a known file signature, and a second map
  level (double indirect) continues files past the first map. Nothing is
  listed from a map directly: carving reads each such file's pieces in order
  and still checks the result, so a wrong guess only changes the order of
  unrelated free space. Where two maps claim the same space, the longer one is
  used, since the kernel can leave a shorter, older copy. Bounded at 200,000
  maps and two million block runs; this pass is half of the scan's progress.

**ext2 limit:** when a file's first 12 blocks were themselves split (a nearly
full drive), their locations are erased with the inode and the file is only
found if carving joins the pieces. Files above 4 GiB (triple indirect) are not
rebuilt from maps.

**Carving across used space.** All free ranges are carved as one stream in disk
order, leaving out the space still in use. When a drive fills its gaps in
order (as FAT32 does), a fragmented file continues in the next free range and
comes back whole; its pieces are recorded so recovery reads the same bytes.
Formats with an internal check (image decode, checksums, ZIP directory) reject
a wrong join. Occupied space is never read for carving. After a read error,
the skipped area never reaches past the free range it started in.

Found offsets map back to original volume offsets for export. Aggregate
progress counts **unused bytes**; 100% means all mapped free space was searched.
A volume with no free clusters finishes without carving data.

The filesystem map is read at scan start, not an operating-system snapshot. Other
programs can change a mounted volume afterward. The existing advice to stop using
the drive, or scan an unchanged volume image, still applies. Sector-aligned source
caching can read adjacent bytes, but signatures are never searched in occupied
clusters. Allocation metadata itself must be read even though it is allocated.

Whole-disk MBR/GPT images, unsupported filesystems (including APFS, Btrfs, XFS and ReFS), and
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
- [Linux kernel ext4 disk layout](https://docs.kernel.org/filesystems/ext4/globals.html): superblock fields, feature flags, group descriptors and block-group flags.
- [Apple Technical Note TN1150](https://developer.apple.com/library/archive/technotes/tn/tn1150.html): HFS+ volume header, fork data and allocation file bit order.

## Validation

Unit tests, kernel-written fixtures, images made with real tools and a real
USB drive with every supported filesystem are described, with results, in
[TESTING.md](TESTING.md#real-usb-drive).
