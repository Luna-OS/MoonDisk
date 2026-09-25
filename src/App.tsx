import { useCallback, useEffect, useState, type ReactNode } from "react";
import type {
  AppInfo,
  BusType,
  Disk,
  OperationRequest,
  Partition,
  Platform,
  Segment,
} from "@/types/models";
import { hasFlag, PartitionFlags } from "@/types/models";
import { executeOperation, getAppInfo, listDisks } from "@/lib/ipc";
import { formatBytes } from "@/lib/format";
import {
  diskModel,
  diskUsage,
  fsLabel,
  kindLabel,
  segmentColorClass,
  segmentId,
  segmentLabel,
} from "@/lib/segments";
import { PartitionBar } from "@/components/PartitionBar";
import { PartitionActions } from "@/components/PartitionActions";
import { FreeSpaceActions } from "@/components/FreeSpaceActions";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ImageWriter } from "@/components/ImageWriter";
import { MoonPhase } from "@/components/MoonPhase";
import { Sky } from "@/components/Sky";
import {
  AlertIcon,
  CheckIcon,
  ChevronIcon,
  CloseIcon,
  DiskIcon,
  RefreshIcon,
  UsbIcon,
} from "@/components/icons";

const BUS_LABELS: Record<BusType, string> = {
  sata: "SATA",
  nvme: "NVMe",
  usb: "USB",
  virtual: "Virtual",
  unknown: "Unknown bus",
};

function partitionSummary(disk: Disk, p: Partition): string {
  return [
    `Disk:      ${disk.displayName} – ${diskModel(disk)}`,
    `Partition: #${p.number}${p.driveLetter ? ` (${p.driveLetter}:)` : ""}${
      p.label ? ` “${p.label}”` : ""
    }`,
    `Size:      ${formatBytes(p.size)} · ${fsLabel(p.fs)}`,
  ].join("\n");
}

interface PendingCritical {
  request: OperationRequest;
  title: string;
  consequence: string;
  targetSummary: string;
}

type Status = { kind: "success" | "error"; text: string };

type Tab = "partitions" | "usb";

const TABS: { id: Tab; label: string; icon: ReactNode }[] = [
  { id: "partitions", label: "Partitions", icon: <DiskIcon /> },
  { id: "usb", label: "USB writer", icon: <UsbIcon /> },
];

