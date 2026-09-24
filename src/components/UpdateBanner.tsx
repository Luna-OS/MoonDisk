import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

type UpdateState =
  | { status: "idle" }
  | { status: "available"; update: Update }
  | { status: "downloading"; progress: number | null }
  | { status: "ready" }
  | { status: "error"; message: string };

/** Checks for an update once on mount and, if one is found, offers to
 * download, install, and relaunch. A failed background check is silent —
 * it must never alarm the user just because e.g. the network is
 * unreachable — but a failure once the user has clicked "install" is
 * shown, since that's an action they took. */
export function UpdateBanner() {
  const [state, setState] = useState<UpdateState>({ status: "idle" });

  useEffect(() => {
    let cancelled = false;
    check()
      .then((update) => {
        if (!cancelled && update) {
          setState({ status: "available", update });
        }
      })
      .catch(() => {
        // best-effort; see the doc comment above
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function installUpdate(update: Update) {
    setState({ status: "downloading", progress: null });
    let total = 0;
    let received = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          received = 0;
        } else if (event.event === "Progress") {
          received += event.data.chunkLength;
          setState({
            status: "downloading",
            progress: total > 0 ? Math.round((received / total) * 100) : null,
          });
        } else if (event.event === "Finished") {
          setState({ status: "ready" });
        }
      });
      // On Windows, downloadAndInstall already exits the app after
      // launching the installer, so this never runs there. On Linux/macOS
      // the app must be relaunched explicitly to run the new version.
      await relaunch();
    } catch (e) {
      setState({ status: "error", message: String(e) });
    }
  }

  if (state.status === "idle") return null;

  if (state.status === "available") {
    return (
      <div
        role="status"
        className="flex items-center justify-between gap-3 rounded-md border px-4 py-2 text-sm border-(--md-color-primary) text-(--md-color-primary)"
      >
        <span>Update verfügbar: Version {state.update.version}</span>
        <button
          onClick={() => void installUpdate(state.update)}
          className="rounded-md border px-3 py-1 text-sm border-(--md-color-primary)"
        >
          Herunterladen und installieren
        </button>
      </div>
    );
  }

  if (state.status === "downloading") {
    return (
      <div
        role="status"
        className="rounded-md border px-4 py-2 text-sm border-(--md-color-primary) text-(--md-color-primary)"
      >
        Update wird heruntergeladen{state.progress !== null ? ` … ${state.progress}%` : " …"}
      </div>
    );
  }

  if (state.status === "ready") {
    return (
      <div
        role="status"
        className="rounded-md border px-4 py-2 text-sm border-(--md-color-success) text-(--md-color-success)"
      >
        Update installiert – MoonDisk wird neu gestartet …
      </div>
    );
  }

  return (
    <div
      role="alert"
      className="rounded-md border px-4 py-2 text-sm border-(--md-color-error) text-(--md-color-error)"
    >
      Update fehlgeschlagen: {state.message}
    </div>
  );
}
