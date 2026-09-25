import { useState, type ReactNode } from "react";
import type { Disk, FlashPhase, FlashProgress, ImageInfo, WriteMode } from "@/types/models";
import { cancelFlash, flashImage, onFlashProgress, selectImageFile } from "@/lib/ipc";
import { formatBytes } from "@/lib/format";
import { overallFraction, unsuitableReason } from "@/lib/flash";
import { diskModel } from "@/lib/segments";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { MoonPhase } from "@/components/MoonPhase";
import { AlertIcon, DiscIcon, UsbIcon } from "@/components/icons";
import { DrivePicker, ResultView, Step, type Result } from "@/components/UsbParts";

const PHASE_LABELS: Record<FlashPhase, string> = {
  preparing: "Preparing the drive",
  writing: "Writing",
  verifying: "Verifying",
  finishing: "Finishing",
};

function timeLeft(p: FlashProgress): string | null {
  if (p.bytesPerSecond <= 0) return null;
  const secs = (Number(p.total) - Number(p.done)) / p.bytesPerSecond;
  if (secs < 60) return "less than a minute left";
  return `about ${Math.ceil(secs / 60)} min left`;
}

export function ImageWriter({
  disks,
  loading,
  onFinished,
  onRunningChange,
}: {
  disks: Disk[];
  loading: boolean;
  /** Called after every write attempt, so the disk list can be reloaded. */
  onFinished: () => void;
  onRunningChange: (running: boolean) => void;
}) {
  const [image, setImage] = useState<ImageInfo | null>(null);
  const [picking, setPicking] = useState(false);
  const [pickError, setPickError] = useState<string | null>(null);
  const [diskId, setDiskId] = useState<string | null>(null);
  const [mode, setMode] = useState<WriteMode>("copy");
  const [verify, setVerify] = useState(true);
  const [confirming, setConfirming] = useState(false);
  /** Set while a write runs; names are captured up front so a disk list
   * refresh mid-write can't blank the progress view. */
  const [job, setJob] = useState<{ imageName: string; diskName: string } | null>(null);
  const [progress, setProgress] = useState<FlashProgress | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [result, setResult] = useState<Result | null>(null);

  const usbDisks = disks.filter((d) => d.bus === "usb");
  const disk = usbDisks.find((d) => d.id === diskId) ?? null;
  const diskProblem = disk ? unsuitableReason(disk, image) : null;
  const ready = image !== null && disk !== null && diskProblem === null;

  async function pickImage() {
    setPicking(true);
    setPickError(null);
    try {
      const picked = await selectImageFile();
      if (picked) {
        setImage(picked);
        // Like Rufus: copy the files whenever that boots, else write raw.
        setMode(picked.copyMode.supported ? "copy" : "raw");
      }
    } catch (e) {
      setPickError(String(e));
    } finally {
      setPicking(false);
    }
  }

  async function write() {
    if (!image || !disk) return;
    setConfirming(false);
    const target = disk.displayName;
    setJob({ imageName: image.name, diskName: target });
    onRunningChange(true);
    setProgress(null);
    setCancelling(false);
    setResult(null);
    let unlisten: (() => void) | null = null;
    try {
      unlisten = await onFlashProgress(setProgress);
      await flashImage({ imagePath: image.path, diskId: disk.id, mode, verify });
      setResult({
        kind: "done",
        title: "All done",
        text: `${image.name} was written to ${target}${verify ? " and verified" : ""}. You can remove the drive now.`,
      });
    } catch (e) {
      const message = String(e);
      setResult(
        message === "cancelled"
          ? {
              kind: "cancelled",
              title: "Cancelled",
              text: `${target} now only holds part of the image. Write it again, or restore the drive.`,
            }
          : { kind: "error", title: "Writing failed", text: message },
      );
    } finally {
      unlisten?.();
      setJob(null);
      onRunningChange(false);
      onFinished();
    }
  }

  async function cancel() {
    setCancelling(true);
    try {
      await cancelFlash();
    } catch {
      setCancelling(false);
    }
  }

  let summary: ReactNode;
  if (!image) summary = "Choose an image first.";
  else if (!disk) summary = "Choose the USB drive to write to.";
  else if (diskProblem)
    summary = `${disk.displayName} can't be used: ${diskProblem.toLowerCase()}.`;
  else
    summary = (
      <span className="text-warning-400">
        Everything on {disk.displayName} ({formatBytes(disk.size)}) will be erased.
      </span>
    );

  return (
    <>
      {job ? (
        <ProgressView
          progress={progress}
          verify={verify}
          copying={mode === "copy"}
          imageName={job.imageName}
          diskName={job.diskName}
          cancelling={cancelling}
          onCancel={() => void cancel()}
        />
      ) : result ? (
        <ResultView
          result={result}
          note={
            mode === "copy" ? (
              <>
                The drive is now a normal FAT32 drive
                {image?.copyMode.label ? ` named “${image.copyMode.label}”` : ""} that you can open
                on any computer. It boots on UEFI PCs — every PC from the last decade; for older
                BIOS-only PCs, write the image in raw mode instead.
              </>
            ) : (
              <>
                Your computer usually can't read a bootable drive's system partition, so the drive
                may look mostly empty or unallocated now (e.g. in Disk Management) — that's
                expected. To use it as a normal USB stick again, switch to <strong>Restore</strong>{" "}
                above.
              </>
            )
          }
          backLabel="Write another"
          onBack={() => setResult(null)}
        />
      ) : (
        <div className="grid gap-4 lg:grid-cols-3">
          <Step n={1} title="Image">
            {image ? (
              <div className="flex items-center gap-3">
                <span className="flex size-11 shrink-0 items-center justify-center rounded-full bg-lavender-400/10 text-lavender-300">
                  <DiscIcon size={22} />
                </span>
                <div className="min-w-0">
                  <p className="truncate font-medium" title={image.path}>
                    {image.name}
                  </p>
                  <p className="text-xs text-(--md-color-text-muted) tabular-nums">
                    {formatBytes(image.size)}
                  </p>
                </div>
              </div>
            ) : (
              <p className="text-sm text-(--md-color-text-muted)">
                No image chosen yet. ISO, IMG, RAW and BIN files work.
              </p>
            )}
            {image &&
              (image.hasBootSector || image.copyMode.supported ? (
                <span className="md-chip md-chip-mint self-start">Bootable from USB</span>
              ) : (
                <p className="flex gap-2 rounded-lg bg-warning-400/8 p-3 text-xs leading-relaxed text-warning-400 ring-1 ring-warning-400/30">
                  <span className="mt-px">
                    <AlertIcon />
                  </span>
                  <span>
                    This image has no boot sector. Windows installer ISOs look like this and won't
                    boot when written this way — use Microsoft's Media Creation Tool for those.
                    Linux ISOs and .img files are fine.
                  </span>
                </p>
              ))}
            <button
              onClick={() => void pickImage()}
              disabled={picking}
              className="md-btn md-btn-ghost mt-auto self-start"
            >
              <DiscIcon />
              {image ? "Choose another" : "Choose image"}
            </button>
            {pickError && (
              <p role="alert" className="text-xs text-[#f28b92]">
                {pickError}
              </p>
            )}
          </Step>

          <Step n={2} title="USB drive">
            <DrivePicker
              name="write-drive"
              disks={usbDisks}
              loading={loading}
              selectedId={diskId}
              problem={(d) => unsuitableReason(d, image)}
              onSelect={setDiskId}
            />
          </Step>

          <Step n={3} title="Write">
            <div role="radiogroup" aria-label="Write mode" className="flex flex-col gap-2">
              <ModeOption
                value="copy"
                mode={mode}
                title="Copy files (like Rufus)"
                disabledReason={image && !image.copyMode.supported ? image.copyMode.reason : null}
                onSelect={setMode}
              >
                A normal FAT32 drive you can open anywhere. Boots on UEFI PCs.
              </ModeOption>
              <ModeOption value="raw" mode={mode} title="Raw image (DD)" onSelect={setMode}>
                Byte-for-byte copy. Also boots old BIOS PCs, but looks empty to Windows.
              </ModeOption>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={verify}
                onChange={(e) => setVerify(e.target.checked)}
                className="size-4 accent-lavender-400"
              />
              Verify after writing
            </label>
            <p className="-mt-1 text-xs text-(--md-color-text-muted)">
              Reads the drive back and compares it with the image. Takes longer, but catches faulty
              sticks.
            </p>
            <p className="text-sm">{summary}</p>
            <button
              onClick={() => setConfirming(true)}
              disabled={!ready}
              className="md-btn md-btn-danger-solid mt-auto"
            >
              <UsbIcon />
              Write image
            </button>
          </Step>
        </div>
      )}

      <ConfirmDialog
        open={confirming}
        title="Erase drive and write image"
        targetSummary={
          image && disk
            ? [
                `Image: ${image.name} (${formatBytes(image.size)})`,
                `Drive: ${disk.displayName} – ${diskModel(disk)} (${formatBytes(disk.size)})`,
                `Mode:  ${
                  mode === "copy"
                    ? `copy files to FAT32${image.copyMode.label ? ` “${image.copyMode.label}”` : ""}`
                    : "raw image"
                }`,
              ].join("\n")
            : ""
        }
        consequence="Everything on this drive — all partitions and files — will be erased."
        confirmLabel="Erase and write"
        onCancel={() => setConfirming(false)}
        onConfirm={() => void write()}
      />
    </>
  );
}

