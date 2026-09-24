import { useCallback, useEffect, useState } from "react";
import type { AppInfo, Disk, FileSystem, OperationRequest, Segment } from "@/types/models";
import { executeOperation, getAppInfo, listDisks } from "@/lib/ipc";
import { operationRisk } from "@/types/models";
import { formatBytes } from "@/lib/format";
import { PartitionBar } from "@/components/PartitionBar";
import { ConfirmDialog } from "@/components/ConfirmDialog";

const LANGUAGE = "de";

const FS_OPTIONS: FileSystem[] = ["ntfs", "fat32", "exFat", "ext2", "ext3", "ext4", "btrfs", "xfs"];

function diskSummary(disk: Disk): string {
  return [
    `Datenträger: ${disk.displayName} – ${disk.model || disk.vendor || "unbekannt"}`,
    `Größe: ${formatBytes(disk.size)} · ${disk.table.toUpperCase()}`,
  ].join("\n");
}

function partitionSummary(disk: Disk, p: Extract<Segment, { kind: "partition" }>["value"]): string {
  return [
    diskSummary(disk),
    `Partition: Nr. ${p.number} · ${formatBytes(p.size)} · ${p.fs} ${
      p.label ? `· Label „${p.label}“` : ""
    }`,
  ].join("\n");
}

interface PendingCritical {
  request: OperationRequest;
  title: string;
  consequence: string;
  targetSummary: string;
}

