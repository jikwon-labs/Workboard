import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Workspaces kept on the iPhone and switched between (Workspaces.swift).
@MainActor
final class WorkspaceSwitchTests: FakeServerTestCase {
    func other(_ name: String) -> JSON {
        var w = SampleWorkspace.make()
        w["workspace"] = .string(name)
        w["tasks"] = []
        return w
    }

    func testALeftWorkspaceIsKeptWithItsWaitingEditsAndComesBack() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make())
        let b = FakeServer(snapshot: other("Other Team"))
        let ua = try await start(a), ub = try await start(b)
        defer { a.stop(); b.stop() }
        let s = storage()
        let c = client(ua, me: 1, storage: s)
        await until("first workspace") { c.canEdit && c.state.isConnected }
        let name = c.view.name
        // an edit that hasn't reached a yet when leaving
        c.disconnect()
        XCTAssertTrue(c.edit { d, me in _ = Edits.rename(&d, task: 202, to: "Kept for later", me: me, ts: nowTs()) })
        let waiting = c.waitingCount
        XCTAssertGreaterThan(waiting, 0)

        c.setServer(ub)
        XCTAssertNil(c.data, "the other server starts from its own copy")
        XCTAssertNil(c.me)
        XCTAssertEqual(c.waitingCount, 0)
        XCTAssertEqual(c.savedWorkspaces.count, 1)
        let kept = try XCTUnwrap(c.savedWorkspaces.first)
        XCTAssertEqual(kept.server, ua)
        XCTAssertEqual(kept.name, name)
        XCTAssertEqual(kept.waiting, waiting)
        XCTAssertNotNil(kept.who)
        c.setMe(1)
        await until("second workspace") { c.view.name == "Other Team" && c.state.isConnected }
        try? await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertFalse(b.current!.serialized().contains("Kept for later"), "nothing from the first workspace went to the second")

        // still there after a relaunch
        c.flushSnapshot()
        let relaunched = SyncClient(storage: s, tokens: MemoryTokenStore())
        XCTAssertEqual(relaunched.savedWorkspaces, c.savedWorkspaces)

        // back: opens at once as it was left, then the waiting edit goes up
        c.switchTo(kept)
        XCTAssertEqual(c.settings.server, ua)
        XCTAssertEqual(c.view.name, name)
        XCTAssertEqual(c.me, 1)
        XCTAssertEqual(c.waitingCount, waiting)
        XCTAssertTrue(c.canEdit, "editable before the server answers")
        XCTAssertEqual(c.savedWorkspaces.map(\.name), ["Other Team"], "the one just left is kept in its place")
        await until("waiting edit reached the first server") { c.state.isConnected && c.waitingCount == 0 }
        XCTAssertEqual(task(a.current!, 202)["name"]?.string, "Kept for later")
    }

    func testASignedInWorkspaceOpensWithoutSigningInAgain() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let b = FakeServer(snapshot: other("Other Team"))
        let ua = try await start(a), ub = try await start(b)
        defer { a.stop(); b.stop() }
        let code = a.invite(member: 3)
        let c = device(ua)
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: code, email: nil, newPassword: "sam-password-1"))
        await until("signed in") { c.canEdit && c.state.isConnected }
        let email = c.settings.accountEmail

        c.setServer(ub)
        // a server not opened here before: its welcome says there's no sign-in
        await until("second workspace") { c.view.name == "Other Team" && c.state.isConnected && !c.needsSignIn }
        c.setMe(2)

        c.switchTo(try XCTUnwrap(c.savedWorkspaces.first))
        XCTAssertFalse(c.needsSignIn)
        XCTAssertEqual(c.me, 3)
        XCTAssertTrue(c.settings.meFromServer)
        XCTAssertEqual(c.settings.accountEmail, email)
        await until("signed in with the kept token") { c.state.isConnected }
        XCTAssertFalse(c.needsSignIn)
        XCTAssertEqual(c.me, 3)

        // and the picked one on the open server is kept too
        let kept = try XCTUnwrap(c.savedWorkspaces.first)
        XCTAssertEqual(kept.server, ub)
        c.switchTo(kept)
        XCTAssertEqual(c.me, 2)
        XCTAssertFalse(c.settings.meFromServer)
    }

    func testEachWorkspaceKeepsItsOwnTrash() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make())
        let b = FakeServer(snapshot: SampleWorkspace.make())
        let ua = try await start(a), ub = try await start(b)
        defer { a.stop(); b.stop() }
        let c = client(ua, me: 1)
        await until("first workspace") { c.canEdit }
        XCTAssertTrue(c.trashTask(203))
        c.setServer(ub)
        XCTAssertTrue(c.trash.isEmpty, "another workspace starts with its own Trash")
        c.switchTo(try XCTUnwrap(c.savedWorkspaces.first))
        XCTAssertEqual(c.trash.count, 1, "the first one's Trash comes back with it")
    }

    func testTheSameServerTypedAnotherWayIsOneWorkspace() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make())
        let b = FakeServer(snapshot: other("Other Team"))
        let pa = try await a.start(port: 0)
        let ub = try await start(b)
        defer { a.stop(); b.stop() }
        let c = client("ws://127.0.0.1:\(pa)", me: 1)
        await until("first workspace") { c.canEdit }
        c.setServer(ub)
        c.setMe(1)
        await until("second workspace") { c.view.name == "Other Team" }
        // typed without the scheme: still the kept one
        c.setServer("127.0.0.1:\(pa)")
        XCTAssertEqual(c.me, 1)
        XCTAssertNotNil(c.data)
        XCTAssertEqual(c.settings.server, "127.0.0.1:\(pa)", "the address as typed now")
        XCTAssertEqual(c.savedWorkspaces.map(\.server), [ub])
    }

    func testForgettingAKeptWorkspaceRemovesItsCopyAndSignIn() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let b = FakeServer(snapshot: other("Other Team"))
        let ua = try await start(a), ub = try await start(b)
        defer { a.stop(); b.stop() }
        let tokens = MemoryTokenStore()
        let s = storage()
        let c = device(ua, storage: s, tokens: tokens)
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: a.invite(member: 3), email: nil, newPassword: "sam-password-1"))
        await until("signed in") { c.canEdit }
        c.setServer(ub)
        let kept = try XCTUnwrap(c.savedWorkspaces.first)
        XCTAssertNotNil(tokens.token(for: ua))

        c.forget(kept)
        XCTAssertTrue(c.savedWorkspaces.isEmpty)
        XCTAssertNil(tokens.token(for: ua), "signed out there")
        XCTAssertFalse(FileManager.default.fileExists(atPath: s.kept(kept.folder).folder.path))
        XCTAssertTrue(SyncClient(storage: s, tokens: tokens).savedWorkspaces.isEmpty)

        // going there again starts over
        c.setServer(ua)
        XCTAssertNil(c.data)
        XCTAssertTrue(c.needsSignIn)
    }

    func testNothingIsKeptForAServerThatNeverGaveACopy() async throws {
        let b = FakeServer(snapshot: other("Other Team"))
        let ub = try await start(b)
        defer { b.stop() }
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        c.setServer("ws://127.0.0.1:1") // nothing listens
        c.setServer(ub)
        XCTAssertTrue(c.savedWorkspaces.isEmpty)
    }
}
