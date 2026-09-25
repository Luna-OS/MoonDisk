import type { Disk, FileSystem, PartitionKind, Segment } from "@/types/models";
import { bytesValue } from "@/lib/format";

export function segmentId(seg: Segment, index: number): string {
  return seg.kind === "partition" ? seg.value.id : `free-${index}`;
}

export function segmentLabel(seg: Segment): string {
  if (seg.kind === "unallocated") return "Unallocated";
  return seg.value.label || `Partition ${seg.value.number}`;
}

const FS_LABELS: Record<FileSystem, string> = {
  ntfs: "NTFS",
  fat32: "FAT32",
  exFat: "exFAT",
  ext2: "ext2",
  ext3: "ext3",
  ext4: "ext4",
  btrfs: "Btrfs",
  xfs: "XFS",
  linuxSwap: "Swap",
  apfs: "APFS",
  hfsPlus: "Mac OS Extended",
  unformatted: "Unformatted",
  unknown: "Unknown",
};

export function fsLabel(fs: FileSystem): string {
  return FS_LABELS[fs];
}

const KIND_LABELS: Partial<Record<PartitionKind, string>> = {
  efi: "EFI",
  microsoftReserved: "MSR",
  recovery: "Recovery",
};

export function kindLabel(kind: PartitionKind): string | null {
  return KIND_LABELS[kind] ?? null;
}

// Special partition kinds get a fixed color regardless of filesystem, so
// e.g. a Recovery (NTFS) partition never looks the same as the main
// Windows (NTFS) partition sitting right next to it.
const KIND_COLORS: Partial<Record<PartitionKind, string>> = {
  efi: "bg-sky-300",
  microsoftReserved: "bg-slate-400",
  recovery: "bg-amber-300",
};

// Everything else is colored by filesystem, so partitions with different
// filesystems on the same disk are distinguishable at a glance.
const FS_COLORS: Partial<Record<FileSystem, string>> = {
  ntfs: "bg-blue-400",
  fat32: "bg-teal-300",
  exFat: "bg-cyan-300",
  ext2: "bg-lime-300",
  ext3: "bg-green-300",
  ext4: "bg-emerald-300",
  btrfs: "bg-orange-300",
  xfs: "bg-fuchsia-300",
  linuxSwap: "bg-rose-400",
  apfs: "bg-violet-300",
  hfsPlus: "bg-indigo-300",
};

export function segmentColorClass(seg: Segment): string {
  if (seg.kind === "unallocated") return "md-free";
  const p = seg.value;
  return KIND_COLORS[p.kind] ?? FS_COLORS[p.fs] ?? "bg-zinc-400";
}

export function diskUsage(disk: Disk): { allocated: bigint; free: bigint; fraction: number } {
  let allocated = 0n;
  let free = 0n;
  for (const seg of disk.layout) {
    if (seg.kind === "partition") allocated += bytesValue(seg.value.size);
    else free += bytesValue(seg.value.size);
  }
  const total = bytesValue(disk.size);
  const fraction = total > 0n ? Number((allocated * 1000n) / total) / 1000 : 0;
  return { allocated, free, fraction };
}

export function diskModel(disk: Disk): string {
  return disk.model || disk.vendor || "Unknown model";
}
