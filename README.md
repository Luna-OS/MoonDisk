# MoonDisk

> Deine Laufwerke. Sicher im Mondlicht.
> _Your disks, safely under the moon._

MoonDisk ist ein moderner, quelloffener Partitionsmanager für Windows und
Linux, gebaut mit Tauri 2, Rust und React.

**Status:** frühe, aber echte Version (`0.1.0`). MoonDisk kann reale
Datenträger lesen und – im `real`-Modus, nach expliziter Bestätigung –
Partitionen erstellen, löschen, formatieren und umbenennen. Standardmäßig
startet die App im **Mock-Modus** mit simulierten Beispiel-Datenträgern; siehe
[Modi](#modi) unten.

## Funktionsumfang

- Datenträger- und Partitionsübersicht (GPT/MBR, NTFS/FAT32/exFAT/ext2-4/
  Btrfs/XFS/Swap)
- Partition erstellen, löschen, formatieren, Label ändern
- Sicherheitsdialog mit Bestätigungsphrase für kritische Aktionen
  (siehe [`docs/safety-model.md`](docs/safety-model.md))
- Schutz der Systemplatte: MoonDisk erkennt den Datenträger, auf dem das
  laufende Betriebssystem installiert ist, und blockiert dort jede
  Schreiboperation (`security::system_protection`)
- Reale Linux-Implementierung getestet gegen ein Loop-Device
  (`src-tauri/tests/linux_write_path.rs`)
- Windows-Implementierung über PowerShell-Storage-Cmdlets (strukturierte,
  feste Aufrufe – siehe [`docs/architecture.md`](docs/architecture.md) §6.1);
  mangels Windows-Testumgebung in diesem Projekt bisher nicht auf echter
  Windows-Hardware verifiziert

## Modi

```bash
MOONDISK_MODE=mock   # Standard: eingebaute Beispiel-Datenträger, keine echten Daten
MOONDISK_MODE=real   # echte Datenträger, echte Schreiboperationen
```

**Sicherheitshinweis:** Im `real`-Modus verändert MoonDisk echte
Datenträger. Verkleinern, Verschieben, Löschen und Formatieren von
Partitionen kann zu unwiderruflichem Datenverlust führen, wenn die
Auswahl falsch ist oder ein Fehler auftritt. Erstelle vor jeder Aktion an
echten Daten ein Backup. Die Systemplatte ist geschützt, aber MoonDisk
übernimmt keine Garantie gegen Datenverlust – siehe
[`docs/safety-model.md`](docs/safety-model.md).

Das dem ursprünglichen Projektauftrag beigefügte Stimmungsbild diente nur
als grobe visuelle Stilreferenz; das tatsächliche App-Icon
(`assets/branding/moondisk-icon-source.png`) ist ein separates, vom
Projektverantwortlichen freigegebenes Bild – siehe
[`docs/branding.md`](docs/branding.md) §1 für den Hintergrund dieser
Entscheidung.

## Entwicklung

Voraussetzungen:

- [Node.js](https://nodejs.org/) ≥ 20
- [Rust](https://www.rust-lang.org/tools/install) (stable, MSRV 1.77)
- [Tauri-Systemabhängigkeiten](https://v2.tauri.app/start/prerequisites/) für
  das jeweilige Betriebssystem (unter Linux u. a. `libwebkit2gtk-4.1-dev`,
  `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`)
- Für echte Schreiboperationen unter Linux: `parted`, `dosfstools`,
  `exfatprogs`, `ntfs-3g`, `e2fsprogs` (Paketnamen je Distribution)

```bash
npm install        # Frontend-Abhängigkeiten installieren
npm run dev         # Vite-Dev-Server (nur Frontend, ohne Tauri-Fenster)
npm run tauri dev   # MoonDisk als Desktop-Fenster starten
```

Qualitätsprüfungen (siehe auch [`.github/workflows/ci.yml`](.github/workflows/ci.yml)):

```bash
npm run typecheck && npm run lint && npm run format:check && npm run test && npm run build

cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
node scripts/safety-scan.mjs
```

Der reale Schreibpfad ist zusätzlich end-to-end gegen ein Loop-Device
getestet (nie gegen echte Hardware):

```bash
cd src-tauri
cargo test --test linux_write_path -- --ignored --nocapture
```

## Architektur und Sicherheitsmodell

Ausführliche Dokumentation in [`docs/`](docs/):

- [`docs/architecture.md`](docs/architecture.md) – Architektur und Technologiewahl
- [`docs/safety-model.md`](docs/safety-model.md) – Sicherheitsmodell, Bestätigungsdialog, Systemschutz
- [`docs/supported-operations.md`](docs/supported-operations.md) – unterstützte Dateisysteme und Operationen
- [`docs/branding.md`](docs/branding.md) – Branding-Konzept

## Release

`.github/workflows/release.yml` baut bei manuellem Start (oder einem
`v*`-Tag) echte Installationspakete für Windows (`.exe`, NSIS) und Linux
(`.deb`, `.rpm`) und veröffentlicht sie als GitHub Release mit
SHA-256-Prüfsummen.

## Lizenz

Diese Repository steht aktuell unter der [MIT-Lizenz](LICENSE). Ein Wechsel
zu GPL-3.0-or-later ist vorgesehen, aber noch nicht umgesetzt – siehe die
offene Entscheidung D1 in [`docs/roadmap.md`](docs/roadmap.md#6-offene-entscheidungen).
