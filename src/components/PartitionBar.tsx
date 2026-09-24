import type { Disk, Segment } from "@/types/models";
import { bytesValue, formatBytes } from "@/lib/format";

const SEGMENT_COLORS: Record<string, string> = {
  efi: "bg-sky-300",
  microsoftReserved: "bg-violet-700",
  recovery: "bg-warning-400",
  basicData: "bg-lavender-400",
  linuxFilesystem: "bg-mint-400",
  linuxSwap: "bg-error-500",
  other: "bg-lavender-300",
};

function segmentLabel(seg: Segment): string {
  if (seg.kind === "unallocated") return "Nicht zugewiesen";
  const p = seg.value;
  return p.label || `Partition ${p.number}`;
}

function segmentColorClass(seg: Segment): string {
  if (seg.kind === "unallocated") return "bg-(--md-color-surface-border)";
  return SEGMENT_COLORS[seg.value.kind] ?? "bg-lavender-300";
}

export interface PartitionBarProps {
  disk: Disk;
  selectedId: string | null;
  onSelect: (seg: Segment) => void;
}

/** Keyboard-navigable partition bar: arrow keys move between segments,
 * Enter/Space selects. See docs/architecture.md §7. */
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