export default function App() {
  const [tab, setTab] = useState<Tab>("partitions");
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [disks, setDisks] = useState<Disk[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedDiskId, setSelectedDiskId] = useState<string | null>(null);
  const [selectedSegmentId, setSelectedSegmentId] = useState<string | null>(null);
  const [pendingCritical, setPendingCritical] = useState<PendingCritical | null>(null);
  const [confirmError, setConfirmError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<Status | null>(null);

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
  const platform = appInfo?.platform ?? "linux";
  const loading = appInfo === null && loadError === null;

  async function runDirect(request: OperationRequest) {
    setBusy(true);
    setStatus(null);
    try {
      await executeOperation({ request });
      setStatus({ kind: "success", text: "Done — the disk has been updated." });
      await refresh();
    } catch (e) {
      setStatus({ kind: "error", text: `Failed: ${String(e)}` });
    } finally {
      setBusy(false);
    }
  }

  async function runCritical() {
    if (!pendingCritical) return;
    setBusy(true);
    setConfirmError(null);
    try {
      await executeOperation({ request: pendingCritical.request, confirmed: true });
      setStatus({ kind: "success", text: "Done — the disk has been updated." });
      setPendingCritical(null);
      setSelectedSegmentId(null);
      await refresh();
    } catch (e) {
      setConfirmError(String(e));
    } finally {
      setBusy(false);
    }
  }

  function askCritical(disk: Disk, partition: Partition, request: OperationRequest) {
    const deleting = request.type === "deletePartition";
    setPendingCritical({
      request,
      title: deleting ? "Delete partition" : "Format partition",
      consequence: deleting
        ? "The partition and all data on it will be permanently removed."
        : "All data on this partition will be permanently overwritten.",
      targetSummary: partitionSummary(disk, partition),
    });
    setConfirmError(null);
  }

  return (
    <div className="relative min-h-screen">
      <Sky />

      <div className="relative z-10 mx-auto flex max-w-6xl flex-col gap-6 px-6 py-7">
        <header className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-4">
            <img
              src="/moondisk-logo.png"
              alt=""
              className="size-14 drop-shadow-[0_0_14px_rgb(185_174_251/0.45)]"
            />
            <div>
              <h1 className="md-title text-3xl font-semibold tracking-tight">MoonDisk</h1>
              <p className="text-sm text-(--md-color-text-muted)">
                Your disks, safely under the moon.
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {appInfo && <span className="md-chip">v{appInfo.version}</span>}
            <button onClick={() => void refresh()} disabled={busy} className="md-btn md-btn-ghost">
              <RefreshIcon />
              Refresh
            </button>
          </div>
        </header>

        {appInfo && (
          <Banner tone="warning" icon={<AlertIcon />}>
            Changes are applied to real disks. Make a backup first.
          </Banner>
        )}
        {loadError && (
          <Banner tone="error" icon={<AlertIcon />}>
            Could not load disks: {loadError}
          </Banner>
        )}

        <Tabs tab={tab} onChange={setTab} />

        <div id="panel-usb" role="tabpanel" aria-labelledby="tab-usb" hidden={tab !== "usb"}>
          <ImageWriter disks={disks} loading={loading} onFinished={() => void refresh()} />
        </div>

        <div
          id="panel-partitions"
          role="tabpanel"
          aria-labelledby="tab-partitions"
          hidden={tab !== "partitions"}
          className="grid items-start gap-6 md:grid-cols-[300px_minmax(0,1fr)]"
        >
          <aside aria-label="Disks" className="flex flex-col gap-3">
            <h2 className="md-eyebrow px-1">Disks</h2>
            {loading &&
              [0, 1].map((i) => <div key={i} className="md-glass md-skeleton h-[106px]" />)}
            {disks.map((disk) => (
              <DiskCard
                key={disk.id}
                disk={disk}
                selected={disk.id === selectedDiskId}
                onSelect={() => {
                  setSelectedDiskId(disk.id);
                  setSelectedSegmentId(null);
                }}
              />
            ))}
            {!loading && disks.length === 0 && !loadError && (
              <p className="px-1 text-sm text-(--md-color-text-muted)">No disks found.</p>
            )}
          </aside>

          <main>
            {selectedDisk ? (
              <DiskDetail
                disk={selectedDisk}
                platform={platform}
                busy={busy}
                selectedSegmentId={selectedSegmentId}
                onSelectSegment={setSelectedSegmentId}
                onRun={(req) => void runDirect(req)}
                onCritical={(partition, req) => askCritical(selectedDisk, partition, req)}
              />
            ) : (
              <EmptyState />
            )}
          </main>
        </div>
      </div>

      {status && (
        <div className="fixed right-6 bottom-6 z-20 w-[min(28rem,calc(100vw-3rem))] shadow-[0_18px_50px_-12px_rgb(0_0_0/0.8)]">
          <Banner
            tone={status.kind}
            icon={status.kind === "success" ? <CheckIcon /> : <AlertIcon />}
            onClose={() => setStatus(null)}
          >
            {status.text}
          </Banner>
        </div>
      )}

      <ConfirmDialog
        open={pendingCritical !== null}
        title={pendingCritical?.title ?? ""}
        consequence={pendingCritical?.consequence ?? ""}
        targetSummary={pendingCritical?.targetSummary ?? ""}
        busy={busy}
        error={confirmError}
        onCancel={() => {
          setPendingCritical(null);
          setConfirmError(null);
        }}
        onConfirm={() => void runCritical()}
      />
    </div>
  );
}

function Tabs({ tab, onChange }: { tab: Tab; onChange: (tab: Tab) => void }) {
  function focusTab(index: number) {
    const next = TABS[(index + TABS.length) % TABS.length];
    onChange(next.id);
    document.getElementById(`tab-${next.id}`)?.focus();
  }

  return (
    <div
      role="tablist"
      aria-label="Sections"
      className="md-glass flex gap-1 self-start rounded-[0.95rem] p-1"
    >
      {TABS.map((t, i) => {
        const selected = t.id === tab;
        return (
          <button
            key={t.id}
            id={`tab-${t.id}`}
            role="tab"
            aria-selected={selected}
            aria-controls={`panel-${t.id}`}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(t.id)}
            onKeyDown={(e) => {
              if (e.key === "ArrowRight") focusTab(i + 1);
              if (e.key === "ArrowLeft") focusTab(i - 1);
            }}
            className={`inline-flex h-9 items-center gap-2 rounded-[0.7rem] px-4 text-sm font-medium transition-colors duration-200 ${
              selected
                ? "bg-lavender-400/15 text-lavender-300 shadow-[inset_0_0_0_1px_rgb(185_174_251/0.35)]"
                : "text-(--md-color-text-muted) hover:bg-white/4 hover:text-(--md-color-text)"
            }`}
          >
            {t.icon}
            {t.label}
          </button>
        );
      })}
    </div>
  );
}

