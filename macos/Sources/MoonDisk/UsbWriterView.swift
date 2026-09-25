import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// How a finished write or restore went.
struct Outcome: Equatable {
    let success: Bool
    let title: String
    let text: String
    var note: String?
}

@MainActor
struct UsbWriterView: View {
    @EnvironmentObject private var model: AppModel
    @State private var imageURL: URL?
    @State private var image: ImageInfo?
    @State private var imageError: String?
    @State private var diskID: String?
    @State private var mode = "copy"
    @State private var verify = true
    @State private var confirming = false
    @State private var running: String?
    @State private var outcome: Outcome?

    private var disk: Disk? { model.usbDisks.first { $0.id == diskID } }

    private func problem(_ disk: Disk) -> String? {
        if disk.readOnly { return "Read-only" }
        if let image, (image.size.value + 4095) / 4096 * 4096 > disk.size.value { return "Too small" }
        return nil
    }

    private var ready: Bool {
        guard image != nil, let disk else { return false }
        return problem(disk) == nil
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                PageHeader(
                    symbol: "externaldrive.badge.plus",
                    title: "Write an image to a USB drive",
                    subtitle: "Turn an ISO or IMG file — a Linux installer, for example — into a bootable USB stick."
                )
                if let running {
                    FlashProgressView(target: running, verify: verify, copying: mode == "copy")
                } else if let outcome {
                    OutcomeView(outcome: outcome, buttonTitle: outcome.success ? "Write Another" : "Back") {
                        self.outcome = nil
                    }
                } else {
                    imageStep
                    driveStep
                    writeStep
                }
            }
            .padding(24)
        }
        .confirmationDialog(
            "Erase \(disk?.displayName ?? "the drive") and write the image?",
            isPresented: $confirming,
            titleVisibility: .visible
        ) {
            Button("Erase and Write", role: .destructive) { start() }
            Button("Cancel", role: .cancel) {}
        } message: {
            if let disk, let image {
                Text("\(image.name) will be written to \(disk.displayName) – \(disk.modelName) (\(Format.bytes(disk.size.value))). Everything on the drive — all partitions and files — will be erased.")
            }
        }
    }

    private var imageStep: some View {
        VStack(alignment: .leading, spacing: 12) {
            Eyebrow("1 · Image")
            if let image {
                HStack(spacing: 12) {
                    Image(systemName: "opticaldisc")
                        .font(.title)
                        .foregroundStyle(Theme.lavender)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(image.name)
                            .font(.body.weight(.medium))
                            .lineLimit(1)
                            .truncationMode(.middle)
                        Text(Format.bytes(image.size.value))
                            .font(.caption)
                            .foregroundStyle(Theme.muted)
                    }
                    Spacer()
                    if image.hasBootSector || image.copyMode.supported {
                        Chip(text: "Bootable from USB", color: Theme.mint)
                    }
                }
                if !image.hasBootSector && !image.copyMode.supported {
                    Label(
                        "This image has no boot sector. Windows installer ISOs look like this and won't boot when written this way — use Microsoft's Media Creation Tool for those.",
                        systemImage: "exclamationmark.triangle.fill"
                    )
                    .font(.callout)
                    .foregroundStyle(Theme.warning)
                }
            } else {
                Text("No image chosen yet. ISO, IMG, RAW and BIN files work.")
                    .foregroundStyle(Theme.muted)
            }
            if let imageError {
                Text(imageError)
                    .font(.callout)
                    .foregroundStyle(Theme.danger)
            }
            Button {
                chooseImage()
            } label: {
                Label(image == nil ? "Choose Image …" : "Choose Another …", systemImage: "folder")
            }
        }
        .card()
    }

    private var driveStep: some View {
        VStack(alignment: .leading, spacing: 12) {
            Eyebrow("2 · USB drive")
            DrivePicker(selection: $diskID, problem: problem)
        }
        .card()
    }

    private var writeStep: some View {
        VStack(alignment: .leading, spacing: 12) {
            Eyebrow("3 · Write")
            SelectableRow(
                selected: mode == "copy",
                disabled: image.map { !$0.copyMode.supported } ?? false,
                action: { mode = "copy" }
            ) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Copy files (like Rufus)")
                        .font(.body.weight(.medium))
                    Text("A normal FAT32 drive you can open on any computer. Boots on UEFI PCs.")
                        .font(.caption)
                        .foregroundStyle(Theme.muted)
                    if let image, !image.copyMode.supported, let reason = image.copyMode.reason {
                        Text("Not for this image: \(reason).")
                            .font(.caption)
                            .foregroundStyle(Theme.warning)
                    }
                }
            }
            SelectableRow(selected: mode == "raw", action: { mode = "raw" }) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Raw image (DD)")
                        .font(.body.weight(.medium))
                    Text("Byte-for-byte copy. Also boots old BIOS PCs, but the drive looks empty to macOS and Windows afterwards.")
                        .font(.caption)
                        .foregroundStyle(Theme.muted)
                }
            }
            Toggle("Verify after writing — reads everything back and compares it", isOn: $verify)
                .toggleStyle(.checkbox)
            HStack {
                summary
                Spacer()
                Button {
                    confirming = true
                } label: {
                    Label("Write Image", systemImage: "externaldrive.badge.plus")
                        .padding(.horizontal, 6)
                }
                .buttonStyle(.borderedProminent)
                .tint(Theme.danger)
                .controlSize(.large)
                .disabled(!ready || model.busy)
            }
        }
        .card()
    }

    @ViewBuilder
    private var summary: some View {
        if image == nil {
            Text("Choose an image first.").foregroundStyle(Theme.muted)
        } else if let disk {
            if let issue = problem(disk) {
                Text("\(disk.displayName) can't be used: \(issue.lowercased()).").foregroundStyle(Theme.warning)
            } else {
                Text("Everything on \(disk.displayName) (\(Format.bytes(disk.size.value))) will be erased.")
                    .foregroundStyle(Theme.warning)
            }
        } else {
            Text("Choose the USB drive to write to.").foregroundStyle(Theme.muted)
        }
    }

    private func chooseImage() {
        let panel = NSOpenPanel()
        panel.title = "Choose a disk image"
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.allowedContentTypes = ["iso", "img", "raw", "bin"].compactMap { UTType(filenameExtension: $0) }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        Task {
            do {
                let info = try await model.imageInfo(of: url)
                imageURL = url
                image = info
                imageError = nil
                // Like Rufus: copy the files whenever that boots.
                mode = info.copyMode.supported ? "copy" : "raw"
            } catch {
                imageError = error.localizedDescription
            }
        }
    }

    private func start() {
        guard let url = imageURL, let image, let disk else { return }
        running = disk.displayName
        let copying = mode == "copy"
        Task {
            do {
                try await model.flash(image: url, diskID: disk.id, mode: mode, verify: verify)
                outcome = Outcome(
                    success: true,
                    title: "All done",
                    text: "\(image.name) was written to \(disk.displayName)\(verify ? " and verified" : ""). You can eject the drive now.",
                    note: copying
                        ? "The drive is a normal FAT32 drive\(image.copyMode.label.map { " named “\($0)”" } ?? "") that opens on any computer, and it boots on UEFI PCs."
                        : "macOS usually can't read a bootable drive's system partition, so it may look empty or unreadable now — that's expected. Use Restore Drive to turn it back into a normal stick."
                )
            } catch {
                let message = error.localizedDescription
                outcome = message == "cancelled"
                    ? Outcome(success: false, title: "Cancelled", text: "\(disk.displayName) now only holds part of the image. Write it again, or restore the drive.")
                    : Outcome(success: false, title: "Writing failed", text: message)
            }
            running = nil
            await model.refresh()
        }
    }
}

