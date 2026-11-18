import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

// What many tests share: small workspaces built by hand, finding a task, and
// a base class for tests that run clients against the fake server.

/// A bare workspace with these tasks and channels and this sync record.
func ws(_ tasks: JSON, _ channels: JSON, _ meta: SyncMeta) -> JSON {
    ["workspace": "W", "spaces": [], "tasks": tasks, "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": channels, "docs": [], "extra": ["sync": meta.json]]
}

/// A bare workspace with these tasks, no channels.
func ws(_ tasks: JSON, _ meta: SyncMeta) -> JSON { ws(tasks, [], meta) }

/// A bare workspace with an empty sync record.
func ws(_ tasks: JSON, _ channels: JSON) -> JSON { ws(tasks, channels, SyncMeta()) }

/// The task with this id (it must be there).
func task(_ d: JSON, _ id: UInt64) -> JSON { d["tasks"]!.array!.first { $0["id"]?.uint == id }! }

/// Clients on the fake server, each with its own folder that is removed
/// after the test.
@MainActor
class FakeServerTestCase: XCTestCase {
    var dirs: [URL] = []

    /// The push delay of `device` clients (nil: the client's own).
    var devicePushDelay: TimeInterval? { nil }

    override func tearDown() async throws {
        for d in dirs { try? FileManager.default.removeItem(at: d) }
    }

    func folder() -> URL {
        let d = FileManager.default.temporaryDirectory.appendingPathComponent("wbkit-\(UUID().uuidString)")
        dirs.append(d)
        return d
    }

    func storage() -> Storage { Storage(folder: folder()) }

    func until(_ what: String, timeout: TimeInterval = 5, _ cond: @escaping () -> Bool) async {
        let end = Date().addingTimeInterval(timeout)
        while !cond() {
            if Date() > end { XCTFail("timed out waiting for \(what)"); return }
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
    }

    /// Starts the server on a free port and gives its address.
    func start(_ server: FakeServer) async throws -> String {
        "ws://127.0.0.1:\(try await server.start(port: 0))"
    }

    /// A client that picked member `me` (an open server), pushing quickly.
    func client(_ url: String, me: UInt64, storage s: Storage? = nil) -> SyncClient {
        let c = SyncClient(storage: s ?? storage(), tokens: MemoryTokenStore())
        c.pushDelay = 0.05
        c.maxRetryDelay = 0.3
        c.setMe(me)
        c.setServer(url)
        return c
    }

    /// A device that signs in (the server tells it who it is).
    func device(_ url: String, storage s: Storage? = nil, tokens: MemoryTokenStore = MemoryTokenStore()) -> SyncClient {
        let c = SyncClient(storage: s ?? storage(), tokens: tokens)
        if let devicePushDelay { c.pushDelay = devicePushDelay }
        c.maxRetryDelay = 0.3
        c.setServer(url)
        return c
    }
}
