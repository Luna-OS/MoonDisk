import Darwin
import Foundation

/// Talking to `moondisk-helper`, the Rust program that does the actual
/// disk work with administrator rights (src-tauri/src/helper.rs).
///
/// At launch the app asks for the administrator password (macOS' own
/// dialog, through `osascript`) and starts the helper as root. The helper
/// listens on a Unix socket inside a directory only this user can enter,
/// accepts this app's connection and then answers JSON requests, one per
/// line, until the app quits.

enum HelperError: LocalizedError {
    case cancelled
    case message(String)

    var errorDescription: String? {
        switch self {
        case .cancelled: return "Administrator access was not granted."
        case .message(let text): return text
        }
    }
}

@MainActor
final class HelperConnection {
    private let socket: Int32
    private let handle: FileHandle
    private var buffer = Data()
    private var nextID = 1
    private var pending: [Int: CheckedContinuation<Data, Error>] = [:]
    private var reader: Task<Void, Never>?

    /// `flashProgress` events while an image is written.
    var onEvent: (@MainActor (String, Data) -> Void)?
    /// The helper went away.
    var onClose: (@MainActor () -> Void)?

    init(socket: Int32) {
        self.socket = socket
        handle = FileHandle(fileDescriptor: socket, closeOnDealloc: true)

        // Chunks arrive on a background queue; an AsyncStream hands them
        // over to the main actor in order.
        var streamContinuation: AsyncStream<Data>.Continuation?
        let stream = AsyncStream<Data> { streamContinuation = $0 }
        let chunks = streamContinuation!
        handle.readabilityHandler = { handle in
            let data = handle.availableData
            chunks.yield(data)
            if data.isEmpty {
                handle.readabilityHandler = nil
                chunks.finish()
            }
        }
        reader = Task { [weak self] in
            for await chunk in stream {
                guard let self else { return }
                self.receive(chunk)
            }
        }
    }

    private func receive(_ data: Data) {
        if data.isEmpty {
            closed()
            return
        }
        buffer.append(data)
        while let newline = buffer.firstIndex(of: 0x0A) {
            let line = Data(buffer[buffer.startIndex..<newline])
            buffer = Data(buffer[buffer.index(after: newline)...])
            if !line.isEmpty {
                process(line: line)
            }
        }
    }

    private func process(line: Data) {
        guard let object = (try? JSONSerialization.jsonObject(with: line)) as? [String: Any] else {
            return
        }
        if let event = object["event"] as? String {
            let payload = object["data"].flatMap {
                try? JSONSerialization.data(withJSONObject: $0, options: [.fragmentsAllowed])
            }
            onEvent?(event, payload ?? Data())
            return
        }
        guard let id = object["id"] as? Int, let continuation = pending.removeValue(forKey: id) else {
            return
        }
        if let error = object["err"] as? String {
            continuation.resume(throwing: HelperError.message(error))
        } else {
            let ok = object["ok"] ?? NSNull()
            let data = (try? JSONSerialization.data(withJSONObject: ok, options: [.fragmentsAllowed]))
                ?? Data("null".utf8)
            continuation.resume(returning: data)
        }
    }

    private func closed() {
        let waiting = pending
        pending.removeAll()
        for continuation in waiting.values {
            continuation.resume(throwing: HelperError.message("The MoonDisk helper stopped."))
        }
        onClose?()
    }

    /// Sends one request and returns the JSON of its result. `file` is
    /// handed over to the helper first (see `sendFileDescriptor`).
    func call(_ command: String, _ parameters: [String: Any] = [:], file: Int32? = nil) async throws -> Data {
        let id = nextID
        nextID += 1
        var message = parameters
        message["id"] = id
        message["cmd"] = command
        var body = try JSONSerialization.data(withJSONObject: message)
        body.append(0x0A)
        if let file {
            try sendFileDescriptor(file, over: socket)
        }
        return try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Data, Error>) in
            pending[id] = continuation
            do {
                try handle.write(contentsOf: body)
            } catch {
                pending.removeValue(forKey: id)
                continuation.resume(throwing: error)
            }
        }
    }

    func request<T: Decodable>(
        _ type: T.Type,
        _ command: String,
        _ parameters: [String: Any] = [:],
        file: Int32? = nil
    ) async throws -> T {
        let data = try await call(command, parameters, file: file)
        return try JSONDecoder().decode(T.self, from: data)
    }
}