function ModeOption({
  value,
  mode,
  title,
  disabledReason = null,
  onSelect,
  children,
}: {
  value: WriteMode;
  mode: WriteMode;
  title: string;
  disabledReason?: string | null;
  onSelect: (mode: WriteMode) => void;
  children: ReactNode;
}) {
  const selected = value === mode;
  const disabled = disabledReason !== null;
  return (
    <label
      className={`flex gap-3 rounded-xl border p-3 transition-colors duration-200 ${
        disabled
          ? "cursor-not-allowed border-lavender-400/10"
          : selected
            ? "cursor-pointer border-lavender-300/60 bg-lavender-400/8"
            : "cursor-pointer border-lavender-400/15 hover:border-lavender-400/35"
      }`}
    >
      <input
        type="radio"
        name="write-mode"
        value={value}
        checked={selected && !disabled}
        disabled={disabled}
        onChange={() => onSelect(value)}
        className="mt-0.5 size-4 shrink-0 accent-lavender-400"
      />
      <span className="flex min-w-0 flex-col gap-0.5">
        <span className={`text-sm font-medium ${disabled ? "opacity-50" : ""}`}>{title}</span>
        <span className={`text-xs text-(--md-color-text-muted) ${disabled ? "opacity-50" : ""}`}>
          {children}
        </span>
        {disabled && (
          <span className="text-xs text-warning-400">Not for this image: {disabledReason}.</span>
        )}
      </span>
    </label>
  );
}