struct PageHeader: View {
    let symbol: String
    let title: String
    let subtitle: String

    var body: some View {
        HStack(spacing: 14) {
            Image(systemName: symbol)
                .font(.system(size: 22))
                .foregroundStyle(Theme.moonlight)
                .frame(width: 52, height: 52)
                .background(Circle().fill(Theme.lavender.opacity(0.12)))
                .overlay(Circle().stroke(Theme.lavender.opacity(0.3)))
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.system(size: 22, weight: .semibold, design: .rounded))
                Text(subtitle)
                    .foregroundStyle(Theme.muted)
            }
        }
    }
}

/// The USB drives, one selectable row each.
struct DrivePicker: View {
    @EnvironmentObject private var model: AppModel
    @Binding var selection: String?
    let problem: @MainActor (Disk) -> String?

    var body: some View {
        if model.usbDisks.isEmpty {
            Text("No USB drive found. Plug one in and press Refresh (⌘R).")
                .foregroundStyle(Theme.muted)
        } else {
            VStack(spacing: 8) {
                ForEach(model.usbDisks) { disk in
                    let reason = problem(disk)
                    SelectableRow(
                        selected: selection == disk.id,
                        disabled: reason != nil,
                        action: { selection = disk.id }
                    ) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(disk.displayName)
                                .font(.body.weight(.medium))
                            Text(disk.modelName)
                                .font(.caption)
                                .foregroundStyle(Theme.muted)
                        }
                        Spacer()
                        if let reason {
                            Chip(text: reason, color: Theme.warning)
                        } else if disk.isSystemDisk {
                            Chip(text: "System", color: Theme.warning)
                        }
                        Text(Format.bytes(disk.size.value))
                            .monospacedDigit()
                            .foregroundStyle(Theme.muted)
                    }
                }
            }
        }
    }
}

