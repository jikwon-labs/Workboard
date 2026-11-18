import XCTest
@testable import WorkboardKit

/// The fingerprints this app works out must be the Mac app's and the
/// server's to the bit, or every copy looks edited to the other side. The
/// same fixed workspace and expected values as crates/wb-sync/tests/golden.rs.
final class GoldenTests: XCTestCase {
    /// crates/wb-sync/tests/golden, from this file's place in the repo.
    var golden: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()  // WorkboardKitTests
            .deletingLastPathComponent()  // Tests
            .deletingLastPathComponent()  // WorkboardKit
            .deletingLastPathComponent()  // ios
            .deletingLastPathComponent()  // repo
            .appendingPathComponent("crates/wb-sync/tests/golden")
    }

    func read(_ name: String) throws -> JSON {
        try JSON(parsing: Data(contentsOf: golden.appendingPathComponent(name)))
    }

    func strings<K, V>(_ m: [K: V]) -> [String: String] {
        Dictionary(uniqueKeysWithValues: m.map { ("\($0.key)", "\($0.value)") })
    }

    func expected(_ v: JSON?) -> [String: String] {
        (v?.object ?? [:]).compactMapValues { $0.string }
    }

    func testFingerprintsOfAFixedWorkspaceMatchTheMacApp() throws {
        let data = try read("workspace.json")
        let want = try read("expected.json")
        XCTAssertFalse(expected(want["fingerprints"]).isEmpty)
        XCTAssertEqual(strings(Sync.fingerprints(data)), expected(want["fingerprints"]))
        XCTAssertEqual(strings(Sync.valueHashes(data)), expected(want["value_hashes"]))
        let parts = Sync.partHashes(data)
        let wantParts = want["part_hashes"]?.object ?? [:]
        XCTAssertEqual(Set(parts.keys.map { "\($0)" }), Set(wantParts.keys))
        for (id, p) in parts {
            XCTAssertEqual(strings(p), expected(wantParts["\(id)"]), "parts of \(id)")
        }
        XCTAssertEqual(
            strings([
                "": Sync.fnv("".utf8),
                "a": Sync.fnv("a".utf8),
                "팀 👍": Sync.fnv("팀 👍".utf8),
            ]),
            expected(want["fnv"]))
        XCTAssertEqual(
            "\(Sync.activityId(task: 1001, ts: 1763000000, author: 2, comment: true, text: "made it ✓", nth: 1))",
            want["activity_id"]?.string)
        XCTAssertEqual(
            "\(Sync.boardItemId(board: 6001, kind: "strokes", sig: "0,0,0|1.5,2.25", nth: 2))",
            want["board_item_id"]?.string)
    }
}
