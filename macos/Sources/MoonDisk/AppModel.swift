import Darwin
import Foundation
import SwiftUI

enum SidebarItem: Hashable {
    case writer
    case restore
    case disk(String)
}

struct StatusMessage: Identifiable, Equatable {
    let id = UUID()
    let text: String
    let isError: Bool
}

@MainActor
final class AppModel: ObservableObject {
    enum Connection: Equatable {
        case connecting
        case ready
        case failed(String)
    }

    @Published var connection: Connection = .connecting
    @Published var disks: [Disk] = []
    @Published var selection: SidebarItem? = .writer
    @Published var status: StatusMessage?
    @Published var busy = false
    @Published var progress: FlashProgress?
    @Published var version = ""

    private var helper: HelperConnection?

    var usbDisks: [Disk] { disks.filter(\.isUSB) }

    /// Asks for administrator access and starts the helper.
    func connect() async {
        connection = .connecting
        do {
            let helper = try await HelperLauncher.launch()
            helper.onEvent = { [weak self] name, data in
                guard name == "flashProgress",
                      let progress = try? JSONDecoder().decode(FlashProgress.self, from: data)
                else { return }
                self?.progress = progress
            }
            helper.onClose = { [weak self] in
                self?.helper = nil
                self?.connection = .failed("The MoonDisk helper stopped. Grant access again to continue.")
            }
            self.helper = helper
            let info = try await helper.request(HelperInfo.self, "ping")
            version = info.version
            connection = .ready
            await refresh()
        } catch HelperError.cancelled {
            connection = .failed("MoonDisk needs administrator access to read and change disks.")
        } catch {
            connection = .failed(error.localizedDescription)
        }
    }

    private func connectedHelper() throws -> HelperConnection {
        guard let helper else { throw HelperError.message("MoonDisk isn't connected to its helper.") }
        return helper
    }

    func refresh() async {
        guard let helper else { return }
        do {
            disks = try await helper.request([Disk].self, "listDisks")
        } catch {
            show(error: "Couldn't read the disks: \(error.localizedDescription)")
        }
    }

    func show(success text: String) {
        status = StatusMessage(text: text, isError: false)
    }

    func show(error text: String) {
        status = StatusMessage(text: text, isError: true)
    }

    /// Runs one operation (always confirmed: the views ask first). Returns
    /// the error message, or nil on success.
    func perform(_ request: [String: Any]) async -> String? {
        busy = true
        defer { busy = false }
        var failure: String?
        do {
            _ = try await connectedHelper().call("execute", ["request": request, "confirmed": true])
        } catch {
            failure = error.localizedDescription
        }
        await refresh()
        return failure
    }

    /// Like `perform`, reporting the outcome in the status banner.
    func execute(_ request: [String: Any], success: String) async {
        if let failure = await perform(request) {
            show(error: failure)
        } else {
            show(success: success)
        }
    }

    private func openForHelper(_ url: URL) throws -> Int32 {
        let fd = Darwin.open(url.path, O_RDONLY)
        guard fd >= 0 else {
            throw HelperError.message("Can't open \(url.lastPathComponent) (error \(errno)).")
        }
        return fd
    }

    func imageInfo(of url: URL) async throws -> ImageInfo {
        let helper = try connectedHelper()
        let fd = try openForHelper(url)
        defer { close(fd) }
        return try await helper.request(ImageInfo.self, "imageInfo", ["path": url.path, "fd": true], file: fd)
    }

    func flash(image url: URL, diskID: String, mode: String, verify: Bool) async throws {
        let helper = try connectedHelper()
        let fd = try openForHelper(url)
        defer { close(fd) }
        progress = nil
        busy = true
        defer { busy = false }
        _ = try await helper.call(
            "flash",
            ["imagePath": url.path, "fd": true, "diskId": diskID, "mode": mode, "verify": verify],
            file: fd
        )
    }

    func cancelFlash() {
        guard let helper else { return }
        Task { _ = try? await helper.call("cancelFlash") }
    }
}

/// `NSNull` for an empty string, for optional request fields.
func nullIfEmpty(_ text: String) -> Any {
    let trimmed = text.trimmingCharacters(in: .whitespaces)
    return trimmed.isEmpty ? NSNull() : trimmed
}
