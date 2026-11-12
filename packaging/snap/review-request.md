# Storage-interface declaration request for Reclaim

Snap name: `reclaim`
Publisher: `eliezerdayan`
Source: https://github.com/sudo-py-dev/reclaim
License: MIT
Confinement: strict

Please allow the `block-devices` plug with `allow-partitions: true` for this
read-only file-recovery application. The app also declares `udisks2`,
`hardware-observe`, `mount-observe`, and `removable-media` for user-approved
storage access. Automatic connection is not requested.

Reclaim scans NTFS, FAT12/16/32, and exFAT volumes for recoverable files. It
requires partition access so it can inspect allocation metadata and scan free
clusters. Whole disks without a supported volume layout are not silently scanned
as if they were formatted volumes.

Source files are opened with `std::fs::File::open`, which requests read access.
When ordinary Linux device access is denied, the app calls UDisks2
`org.freedesktop.UDisks2.Block.OpenDevice` with mode `r`, allowing the host's
normal polkit authentication prompt. The app does not request a writable source
handle or perform mounts, formatting, partition changes, or device writes.

Recovered files are written only to an output folder selected by the user.
The Linux storage checks block recovery to the same physical source drive when
its backing devices can be identified. Source data is never overwritten.

The home interface supports ordinary disk images. Removable-media access supports
images and recovery destinations on removable drives. Hardware-observe and
mount-observe support the disk inventory and source/destination safety checks.
The Snap Store manages application updates; the native package updater is
disabled inside the snap.

Relevant code:

- Read-only source opening and UDisks2 authentication: `src/drives.rs`
- Linux device discovery and destination checks: `src/drives/linux.rs`
- Source reader: `src/source.rs`
- Unused-space allocation maps: `src/allocation.rs`
- Packaging: `snap/snapcraft.yaml`

Packaging and preparation were assisted by OpenAI Codex. Native Snap build,
startup-test, upload, and review results should be linked here when available.
