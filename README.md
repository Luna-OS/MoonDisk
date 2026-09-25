# MoonDisk

> Your disks, safely under the moon.

MoonDisk is a modern, open-source partition manager for Windows, macOS and Linux.

## Features

- Overview of disks and partitions (GPT/MBR, NTFS/FAT32/exFAT/ext2-4/Btrfs/XFS/Swap/APFS/HFS+)
- Create partitions with a chosen size, file system, label and (on Windows) drive letter
- Format and delete partitions, change labels and drive letters
- USB writer: write an ISO/IMG file onto a USB stick (e.g. a Linux installer), with
  verification and live progress — and restore such a stick to a normal, empty drive afterwards
- Confirmation dialog for destructive actions (delete/format/write image)

The USB writer has two modes:

- **Copy files** (default, like Rufus' ISO mode): the stick gets one FAT32 partition with the
  image's files. Windows, macOS and Linux can open it, and it boots on UEFI PCs through the
  image's own EFI boot loader. When the image's label is too long for FAT32, the boot
  configuration is patched to the shortened label. Needs an ISO with a UEFI boot loader and no
  file over 4 GB — MoonDisk says so when an image doesn't qualify.
- **Raw image** (like balenaEtcher or Rufus' DD mode): the image byte for byte. Also boots on old
  BIOS-only PCs, but the stick looks empty to Windows afterwards.

Windows installer ISOs aren't supported yet in either mode; use Microsoft's Media Creation Tool
for them.

## Installation

Ready-to-use installers for Windows (`.exe`), macOS (`.dmg`, Apple Silicon and Intel) and Linux
(`.deb`/`.rpm`) are available on the [Releases](https://github.com/Luna-OS/MoonDisk/releases)
page.

On Windows, MoonDisk asks for administrator rights when it starts (UAC prompt) — partition
operations through PowerShell's Storage module require them.

On macOS, MoonDisk is a native SwiftUI app. Open the `.dmg` and drag MoonDisk into
Applications. The app isn't notarized by Apple yet, so the first start is blocked: open
**System Settings → Privacy & Security** and click **Open Anyway** next to the MoonDisk message.
At every launch macOS asks for your password: MoonDisk then runs a small helper with
administrator rights (`moondisk-helper`, the same Rust core as on Windows and Linux), which
erasing, partitioning and writing drives requires. The Mac version hasn't been tested on real
hardware yet, so please report anything that doesn't work. On macOS a new partition can only go
directly after an existing one (a `diskutil` limitation), and NTFS and Linux file systems can be
shown but not created.

**Warning:** MoonDisk modifies real disks. Creating, deleting, formatting and relabeling
partitions can cause permanent data loss — including on system, boot or EFI partitions — if
the wrong target is selected or something goes wrong. Back up your data before every change.

## License

This repository is currently licensed under the [MIT License](LICENSE). A move to
GPL-3.0-or-later is planned but not done yet.
