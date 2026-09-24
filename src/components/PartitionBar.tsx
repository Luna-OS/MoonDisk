import type { Disk, FileSystem, PartitionKind, Segment } from "@/types/models";
import { bytesValue, formatBytes } from "@/lib/format";

// Special partition kinds get a fixed color regardless of filesystem, so
// e.g. a Recovery (NTFS) partition never looks the same as the main
// Windows (NTFS) partition sitting right next to it.
const KIND_COLORS: Partial<Record<PartitionKind, string>> = {
  efi: "bg-sky-300",
  microsoftReserved: "bg-slate-500",
  recovery: "bg-amber-400",
};

// Everything else is colored by filesystem, so partitions with different
// filesystems on the same disk are distinguishable at a glance instead of
// all blending into one or two colors.
const FS_COLORS: Partial<Record<FileSystem, string>> = {
  ntfs: "bg-blue-400",
  fat32: "bg-teal-400",
  exFat: "bg-cyan-400",
  ext2: "bg-lime-400",
  ext3: "bg-green-400",
  ext4: "bg-emerald-400",
  btrfs: "bg-orange-400",
  xfs: "bg-fuchsia-400",
  linuxSwap: "bg-rose-500",
};

function segmentLabel(seg: Segment): string {
  if (seg.kind === "unallocated") return "Nicht zugewiesen";
  const p = seg.value;
  return p.label || `Partition ${p.number}`;
}

function segmentColorClass(seg: Segment): string {
  if (seg.kind === "unallocated") return "bg-(--md-color-surface-border)";
  const p = seg.value;
  return KIND_COLORS[p.kind] ?? FS_COLORS[p.fs] ?? "bg-zinc-400";
}

export interface PartitionBarProps {
  disk: Disk;
  selectedId: string | null;
  onSelect: (seg: Segment) => void;
}

/** Keyboard-navigable partition bar: arrow keys move between segments,
 * Enter/Space selects. */
export function PartitionBar({ disk, selectedId, onSelect }: PartitionBarProps) {
  const total = bytesValue(disk.size);

  return (
    <div
      role="listbox"
      aria-label={`Partitionslayout von ${disk.displayName}`}
      className="flex h-12 w-full overflow-hidden rounded-md border border-(--md-color-surface-border)"
    >
      {disk.layout.map((seg, i) => {
        const size = bytesValue(seg.kind === "partition" ? seg.value.size : seg.value.size);
        const pct = total > 0n ? Number((size * 1000n) / total) / 10 : 0;
        const id = seg.kind === "partition" ? seg.value.id : `free-${i}`;
        const selected = id === selectedId;
        return (
          <button
            key={id}
            role="option"
            aria-selected={selected}
            title={`${segmentLabel(seg)} · ${formatBytes(size.toString())}`}
            onClick={() => onSelect(seg)}
            style={{ width: `${Math.max(pct, 1.5)}%` }}
            className={`h-full border-r last:border-r-0 border-(--md-color-bg) transition-opacity focus-visible:z-10 ${segmentColorClass(seg)} ${
              selected
                ? "opacity-100 ring-2 ring-inset ring-(--md-color-focus-ring)"
                : "opacity-85 hover:opacity-100"
            }`}
          >
            <span className="sr-only">
              {segmentLabel(seg)}, {formatBytes(size.toString())}
            </span>
          </button>
        );
      })}
    </div>
  );
}
