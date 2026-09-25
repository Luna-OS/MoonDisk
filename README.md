# MoonDisk

> Deine Laufwerke. Sicher im Mondlicht.
> _Your disks, safely under the moon._

MoonDisk ist ein moderner, quelloffener Partitionsmanager für Windows und
Linux.

## Funktionsumfang

- Datenträger- und Partitionsübersicht (GPT/MBR, NTFS/FAT32/exFAT/ext2-4/
  Btrfs/XFS/Swap)
- Partition erstellen, löschen, formatieren, Label ändern
- Sicherheitsdialog für kritische Aktionen (Löschen/Formatieren)

## Installation

Fertige Installationspakete für Windows (`.exe`) und Linux (`.deb`/`.rpm`)
gibt es unter [Releases](https://github.com/Luna-OS/MoonDisk/releases).

Unter Windows fordert MoonDisk beim Start Administratorrechte an (UAC-
Abfrage) – Partitionsoperationen über PowerShell/Storage benötigen das.

**Sicherheitshinweis:** MoonDisk verändert echte Datenträger. Erstellen,
Löschen, Formatieren und Label-Änderungen von Partitionen können zu
unwiderruflichem Datenverlust führen, auch auf der System-, Boot- oder
EFI-Partition, wenn die Auswahl falsch ist oder ein Fehler auftritt.
Erstelle vor jeder Aktion an echten Daten ein Backup.

## Lizenz

Diese Repository steht aktuell unter der [MIT-Lizenz](LICENSE). Ein Wechsel
zu GPL-3.0-or-later ist vorgesehen, aber noch nicht umgesetzt.
