import SwiftUI

private let mib: UInt64 = 1024 * 1024

struct DiskView: View {
    @EnvironmentObject private var model: AppModel
    let disk: Disk
    @State private var selectedSegment: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                header

                HStack(spacing: 10) {
                    StatTile(label: "Capacity", value: Format.bytes(disk.size.value))
                    StatTile(label: "Allocated", value: Format.bytes(disk.allocated))
                    StatTile(label: "Free", value: Format.bytes(disk.free))
                    StatTile(label: "Partitions", value: "\(disk.partitions.count)")
                }

                VStack(alignment: .leading, spacing: 8) {
                    Eyebrow("Layout")
                    PartitionBar(disk: disk, selection: $selectedSegment)
                }

                VStack(spacing: 8) {
                    ForEach(disk.layout) { segment in
                        SegmentRow(segment: segment, selected: segment.id == selectedSegment)
                            .onTapGesture {
                                selectedSegment = segment.id == selectedSegment ? nil : segment.id
                            }
                    }
                }

                if let segment = disk.layout.first(where: { $0.id == selectedSegment }) {
                    switch segment {
                    case .partition(let partition):
                        PartitionActions(disk: disk, partition: partition)
                            .id(partition.id)
                    case .free(let start, let size):
                        CreatePartitionForm(disk: disk, start: start, size: size)
                            .id(segment.id)
                    }
                }
            }
            .padding(24)
        }
        .onChange(of: disk.id) { _ in
            selectedSegment = nil
        }
    }

    private var header: some View {
        HStack(spacing: 16) {
            MoonPhase(fraction: disk.usedFraction, size: 60)
            VStack(alignment: .leading, spacing: 4) {
                Text(disk.displayName)
                    .font(.system(size: 24, weight: .semibold, design: .rounded))
                Text([disk.modelName, disk.serial].compactMap { $0 }.joined(separator: " · "))
                    .foregroundStyle(Theme.muted)
                    .lineLimit(1)
                HStack(spacing: 6) {
                    Chip(text: disk.busLabel)
                    Chip(text: disk.tableLabel)
                    if disk.isSystemDisk {
                        Chip(text: "System", color: Theme.warning)
                    }
                    if disk.readOnly {
                        Chip(text: "Read-only", color: Theme.warning)
                    }
                }
            }
            Spacer()
        }
    }
}

struct PartitionBar: View {
    let disk: Disk
    @Binding var selection: String?

