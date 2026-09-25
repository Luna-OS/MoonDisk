import SwiftUI

/// Turns a USB drive — typically one an image was written to — back into a
/// normal, empty drive with one partition.
@MainActor
struct RestoreView: View {
    @EnvironmentObject private var model: AppModel
    @State private var diskID: String?
    @State private var filesystem = "exFat"
    @State private var name = "USB"
    @State private var confirming = false
    @State private var running: String?
    @State private var outcome: Outcome?

    private var disk: Disk? { model.usbDisks.first { $0.id == diskID } }

    private func problem(_ disk: Disk) -> String? {
        if disk.readOnly { return "Read-only" }
        if disk.isSystemDisk { return "System disk" }
        return nil
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                PageHeader(
                    symbol: "arrow.uturn.backward.circle",
                    title: "Restore a USB drive",
                    subtitle: "Erase a stick — for example one an image was written to — and turn it back into a normal, empty drive."
                )
                if let running {
                    VStack(spacing: 14) {
                        MoonPhase(fraction: 0.5, size: 96)
                        ProgressView()
                            .controlSize(.small)
                        Text("Erasing and formatting \(running) …")
                            .font(.headline)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 20)
                    .card()
                } else if let outcome {
                    OutcomeView(outcome: outcome, buttonTitle: outcome.success ? "Done" : "Back") {
                        self.outcome = nil
                    }
                } else {
                    VStack(alignment: .leading, spacing: 12) {
                        Eyebrow("1 · USB drive")
                        DrivePicker(selection: $diskID, problem: problem)
                    }
                    .card()

                    VStack(alignment: .leading, spacing: 12) {
                        Eyebrow("2 · Restore")
                        HStack(spacing: 10) {
                            Picker("File system", selection: $filesystem) {
                                ForEach(FileSystems.creatable, id: \.self) { fs in
                                    Text(FileSystems.label(fs)).tag(fs)
                                }
                            }
                            .frame(maxWidth: 260)
                            TextField("Name", text: $name)
                                .textFieldStyle(.roundedBorder)
                                .frame(maxWidth: 220)
                        }
                        Text("exFAT works on macOS, Windows and Linux and holds files of any size.")
                            .font(.caption)
                            .foregroundStyle(Theme.muted)
                        HStack {
                            if let disk {
                                Text("Everything on \(disk.displayName) (\(Format.bytes(disk.size.value))) will be erased.")
                                    .foregroundStyle(Theme.warning)
                            } else {
                                Text("Choose the USB drive to restore.")
                                    .foregroundStyle(Theme.muted)
                            }
                            Spacer()
                            Button {
                                confirming = true
                            } label: {
                                Label("Restore Drive", systemImage: "arrow.uturn.backward.circle")
                                    .padding(.horizontal, 6)
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(Theme.danger)
                            .controlSize(.large)
                            .disabled(disk.map { problem($0) != nil } ?? true || model.busy)
                        }
                    }
                    .card()
                }
            }
            .padding(24)
        }
        .confirmationDialog(
            "Erase \(disk?.displayName ?? "the drive") and restore it?",
            isPresented: $confirming,
            titleVisibility: .visible
        ) {
            Button("Erase and Restore", role: .destructive) { restore() }
            Button("Cancel", role: .cancel) {}
        } message: {
            if let disk {
                Text("\(disk.displayName) – \(disk.modelName) (\(Format.bytes(disk.size.value))) gets one empty \(FileSystems.label(filesystem)) partition. Everything on it — all partitions and files — will be erased.")
            }
        }
    }

    private func restore() {
        guard let disk else { return }
        running = disk.displayName
        let label = name.trimmingCharacters(in: .whitespaces)
        let fs = filesystem
        Task {
            let failure = await model.perform([
                "type": "eraseDisk",
                "disk": disk.id,
                "filesystem": fs,
                "label": nullIfEmpty(label),
            ])
            if let failure {
                outcome = Outcome(success: false, title: "Restoring failed", text: failure)
            } else {
                outcome = Outcome(
                    success: true,
                    title: "Drive restored",
                    text: "\(disk.displayName) is now an empty \(FileSystems.label(fs)) drive\(label.isEmpty ? "" : " named “\(label)”") that uses all of its \(Format.bytes(disk.size.value))."
                )
            }
            running = nil
        }
    }
}
