# MoonDisk – CI/CD, Alpha-Test- und Bugfix-Prozess

> **Status:** Konzept (Phase 1). Die eigentlichen Workflow-Dateien
> (`.github/workflows/*.yml`) entstehen in **Phase 17**.
>
> Verwandte Dokumente: [Roadmap](roadmap.md) · [Sicherheitsmodell](safety-model.md) ·
> [Alpha-Testplan](alpha-testing.md) · [Alpha-Release-Checkliste](alpha-release-checklist.md)

## 1. CI-Workflow (`ci.yml`)

Läuft bei jedem Push und jedem Pull Request.

| Job | Plattform(en) | Schritte |
| --- | -------------- | -------- |
| `frontend` | `ubuntu-latest` | `npm ci` → `tsc --noEmit` → `eslint` → `prettier --check` → `vitest run` (inkl. `vitest-axe`-Checks) → `npm run build` |
| `backend` | `ubuntu-latest`, `windows-latest` | `cargo fmt --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test` |
| `tauri-build` | `ubuntu-latest`, `windows-latest` | System-Abhängigkeiten installieren (Linux: WebKitGTK 4.1, libayatana-appindicator; Windows: WebView2 ist auf `windows-latest` vorinstalliert) → `cargo tauri build --debug` als reiner Build-Check, keine Artefakt-Veröffentlichung |
| `i18n-check` | `ubuntu-latest` | Skript vergleicht Schlüsselmengen von `de.json` und `en.json`, schlägt bei Abweichung fehl |
| `safety-scan` | `ubuntu-latest` | siehe §3, **Pflicht für Merge** |
| `deny` | `ubuntu-latest` | `cargo deny check` (Lizenzen, Advisories, verbotene Crates), `npm audit --omit=dev --audit-level=high` als nicht blockierender Hinweis-Job zu Beginn, blockierend sobald das Projekt stabil läuft (siehe Vorgabe „wenn stabil umsetzbar“) |

Alle Jobs laufen parallel, wo keine Abhängigkeit besteht. Frontend-Typen, die
aus Rust generiert werden (`ts-rs`), werden im `backend`-Job erzeugt und im
Job `types-check` (Teil von `frontend` oder eigener kleiner Job) gegen den
eingecheckten Stand verglichen, damit generierte Dateien nicht veralten.