/// Hands an open file to the helper. It runs as root, but macOS' privacy
/// protection can still keep it out of the user's Downloads or Documents
/// folder — this app may read the image the user picked, so it opens it
/// and passes the open file along (`SCM_RIGHTS`), attached to a newline
/// the helper skips.
private func sendFileDescriptor(_ fd: Int32, over socket: Int32) throws {
    var byte: UInt8 = 0x0A
    // One control message carrying one int: cmsghdr (12 bytes) + fd.
    var control = [UInt8](repeating: 0, count: 16)
    control.withUnsafeMutableBytes { raw in
        raw.storeBytes(of: UInt32(16), toByteOffset: 0, as: UInt32.self)
        raw.storeBytes(of: Int32(SOL_SOCKET), toByteOffset: 4, as: Int32.self)
        raw.storeBytes(of: Int32(SCM_RIGHTS), toByteOffset: 8, as: Int32.self)
        raw.storeBytes(of: fd, toByteOffset: 12, as: Int32.self)
    }
    let sent: Int = withUnsafeMutablePointer(to: &byte) { (bytePointer: UnsafeMutablePointer<UInt8>) -> Int in
        var iov = iovec(iov_base: UnsafeMutableRawPointer(bytePointer), iov_len: 1)
        return withUnsafeMutablePointer(to: &iov) { (iovPointer: UnsafeMutablePointer<iovec>) -> Int in
            control.withUnsafeMutableBytes { (raw: UnsafeMutableRawBufferPointer) -> Int in
                var message = msghdr()
                message.msg_iov = iovPointer
                message.msg_iovlen = 1
                message.msg_control = raw.baseAddress
                message.msg_controllen = socklen_t(raw.count)
                return sendmsg(socket, &message, 0)
            }
        }
    }
    if sent < 0 {
        throw HelperError.message("Couldn't hand the image over to the helper (error \(errno)).")
    }
}

private func connectSocket(path: String) -> Int32? {
    let fd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
    guard fd >= 0 else { return nil }
    var address = sockaddr_un()
    address.sun_family = sa_family_t(AF_UNIX)
    let bytes = Array(path.utf8)
    guard bytes.count < MemoryLayout.size(ofValue: address.sun_path) else {
        close(fd)
        return nil
    }
    withUnsafeMutableBytes(of: &address.sun_path) { raw in
        raw.copyBytes(from: bytes)
    }
    let length = socklen_t(MemoryLayout<sockaddr_un>.size)
    let result = withUnsafePointer(to: &address) { pointer in
        pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(fd, $0, length) }
    }
    if result != 0 {
        close(fd)
        return nil
    }
    return fd
}

private func shellQuoted(_ text: String) -> String {
    "'" + text.replacingOccurrences(of: "'", with: "'\\''") + "'"
}

private func appleScriptQuoted(_ text: String) -> String {
    "\"" + text.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
}

enum HelperLauncher {
    static let prompt = "MoonDisk needs administrator access to read, partition and write your disks."

    /// Asks for the administrator password and starts the helper.
    @MainActor
    static func launch() async throws -> HelperConnection {
        guard let helper = Bundle.main.url(forAuxiliaryExecutable: "moondisk-helper")?.path else {
            throw HelperError.message("moondisk-helper is missing from the app bundle.")
        }
        // A downloaded app is quarantined; a quarantined helper started
        // from a shell would be blocked by Gatekeeper.
        _ = removexattr(helper, "com.apple.quarantine", 0)

        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("moondisk-\(UUID().uuidString.prefix(8))")
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        defer { try? FileManager.default.removeItem(at: directory) }
        let socketPath = directory.appendingPathComponent("h.sock").path

        let command = "\(shellQuoted(helper)) --listen \(shellQuoted(socketPath)) --owner \(getuid()) > /dev/null 2>&1 &"
        let script = "do shell script \(appleScriptQuoted(command)) with prompt \(appleScriptQuoted(prompt)) with administrator privileges"
        let (status, errorOutput) = try await runOsascript(script)
        if status != 0 {
            if errorOutput.contains("-128") { throw HelperError.cancelled }
            throw HelperError.message(errorOutput.isEmpty ? "The helper couldn't be started." : errorOutput)
        }

        for _ in 0..<150 {
            if let fd = connectSocket(path: socketPath) {
                return HelperConnection(socket: fd)
            }
            try await Task.sleep(nanoseconds: 100_000_000)
        }
        throw HelperError.message("The helper didn't start.")
    }

    private static func runOsascript(_ source: String) async throws -> (Int32, String) {
        try await Task.detached(priority: .userInitiated) { () throws -> (Int32, String) in
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
            process.arguments = ["-e", source]
            let errors = Pipe()
            process.standardError = errors
            process.standardOutput = FileHandle.nullDevice
            try process.run()
            process.waitUntilExit()
            let data = errors.fileHandleForReading.readDataToEndOfFile()
            let text = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            return (process.terminationStatus, text)
        }.value
    }
}