const BANNER_TONES = {
  warning: "border-warning-400/35 bg-warning-400/8 text-warning-400",
  error: "border-error-500/40 bg-[#2a1530]/95 text-[#f28b92]",
  success: "border-mint-400/35 bg-[#132a33]/95 text-mint-400",
};

function Banner({
  tone,
  icon,
  onClose,
  children,
}: {
  tone: keyof typeof BANNER_TONES;
  icon: ReactNode;
  onClose?: () => void;
  children: ReactNode;
}) {
  return (
    <div
      role={tone === "error" ? "alert" : "status"}
      className={`flex items-start gap-3 rounded-xl border px-4 py-2.5 text-sm backdrop-blur-md ${BANNER_TONES[tone]}`}
    >
      <span className="mt-0.5">{icon}</span>
      <span className="min-w-0 flex-1 break-words">{children}</span>
      {onClose && (
        <button
          onClick={onClose}
          aria-label="Dismiss"
          className="mt-0.5 rounded-md opacity-70 hover:opacity-100"
        >
          <CloseIcon />
        </button>
      )}
    </div>
  );
}

function DiskCard({
  disk,
  selected,
  onSelect,
}: {
  disk: Disk;
  selected: boolean;
  onSelect: () => void;
}) {
  const usage = diskUsage(disk);
  return (
    <button
      onClick={onSelect}
      aria-pressed={selected}
      className={`md-glass flex w-full items-center gap-3 p-4 text-left transition-[border-color,box-shadow] duration-200 ${
        selected
          ? "border-lavender-300/60 shadow-[0_0_0_1px_rgb(214_207_253/0.3),0_14px_40px_-14px_rgb(185_174_251/0.55)]"
          : "hover:border-lavender-400/35"
      }`}
    >
      <MoonPhase fraction={usage.fraction} size={44} />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-2">
          <span className="font-semibold">{disk.displayName}</span>
          <span className="text-xs text-(--md-color-text-muted) tabular-nums">
            {formatBytes(disk.size)}
          </span>
        </div>
        <div className="truncate text-sm text-(--md-color-text-muted)">{diskModel(disk)}</div>
        <div className="mt-2 flex flex-wrap gap-1.5">
          <span className="md-chip">{BUS_LABELS[disk.bus]}</span>
          <span className="md-chip">
            {disk.table === "none" ? "No table" : disk.table.toUpperCase()}
          </span>
          {disk.isSystemDisk && <span className="md-chip md-chip-warning">System</span>}
          {disk.readOnly && <span className="md-chip">Read-only</span>}
        </div>
      </div>
    </button>
  );
}

