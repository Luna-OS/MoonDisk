// swift-tools-version:5.9
// The native macOS front end of MoonDisk. Disk work happens in the Rust
// helper (src-tauri/src/helper.rs), which this app starts with
// administrator rights; build the whole app with macos/build-app.sh.
import PackageDescription

let package = Package(
    name: "MoonDisk",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(name: "MoonDisk", path: "Sources/MoonDisk")
    ]
)
