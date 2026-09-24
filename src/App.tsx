import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

/**
 * Phase-2 placeholder shell.
 *
 * This only proves that the Tauri + React + Tailwind stack starts up and
 * can round-trip a single IPC call (`app.getVersion`). The real home screen
 * (mock-mode banner, disk overview, …) is built in later roadmap phases —
 * see docs/roadmap.md. Nothing here should read as a finished feature.
 *
 * Styling intentionally avoids the `style` prop / inline styles so the
 * strict CSP in tauri.conf.json (no `style-src 'unsafe-inline'`) can stay
 * that way — see docs/architecture.md §6.2. Tailwind's `bg-(--token)`
 * syntax binds a utility class straight to a CSS custom property from
 * src/styles/tokens.css instead.
 */
function App() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getVersion()
      .then((v) => {
        if (!cancelled) setVersion(v);
      })
      .catch(() => {
        // In a plain browser preview (npm run dev without `tauri dev`) the
        // Tauri IPC bridge does not exist yet — that is expected, not an
        // error worth surfacing to a user.
        if (!cancelled) setVersion(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4 px-6 text-center">
      <header>
        <h1 className="text-3xl font-semibold text-lavender-400">MoonDisk</h1>
        <p className="mt-2 text-(--md-color-text-muted)">Deine Laufwerke. Sicher im Mondlicht.</p>
      </header>
      <main className="rounded-lg border px-6 py-4 text-sm text-(--md-color-text-muted) bg-(--md-color-surface) border-(--md-color-surface-border)">
        <p>Projektgrundgerüst (Phase 2). Es sind noch keine Datenträgerfunktionen vorhanden.</p>
        <p className="mt-1">{version ? `MoonDisk ${version}` : "Version wird geladen …"}</p>
      </main>
    </div>
  );
}

export default App;
