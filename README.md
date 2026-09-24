# MoonDisk

> Deine Laufwerke. Sicher im Mondlicht.
> _Your disks, safely under the moon._

MoonDisk ist ein geplanter moderner, sicherer, vollständig lokaler und
quelloffener Partitionsmanager für Windows und Linux.

**Status: früher Aufbau (Phase 2 von 22 des Alpha-Plans).** Es gibt noch
keine Datenträger- oder Partitionsfunktionen — weder echte noch simulierte.
Aktuell steht nur das leere Projektgrundgerüst (Tauri 2 + React/TypeScript +
Rust). Die vollständige Projektbeschreibung, das Sicherheitsmodell und der
Entwicklungsplan bis zur ersten Testversion `0.1.0-alpha.1` stehen in
[`docs/`](docs/), insbesondere:

- [`docs/architecture.md`](docs/architecture.md) – Architektur und Technologiewahl
- [`docs/safety-model.md`](docs/safety-model.md) – Sicherheitsmodell und Mock-Modus
- [`docs/roadmap.md`](docs/roadmap.md) – Entwicklungsplan und Alpha-Nichtziele
- [`docs/branding.md`](docs/branding.md) – Branding- und Maskottchen-Konzept

**Sicherheitshinweis:** MoonDisk führt in dieser frühen Phase keinerlei
Operationen auf echten Datenträgern aus – weder lesend noch schreibend. Die
geplante Alpha-Version wird ausschließlich mit simulierten (Mock-)
Datenträgern arbeiten; siehe [`docs/safety-model.md`](docs/safety-model.md).

Das dem ursprünglichen Projektauftrag beigefügte Stimmungsbild (ein
schlafendes Axolotl-Wesen auf einem Mond) diente nur als grobe visuelle
Stilreferenz für Farbwelt und Atmosphäre. Es ist **nicht** Teil von MoonDisk
und wird nirgends als Asset verwendet – siehe
[`docs/branding.md` §1](docs/branding.md#1-hinweis-zur-stilreferenz).

## Entwicklung

Voraussetzungen:

- [Node.js](https://nodejs.org/) ≥ 20
- [Rust](https://www.rust-lang.org/tools/install) (stable, MSRV 1.77)
- [Tauri-Systemabhängigkeiten](https://v2.tauri.app/start/prerequisites/) für
  das jeweilige Betriebssystem (unter Linux u. a. `libwebkit2gtk-4.1-dev`,
  `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`)

```bash
npm install        # Frontend-Abhängigkeiten installieren
npm run dev         # Vite-Dev-Server (nur Frontend, ohne Tauri-Fenster)
npm run tauri dev   # MoonDisk als Desktop-Fenster starten
```

Qualitätsprüfungen (siehe auch die CI-Pläne in
[`docs/release-process.md`](docs/release-process.md)):

```bash
npm run typecheck && npm run lint && npm run format:check && npm run test && npm run build

cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
```

## Lizenz

Diese Repository steht aktuell unter der [MIT-Lizenz](LICENSE). Ein Wechsel
zu GPL-3.0-or-later ist vorgesehen, aber noch nicht umgesetzt – siehe die
offene Entscheidung D1 in [`docs/roadmap.md`](docs/roadmap.md#6-offene-entscheidungen).
