import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Round 2: stamps in milliseconds, moving to another server, files, empty servers.
@MainActor
final class Round2ClientTests: FakeServerTestCase {
    func testEditsAreStampedInMilliseconds() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        c.setMe(1)
        c.setServer("ws://127.0.0.1:\(port)")
        await until("connected") { c.canEdit }
        let before = nowMs()
        c.edit { d, _ in Edits.rename(&d, task: 201, to: "Stamped", ts: nowTs()) }
        let stamp = try XCTUnwrap(SyncMeta(of: c.data!).stamps[201])
        XCTAssertGreaterThanOrEqual(stamp, before, "milliseconds, like the Mac app and the server")
        XCTAssertLessThan(stamp, before + 60_000)
        // a desktop edit made a second earlier (in ms) loses to the phone's
        var desk = c.data!
        var dm = SyncMeta(of: desk)
        desk.updateItem(at: "tasks", id: 201) { $0["name"] = "Desk" }
        dm.stamps[201] = before - 1000
        desk.set("extra.sync", dm.json)
        var mine = c.data!
        Sync.merge(&mine, [desk])
        XCTAssertEqual(mine["tasks"]!.array!.first { $0["id"] == 201 }!["name"], "Stamped")
    }

    func testMovingToAnotherServerStartsFromAFreshCopy() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make())
        var other = SampleWorkspace.make()
        other["workspace"] = "Other Team"
        other["tasks"] = [] // nothing in common
        let b = FakeServer(snapshot: other)
        let pa = try await a.start(port: 0), pb = try await b.start(port: 0)
        defer { a.stop(); b.stop() }
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        c.setMe(1)
        c.setServer("ws://127.0.0.1:\(pa)")
        await until("first server") { c.canEdit }
        // the same server typed another way keeps the copy
        XCTAssertFalse(c.isDifferentServer("127.0.0.1:\(pa)"))
        XCTAssertTrue(c.isDifferentServer("ws://127.0.0.1:\(pb)"))
        c.setServer("ws://127.0.0.1:\(pb)")
        XCTAssertNil(c.data, "the first workspace is dropped")
        XCTAssertNil(c.me, "people are numbered per workspace")
        XCTAssertEqual(c.waitingCount, 0)
        c.setMe(1)
        await until("second server's workspace") { c.view.name == "Other Team" && c.state.isConnected }
        try? await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertEqual(b.current?["tasks"]?.array?.count, 0, "nothing from the first server went to the second")
    }

    func testPicturesGoUpAndComeBackWithTheDeviceToken() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let code = server.invite(member: 3)
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        let url = "ws://127.0.0.1:\(port)"
        c.setServer(url)
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: code, email: nil, newPassword: "sam-password-1"))
        await until("signed in") { c.canEdit }
        let auth = try XCTUnwrap(c.fileAuth)
        XCTAssertTrue(auth.hasPrefix("Bearer \(c.settings.device):"))

        let picture = Data((0..<5000).map { UInt8($0 % 251) })
        let sha = try await FileAPI.upload(server: url, auth: auth, data: picture, contentType: "image/jpeg")
        XCTAssertEqual(sha, FileAPI.sha256(picture))
        XCTAssertEqual(server.storedFile(sha), picture)
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("wbfiles-\(UUID().uuidString)")
        let local = try await FileAPI.download(server: url, auth: auth, sha: sha, ext: "jpg", folder: folder)
        XCTAssertEqual(try Data(contentsOf: local), picture)
        XCTAssertEqual(local.pathExtension, "jpg")

        // without the token the server says no
        do {
            _ = try await FileAPI.download(server: url, auth: nil, sha: sha, folder: folder.appendingPathComponent("x"))
            XCTFail("expected notSignedIn")
        } catch let e as FileError { XCTAssertEqual(e, .notSignedIn) }
        do {
            _ = try await FileAPI.download(server: url, auth: auth, sha: String(repeating: "0", count: 64), folder: folder)
            XCTFail("expected missing")
        } catch let e as FileError { XCTAssertEqual(e, .missing) }
    }

    func testEmptyServerIsToldApart() async throws {
        let empty = FakeServer(snapshot: nil, requireSignIn: true)
        let full = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let pe = try await empty.start(port: 0), pf = try await full.start(port: 0)
        defer { empty.stop(); full.stop() }
        let e = await AuthAPI.hasWorkspace(server: "ws://127.0.0.1:\(pe)")
        let f = await AuthAPI.hasWorkspace(server: "ws://127.0.0.1:\(pf)")
        XCTAssertEqual(e, false)
        XCTAssertEqual(f, true)
        let none = await AuthAPI.hasWorkspace(server: "ws://127.0.0.1:1")
        XCTAssertNil(none)
    }

}