Alle Actions werden **per Commit-SHA gepinnt** (nicht per Tag), um
Lieferkettenangriffe zu erschweren (siehe [Sicherheitsmodell, T9](safety-model.md#2-bedrohungs--und-fehlermodell)).

## 2. Teststrategie

| Ebene | Werkzeug | Abdeckt |
| ----- | -------- | ------- |
| Rust unit | `cargo test` | Datenmodelle, Größenberechnungen, Validator-Regeln, Planer, Dry-Run, Sanitizer, Redaction |
| Rust integration | `cargo test --test *` | Vollständige Workflows (laden → planen → Dry-Run → anwenden), Nachweis „keine Prozess-/Gerätezugriffe im Mock-Modus“ ([Sicherheitsmodell §4.6](safety-model.md#46-nachweis-mock-berührt-nie-echte-datenträger)), Modus-Umschaltung, GitHub-Client gegen einen Mock-Transport (nie echtes Netzwerk, siehe §6) |
| Frontend unit/component | Vitest + Testing Library | Komponenten, Formularvalidierung, i18n-Interpolation |
| Frontend a11y | `vitest-axe` | Automatisierte Barrierefreiheitsprüfung für Kernkomponenten (Dialoge, Partitionsbalken, Formulare) |
| Property-based (Rust) | `proptest` (neue Dev-Abhängigkeit, in Phase 4 zu bestätigen) | Ausrichtung, Überlappungserkennung, Rundtrip von Größenberechnungen |
| Manuell | siehe [alpha-testing.md](alpha-testing.md) | UI-Fluss, Tastaturnavigation, Zusammenspiel beider Plattform-Webviews |

Ziel ist keine feste Prozentzahl an Codeabdeckung, sondern: **jede
sicherheitsrelevante Regel aus [Sicherheitsmodell](safety-model.md) und
[Unterstützte Operationen](supported-operations.md) hat mindestens einen
Test.**

## 3. Safety-Scan

Ein eigenständiges Skript (`scripts/safety-scan.mjs`, Node, ohne zusätzliche
Abhängigkeit) durchsucht den Quellcode bei jedem CI-Lauf und lässt den Build
scheitern, wenn eines der folgenden Muster außerhalb ausdrücklich erlaubter
Ausnahmen (`platform/mock/`, Tests, Dokumentation) auftaucht:

- Aufrufe von `std::process::Command` außerhalb von `platform/process.rs`
- Die Tauri-Plugins `shell`, `fs` (schreibend), `http`, `updater`, `process`
  in `Cargo.toml`, `capabilities/*.json` oder `tauri.conf.json`
- Literale schreibende Befehle: `diskpart`, `format`, `mkfs`, `dd`, `parted`
  (mit schreibenden Unterbefehlen wie `mkpart`, `rm`, `resizepart`), `fdisk`
  (interaktive Schreibmodi), `wipefs`, `sgdisk --delete`/`--new`, PowerShell-
  Cmdlets wie `Clear-Disk`, `New-Partition`, `Format-Volume`,
  `Remove-Partition`, `Set-Partition`
- `REAL_WRITES_ENABLED` außerhalb seiner einzigen Definition in
  `security/write_barrier.rs` oder mit einem anderen Wert als `false`
- Rohe Gerätepfade (`/dev/sd*`, `/dev/nvme*`, `\\.\PhysicalDrive*`) außerhalb
  von `platform/*_readonly.rs`, `platform/mock/`, Tests und Dokumentation
- `unwrap()`/`expect()` in `commands/` (erzwingt saubere Fehlerbehandlung an
  der IPC-Grenze)
- `dangerouslySetInnerHTML` im Frontend
- Potenzielle Geheimnis-Literale (grobe Muster für Tokens) außerhalb von
  Testfixtures mit offensichtlichen Platzhalterwerten

Der Scan ist bewusst zusätzlich zu Code-Review und Typsystem vorhanden
(Schicht L5 der Verteidigung, siehe
[Sicherheitsmodell §3](safety-model.md#3-alpha-schreibsperre)) und **muss vor
jedem Merge grün sein**.

## 4. Branch-Schutz und Review

- `main` ist geschützt: Pull Request erforderlich, CI muss grün sein, keine
  Force-Pushes.
- `CODEOWNERS` verlangt Review für `src-tauri/src/security/`,
  `src-tauri/src/platform/`, `src-tauri/src/operations/`,
  `src-tauri/src/github/`, `capabilities/` und `.github/workflows/`.
- PR-Vorlage (`.github/pull_request_template.md`) enthält eine Checkliste,
  unter anderem: „Diese Änderung fügt keinen Schreibzugriff auf echte
  Datenträger hinzu“, „Neue Abhängigkeiten wurden geprüft (Lizenz, Wartungsstand)“.

## 5. Alpha-Release-Workflow (`release-alpha.yml`)

- **Manueller Start** über `workflow_dispatch` mit Eingabe der Versionsnummer.
- Vorbedingungen (Job schlägt sonst fehl, siehe [Alpha-Release-Checkliste](alpha-release-checklist.md)):
  - Version entspricht dem Muster `0.x.y-alpha.z` (Regex-Prüfung).
  - `CHANGELOG.md` enthält einen Abschnitt `## [<Version>]`.
  - Alle CI-Prüfungen aus `ci.yml` für den Ziel-Commit sind erfolgreich.
- Schritte:
  1. Linux-Build (`.deb`, `.rpm`; AppImage optional, siehe [Architektur §11.2](architecture.md#112-linux-ubuntu-debian-fedora-arch)) auf `ubuntu-22.04` für breite glibc-Kompatibilität.
  2. Windows-Build (NSIS-Installer `.exe`) auf `windows-latest`; MSI folgt später, siehe [Architektur, A4](architecture.md#23-erkenntnisse-und-konflikte-aus-der-analyse).
  3. SHA-256-Prüfsummen für jedes Artefakt erzeugen (`checksums.txt`).
  4. Artefakte als Workflow-Artefakte hochladen.
  5. Einen **GitHub-Release-Entwurf** (`draft: true`) erzeugen, Titel
     `MoonDisk <Version>`, Release Notes aus einer Vorlage plus dem
     passenden Changelog-Abschnitt.
  6. Release Notes enthalten zwingend: Alpha-/Teststatus, den Hinweis „Diese
     Version unterstützt keine echten Schreiboperationen auf Datenträgern“,
     getestete Windows-/Linux-Umgebungen, einen Link zu
     [alpha-testing.md](alpha-testing.md).
- **Kein Schritt veröffentlicht automatisch einen finalen Stable-Release.**
  Der Entwurf verlangt eine manuelle Freigabe durch die Maintainer
  (letzter Punkt der [Alpha-Release-Checkliste](alpha-release-checklist.md)).

## 6. GitHub-API-Tests (Bug-Reporter)

- Alle Tests in `src-tauri/src/github/tests.rs` verwenden einen **Mock-Transport**
  (ein `Transport`-Trait mit einer Testimplementierung, die feste Antworten
  liefert). Es gibt in CI keinen echten Netzwerkzugriff auf `api.github.com`.
- Abgedeckte Fälle: erfolgreiche Issue-Erstellung, HTTP 401/403 (ungültiges
  oder unzureichendes Token), HTTP 404 (Repository/Issues nicht gefunden oder
  deaktiviert), HTTP 422 mit ungültigen Labels (→ Wiederholung ohne optionale
  Labels), Zeitüberschreitung, unklare Serverfehler (5xx).
- Ein CI-Schritt stellt sicher, dass `MOONDISK_GITHUB_TOKEN` und vergleichbare
  Variablen in Test-Workflows **nicht gesetzt** sind, damit ein
  Implementierungsfehler nicht versehentlich echte Aufrufe auslösen kann.

## 7. Bug-Triage und Priorität

Label- und Prioritätsschema wie im Auftrag vorgegeben
(`bug`, `critical`, `security`, `data-safety`, `alpha`, `ui`, `ux`, `windows`,
`linux`, `mock-mode`, `operations-planner`, `i18n`, `documentation`,
`good first issue`, `help wanted`; Prioritäten `priority: p0`…`p3`). Verwaltet
über `.github/labels.yml` und einen `sync-labels`-Workflow, damit alle
Repository-Labels reproduzierbar und mit fester Farbe angelegt sind.

Bearbeitungsreihenfolge, wie im Auftrag festgelegt: Sicherheitsprobleme →
Risiken echter Datenveränderung → Abstürze → falsche Berechnungen/ungültige
Pläne → Fehler im Mock-Modus → Fehler in Sicherheitsdialogen →
Windows-/Linux-Probleme → Lokalisierung → Barrierefreiheit → UI/Design →
Dokumentation.

Ablauf pro Bug (Issue → Labels/Priorität → Reproduktion im Mock-Modus →
möglichst erst ein fehlschlagender Test → kleinste sichere Lösung → Tests/
Linting/Build → Regressionstest → Changelog/Doku → Commit `fix: …` mit
Issue-Bezug → Schließen erst nach grünem Test und Build) folgt exakt der
Vorgabe im Startauftrag und wird nicht dupliziert dokumentiert.

## 8. Versionsschema für die Alpha-Serie

`0.1.0-alpha.1`, `0.1.0-alpha.2`, `0.1.0-alpha.3`, … Jede Version bekommt einen
Changelog-Abschnitt mit behobenen Fehlern, bekannten Problemen, dem
Alpha-Hinweis und der Bestätigung, dass keine echten Schreiboperationen aktiv
sind (siehe [CHANGELOG.md](../CHANGELOG.md)). Ein Wechsel zu `0.2.0-alpha.1`
oder `0.1.0-beta.1` setzt eine bewusste Entscheidung der Maintainer voraus,
keinen automatischen Versionssprung.
