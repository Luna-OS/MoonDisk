# MoonDisk – Branding-Konzept

> **Status:** Konzept (Phase 1). Die tatsächlichen SVG- und Icon-Dateien
> entstehen in **Phase 3** nach Zustimmung zur Implementierung. Dieses
> Dokument legt das Konzept fest, an dem sich diese Dateien orientieren.

## 1. Hinweis zur Stilreferenz

Dem ursprünglichen Auftrag lag ein Bild bei (ein schlafendes Axolotl-Wesen auf
einem Halbmond, in Pastelltönen, vor nachtblauem Hintergrund mit Sternen).

**Dieses Bild ist ausschließlich eine visuelle Stilreferenz.** Es ist nicht
Teil des MoonDisk-Projekts, wird nicht als App-Icon, Logo oder Asset verwendet
und keine seiner konkreten Formen, Proportionen oder Details werden
übernommen. Aus dem Bild übernommen werden nur **allgemeine Stilideen**:
nachtblau-violetter Verlauf, Pastellpalette, Halbmond-Motiv, ruhige
Sternenstimmung, weiche runde Formen. Der MoonDisk-Axolotl wird davon
ausgehend **eigenständig neu entworfen** (andere Pose, andere Proportionen,
andere charakteristische Merkmale, siehe §3). README und `THIRD_PARTY_LICENSES.md`
werden in Phase 3/18 denselben Hinweis enthalten.

## 2. Markenkern

| Element | Festlegung |
| ------- | ---------- |
| Name | MoonDisk |
| Slogan (DE) | „MoonDisk – Deine Laufwerke. Sicher im Mondlicht.“ |
| Slogan (EN) | „MoonDisk – Your disks, safely under the moon.“ |
| Charakter | ruhig, vertrauenswürdig, präzise, ein wenig magisch – nie albern bei kritischen Themen |
| Maskottchen | eigenständiger Axolotl-Datenträgergeist (kein bestehender Charakter, keine bestehende Marke) |

## 3. Der MoonDisk-Axolotl (eigenständiger Entwurf)

Um jede Verwechslung mit der Stilreferenz auszuschließen, unterscheidet sich
der MoonDisk-Axolotl bewusst in Pose, Silhouette und charakteristischem Detail:

| Merkmal | MoonDisk-Ausprägung |
| ------- | -------------------- |
| Pose | **Schwebend, eingerollt wie ein Datenring** (nicht liegend/schlafend auf dem Mond) – Kopf und Schwanzspitze berühren sich fast zu einem Kreis, wie ein rotierender Datenträger |
| Kiemen | Sechs äußere Kiemenäste, symmetrisch, deren Spitzen zu kleinen **Fünfeck-Sternen** werden – nicht zu Flossen oder Federn |
| Bauch-Detail | Trägt kein Objekt in den Pfoten; stattdessen ziehen sich **drei konzentrische Ringsegmente** (Partitionsring) unter dem Bauch entlang, als abstrahierte Speicherscheibe |
| Gesicht | Geschlossene, geschwungene Augen (schläft/schwebt entspannt), sehr kleines Lächeln, keine Zähne, keine Wangenflecken wie bei bekannten Kawaii-Figuren |
| Farbverlauf am Körper | Lavendel (Kopf) → Hellblau (Mitte) → Mint (Schwanzspitze) – ein Farbverlauf **im Körper selbst**, nicht nur als Hintergrund |
| Dekoration | 3–5 kleine Sterne und ein **eigener, einfacher** Halbmond (dünner Ring statt vollflächiger Sichel) im Hintergrund, nie unter dem Axolotl liegend |
| Leuchteffekt | Sehr weicher äußerer Glow in Lavendel, max. 15 % Deckkraft, kein harter Rand |
| Silhouette bei 16 px | Rundlicher Tropfen mit kleinem gekrümmtem Schwanz – bleibt als einfache Form erkennbar |

Der Axolotl symbolisiert einen „Datenträgergeist“: neugierig, aufmerksam,
schützt die eingerollte Datenscheibe. Er hat **keinen Bezug zu einer
bestehenden Marke, keinem Videospiel- oder Comic-Charakter**.

### 3.1 Varianten (`moondisk-axolotl.svg` / `moondisk-mascot.svg`)

- `moondisk-axolotl.svg`: einzelnes Charaktermotiv, freistehend, für
  Leerzustände, Begrüßung, Bug-Reporter.
- `moondisk-mascot.svg`: Axolotl **mit** Halbmond und Sternen als
  Gesamtkomposition, für größere Illustrationsflächen (Startseite-Empty-State,
  Willkommensbereich).
- Zusätzliche Posen (`assets/branding/mascot/`, optional, Phase 3+): wach/
  aufmerksam für Erfolgsmeldungen, „besorgt, aber nicht ängstlich“ nur als
  **Illustration neben**, nie **anstelle von** Sicherheitswarnungen.

## 4. App-Icon (`moondisk-icon.svg`)

