import { useState } from "react";
import type { FileSystem, OperationRequest, Partition, Platform } from "@/types/models";
import { DRIVE_LETTERS, fsOptions } from "@/lib/options";
import { fsLabel } from "@/lib/segments";
import { EraseIcon, TrashIcon } from "@/components/icons";

export function PartitionActions({
  partition,
  busy,
  platform,
  onRun,
  onFormat,
  onDelete,
}: {
  partition: Partition;
  busy: boolean;
  platform: Platform;
  onRun: (req: OperationRequest) => void;
  onFormat: (req: OperationRequest) => void;
  onDelete: (req: OperationRequest) => void;
}) {
  const windows = platform === "windows";
  const [label, setLabel] = useState(partition.label ?? "");
  const [formatFs, setFormatFs] = useState<FileSystem>(fsOptions(platform)[0]);
  const [driveLetter, setDriveLetter] = useState(partition.driveLetter ?? DRIVE_LETTERS[1]);

  return (
    <div className="flex flex-col gap-4 px-4 pt-1 pb-4">
      <section className="flex flex-col gap-3">
        <h4 className="md-eyebrow">Properties</h4>
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="flex items-end gap-2">
            <label className="md-field min-w-0 flex-1">
              Label
              <input
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="No label"
                className="md-input"
              />
            </label>
            <button
              disabled={busy || label === (partition.label ?? "")}
              onClick={() => onRun({ type: "setLabel", partition: partition.id, label })}
              className="md-btn md-btn-ghost"
            >
              Save
            </button>
          </div>

          {windows && (
            <div className="flex items-end gap-2">
              <label className="md-field min-w-0 flex-1">
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
              <button
                disabled={busy || driveLetter === partition.driveLetter}
                onClick={() =>
                  onRun({ type: "setDriveLetter", partition: partition.id, driveLetter })
                }
                className="md-btn md-btn-ghost"
              >
                {partition.driveLetter ? "Change" : "Assign"}
              </button>
            </div>
          )}
        </div>
      </section>

      <section className="flex flex-col gap-3 rounded-xl border border-error-500/25 bg-error-500/5 p-3">
        <h4 className="md-eyebrow text-[#f28b92]">Danger zone</h4>
        <div className="flex flex-wrap items-end gap-2">
          <label className="md-field w-44">
            Reformat as
            <select
              value={formatFs}
              onChange={(e) => setFormatFs(e.target.value as FileSystem)}
              className="md-input"
            >
              {fsOptions(platform).map((fs) => (
                <option key={fs} value={fs}>
                  {fsLabel(fs)}
                </option>
              ))}
            </select>
          </label>
          <button
            disabled={busy}
            onClick={() =>
              onFormat({
                type: "formatPartition",
                partition: partition.id,
                filesystem: formatFs,
                label: label || null,
              })
            }
            className="md-btn md-btn-warning"
          >
            <EraseIcon />
            Format …
          </button>
          <div className="flex-1" />
          <button
            disabled={busy}
            onClick={() => onDelete({ type: "deletePartition", partition: partition.id })}
            className="md-btn md-btn-danger"
          >
            <TrashIcon />
            Delete …
          </button>
        </div>
      </section>
    </div>
  );
}