struct FlashProgressView: View {
    @EnvironmentObject private var model: AppModel
    let target: String
    let verify: Bool
    let copying: Bool

    private func overall(_ progress: FlashProgress?) -> Double {
        guard let progress else { return 0 }
        let total = Double(progress.total.value)
        let part = total > 0 ? Double(progress.done.value) / total : 0
        switch progress.phase {
        case "writing": return verify ? part / 2 : part
        case "verifying": return 0.5 + part / 2
        case "finishing": return 1
        default: return 0
        }
    }

    private func detail(_ progress: FlashProgress) -> String {
        var parts: [String] = ["\(Format.bytes(progress.done.value)) of \(Format.bytes(progress.total.value))"]
        if progress.bytesPerSecond > 0 {
            parts.append("\(Format.bytes(UInt64(progress.bytesPerSecond)))/s")
        }
        if let left = Format.timeLeft(progress) {
            parts.append(left)
        }
        return parts.joined(separator: " · ")
    }

    private func phaseLabel(_ progress: FlashProgress?) -> String {
        switch progress?.phase {
        case "writing"?: return copying ? "Copying files" : "Writing"
        case "verifying"?: return "Verifying"
        case "finishing"?: return "Finishing"
        default: return "Preparing the drive"
        }
    }

    var body: some View {
        let progress = model.progress
        let fraction = overall(progress)
        VStack(spacing: 16) {
            MoonPhase(fraction: fraction, size: 128)
            Text("\(Int(fraction * 100))%")
                .font(.system(size: 36, weight: .semibold, design: .rounded))
                .monospacedDigit()
                .foregroundStyle(Theme.moonlight)
            Text("\(phaseLabel(progress)) … → \(target)")
                .font(.headline)
            ProgressView(value: fraction)
                .frame(maxWidth: 460)
            if let progress, progress.phase == "writing" || progress.phase == "verifying" {
                Text(detail(progress))
                .font(.caption)
                .monospacedDigit()
                .foregroundStyle(Theme.muted)
            }
            Button("Cancel") { model.cancelFlash() }
                .disabled(progress?.phase == "finishing")
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 20)
        .card()
    }
}

struct OutcomeView: View {
    let outcome: Outcome
    let buttonTitle: String
    let onDone: () -> Void

    var body: some View {
        VStack(spacing: 14) {
            if outcome.success {
                MoonPhase(fraction: 1, size: 96)
            } else {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 40))
                    .foregroundStyle(outcome.title == "Cancelled" ? Theme.warning : Theme.danger)
            }
            Text(outcome.title)
                .font(.title2.weight(.semibold))
            Text(outcome.text)
                .multilineTextAlignment(.center)
                .foregroundStyle(Theme.muted)
                .textSelection(.enabled)
                .frame(maxWidth: 480)
            if outcome.success, let note = outcome.note {
                Text(note)
                    .font(.callout)
                    .multilineTextAlignment(.center)
                    .foregroundStyle(Theme.muted)
                    .padding(12)
                    .frame(maxWidth: 480)
                    .background(RoundedRectangle(cornerRadius: 10).fill(Color.black.opacity(0.25)))
            }
            Button(buttonTitle, action: onDone)
                .controlSize(.large)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 20)
        .card()
    }
}
