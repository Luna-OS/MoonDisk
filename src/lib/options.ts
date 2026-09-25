import type { FileSystem, Platform } from "@/types/models";

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
// Windows and macOS can only format these without extra software.
const WINDOWS_FS_OPTIONS: FileSystem[] = ["ntfs", "exFat", "fat32"];
const MACOS_FS_OPTIONS: FileSystem[] = ["apfs", "exFat", "fat32", "hfsPlus"];

const FS_OPTIONS: Record<Platform, FileSystem[]> = {
  linux: LINUX_FS_OPTIONS,
  windows: WINDOWS_FS_OPTIONS,
  macos: MACOS_FS_OPTIONS,
};

export function fsOptions(platform: Platform): FileSystem[] {
  return FS_OPTIONS[platform];
}

// C..Z (24 letters) — A/B are reserved for legacy floppy drives.
export const DRIVE_LETTERS = Array.from({ length: 24 }, (_, i) => String.fromCharCode(67 + i));
