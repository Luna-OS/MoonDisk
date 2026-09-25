import AppKit
import SwiftUI

final class AppDelegate: NSObject, NSApplicationDelegate {
    /// Closing the window quits the app — and with it the helper.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

@main
struct MoonDiskApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup("MoonDisk") {
            RootView()
                .environmentObject(model)
                .frame(minWidth: 980, minHeight: 640)
                .preferredColorScheme(.dark)
                .tint(Theme.lavender)
        }
        .commands {
            CommandGroup(replacing: .newItem) {}
        }
    }
}

struct RootView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            switch model.connection {
            case .ready:
                MainView()
            case .connecting:
                ConnectingView()
            case .failed(let message):
                PermissionView(message: message)
            }
        }
        // Ask for administrator access right at launch.
        .task {
            if model.connection == .connecting {
                await model.connect()
            }
        }
    }
}

struct AppIcon: View {
    var size: CGFloat

    var body: some View {
        Image(nsImage: NSApplication.shared.applicationIconImage)
            .resizable()
            .interpolation(.high)
            .frame(width: size, height: size)
    }
}

struct ConnectingView: View {
    var body: some View {
        VStack(spacing: 18) {
            AppIcon(size: 96)
            ProgressView()
                .controlSize(.small)
            Text("Waiting for administrator access …")
                .foregroundStyle(Theme.muted)
        }
    }
}

struct PermissionView: View {
    @EnvironmentObject private var model: AppModel
    let message: String

    var body: some View {
        VStack(spacing: 18) {
            AppIcon(size: 110)
            Text("MoonDisk")
                .font(.system(size: 34, weight: .semibold, design: .rounded))
                .foregroundStyle(Theme.moonlight)
            Text(message)
                .multilineTextAlignment(.center)
                .foregroundStyle(Theme.muted)
                .frame(maxWidth: 440)
            Button {
                Task { await model.connect() }
            } label: {
                Label("Grant Access", systemImage: "lock.open")
                    .padding(.horizontal, 8)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            Text("macOS asks for your password once per launch. MoonDisk then runs a small helper with administrator rights — erasing, partitioning and writing drives requires them.")
                .font(.caption)
                .multilineTextAlignment(.center)
                .foregroundStyle(Theme.muted.opacity(0.8))
                .frame(maxWidth: 440)
        }
        .padding(40)
    }
}

struct MainView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        NavigationSplitView {
            Sidebar()
                .navigationSplitViewColumnWidth(min: 240, ideal: 270, max: 340)
        } detail: {
            ZStack {
                Theme.background.ignoresSafeArea()
                detail
            }
        }
        .toolbar {
            ToolbarItem {
                Button {
                    Task { await model.refresh() }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .help("Read the disks again")
                .keyboardShortcut("r", modifiers: .command)
                .disabled(model.busy)
            }
        }
        .overlay(alignment: .bottom) {
            StatusBanner()
        }
    }

    @ViewBuilder
    private var detail: some View {
        switch model.selection {
        case .writer?:
            UsbWriterView()
        case .restore?:
            RestoreView()
        case .disk(let id)?:
            if let disk = model.disks.first(where: { $0.id == id }) {
                DiskView(disk: disk)
            } else {
                EmptyDetail()
            }
        case nil:
            EmptyDetail()
        }
    }
}

struct EmptyDetail: View {
    var body: some View {
        VStack(spacing: 14) {
            MoonPhase(fraction: 0.4, size: 88)
            Text("Pick a disk")
                .font(.title3.weight(.semibold))
            Text("Choose a disk on the left to see its partitions. The moon next to each disk shows how much of it is already allocated.")
                .multilineTextAlignment(.center)
                .foregroundStyle(Theme.muted)
                .frame(maxWidth: 380)
        }
        .padding(40)
    }
}

struct Sidebar: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        List(selection: $model.selection) {
            Section("USB Tools") {
                Label("Write Image", systemImage: "externaldrive.badge.plus")
                    .tag(SidebarItem.writer)
                Label("Restore Drive", systemImage: "arrow.uturn.backward.circle")
                    .tag(SidebarItem.restore)
            }
            Section("Disks") {
                ForEach(model.disks) { disk in
                    DiskRow(disk: disk)
                        .tag(SidebarItem.disk(disk.id))
                }
                if model.disks.isEmpty {
                    Text("No disks found")
                        .foregroundStyle(Theme.muted)
                }
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .top) {
            HStack(spacing: 10) {
                AppIcon(size: 34)
                VStack(alignment: .leading, spacing: 0) {
                    Text("MoonDisk")
                        .font(.system(size: 17, weight: .semibold, design: .rounded))
                        .foregroundStyle(Theme.moonlight)
                    Text(model.version.isEmpty ? "Partition manager" : "Version \(model.version)")
                        .font(.caption)
                        .foregroundStyle(Theme.muted)
                }
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
        }
    }
}

struct DiskRow: View {
    let disk: Disk

    var body: some View {
        HStack(spacing: 10) {
            MoonPhase(fraction: disk.usedFraction, size: 26)
            VStack(alignment: .leading, spacing: 1) {
                HStack(spacing: 6) {
                    Text(disk.displayName)
                        .font(.body.weight(.semibold))
                    if disk.isSystemDisk {
                        Chip(text: "System", color: Theme.warning)
                    }
                }
                Text("\(disk.modelName) · \(Format.bytes(disk.size.value))")
                    .font(.caption)
                    .foregroundStyle(Theme.muted)
                    .lineLimit(1)
            }
        }
        .padding(.vertical, 2)
    }
}

struct StatusBanner: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        if let status = model.status {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: status.isError ? "exclamationmark.triangle.fill" : "checkmark.circle.fill")
                Text(status.text)
                    .foregroundStyle(Color.white.opacity(0.9))
                    .textSelection(.enabled)
                Spacer(minLength: 0)
                Button {
                    model.status = nil
                } label: {
                    Image(systemName: "xmark")
                }
                .buttonStyle(.plain)
            }
            .foregroundStyle(status.isError ? Theme.danger : Theme.mint)
            .padding(14)
            .frame(maxWidth: 560)
            .background(RoundedRectangle(cornerRadius: 12).fill(Theme.dusk))
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke((status.isError ? Theme.danger : Theme.mint).opacity(0.4))
            )
            .shadow(color: .black.opacity(0.5), radius: 18, y: 8)
            .padding(20)
            .task(id: status.id) {
                try? await Task.sleep(nanoseconds: 8_000_000_000)
                if model.status?.id == status.id {
                    model.status = nil
                }
            }
        }
    }
}