function DiskDetail({
  disk,
  platform,
  busy,
  selectedSegmentId,
  onSelectSegment,
  onRun,
  onCritical,
}: {
  disk: Disk;
  platform: Platform;
  busy: boolean;
  selectedSegmentId: string | null;
  onSelectSegment: (id: string | null) => void;
  onRun: (req: OperationRequest) => void;
  onCritical: (partition: Partition, req: OperationRequest) => void;
}) {
  const usage = diskUsage(disk);
  const partitionCount = disk.layout.filter((s) => s.kind === "partition").length;

  return (
    <section aria-label="Partitions" className="md-glass flex flex-col gap-5 p-5">
      <div className="flex items-center gap-4">
        <MoonPhase fraction={usage.fraction} size={56} />
        <div className="min-w-0 flex-1">
          <h2 className="text-xl font-semibold">{disk.displayName}</h2>
          <p className="truncate text-sm text-(--md-color-text-muted)">
            {diskModel(disk)}
            {disk.serial ? ` · ${disk.serial}` : ""}
          </p>
        </div>
      </div>

      <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Stat label="Capacity" value={formatBytes(disk.size)} />
        <Stat label="Allocated" value={formatBytes(usage.allocated.toString())} />
        <Stat label="Free" value={formatBytes(usage.free.toString())} />
        <Stat label="Partitions" value={String(partitionCount)} />
      </dl>

      <div className="flex flex-col gap-2">
        <h3 className="md-eyebrow">Layout</h3>
        <PartitionBar
          disk={disk}
          selectedId={selectedSegmentId}
          onSelect={(_, id) => onSelectSegment(id)}
        />
      </div>

      <ul className="flex flex-col gap-2">
        {disk.layout.map((seg, i) => {
          const id = segmentId(seg, i);
          const selected = id === selectedSegmentId;
          return (
            <li
              key={id}
              className={`md-inset overflow-hidden transition-colors duration-200 ${
                selected ? "border-lavender-400/40 bg-lavender-400/5" : ""
              }`}
            >
              <SegmentRow
                seg={seg}
                selected={selected}
                onToggle={() => onSelectSegment(selected ? null : id)}
              />
              {selected &&
                (seg.kind === "unallocated" ? (
                  <FreeSpaceActions
                    disk={disk}
                    start={seg.value.start}
                    size={seg.value.size}
                    busy={busy}
                    platform={platform}
                    onCreate={onRun}
                  />
                ) : (
                  <PartitionActions
                    partition={seg.value}
                    busy={busy}
                    platform={platform}
                    onRun={onRun}
                    onFormat={(req) => onCritical(seg.value, req)}
                    onDelete={(req) => onCritical(seg.value, req)}
                  />
                ))}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="md-inset px-3 py-2">
      <dt className="text-[0.7rem] text-(--md-color-text-muted)">{label}</dt>
      <dd className="font-semibold tabular-nums">{value}</dd>
    </div>
  );
}

function SegmentRow({
  seg,
  selected,
  onToggle,
}: {
  seg: Segment;
  selected: boolean;
  onToggle: () => void;
}) {
  const free = seg.kind === "unallocated";
  const p = free ? null : seg.value;
  const kind = p ? kindLabel(p.kind) : null;

  return (
    <button
      onClick={onToggle}
      aria-expanded={selected}
      className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-white/3"
    >
      <span className={`size-3 shrink-0 rounded-full ${segmentColorClass(seg)}`} />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium">
          {p?.driveLetter ? `${p.driveLetter}: ` : ""}
          {segmentLabel(seg)}
        </span>
        <span className="block text-xs text-(--md-color-text-muted)">
          {p ? `Partition ${p.number} · ${fsLabel(p.fs)}` : "Create a new partition here"}
        </span>
      </span>
      <span className="hidden flex-wrap justify-end gap-1.5 sm:flex">
        {kind && <span className="md-chip">{kind}</span>}
        {p && hasFlag(p.flags, PartitionFlags.SYSTEM) && (
          <span className="md-chip md-chip-warning">System</span>
        )}
        {p && hasFlag(p.flags, PartitionFlags.BOOT) && <span className="md-chip">Boot</span>}
        {free && <span className="md-chip md-chip-mint">Free</span>}
      </span>
      <span className="w-20 text-right text-sm text-(--md-color-text-muted) tabular-nums">
        {formatBytes(seg.value.size)}
      </span>
      <ChevronIcon open={selected} />
    </button>
  );
}

function EmptyState() {
  return (
    <div className="md-glass flex min-h-80 flex-col items-center justify-center gap-4 p-10 text-center">
      <MoonPhase fraction={0.4} size={88} />
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-semibold">Pick a disk</h2>
        <p className="max-w-sm text-sm text-(--md-color-text-muted)">
          Choose a disk on the left to see its partitions. The moon next to each disk shows how much
          of it is already allocated.
        </p>
      </div>
    </div>
  );
}
