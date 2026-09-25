# MoonDisk

> Your disks, safely under the moon.

MoonDisk is a modern, open-source partition manager for Windows and Linux.

## Features

- Overview of disks and partitions (GPT/MBR, NTFS/FAT32/exFAT/ext2-4/Btrfs/XFS/Swap)
- Create partitions with a chosen size, file system, label and (on Windows) drive letter
- Format and delete partitions, change labels and drive letters
- USB writer: write an ISO/IMG file onto a USB stick (e.g. a Linux installer), with
  verification and live progress
- Confirmation dialog for destructive actions (delete/format/write image)

The USB writer copies the image byte for byte, like balenaEtcher or Rufus' DD mode. That works
for Linux ISOs and `.img` files; Windows installer ISOs need Microsoft's Media Creation Tool
instead — MoonDisk warns when an image won't boot this way.

## Installation

Ready-to-use installers for Windows (`.exe`) and Linux (`.deb`/`.rpm`) are available on the
[Releases](https://github.com/Luna-OS/MoonDisk/releases) page.

On Windows, MoonDisk asks for administrator rights when it starts (UAC prompt) — partition
operations through PowerShell's Storage module require them.

**Warning:** MoonDisk modifies real disks. Creating, deleting, formatting and relabeling
partitions can cause permanent data loss — including on system, boot or EFI partitions — if
the wrong target is selected or something goes wrong. Back up your data before every change.

## License

This repository is currently licensed under the [MIT License](LICENSE). A move to
GPL-3.0-or-later is planned but not done yet.