export default function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [disks, setDisks] = useState<Disk[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedDiskId, setSelectedDiskId] = useState<string | null>(null);
  const [selectedSegmentId, setSelectedSegmentId] = useState<string | null>(null);
  const [pendingCritical, setPendingCritical] = useState<PendingCritical | null>(null);
  const [confirmError, setConfirmError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [info, diskList] = await Promise.all([getAppInfo(), listDisks()]);
      setAppInfo(info);
      setDisks(diskList);
      setLoadError(null);
    } catch (e) {
      setLoadError(String(e));
    }
  }, []);

  useEffect(() => {
    // Loading data on mount via an already-memoized async function is the
    // standard pattern here; the setState calls inside `refresh` happen
    // after its `await`, not synchronously within this effect body, so
    // this doesn't cause the cascading-render pattern the rule guards
    // against.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void refresh();
  }, [refresh]);

  const selectedDisk = disks.find((d) => d.id === selectedDiskId) ?? null;

  async function runDirect(request: OperationRequest) {
    setBusy(true);
    setStatus(null);
    try {
      await executeOperation({ request, language: LANGUAGE });
      setStatus("Operation erfolgreich ausgeführt.");
      await refresh();
    } catch (e) {
      setStatus(`Fehlgeschlagen: ${String(e)}`);
    } finally {
      setBusy(false);
    }
  }

  async function runCritical(phrase: string) {
    if (!pendingCritical) return;
    setBusy(true);
    setConfirmError(null);
    try {
      await executeOperation({
        request: pendingCritical.request,
        confirmationPhrase: phrase,
        language: LANGUAGE,
      });
      setStatus("Operation erfolgreich ausgeführt.");
      setPendingCritical(null);
      await refresh();
    } catch (e) {
      setConfirmError(String(e));
    } finally {
      setBusy(false);
    }
  }

  function askCritical(disk: Disk, request: OperationRequest, action: "delete" | "format") {
    const partition =
      request.type === "deletePartition" || request.type === "formatPartition"
        ? disk.layout
            .filter((s): s is Extract<Segment, { kind: "partition" }> => s.kind === "partition")
            .find((s) => s.value.id === request.partition)?.value
        : undefined;
    if (!partition) return;
    setPendingCritical({
      request,
      title: action === "delete" ? "Partition löschen" : "Partition formatieren",
      consequence:
        action === "delete"
          ? "Die Partition und alle darauf gespeicherten Daten werden unwiderruflich entfernt."
          : "Alle Daten auf dieser Partition werden unwiderruflich überschrieben.",
      targetSummary: partitionSummary(disk, partition),
    });
    setConfirmError(null);
  }

  return (
    <div className="min-h-screen px-6 py-8">
      <header className="mx-auto mb-8 max-w-4xl">
        <h1 className="text-2xl font-semibold text-lavender-400">MoonDisk</h1>
        <p className="text-sm text-(--md-color-text-muted)">
          Deine Laufwerke. Sicher im Mondlicht.
        </p>
      </header>

      <main className="mx-auto flex max-w-4xl flex-col gap-6">
        {appInfo && (
          <div
            role="status"
            className={`rounded-md border px-4 py-2 text-sm ${
              appInfo.mode === "mock"
                ? "border-(--md-color-surface-border) text-(--md-color-text-muted)"
                : "border-(--md-color-warning) text-(--md-color-warning)"
            }`}
          >
            {appInfo.mode === "mock"
              ? "Mock-Modus aktiv – es werden keine echten Laufwerke verändert."
              : "Produktionsmodus aktiv – Änderungen wirken auf echte Datenträger. Erstelle vorher ein Backup."}
          </div>
        )}

        {loadError && (
          <div
            role="alert"
            className="rounded-md border border-(--md-color-error) px-4 py-2 text-sm text-(--md-color-error)"
          >
            Datenträger konnten nicht geladen werden: {loadError}
          </div>
        )}
        {status && (
          <div
            role="status"
            className="rounded-md border border-(--md-color-surface-border) px-4 py-2 text-sm"
          >
            {status}
          </div>
        )}

        <section aria-label="Datenträgerübersicht" className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-medium">Datenträger</h2>
            <button
              onClick={() => void refresh()}
              className="rounded-md border px-3 py-1 text-sm border-(--md-color-surface-border)"
            >
              Aktualisieren
            </button>
          </div>
          <ul className="flex flex-col gap-2">
            {disks.map((disk) => (
              <li key={disk.id}>
                <button
                  onClick={() => {
                    setSelectedDiskId(disk.id);
                    setSelectedSegmentId(null);
                  }}
                  className={`w-full rounded-md border px-4 py-3 text-left bg-(--md-color-surface) border-(--md-color-surface-border) ${
                    disk.id === selectedDiskId ? "ring-2 ring-(--md-color-focus-ring)" : ""
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-medium">
                      {disk.displayName} – {disk.model || disk.vendor || "unbekannt"}
                    </span>
                    <span className="text-sm text-(--md-color-text-muted)">
                      {formatBytes(disk.size)}
                    </span>
                  </div>
                  <div className="mt-1 text-xs text-(--md-color-text-muted)">
                    {disk.bus.toUpperCase()} · {disk.table.toUpperCase()}
                    {disk.isSystemDisk && " · Systemdatenträger (geschützt)"}
                    {disk.readOnly && " · schreibgeschützt"}
                  </div>
                </button>
              </li>
            ))}
            {disks.length === 0 && !loadError && (
              <li className="text-sm text-(--md-color-text-muted)">Keine Datenträger gefunden.</li>
            )}
          </ul>
        </section>

        {selectedDisk && (
          <section aria-label="Partitionsdetails" className="flex flex-col gap-4">
            <h2 className="text-lg font-medium">Partitionen von {selectedDisk.displayName}</h2>
            <PartitionBar
              disk={selectedDisk}
              selectedId={selectedSegmentId}
              onSelect={(seg) =>
                setSelectedSegmentId(
                  seg.kind === "partition"
                    ? seg.value.id
                    : `free-${selectedDisk.layout.indexOf(seg)}`,
                )
              }
            />

            <ul className="flex flex-col gap-3">
              {selectedDisk.layout.map((seg, i) => {
                const id = seg.kind === "partition" ? seg.value.id : `free-${i}`;
                if (id !== selectedSegmentId) return null;
                if (seg.kind === "unallocated") {
                  return (
                    <FreeSpaceActions
                      key={id}
                      disk={selectedDisk}
                      start={seg.value.start}
                      size={seg.value.size}
                      busy={busy}
                      onCreate={(req) => void runDirect(req)}
                    />
                  );
                }
                return (
                  <PartitionActions
                    key={id}
                    disk={selectedDisk}
                    partition={seg.value}
                    busy={busy}
                    onLabel={(req) => void runDirect(req)}
                    onFormat={(req) => askCritical(selectedDisk, req, "format")}
                    onDelete={(req) => askCritical(selectedDisk, req, "delete")}
                  />
                );
              })}
            </ul>
          </section>
        )}
      </main>

      <ConfirmDialog
        open={pendingCritical !== null}
        title={pendingCritical?.title ?? ""}
        consequence={pendingCritical?.consequence ?? ""}
        targetSummary={pendingCritical?.targetSummary ?? ""}
        language={LANGUAGE}
        busy={busy}
        error={confirmError}
        onCancel={() => {
          setPendingCritical(null);
          setConfirmError(null);
        }}
        onConfirm={(phrase) => void runCritical(phrase)}
      />
    </div>
  );
}

function PartitionActions({
  disk,
  partition,
  busy,
  onLabel,
  onFormat,
  onDelete,
}: {
  disk: Disk;
  partition: Extract<Segment, { kind: "partition" }>["value"];
  busy: boolean;
  onLabel: (req: OperationRequest) => void;
  onFormat: (req: OperationRequest) => void;
  onDelete: (req: OperationRequest) => void;
}) {
  const [label, setLabel] = useState(partition.label ?? "");
  const [formatFs, setFormatFs] = useState<FileSystem>("ext4");
  const locked = (partition.flags & 0b1000) !== 0; // PartitionFlags.LOCKED
  const protectedReason = disk.isSystemDisk
    ? "Systemdatenträger ist geschützt"
    : locked
      ? "geschützte System-/Boot-/EFI-Partition"
      : null;

  return (
    <li className="flex flex-col gap-3 rounded-md border border-(--md-color-surface-border) bg-(--md-color-surface) p-4">
      <div className="text-sm font-medium">
        Partition {partition.number} · {formatBytes(partition.size)} · {partition.fs}
      </div>

      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col text-xs">
          Label
          <input
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            className="rounded border px-2 py-1 border-(--md-color-surface-border) bg-(--md-color-bg)"
          />
        </label>
        <button
          disabled={busy || !!protectedReason || label === (partition.label ?? "")}
          onClick={() => onLabel({ type: "setLabel", partition: partition.id, label })}
          className="rounded-md border px-3 py-1 text-sm border-(--md-color-surface-border) disabled:opacity-40"
        >
          Label speichern
        </button>
      </div>

      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col text-xs">
          Neu formatieren als
          <select
            value={formatFs}
            onChange={(e) => setFormatFs(e.target.value as FileSystem)}
            className="rounded border px-2 py-1 border-(--md-color-surface-border) bg-(--md-color-bg)"
          >
            {FS_OPTIONS.map((fs) => (
              <option key={fs} value={fs}>
                {fs}
              </option>
            ))}
          </select>
        </label>
        <button
          disabled={busy || !!protectedReason}
          onClick={() =>
            onFormat({
              type: "formatPartition",
              partition: partition.id,
              filesystem: formatFs,
              label: label || null,
            })
          }
          className="rounded-md border px-3 py-1 text-sm border-(--md-color-warning) text-(--md-color-warning) disabled:opacity-40"
        >
          Formatieren …
        </button>
        <button
          disabled={busy || !!protectedReason}
          onClick={() => onDelete({ type: "deletePartition", partition: partition.id })}
          className="rounded-md border px-3 py-1 text-sm border-(--md-color-error) text-(--md-color-error) disabled:opacity-40"
        >
          Löschen …
        </button>
      </div>

      {protectedReason && (
        <p className="text-xs text-(--md-color-text-muted)">
          Aktionen deaktiviert: {protectedReason}.
        </p>
      )}
    </li>
  );
}

function FreeSpaceActions({
  disk,
  start,
  size,
  busy,
  onCreate,
}: {
  disk: Disk;
  start: string;
  size: string;
  busy: boolean;
  onCreate: (req: OperationRequest) => void;
}) {
  const [fs, setFs] = useState<FileSystem>("ext4");
  const [label, setLabel] = useState("");

  return (
    <li className="flex flex-col gap-3 rounded-md border border-dashed border-(--md-color-surface-border) p-4">
      <div className="text-sm font-medium">Nicht zugewiesen · {formatBytes(size)}</div>
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col text-xs">
          Dateisystem
          <select
            value={fs}
            onChange={(e) => setFs(e.target.value as FileSystem)}
            className="rounded border px-2 py-1 border-(--md-color-surface-border) bg-(--md-color-bg)"
          >
            {FS_OPTIONS.map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col text-xs">
          Label (optional)
          <input
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            className="rounded border px-2 py-1 border-(--md-color-surface-border) bg-(--md-color-bg)"
          />
        </label>
        <button
          disabled={busy}
          onClick={() =>
            onCreate({
              type: "createPartition",
              disk: disk.id,
              start,
              size,
              filesystem: fs,
              label: label || null,
            })
          }
          className="rounded-md border px-3 py-1 text-sm border-(--md-color-primary) text-(--md-color-primary) disabled:opacity-40"
        >
          Partition über gesamten freien Bereich erstellen
        </button>
      </div>
      <p className="text-xs text-(--md-color-text-muted)">
        Risiko:{" "}
        {operationRisk({
          type: "createPartition",
          disk: disk.id,
          start,
          size,
          filesystem: fs,
          label: null,
        })}
      </p>
    </li>
  );
}
