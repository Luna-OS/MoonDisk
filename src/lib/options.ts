import type { FileSystem } from "@/types/models";

const LINUX_FS_OPTIONS: FileSystem[] = [
  "ext4",
  "btrfs",
  "xfs",
  "ext3",
  "ext2",
  "ntfs",
  "exFat",
  "fat32",
];
// Windows can only format these without extra software.
const WINDOWS_FS_OPTIONS: FileSystem[] = ["ntfs", "exFat", "fat32"];

export function fsOptions(windows: boolean): FileSystem[] {
  return windows ? WINDOWS_FS_OPTIONS : LINUX_FS_OPTIONS;
}

// C..Z (24 letters) — A/B are reserved for legacy floppy drives.
export const DRIVE_LETTERS = Array.from({ length: 24 }, (_, i) => String.fromCharCode(67 + i));
