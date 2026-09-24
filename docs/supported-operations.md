# MoonDisk – Unterstützte Operationen

> **Status:** Konzept (Phase 1) · **Gilt für:** `0.1.0-alpha.x`
>
> In dieser Alpha werden **alle** Operationen ausschließlich im Mock-Modus
> **simuliert**. Echte Datenträger werden nie verändert.
>
> Die Grenzwerte in diesem Dokument sind **Mock-Regeln (vereinfachte
> Näherungen)**. Sie sind keine verbindlichen Spezifikationen der jeweiligen
> Dateisysteme und werden vor echten Operationen in späteren Versionen neu
> geprüft.

## 1. Operationen und Verfügbarkeit

| Operation | Risiko | Mock | Read-only | Production (Alpha) |
| --------- | ------ | ---- | --------- | ------------------ |
| Partition erstellen | Mittel | simuliert | – | – |
| Partition löschen | **Kritisch** | simuliert | – | – |
| Partition formatieren | **Kritisch** | simuliert | – | – |
| Partition vergrößern | Mittel | simuliert | – | – |
| Partition verkleinern | Hoch | simuliert | – | – |
| Partition verschieben | Hoch | simuliert | – | – |
| Partitionslabel ändern | Niedrig | simuliert | – | – |
| Laufwerksbuchstaben zuweisen oder entfernen (Windows-Ansicht) | Niedrig | simuliert | – | – |
| Mountpoint setzen oder entfernen (Linux-Ansicht) | Niedrig | simuliert | – | – |