function ProgressView({
  progress,
  verify,
  copying,
  imageName,
  diskName,
  cancelling,
  onCancel,
}: {
  progress: FlashProgress | null;
  verify: boolean;
  copying: boolean;
  imageName: string;
  diskName: string;
  cancelling: boolean;
  onCancel: () => void;
}) {
  const fraction = overallFraction(progress, verify);
  const percent = Math.floor(fraction * 100);
  const phase = progress?.phase ?? "preparing";
  const moving = progress && (phase === "writing" || phase === "verifying");
  const eta = moving ? timeLeft(progress) : null;

  return (
    <div className="flex flex-col items-center gap-5 py-4 text-center">
      <MoonPhase fraction={fraction} size={128} />
      <div className="flex flex-col gap-1">
        <p className="text-3xl font-semibold text-lavender-300 tabular-nums">{percent}%</p>
        <p className="text-sm font-medium">
          {copying && phase === "writing" ? "Copying files" : PHASE_LABELS[phase]} …
          {verify && moving && (
            <span className="ml-1.5 text-(--md-color-text-muted)">
              (step {phase === "writing" ? 1 : 2} of 2)
            </span>
          )}
        </p>
        <p className="max-w-md truncate text-xs text-(--md-color-text-muted)">
          {imageName} → {diskName}
        </p>
      </div>
      <div
        role="progressbar"
        aria-label="Write progress"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
        className="h-2 w-full max-w-xl overflow-hidden rounded-full bg-night-950/70 ring-1 ring-lavender-400/15"
      >
        <div
          className="h-full rounded-full bg-linear-to-r from-lavender-300 to-[#a597f5] shadow-[0_0_12px_rgb(185_174_251/0.6)] transition-[width] duration-300"
          style={{ width: `${percent}%` }}
        />
      </div>
      <p className="min-h-4 text-xs text-(--md-color-text-muted) tabular-nums">
        {moving &&
          [
            `${formatBytes(progress.done)} of ${formatBytes(progress.total)}`,
            progress.bytesPerSecond > 0 &&
              `${formatBytes(Math.round(progress.bytesPerSecond).toString())}/s`,
            eta,
          ]
            .filter(Boolean)
            .join(" · ")}
      </p>
      <button
        onClick={onCancel}
        disabled={cancelling || phase === "finishing"}
        className="md-btn md-btn-ghost"
      >
        {cancelling ? "Cancelling …" : "Cancel"}
      </button>
    </div>
  );
}
