import { useState } from "react";
import type { Disk, Platform } from "@/types/models";
import { ImageWriter } from "@/components/ImageWriter";
import { RestoreDrive } from "@/components/RestoreDrive";
import { EraseIcon, UsbIcon } from "@/components/icons";

type Mode = "write" | "restore";

const MODES: Record<Mode, { label: string; title: string; description: string }> = {
  write: {
    label: "Write",
    title: "Write an image to a USB drive",
    description:
      "Turn an ISO or IMG file — a Linux installer, for example — into a bootable USB stick.",
  },
  restore: {
    label: "Restore",
    title: "Restore a USB drive",
    description:
      "Erase a stick — for example one an image was written to — and turn it back into a normal, empty drive.",
  },
};

export function UsbTab({
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
  const [mode, setMode] = useState<Mode>("write");
  // A running write keeps its progress view; switching away mid-write
  // would only hide it.
  const [writing, setWriting] = useState(false);

  return (
    <section aria-label="USB writer" className="md-glass flex flex-col gap-6 p-5">
      <div className="flex flex-wrap items-center gap-4">
        <span className="flex size-14 shrink-0 items-center justify-center rounded-full bg-lavender-400/10 text-lavender-300 ring-1 ring-lavender-400/25">
          {mode === "write" ? <UsbIcon size={26} /> : <EraseIcon size={26} />}
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="text-xl font-semibold">{MODES[mode].title}</h2>
          <p className="text-sm text-(--md-color-text-muted)">{MODES[mode].description}</p>
        </div>
        <div
          role="group"
          aria-label="USB tools"
          className="flex gap-1 rounded-[0.8rem] border border-lavender-400/15 bg-night-950/40 p-1"
        >
          {(Object.keys(MODES) as Mode[]).map((m) => (
            <button
              key={m}
              aria-pressed={mode === m}
              disabled={writing && mode !== m}
              onClick={() => setMode(m)}
              className={`h-8 rounded-[0.6rem] px-3 text-sm font-medium transition-colors duration-200 disabled:opacity-40 ${
                mode === m
                  ? "bg-lavender-400/15 text-lavender-300 shadow-[inset_0_0_0_1px_rgb(185_174_251/0.35)]"
                  : "text-(--md-color-text-muted) hover:text-(--md-color-text)"
              }`}
            >
              {MODES[m].label}
            </button>
          ))}
        </div>
      </div>

      <div hidden={mode !== "write"}>
        <ImageWriter
          disks={disks}
          loading={loading}
          onFinished={onFinished}
          onRunningChange={setWriting}
        />
      </div>
      <div hidden={mode !== "restore"}>
        <RestoreDrive disks={disks} loading={loading} platform={platform} onFinished={onFinished} />
      </div>
    </section>
  );
}
