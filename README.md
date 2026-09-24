# MoonDisk

> Deine Laufwerke. Sicher im Mondlicht.
> _Your disks, safely under the moon._

MoonDisk ist ein moderner, quelloffener Partitionsmanager für Windows und
Linux.

## Funktionsumfang

- Datenträger- und Partitionsübersicht (GPT/MBR, NTFS/FAT32/exFAT/ext2-4/
  Btrfs/XFS/Swap)
- Partition erstellen, löschen, formatieren, Label ändern
- Sicherheitsdialog mit Bestätigungsphrase für kritische Aktionen
- Schutz der Systemplatte: MoonDisk erkennt den Datenträger, auf dem das
  laufende Betriebssystem installiert ist, und blockiert dort jede
  Schreiboperation

## Installation

Fertige Installationspakete für Windows (`.exe`) und Linux (`.deb`/`.rpm`)
gibt es unter [Releases](https://github.com/Luna-OS/MoonDisk/releases).

## Modi

```text
MOONDISK_MODE=mock   # Standard: eingebaute Beispiel-Datenträger, keine echten Daten
MOONDISK_MODE=real   # echte Datenträger, echte Schreiboperationen
```

**Sicherheitshinweis:** Im `real`-Modus verändert MoonDisk echte
Datenträger. Verkleinern, Verschieben, Löschen und Formatieren von
Partitionen kann zu unwiderruflichem Datenverlust führen, wenn die
Auswahl falsch ist oder ein Fehler auftritt. Erstelle vor jeder Aktion an
echten Daten ein Backup. Die Systemplatte ist geschützt, aber MoonDisk
übernimmt keine Garantie gegen Datenverlust.

## Dokumentation

Ausführliche Architektur- und Sicherheitsdokumentation in [`docs/`](docs/).

## Lizenz

Diese Repository steht aktuell unter der [MIT-Lizenz](LICENSE). Ein Wechsel
zu GPL-3.0-or-later ist vorgesehen, aber noch nicht umgesetzt.