„–“ bedeutet: Die Aktion ist in der UI sichtbar, aber deaktiviert, mit der
Begründung „In diesem Modus sind Änderungen nicht verfügbar“.
Welche Bestätigungen jede Risikostufe verlangt, steht in
[Sicherheitsmodell §7](safety-model.md#7-risikostufen-und-bestätigungen).

## 2. Dateisysteme

| Dateisystem | Als Ziel formatieren | Vergrößern | Verkleinern | Verschieben | Label (max.) | Mindestgröße (Mock) | Höchstgröße (Mock) | Laufwerksbuchstabe | Mountpoint |
| ----------- | :-: | :-: | :-: | :-: | ------------ | ------------------- | ------------------ | :-: | :-: |
| NTFS        | ✓ | ✓ | ✓ | ✓ | 32 Zeichen | 8 MiB | Grenze der Tabelle | ✓ | ✓ |
| FAT32       | ✓ | ✓ | ✓ ¹ | ✓ | 11 Zeichen ² | 33 MiB | 2 TiB ³ | ✓ | ✓ |
| exFAT       | ✓ | ✗ ⁴ | ✗ ⁴ | ✓ | 11 Zeichen | 1 MiB | Grenze der Tabelle | ✓ | ✓ |
| ext2        | ✓ | ✓ | ✓ | ✓ | 16 Bytes | 16 MiB | Grenze der Tabelle | ✗ ⁵ | ✓ |
| ext3        | ✓ | ✓ | ✓ | ✓ | 16 Bytes | 16 MiB | Grenze der Tabelle | ✗ ⁵ | ✓ |
| ext4        | ✓ | ✓ | ✓ | ✓ | 16 Bytes | 16 MiB | Grenze der Tabelle | ✗ ⁵ | ✓ |
| Btrfs       | ✓ | ✓ | ✓ | ✓ | 255 Bytes | 128 MiB | Grenze der Tabelle | ✗ ⁵ | ✓ |
| XFS         | ✓ | ✓ | ✗ ⁶ | ✓ | 12 Bytes | 300 MiB | Grenze der Tabelle | ✗ ⁵ | ✓ |
| Linux Swap  | ✓ | ↻ ⁷ | ↻ ⁷ | ✓ | 16 Bytes | 1 MiB | Grenze der Tabelle | ✗ | ✗ ⁸ |
| Unbekannt   | – | ✗ | ✗ | ✗ | ✗ | – | – | ✗ | ✗ |
| Nicht formatiert | – | ✓ ⁹ | ✓ ⁹ | ✓ ⁹ | ✗ | 1 MiB | Grenze der Tabelle | ✗ | ✗ |

1. FAT32 verkleinern: erlaubt, mit Warnung `W_FS_RESIZE_LIMITED`.
2. FAT32-Label: nur druckbare ASCII-Zeichen ohne `" * / : < > ? \ | + , . ; = [ ]`,
   wird in Großbuchstaben gespeichert.
3. FAT32 größer als 32 GiB: Warnung `W_FAT32_WINDOWS_32GIB`. Windows formatiert
   FAT32 mit Bordmitteln nur bis 32 GiB.
4. Für exFAT gibt es keine verbreiteten Werkzeuge zur Größenänderung. Es bleibt
   nur Sichern, Neuformatieren und Zurückspielen (`E_FS_OPERATION_UNSUPPORTED`).
5. Windows kann ext2/3/4, Btrfs und XFS ohne Zusatzsoftware nicht lesen
   (`E_FS_NOT_WINDOWS_READABLE`, Hinweis statt Zuweisung).
6. XFS lässt sich nicht verkleinern (`E_FS_OPERATION_UNSUPPORTED`).
7. Swap-Größe ändern bedeutet Swap neu anlegen (`W_SWAP_RECREATE`). Aktiver Swap
   müsste vorher deaktiviert werden (`W_UNMOUNT_REQUIRED`).
8. Swap hat keinen Mountpoint. Angezeigt wird `[SWAP]`.
9. Nur der Partitionsbereich ändert sich, es gibt kein Dateisystem.

Für alle Labels gilt: keine Steuerzeichen (U+0000–U+001F, U+007F), Unicode NFC,
leeres Label entfernt das Label.

## 3. Partitionstabellen

| Regel | GPT | MBR |
| ----- | --- | --- |
| Max. Partitionen | 128 Einträge (`E_GPT_ENTRY_LIMIT`) | 4 primäre (`E_MBR_PRIMARY_LIMIT`) ¹ |
| Nutzbarer Bereich | ab 1 MiB bis vor die Sicherungs-GPT am Ende (letzte 33 Sektoren bei 512 B) | ab 1 MiB bis Datenträgerende |
| Größengrenze | Datenträgergröße | Start und Ende ≤ 2³² Sektoren, also 2 TiB bei 512 B (`E_MBR_2TIB_LIMIT`) |
| Partitionstypen im Mock | EFI-Systempartition, Microsoft Reserved (MSR), Microsoft Basic Data, Windows Recovery, Linux Filesystem, Linux Swap, Linux Extended Boot | 0x07 (NTFS/exFAT), 0x0C (FAT32 LBA), 0x83 (Linux), 0x82 (Linux Swap), 0xEF (EFI) |

1. Erweiterte und logische MBR-Partitionen sind **nicht Teil der Alpha**
   ([Roadmap](roadmap.md#4-nichtziele-für-010-alpha1)).

## 4. Ausrichtung und Größen

- Anfang und Größe jeder Partition sind Vielfache von **1 MiB** (2048 × 512 B
  bzw. 256 × 4096 B), sonst `E_ALIGNMENT`. Die UI rundet Eingaben sichtbar auf
  1 MiB.
- Die kleinste Partition ist 1 MiB groß, außerdem gilt die Mindestgröße des
  Dateisystems (§2).
- Überlappungen sind verboten (`E_OVERLAP`).
- Freie Bereiche unter 1 MiB (Reste durch die Ausrichtung) werden nicht als
  nutzbarer Speicher angezeigt.
- Intern wird in Sektoren (LBA) gerechnet, damit keine Rundungsfehler durch
  Gleitkommazahlen entstehen. Gerundet wird nur für die Anzeige.

## 5. Regeln je Operation

| Operation | Voraussetzungen (Auswahl) | Mögliche Fehler | Hinweise |
| --------- | ------------------------- | --------------- | -------- |
| **Erstellen** | Zielbereich liegt vollständig in einem nicht zugewiesenen Bereich. Größe innerhalb der Grenzen. Tabellenlimit nicht erreicht. Datenträger nicht schreibgeschützt und nicht `Failing`. | `E_NO_FREE_SPACE`, `E_OVERLAP`, `E_ALIGNMENT`, `E_SIZE_TOO_SMALL`, `E_SIZE_TOO_LARGE`, `E_MBR_PRIMARY_LIMIT`, `E_GPT_ENTRY_LIMIT`, `E_MBR_2TIB_LIMIT` | Optional mit Dateisystem und Label in einem Schritt (intern: Erstellen und Formatieren) |
| **Löschen** | Partition nicht geschützt (§6) | `E_PROTECTED_PARTITION`, `E_DISK_READ_ONLY` | `W_UNMOUNT_REQUIRED`, wenn eingehängt |
| **Formatieren** | Partition nicht geschützt. Zieldateisystem passt zur Größe. | `E_PROTECTED_PARTITION`, `E_SIZE_TOO_SMALL`, `E_SIZE_TOO_LARGE` | Alle Daten der Partition gingen verloren |
| **Vergrößern** | **Direkt dahinter** liegt nicht zugewiesener Speicher. Das Dateisystem unterstützt Vergrößern. | `E_NO_ADJACENT_SPACE`, `E_FS_OPERATION_UNSUPPORTED` | Nur das Ende verschiebt sich. Nach links vergrößern heißt: erst Verschieben, dann Vergrößern. |
| **Verkleinern** | Neue Größe ≥ belegter Speicher + Sicherheitsreserve (max(5 %, 64 MiB) der belegten Größe). Das Dateisystem unterstützt Verkleinern. | `E_SHRINK_BELOW_USED`, `E_FS_OPERATION_UNSUPPORTED` | Reserve als Mock-Regel |
| **Verschieben** | Der neue Bereich liegt vollständig im eigenen Bereich plus angrenzendem freiem Speicher. Partition nicht geschützt. | `E_NO_ADJACENT_SPACE`, `E_PROTECTED_PARTITION`, `E_OVERLAP` | `W_LONG_RUNNING`: Bei echter Ausführung dauert das lange, und eine Unterbrechung kann zu Datenverlust führen. |
| **Label ändern** | Dateisystem bekannt, Label gültig (§2) | `E_LABEL_INVALID`, `E_FS_OPERATION_UNSUPPORTED` | |
| **Laufwerksbuchstabe** | `C`–`Z`. Nicht bereits vergeben. Dateisystem unter Windows lesbar. Nicht auf EFI, MSR, Recovery oder Swap. | `E_DRIVE_LETTER_IN_USE`, `E_DRIVE_LETTER_RESERVED` (`A`, `B`), `E_FS_NOT_WINDOWS_READABLE`, `E_PROTECTED_PARTITION` | Der Buchstabe der Systempartition (C:) ist gesperrt |
| **Mountpoint** | Absoluter Pfad, normalisiert, ohne `..`, ohne abschließenden `/` (außer `/`), höchstens 255 Bytes, eindeutig. Nicht unter `/proc`, `/sys`, `/dev`, `/run`. | `E_MOUNTPOINT_INVALID`, `E_MOUNTPOINT_IN_USE`, `E_MOUNTPOINT_RESERVED`, `E_PROTECTED_PARTITION` | `/`, `/boot` und `/boot/efi` des Systems sind gesperrt |

**Datenträgerbezogene Regeln** für alle ändernden Operationen:

- `read_only` → `E_DISK_READ_ONLY`
- Zustand `Failing` → `E_DISK_FAILING` (Empfehlung: zuerst Daten sichern)
- Zustand `Warning` → Warnung `W_DISK_HEALTH_WARNING`, Operation erlaubt

## 6. Sonderpartitionen und Systemschutz

`SystemProtection` gilt bereits im Mock und genauso später für echte
Operationen.

| Partition | Löschen | Formatieren | Verkleinern / Verschieben | Vergrößern | Label | Buchstabe / Mountpoint |
| --------- | :-: | :-: | :-: | :-: | :-: | :-: |
| Systempartition (Windows C:, Linux `/`) | ✗ | ✗ | ✗ | ✓ ¹ | ✓ | ✗ |
| EFI-Systempartition auf dem Systemdatenträger | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Boot-Partition (`/boot`) auf dem Systemdatenträger | ✗ | ✗ | ✗ | ✓ ¹ | ✓ | ✗ |
| Microsoft Reserved (MSR) | ✗ | ✗ | ✗ | ✗ | – | ✗ |
| Windows Recovery auf dem Systemdatenträger | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Aktiver Swap | ✓ ² | ✓ ² | ↻ ² | ↻ ² | ✓ | – |
| EFI-, Boot- oder Recovery-Partition auf einem Nicht-Systemdatenträger | ✓ ³ | ✓ ³ | ✓ ³ | ✓ | ✓ | ✓ |

1. Nur mit direkt angrenzendem Speicher. Hinweis `RebootRequired`/`UnmountRequired`
   für spätere echte Versionen.
2. Mit `W_UNMOUNT_REQUIRED` (Swap müsste deaktiviert werden).
3. Mit zusätzlicher Warnung `W_SPECIAL_PARTITION`.

Geschützte Aktionen erscheinen deaktiviert und nennen den Grund, zum Beispiel
„Die Systempartition kann nicht gelöscht werden (E_PROTECTED_PARTITION).“

## 7. Fehler- und Warncodes

Die Codes sind stabil. Sie dürfen in Logs und Bug-Reports erscheinen und haben
Übersetzungen in `de.json` und `en.json`.

| Code | Bedeutung |
| ---- | --------- |
| `E_VALIDATION_INPUT` | Ungültige Eingabe (Format, Länge, Zeichen) |
| `E_DISK_NOT_FOUND` / `E_PARTITION_NOT_FOUND` | Datenträger oder Partition existiert nicht (mehr) |
| `E_DISK_READ_ONLY` | Datenträger ist schreibgeschützt |
| `E_DISK_FAILING` | Datenträgerzustand kritisch, Änderungen gesperrt |
| `E_PROTECTED_PARTITION` | Geschützte System-, Boot-, EFI-, MSR- oder Recovery-Partition |
| `E_NO_FREE_SPACE` | Kein ausreichender nicht zugewiesener Speicher |
| `E_NO_ADJACENT_SPACE` | Kein direkt angrenzender freier Speicher |
| `E_OVERLAP` | Bereich überlappt eine andere Partition |
| `E_ALIGNMENT` | Anfang oder Größe nicht auf 1 MiB ausgerichtet |
| `E_SIZE_TOO_SMALL` / `E_SIZE_TOO_LARGE` | Größe außerhalb der Grenzen |
| `E_SHRINK_BELOW_USED` | Neue Größe kleiner als belegter Speicher plus Reserve |
| `E_FS_OPERATION_UNSUPPORTED` | Dateisystem unterstützt die Operation nicht |
| `E_FS_NOT_WINDOWS_READABLE` | Dateisystem unter Windows nicht ohne Zusatzsoftware lesbar |
| `E_MBR_PRIMARY_LIMIT` / `E_GPT_ENTRY_LIMIT` / `E_MBR_2TIB_LIMIT` | Grenzen der Partitionstabelle |
| `E_LABEL_INVALID` | Label zu lang oder mit unzulässigen Zeichen |
| `E_DRIVE_LETTER_IN_USE` / `E_DRIVE_LETTER_RESERVED` | Laufwerksbuchstabe vergeben oder reserviert |
| `E_MOUNTPOINT_INVALID` / `E_MOUNTPOINT_IN_USE` / `E_MOUNTPOINT_RESERVED` | Mountpoint ungültig, vergeben oder reserviert |
| `E_PLAN_CONFLICT` | Operation widerspricht einer früheren Operation im Plan |
| `E_PLAN_TOO_LARGE` / `E_PLAN_EMPTY` | Plan zu groß oder leer |
| `E_STATE_CHANGED` | Zustand seit der Planung geändert, erneute Prüfung nötig |
| `E_DRY_RUN_REQUIRED` / `E_DRY_RUN_EXPIRED` | Kein oder abgelaufenes Dry-Run-Ticket |
| `E_CONFIRMATION_MISSING` | Bestätigungsphrase oder Backup-Bestätigung fehlt |
| `E_MODE_NOT_ALLOWED` | Aktion im aktuellen Modus nicht erlaubt |
| `E_WRITES_DISABLED` | Echte Schreiboperationen sind in dieser Version deaktiviert |
| `E_SIM_IO_ERROR` | Simulierter Ein-/Ausgabefehler (Fehlerinjektion) |
| `E_SIM_DEVICE_REMOVED` | Datenträger wurde simuliert getrennt |
| `W_DISK_HEALTH_WARNING` | Datenträger meldet einen (simulierten) Warnzustand |
| `W_UNMOUNT_REQUIRED` | Volume müsste getrennt bzw. Swap deaktiviert werden |
| `W_LONG_RUNNING` | Operation würde lange dauern |
| `W_SWAP_RECREATE` | Swap wird neu angelegt |
| `W_FS_RESIZE_LIMITED` | Größenänderung dieses Dateisystems ist eingeschränkt |
| `W_FAT32_WINDOWS_32GIB` | FAT32 über 32 GiB kann Windows nicht selbst formatieren |
| `W_SPECIAL_PARTITION` | Sonderpartition auf einem Nicht-Systemdatenträger |
