import Foundation
import XCTest
@testable import MoonDisk

/// Decodes what the Rust helper sends, pinned in
/// src-tauri/tests/fixtures/helper-messages.json by the Rust test
/// `helper_contract` — the same file on both sides.
final class HelperContractTests: XCTestCase {
    private func message(_ key: String) throws -> Data {
        let fixture = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .appendingPathComponent("../../../src-tauri/tests/fixtures/helper-messages.json")
            .standardizedFileURL
        let object = try JSONSerialization.jsonObject(with: Data(contentsOf: fixture))
        let value = try XCTUnwrap((object as? [String: Any])?[key], "no \(key) in the fixture")
        return try JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed])
    }

    private func decode<T: Decodable>(_ type: T.Type, _ key: String) throws -> T {
        do {
            return try JSONDecoder().decode(type, from: message(key))
        } catch let error as DecodingError {
            XCTFail("\(key): \(describe(error))")
            throw error
        }
    }

    func testListDisks() throws {
        let disks = try decode([Disk].self, "listDisks")
        XCTAssertEqual(disks.count, 2)

        let internal = disks[0]
        XCTAssertTrue(internal.isSystemDisk)
        XCTAssertEqual(internal.size.value, 500_277_790_720)
        XCTAssertEqual(internal.partitions.count, 3)
        XCTAssertTrue(internal.partitions[0].isBoot)
        XCTAssertTrue(internal.partitions[0].isSystem)
        XCTAssertEqual(internal.partitions[1].mountpoints, ["/", "/System/Volumes/Data"])
        XCTAssertEqual(internal.partitions[1].title, "Partition 2")

        let stick = disks[1]
        XCTAssertTrue(stick.isUSB)
        XCTAssertEqual(stick.tableLabel, "MBR")
        XCTAssertEqual(stick.layout.count, 3)
        XCTAssertEqual(stick.partitions.map(\.title), ["ARCHISO_EFI"])
        if case .free(let start, let size) = stick.layout[2] {
            XCTAssertEqual(start, 25 * 1024 * 1024)
            XCTAssertEqual(start + size, stick.size.value)
        } else {
            XCTFail("the last segment should be free space")
        }
        XCTAssertEqual(stick.free, stick.size.value - 24 * 1024 * 1024)
    }

    func testImageInfo() throws {
        let images = try decode([ImageInfo].self, "imageInfo")
        XCTAssertTrue(images[0].copyMode.supported)
        XCTAssertEqual(images[0].copyMode.label, "ARCH_202409")
        XCTAssertTrue(images[0].hasBootSector)
        XCTAssertFalse(images[1].copyMode.supported)
        XCTAssertNotNil(images[1].copyMode.reason)
        XCTAssertEqual(images[1].size.value, 5_819_484_160)
    }

    func testFlashProgress() throws {
        let progress = try decode(FlashProgress.self, "flashProgress")
        XCTAssertEqual(progress.phase, "writing")
        XCTAssertEqual(progress.done.value, 536_870_912)
        XCTAssertNotNil(Format.timeLeft(progress))
    }

    func testDecodingErrorsNameTheField() {
        let json = Data(#"[{"id": "disk0", "flags": "BOOT"}]"#.utf8)
        struct Item: Decodable { let flags: Int }
        do {
            _ = try JSONDecoder().decode([Item].self, from: json)
            XCTFail("should not decode")
        } catch let error as DecodingError {
            XCTAssertTrue(describe(error).hasPrefix("[0].flags"), describe(error))
        } catch {
            XCTFail("\(error)")
        }
    }
}
