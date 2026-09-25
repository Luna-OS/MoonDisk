import { useState } from "react";
import type { Disk, FileSystem, OperationRequest } from "@/types/models";
import { operationRisk } from "@/types/models";
import { bytesValue, formatBytes } from "@/lib/format";
import { alignedFreeRange } from "@/lib/alignment";
import { DRIVE_LETTERS, fsOptions } from "@/lib/options";
import { fsLabel } from "@/lib/segments";
import { PlusIcon } from "@/components/icons";

const MIB = 1024n * 1024n;

const RISK_LABELS = { low: "low", medium: "medium", high: "high", critical: "critical" };

export function FreeSpaceActions({
  disk,
  start,
  size,
  busy,
  windows,
  onCreate,
}: {
  disk: Disk;
  start: string;
  size: string;
  busy: boolean;
  windows: boolean;
  onCreate: (req: OperationRequest) => void;
}) {
  const [fs, setFs] = useState<FileSystem>(fsOptions(windows)[0]);
  const [label, setLabel] = useState("");
  const [driveLetter, setDriveLetter] = useState(DRIVE_LETTERS[1]);

  // The free region's own bytes aren't guaranteed to be 1-MiB-aligned
  // (gaps between existing partitions on a real disk often aren't), but
  // MoonDisk's own partitions always are, so the maximum usable size has
  // to be the largest aligned sub-range that fits rather than the free
  // region's raw byte count.
  const aligned = alignedFreeRange(start, size);
  const maxSizeMib = aligned ? bytesValue(aligned.size) / MIB : 0n;
  const [sizeMib, setSizeMib] = useState(() => maxSizeMib.toString());

  let sizeMibValue: bigint;
  try {
    sizeMibValue = BigInt(sizeMib || "-1");
  } catch {
    sizeMibValue = -1n;
  }
  const sizeValid = aligned !== null && sizeMibValue >= 1n && sizeMibValue <= maxSizeMib;
  const chosenSizeBytes = sizeValid ? (sizeMibValue * MIB).toString() : null;

  if (!aligned) {
    return (
      <p className="px-4 pb-4 text-sm text-(--md-color-text-muted)">
        After 1 MiB alignment this region is smaller than 1 MiB — too small for a partition.
      </p>
    );
  }

  const request: OperationRequest | null = chosenSizeBytes
    ? {
        type: "createPartition",
        disk: disk.id,
        start: aligned.start,
        size: chosenSizeBytes,
        filesystem: fs,
        label: label || null,
        driveLetter: windows ? driveLetter : null,
      }
    : null;

  return (
    <div className="flex flex-col gap-4 px-4 pt-1 pb-4">
      <h4 className="md-eyebrow">New partition</h4>

      <div className={`grid gap-3 ${windows ? "sm:grid-cols-3" : "sm:grid-cols-2"}`}>
        <label className="md-field">
          File system
          <select
            value={fs}
            onChange={(e) => setFs(e.target.value as FileSystem)}
            className="md-input"
          >
            {fsOptions(windows).map((f) => (
              <option key={f} value={f}>
                {fsLabel(f)}
              </option>
            ))}
          </select>
        </label>
        <label className="md-field">
          Label (optional)
          <input
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="e.g. Data"
            className="md-input"
          />
        </label>
        {windows && (
          <label className="md-field">
            Drive letter
            <select
              value={driveLetter}
              onChange={(e) => setDriveLetter(e.target.value)}
              className="md-input"
            >
              {DRIVE_LETTERS.map((l) => (
                <option key={l} value={l}>
                  {l}:
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      <div className="md-inset flex flex-col gap-3 p-3">
        <div className="flex items-baseline justify-between gap-3">
          <span className="text-xs font-medium text-(--md-color-text-muted)">Size</span>
          <span className="text-lg font-semibold tabular-nums text-lavender-300">
            {chosenSizeBytes ? formatBytes(chosenSizeBytes) : "–"}
            <span className="ml-2 text-xs font-normal text-(--md-color-text-muted)">
              of {formatBytes(aligned.size)}
            </span>
          </span>
        </div>
        <input
          type="range"
          aria-label="Size"
          min={1}
          max={Number(maxSizeMib)}
          step={1}
          value={sizeValid ? Number(sizeMibValue) : Number(maxSizeMib)}
          onChange={(e) => setSizeMib(e.target.value)}
          className="md-range"
        />
        <div className="flex flex-wrap items-end gap-2">
          <label className="md-field">
            Exact (MiB)
            <input
              type="number"
              min="1"
              max={maxSizeMib.toString()}
              step="1"
              value={sizeMib}
              onChange={(e) => setSizeMib(e.target.value)}
              className="md-input w-36 tabular-nums"
            />
          </label>
          <button
            type="button"
            onClick={() => setSizeMib(maxSizeMib.toString())}
            className="md-btn md-btn-ghost"
          >
            Maximum
          </button>
          {!sizeValid && (
            <span className="pb-2 text-xs text-[#f28b92]">
              Choose between 1 and {maxSizeMib.toString()} MiB.
            </span>
          )}
        </div>
      </div>

      <div className="flex items-center justify-between gap-3">
        <span className="md-chip">Risk: {request ? RISK_LABELS[operationRisk(request)] : "–"}</span>
        <button
          disabled={busy || !request}
          onClick={() => request && onCreate(request)}
          className="md-btn md-btn-primary"
        >
          <PlusIcon />
          {busy ? "Creating …" : "Create partition"}
        </button>
      </div>
    </div>
  );
}