- Abgerundetes Quadrat (Superellipse, Eckenradius ≈ 22 % der Kantenlänge,
  angelehnt an gängige Desktop-Icon-Konventionen, keine 1:1-Kopie einer
  bestimmten Plattformvorlage).
- Hintergrund: radialer Verlauf Nachtblau (`#161233`) zu Violett (`#3B2E6B`).
- Eigener, einfacher Halbmond aus zwei überlappenden Kreisen (Sichel als
  Sichel-Differenz, nicht als plastisch schattierte 3D-Form).
- Bezug zu Partitionen: ein schmaler **Ring aus 3–4 Segmenten unterschiedlicher
  Pastellfarbe**, der wie ein Partitionsbalken um die untere Sichelrundung
  verläuft – das eigentliche Wiedererkennungsmerkmal des Icons.
- Der Axolotl erscheint im App-Icon nur **stark vereinfacht** (Kopf- und
  Schwanzform als einzelne helle Kontur innerhalb der Sichel), damit er bei
  16×16 px nicht zu Matsch wird. Bei größeren Kontexten (Startbildschirm-Card,
  About-Dialog) wird die volle Illustration (`moondisk-mascot.svg`) benutzt.
- 2–4 kleine Sterne, kein Text, keine fremden Logos.

## 5. Logo (`moondisk-logo.svg`)

- Icon-Symbol (vereinfachte Sichel mit Partitionsring, ohne Axolotl) plus
  Wortmarke „MoonDisk“ in einer offen lizenzierten, geometrischen
  Sans-Serif-Schrift (Vorschlag: **Inter** oder **Space Grotesk**, beide
  SIL Open Font License – endgültige Wahl in Phase 3, siehe
  `THIRD_PARTY_LICENSES.md`).
- Zwei Varianten: horizontal (Symbol + Schriftzug) und quadratisch
  (nur Symbol) für Kontexte ohne Platz für Text.
- Farbvarianten: „Nacht“ (für helle Flächen: farbiges Symbol, dunkler
  Schriftzug) und „Mondlicht“ (für dunkle Flächen: farbiges Symbol, cremefarbener
  Schriftzug).

## 6. Farbpalette

| Token | Wert (Vorschlag) | Verwendung |
| ----- | ----------------- | ---------- |
| `--md-night-950` | `#0E0B22` | App-Hintergrund dunkel |
| `--md-night-900` | `#161233` | Icon-Verlauf Start, Oberflächen dunkel |
| `--md-violet-700` | `#3B2E6B` | Icon-Verlauf Ende, Akzentflächen |
| `--md-lavender-400` | `#B9AEFB` | Primärfarbe (Buttons, Fokus, Axolotl-Kopf) |
| `--md-lavender-300` | `#D6CFFD` | Primär hell (Hover, helles Theme) |
| `--md-mint-400` | `#7FE3C6` | Sekundärfarbe (Erfolg, Axolotl-Schwanz) |
| `--md-sky-300` | `#9AD7F5` | Akzent (Axolotl-Mitte, Infotöne) |
| `--md-cream-100` | `#FBF7F0` | Neutrale Akzentfarbe, Text auf dunklem Grund |
| `--md-success-400` | `#7FE3C6` | Erfolg (= Mint) |
| `--md-warning-400` | `#F3C766` | Warnung, warmes Gelb |
| `--md-error-500` | `#E5626B` | Fehler, gut sichtbar, nicht grell |

Kontrast wird in Phase 3 gegen WCAG AA (4,5∶1 für Text) geprüft, insbesondere
Lavendel/Mint-Text auf Nachtblau und dunkler Text im hellen Theme.

## 7. Verwendungsregeln

- Der Axolotl erscheint **dekorativ und unterstützend** (Startseite, Leerzustände,
  Mock-Banner-Umfeld, Erfolgsmeldungen, Ladezustände, Bug-Reporter) – siehe
  Vorgabe im Startauftrag.
- Er verdeckt **nie** Warnungen, Sicherheitsdialoge, Fehlermeldungen oder
  Formularfelder. Mindestabstand zu solchen Elementen wird in Phase 3 als
  Layout-Regel festgelegt (z. B. eigener Illustrationsbereich, kein Overlay).
- In Sicherheitsdialogen (§8 im Sicherheitsmodell) erscheint der Axolotl
  **nicht**. Diese Dialoge bleiben bewusst nüchtern.
- Kein Einsatz für Humor bei Datenverlustrisiko, keine Verniedlichung von
  Fehlermeldungen.

## 8. Bezug zur Lizenzfrage der bestehenden `LICENSE`-Datei

Die aktuelle `LICENSE`-Datei ist MIT-lizenziert; das Projekt soll laut Auftrag
unter GPL-3.0-or-later stehen. Diese Entscheidung kann nur der/die
Rechteinhaber:in treffen – siehe [Roadmap, Entscheidung D1](roadmap.md#6-offene-entscheidungen).
Eigene Branding-Assets werden unabhängig davon unter derselben Lizenz wie der
restliche Quellcode veröffentlicht, sobald sie erstellt sind.