    var body: some View {
        GeometryReader { geometry in
            let total = max(Double(disk.size.value), 1)
            let spacing: CGFloat = 3
            let gaps = spacing * CGFloat(max(disk.layout.count - 1, 0))
            let usable = max(geometry.size.width - gaps, 0)
            HStack(spacing: spacing) {
                ForEach(disk.layout) { segment in
                    RoundedRectangle(cornerRadius: 6)
                        .fill(segment.color)
                        .overlay(
                            RoundedRectangle(cornerRadius: 6)
                                .stroke(selection == segment.id ? Color.white : Color.clear, lineWidth: 2)
                        )
                        .frame(width: max(6, usable * CGFloat(Double(segment.size) / total)))
                        .onTapGesture { selection = segment.id }
                        .help("\(segment.title) · \(Format.bytes(segment.size))")
                }
            }
        }
        .frame(height: 42)
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

struct SegmentRow: View {
    let segment: Segment
    let selected: Bool

    var body: some View {
        HStack(spacing: 12) {
            Circle()
                .fill(segment.color)
                .frame(width: 10, height: 10)
            VStack(alignment: .leading, spacing: 2) {
                Text(segment.title)
                    .font(.body.weight(.medium))
                Text(segment.subtitle)
                    .font(.caption)
                    .foregroundStyle(Theme.muted)
            }
            Spacer()
            if case .partition(let partition) = segment {
                if partition.kind == "efi" { Chip(text: "EFI") }
                if partition.isSystem { Chip(text: "System", color: Theme.warning) }
                if !partition.mountpoints.isEmpty { Chip(text: "Mounted", color: Theme.mint) }
            } else {
                Chip(text: "Free", color: Theme.mint)
            }
            Text(Format.bytes(segment.size))
                .monospacedDigit()
                .foregroundStyle(Theme.muted)
                .frame(width: 90, alignment: .trailing)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(selected ? Theme.lavender.opacity(0.10) : Color.white.opacity(0.03))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(selected ? Theme.lavender.opacity(0.5) : Theme.lavender.opacity(0.12))
        )
        .contentShape(Rectangle())
    }
}

@MainActor
struct PartitionActions: View {
    @EnvironmentObject private var model: AppModel
    let disk: Disk
    let partition: Partition
    @State private var label = ""
    @State private var formatAs = "exFat"
    @State private var confirmFormat = false
    @State private var confirmDelete = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Eyebrow("\(partition.title) · \(FileSystems.label(partition.fs))")

            HStack(spacing: 10) {
                TextField("Name", text: $label)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 280)
                Button("Rename") {
                    Task {
                        await model.execute(
                            ["type": "setLabel", "partition": partition.id, "label": label],
                            success: "\(partition.title) was renamed to “\(label)”."
                        )
                    }
                }
                .disabled(model.busy || label.isEmpty || label == (partition.label ?? ""))
            }

            VStack(alignment: .leading, spacing: 10) {
                Eyebrow("Danger zone")
                HStack(spacing: 10) {
                    Picker("Format as", selection: $formatAs) {
                        ForEach(FileSystems.creatable, id: \.self) { fs in
                            Text(FileSystems.label(fs)).tag(fs)
                        }
                    }
                    .frame(maxWidth: 260)
                    Button("Format …") { confirmFormat = true }
                        .disabled(model.busy)
                    Spacer()
                    Button(role: .destructive) {
                        confirmDelete = true
                    } label: {
                        Label("Delete …", systemImage: "trash")
                    }
                    .disabled(model.busy)
                }
            }
            .padding(14)
            .background(RoundedRectangle(cornerRadius: 12).fill(Theme.danger.opacity(0.06)))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.danger.opacity(0.3)))
        }
        .card()
        .onAppear { label = partition.label ?? "" }
        .confirmationDialog(
            "Format \(partition.title) as \(FileSystems.label(formatAs))?",
            isPresented: $confirmFormat,
            titleVisibility: .visible
        ) {
            Button("Format", role: .destructive) {
                Task {
                    await model.execute(
                        [
                            "type": "formatPartition",
                            "partition": partition.id,
                            "filesystem": formatAs,
                            "label": nullIfEmpty(label),
                        ],
                        success: "\(partition.title) was formatted as \(FileSystems.label(formatAs))."
                    )
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("All data on this partition (\(Format.bytes(partition.size.value)) on \(disk.displayName)) will be permanently erased.")
        }
        .confirmationDialog(
            "Delete \(partition.title)?",
            isPresented: $confirmDelete,
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive) {
                Task {
                    await model.execute(
                        ["type": "deletePartition", "partition": partition.id],
                        success: "\(partition.title) was deleted."
                    )
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("The partition and all data on it will be permanently removed.")
        }
    }
}

@MainActor
struct CreatePartitionForm: View {
    @EnvironmentObject private var model: AppModel
    let disk: Disk
    let start: UInt64
    let size: UInt64
    @State private var filesystem = "apfs"
    @State private var label = ""
    @State private var sizeMiB: Double = 1

    /// Partitions start and end on 1 MiB boundaries.
    private var alignedStart: UInt64 { (start + mib - 1) / mib * mib }

    private var maxMiB: UInt64 {
        let end = (start + size) / mib * mib
        return end > alignedStart ? (end - alignedStart) / mib : 0
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Eyebrow("New partition")
            if maxMiB < 2 {
                Text("This free space is too small for a partition.")
                    .foregroundStyle(Theme.muted)
            } else {
                HStack(spacing: 10) {
                    Picker("File system", selection: $filesystem) {
                        ForEach(FileSystems.creatable, id: \.self) { fs in
                            Text(FileSystems.label(fs)).tag(fs)
                        }
                    }
                    .frame(maxWidth: 260)
                    TextField("Name (optional)", text: $label)
                        .textFieldStyle(.roundedBorder)
                        .frame(maxWidth: 240)
                }
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text("Size")
                            .foregroundStyle(Theme.muted)
                        Spacer()
                        Text(Format.bytes(UInt64(sizeMiB) * mib))
                            .font(.title3.weight(.semibold))
                            .monospacedDigit()
                            .foregroundStyle(Theme.moonlight)
                        Text("of \(Format.bytes(maxMiB * mib))")
                            .foregroundStyle(Theme.muted)
                    }
                    Slider(value: $sizeMiB, in: 1...Double(maxMiB), step: 1)
                }
                HStack {
                    Text("On macOS a new partition goes directly after the one in front of this space.")
                        .font(.caption)
                        .foregroundStyle(Theme.muted)
                    Spacer()
                    Button {
                        Task {
                            await model.execute(
                                [
                                    "type": "createPartition",
                                    "disk": disk.id,
                                    "start": String(alignedStart),
                                    "size": String(UInt64(sizeMiB) * mib),
                                    "filesystem": filesystem,
                                    "label": nullIfEmpty(label),
                                    "driveLetter": NSNull(),
                                ],
                                success: "The partition was created."
                            )
                        }
                    } label: {
                        Label("Create Partition", systemImage: "plus")
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(model.busy)
                }
            }
        }
        .card()
        .onAppear { sizeMiB = Double(max(maxMiB, 1)) }
    }
}
