import Foundation
import SwiftUI

/// Byte counts arrive as decimal strings: a large disk exceeds what a JSON
/// number can hold exactly.
struct Bytes: Decodable, Hashable {
    let value: UInt64

    init(_ value: UInt64) {
        self.value = value
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let text = try? container.decode(String.self), let number = UInt64(text) {
            value = number
        } else {
            value = try container.decode(UInt64.self)
        }
    }
}

struct Disk: Decodable, Identifiable {
    let id: String
    let displayName: String
    let vendor: String
    let model: String
    let serial: String?
    let bus: String
    let size: Bytes
    let table: String
    let readOnly: Bool
    let isSystemDisk: Bool
    let layout: [Segment]

    var modelName: String {
        if !model.isEmpty { return model }
        return vendor.isEmpty ? "Unknown model" : vendor
    }

    var isUSB: Bool { bus == "usb" }

    var partitions: [Partition] {
        layout.compactMap { segment in
            if case .partition(let partition) = segment { return partition }
            return nil
        }
    }

    var allocated: UInt64 { partitions.reduce(0) { $0 + $1.size.value } }

    var free: UInt64 { size.value > allocated ? size.value - allocated : 0 }

    var usedFraction: Double {
        size.value == 0 ? 0 : Double(allocated) / Double(size.value)
    }

    var busLabel: String {
        switch bus {
        case "usb": return "USB"
        case "nvme": return "NVMe"
        case "sata": return "SATA"
        case "virtual": return "Virtual"
        default: return "Other"
        }
    }

    var tableLabel: String {
        table == "none" ? "No table" : table.uppercased()
    }
}

struct Partition: Decodable, Identifiable {
    let id: String
    let number: Int
    let start: Bytes
    let size: Bytes
    let fs: String
    let kind: String
    let label: String?
    let flags: Int
    let mountpoints: [String]

    var title: String {
        if let label, !label.isEmpty { return label }
        return "Partition \(number)"
    }

    var isSystem: Bool { flags & 0b10 != 0 }
    var isBoot: Bool { flags & 0b1 != 0 }
}

enum Segment: Decodable, Identifiable {
    case partition(Partition)
    case free(start: UInt64, size: UInt64)

    private enum CodingKeys: String, CodingKey {
        case kind, value
    }

    private struct FreeSpace: Decodable {
        let start: Bytes
        let size: Bytes
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if try container.decode(String.self, forKey: .kind) == "partition" {
            self = .partition(try container.decode(Partition.self, forKey: .value))
        } else {
            let space = try container.decode(FreeSpace.self, forKey: .value)
            self = .free(start: space.start.value, size: space.size.value)
        }
    }

    var id: String {
        switch self {
        case .partition(let partition): return partition.id
        case .free(let start, _): return "free-\(start)"
        }
    }

    var size: UInt64 {
        switch self {
        case .partition(let partition): return partition.size.value
        case .free(_, let size): return size
        }
    }

    var title: String {
        switch self {
        case .partition(let partition): return partition.title
        case .free: return "Unallocated"
        }
    }

    var subtitle: String {
        switch self {
        case .partition(let partition):
            return "Partition \(partition.number) · \(FileSystems.label(partition.fs))"
        case .free:
            return "Create a new partition here"
        }
    }

    var color: Color {
        switch self {
        case .partition(let partition): return FileSystems.color(partition.fs, kind: partition.kind)
        case .free: return Theme.lavender.opacity(0.18)
        }
    }
}

struct CopyMode: Decodable {
    let supported: Bool
    let reason: String?
    let label: String?
}

struct ImageInfo: Decodable {
    let path: String
    let name: String
    let size: Bytes
    let hasBootSector: Bool
    let copyMode: CopyMode
}

struct FlashProgress: Decodable {
    let phase: String
    let done: Bytes
    let total: Bytes
    let bytesPerSecond: Double
}

struct HelperInfo: Decodable {
    let version: String
    let platform: String
    let root: Bool
}

enum FileSystems {
    /// What macOS can create.
    static let creatable = ["apfs", "exFat", "fat32", "hfsPlus"]

    static func label(_ fs: String) -> String {
        switch fs {
        case "ntfs": return "NTFS"
        case "fat32": return "FAT32"
        case "exFat": return "exFAT"
        case "ext2", "ext3", "ext4": return fs
        case "btrfs": return "Btrfs"
        case "xfs": return "XFS"
        case "linuxSwap": return "Swap"
        case "apfs": return "APFS"
        case "hfsPlus": return "Mac OS Extended"
        case "unformatted": return "Unformatted"
        default: return "Unknown"
        }
    }

    static func color(_ fs: String, kind: String) -> Color {
        switch kind {
        case "efi": return Color(red: 0.60, green: 0.84, blue: 0.96)
        case "microsoftReserved": return Color(red: 0.58, green: 0.64, blue: 0.72)
        case "recovery": return Color(red: 0.99, green: 0.83, blue: 0.30)
        default: break
        }
        switch fs {
        case "ntfs": return Color(red: 0.38, green: 0.65, blue: 0.98)
        case "fat32": return Color(red: 0.37, green: 0.92, blue: 0.83)
        case "exFat": return Color(red: 0.40, green: 0.91, blue: 0.98)
        case "ext2", "ext3", "ext4": return Color(red: 0.43, green: 0.91, blue: 0.72)
        case "btrfs": return Color(red: 0.99, green: 0.73, blue: 0.45)
        case "xfs": return Color(red: 0.94, green: 0.67, blue: 0.99)
        case "linuxSwap": return Color(red: 0.98, green: 0.44, blue: 0.52)
        case "apfs": return Color(red: 0.77, green: 0.71, blue: 0.99)
        case "hfsPlus": return Color(red: 0.65, green: 0.71, blue: 0.99)
        default: return Color(red: 0.63, green: 0.63, blue: 0.67)
        }
    }
}

enum Format {
    /// Binary units, like the rest of MoonDisk: 28.6 GiB.
    static func bytes(_ value: UInt64) -> String {
        let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"]
        var amount = Double(value)
        var unit = 0
        while amount >= 1024, unit < units.count - 1 {
            amount /= 1024
            unit += 1
        }
        if unit == 0 { return "\(value) B" }
        return String(format: "%.1f %@", amount, units[unit])
    }

    static func timeLeft(_ progress: FlashProgress) -> String? {
        guard progress.bytesPerSecond > 0, progress.total.value > progress.done.value else { return nil }
        let seconds = Double(progress.total.value - progress.done.value) / progress.bytesPerSecond
        if seconds < 60 { return "less than a minute left" }
        return "about \(Int((seconds / 60).rounded(.up))) min left"
    }
}
