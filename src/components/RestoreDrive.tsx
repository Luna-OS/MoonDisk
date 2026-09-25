import { useState, type ReactNode } from "react";
import type { Disk, FileSystem, Platform } from "@/types/models";
import { executeOperation } from "@/lib/ipc";
import { bytesValue, formatBytes } from "@/lib/format";
import { fsOptions } from "@/lib/options";
import { diskModel, fsLabel } from "@/lib/segments";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { MoonPhase } from "@/components/MoonPhase";
import { EraseIcon } from "@/components/icons";
import { DrivePicker, ResultView, Step, type Result } from "@/components/UsbParts";

const GIB = 1024n * 1024n * 1024n;

function restoreProblem(disk: Disk): string | null {
  if (disk.readOnly) return "Read-only";
  if (disk.isSystemDisk) return "System disk";
  return null;
}

/** Erases a USB drive — typically one an image was written to — and
 * leaves one empty partition over all of it. */
export function RestoreDrive({
  disks,
  loading,
  platform,
  onFinished,
}: {
  disks: Disk[];
  loading: boolean;
  platform: Platform;
  onFinished: () => void;
}) {
  const options = fsOptions(platform);
  const [diskId, setDiskId] = useState<string | null>(null);
  const [fs, setFs] = useState<FileSystem>(options.includes("exFat") ? "exFat" : options[0]);
  const [label, setLabel] = useState("USB");
  const [confirming, setConfirming] = useState(false);
  const [running, setRunning] = useState<string | null>(null);
  const [result, setResult] = useState<Result | null>(null);

  const usbDisks = disks.filter((d) => d.bus === "usb");
  const disk = usbDisks.find((d) => d.id === diskId) ?? null;
  const ready = disk !== null && restoreProblem(disk) === null;
  const trimmedLabel = label.trim();

  async function restore() {
    if (!disk) return;
    setConfirming(false);
    setRunning(disk.displayName);
    try {
      await executeOperation({
        request: {
          type: "eraseDisk",
          disk: disk.id,
          filesystem: fs,
          label: trimmedLabel || null,
        },
        confirmed: true,
      });
      setResult({
        kind: "done",
        title: "Drive restored",
        text: `${disk.displayName} is now an empty ${fsLabel(fs)} drive${
          trimmedLabel ? ` named “${trimmedLabel}”` : ""
        } that uses all of its ${formatBytes(disk.size)}.`,
      });
    } catch (e) {
      setResult({ kind: "error", title: "Restoring failed", text: String(e) });
    } finally {
      setRunning(null);
      onFinished();
    }
  }

  if (running) {
    return (
      <div className="flex flex-col items-center gap-4 py-8 text-center" role="status">
        <MoonPhase fraction={0.5} size={96} />
        <p className="text-sm font-medium">Erasing and formatting {running} …</p>
      </div>
    );
  }
  if (result) {
    return <ResultView result={result} backLabel="Done" onBack={() => setResult(null)} />;
  }

  let summary: ReactNode = "Choose the USB drive to restore.";
  if (disk && ready) {
    summary = (
      <span className="text-warning-400">
        Everything on {disk.displayName} ({formatBytes(disk.size)}) will be erased.
      </span>
    );
  }
  const fat32TooBig =
    platform === "windows" && fs === "fat32" && disk !== null && bytesValue(disk.size) > 32n * GIB;

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Step n={1} title="USB drive">
        <DrivePicker
          name="restore-drive"
          disks={usbDisks}
          loading={loading}
          selectedId={diskId}
          problem={restoreProblem}
          onSelect={setDiskId}
        />
      </Step>

      <Step n={2} title="Restore">
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="md-field">
            File system
            <select
              value={fs}
              onChange={(e) => setFs(e.target.value as FileSystem)}
              className="md-input"
            >
              {options.map((f) => (
                <option key={f} value={f}>
                  {fsLabel(f)}
                </option>
              ))}
            </select>
          </label>
          <label className="md-field">
            Name
            <input
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="e.g. USB"
              className="md-input"
            />
          </label>
        </div>
        <p className="-mt-1 text-xs text-(--md-color-text-muted)">
          {fat32TooBig
            ? "Windows only formats FAT32 up to 32 GB — pick exFAT for this drive."
            : "exFAT works on Windows, macOS and Linux and holds files of any size."}
        </p>
        <p className="text-sm">{summary}</p>
        <button
          onClick={() => setConfirming(true)}
          disabled={!ready}
          className="md-btn md-btn-danger-solid mt-auto"
        >
          <EraseIcon />
          Restore drive
        </button>
      </Step>

      <ConfirmDialog
        open={confirming}
        title="Erase and restore drive"
        targetSummary={
          disk
            ? [
                `Drive: ${disk.displayName} – ${diskModel(disk)} (${formatBytes(disk.size)})`,
                `New:   one ${fsLabel(fs)} partition${trimmedLabel ? ` “${trimmedLabel}”` : ""}`,
              ].join("\n")
            : ""
        }
        consequence="Everything on this drive — all partitions and files — will be erased."
        confirmLabel="Erase and restore"
        onCancel={() => setConfirming(false)}
        onConfirm={() => void restore()}
      />
    </div>
  );
}
