import type { Disk, Segment } from "@/types/models";
import { bytesValue, formatBytes } from "@/lib/format";
import { segmentColorClass, segmentId, segmentLabel } from "@/lib/segments";

export interface PartitionBarProps {
  disk: Disk;
  selectedId: string | null;
  onSelect: (seg: Segment, id: string) => void;
}

export function PartitionBar({ disk, selectedId, onSelect }: PartitionBarProps) {
  const total = bytesValue(disk.size);

  return (
    <div
      role="listbox"
      aria-label={`Partitionslayout von ${disk.displayName}`}
      className="md-inset flex h-18 w-full gap-1.5 p-1.5"
    >
      {disk.layout.map((seg, i) => {
        const size = bytesValue(seg.value.size);
        const pct = total > 0n ? Number((size * 1000n) / total) / 10 : 0;
        const id = segmentId(seg, i);
        const selected = id === selectedId;
        const free = seg.kind === "unallocated";
        const letter = seg.kind === "partition" && seg.value.driveLetter;
        return (
          <button
            key={id}
            role="option"
            aria-selected={selected}
            title={`${segmentLabel(seg)} · ${formatBytes(size.toString())}`}
            onClick={() => onSelect(seg, id)}
            style={{ flex: `${Math.max(pct, 1)} 1 0%` }}
            className={`relative min-w-3.5 overflow-hidden rounded-lg text-left transition-[filter,box-shadow] duration-200 ${segmentColorClass(seg)} ${
              selected
                ? "brightness-110 ring-2 ring-lavender-300 ring-offset-2 ring-offset-night-950"
                : "hover:brightness-110"
            }`}
          >
            {!free && (
              <span className="pointer-events-none absolute inset-0 bg-linear-to-b from-white/35 via-white/5 to-black/15" />
            )}
            {pct >= 9 ? (
              <span
                className={`relative flex h-full flex-col justify-center px-3 leading-tight ${
                  free ? "" : "text-night-950"
                }`}
              >
                <span className="truncate text-xs font-semibold">
                  {letter ? `${letter}: ` : ""}
                  {segmentLabel(seg)}
                </span>
                <span className={`truncate text-[0.7rem] ${free ? "opacity-70" : "opacity-75"}`}>
                  {formatBytes(size.toString())}
                </span>
              </span>
            ) : (
              <span className="sr-only">
                {segmentLabel(seg)}, {formatBytes(size.toString())}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
