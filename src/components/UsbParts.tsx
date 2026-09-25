import type { ReactNode } from "react";
import type { Disk } from "@/types/models";
import { formatBytes } from "@/lib/format";
import { diskModel } from "@/lib/segments";
import { MoonPhase } from "@/components/MoonPhase";
import { AlertIcon } from "@/components/icons";

/** Building blocks shared by the USB tab's "Write image" and "Restore
 * drive" modes. */

export type Result = { kind: "done" | "cancelled" | "error"; title: string; text: string };

export function Step({ n, title, children }: { n: number; title: string; children: ReactNode }) {
  return (
    <div className="md-inset flex min-w-0 flex-col gap-3 p-4">
      <h3 className="md-eyebrow flex items-center gap-2">
        <span className="flex size-5 items-center justify-center rounded-full bg-lavender-400/15 text-[0.65rem] text-lavender-300">
          {n}
        </span>
        {title}
      </h3>
      {children}
    </div>
  );
}

/** Radio list of the USB drives in `disks`; `problem` disables a drive
 * and says why. */
export function DrivePicker({
  name,
  disks,
  loading,
  selectedId,
  problem,
  onSelect,
}: {
  /** Radio group name — must differ between pickers on the same page. */
  name: string;
  disks: Disk[];
  loading: boolean;
  selectedId: string | null;
  problem: (disk: Disk) => string | null;
  onSelect: (id: string) => void;
}) {
  if (disks.length === 0) {
    return (
      <p className="text-sm text-(--md-color-text-muted)">
        {loading ? "Looking for drives …" : "No USB drive found. Plug one in and press Refresh."}
      </p>
    );
  }
  return (
    <div role="radiogroup" aria-label="USB drive" className="flex flex-col gap-2">
      {disks.map((d) => (
        <DriveOption
          key={d.id}
          name={name}
          disk={d}
          problem={problem(d)}
          selected={d.id === selectedId}
          onSelect={() => onSelect(d.id)}
        />
      ))}
    </div>
  );
}

function DriveOption({
  name,
  disk,
  problem,
  selected,
  onSelect,
}: {
  name: string;
  disk: Disk;
  problem: string | null;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <label
      className={`flex items-center gap-3 rounded-xl border p-3 transition-colors duration-200 ${
        problem
          ? "cursor-not-allowed border-lavender-400/10 opacity-50"
          : selected
            ? "cursor-pointer border-lavender-300/60 bg-lavender-400/8"
            : "cursor-pointer border-lavender-400/15 hover:border-lavender-400/35"
      }`}
    >
      <input
        type="radio"
        name={name}
        value={disk.id}
        checked={selected}
        disabled={problem !== null}
        onChange={onSelect}
        className="size-4 accent-lavender-400"
      />
      <span className="min-w-0 flex-1">
        <span className="flex items-baseline justify-between gap-2">
          <span className="font-medium">{disk.displayName}</span>
          <span className="text-xs text-(--md-color-text-muted) tabular-nums">
            {formatBytes(disk.size)}
          </span>
        </span>
        <span className="block truncate text-xs text-(--md-color-text-muted)">
          {diskModel(disk)}
        </span>
      </span>
      {problem ? (
        <span className="md-chip md-chip-warning">{problem}</span>
      ) : (
        disk.isSystemDisk && <span className="md-chip md-chip-warning">System</span>
      )}
    </label>
  );
}

export function ResultView({
  result,
  note,
  backLabel,
  onBack,
}: {
  result: Result;
  /** Extra explanation shown after a successful run. */
  note?: ReactNode;
  backLabel: string;
  onBack: () => void;
}) {
  return (
    <div className="flex flex-col items-center gap-4 py-6 text-center">
      {result.kind === "done" ? (
        <MoonPhase fraction={1} size={96} />
      ) : (
        <span
          className={`flex size-16 items-center justify-center rounded-full ring-1 ${
            result.kind === "error"
              ? "bg-error-500/15 text-[#f28b92] ring-error-500/40"
              : "bg-warning-400/10 text-warning-400 ring-warning-400/35"
          }`}
        >
          <AlertIcon />
        </span>
      )}
      <h3 className="text-lg font-semibold">{result.title}</h3>
      <p
        role={result.kind === "error" ? "alert" : "status"}
        className="max-w-md text-sm break-words text-(--md-color-text-muted)"
      >
        {result.text}
      </p>
      {result.kind === "done" && note && (
        <p className="md-inset max-w-md p-3 text-xs leading-relaxed text-(--md-color-text-muted)">
          {note}
        </p>
      )}
      <button onClick={onBack} className="md-btn md-btn-ghost">
        {result.kind === "done" ? backLabel : "Back"}
      </button>
    </div>
  );
}
