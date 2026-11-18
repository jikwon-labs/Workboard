import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Round 4: Trash that leaves with its person, notifications only for news,
/// admin answers matched to their requests, files sent again after a restore.
@MainActor
final class Round4ClientTests: FakeServerTestCase {
    override var devicePushDelay: TimeInterval? { 0.05 }

    func signedIn(_ server: FakeServer, _ url: String, member: UInt64, storage s: Storage? = nil) async -> SyncClient {
        let c = device(url, storage: s)
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: server.invite(member: member), email: nil, newPassword: nil))
        await until("signed in as \(member)") { c.canEdit && c.state.isConnected && c.me == member }
        return c
    }

    func testTrashLeavesWithTheOneWhoSignsOut() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let s = storage()
        let c = await signedIn(server, url, member: 3, storage: s)
        XCTAssertTrue(c.trashTask(202))
        XCTAssertEqual(c.settings.trashOwner, 3)

        // signing out: the next person doesn't see it, now or after a relaunch
        c.signOut()
        XCTAssertTrue(c.trash.isEmpty)
        XCTAssertTrue(SyncClient(storage: s, tokens: MemoryTokenStore()).trash.isEmpty)

        // signed out by the server (access removed): gone too
        c.signIn(.invite(code: server.invite(member: 3), email: nil, newPassword: nil))
        await until("back in") { c.canEdit && c.state.isConnected }
        XCTAssertTrue(c.trashDoc(401))
        XCTAssertEqual(c.trash.count, 1)
        server.revoke(member: 3)
        await until("signed out") { c.state == .signIn }
        XCTAssertTrue(c.trash.isEmpty)
        XCTAssertTrue(SyncClient(storage: s, tokens: MemoryTokenStore()).trash.isEmpty)

        // someone else signing in on this device (without signing out first)
        c.signIn(.invite(code: server.invite(member: 3), email: nil, newPassword: nil))
        await until("in as Sam") { c.canEdit && c.me == 3 && c.state.isConnected }
        XCTAssertTrue(c.trashTask(203))
        c.signIn(.invite(code: server.invite(member: 2), email: "alex@example.test", newPassword: nil))
        await until("in as Alex") { c.me == 2 && c.state.isConnected }
        XCTAssertTrue(c.trash.isEmpty, "Alex doesn't get Sam's Trash")
    }

    func testTrashGoesWhenAnotherPersonIsPickedByHand() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let c = device(url)
        c.setMe(1)
        await until("connected") { c.canEdit && c.state.isConnected }
        XCTAssertTrue(c.trashTask(202))
        c.setMe(1)
        XCTAssertEqual(c.trash.count, 1, "the same person keeps it")
        c.setMe(2)
        XCTAssertTrue(c.trash.isEmpty)
        // a Trash file from an older build belongs to whoever is here now
        let s = storage()
        s.save(trash: [TrashItem(id: 1, kind: "Task", title: "old", deleted: 1, payload: "[]")])
        var set = DeviceSettings()
        set.me = 5
        s.save(set)
        let old = SyncClient(storage: s, tokens: MemoryTokenStore())
        XCTAssertEqual(old.settings.trashOwner, 5)
        XCTAssertEqual(old.trash.count, 1)
    }

    /// The server's push rule (push::notes): restored and old messages aren't news.
    func testOnlyMessagesJustWrittenNotify() {
        let now: Int64 = 1_800_000_000
        func msg(_ id: UInt64, _ ts: Int64, replies: [JSON] = []) -> JSON {
            ["id": .id(id), "author": 2, "text": .string("m\(id)"), "ts": .int(ts), "replies": .array(replies)]
        }
        let before: JSON = ["channels": [["id": 1, "messages": [msg(10, now - 30)]]], "extra": ["sync": SyncMeta().json]]
        var meta = SyncMeta()
        meta.revived = [13: now * 1000, 2: now * 1000]
        let after: JSON = [
            "channels": [
                ["id": 1, "messages": [
                    msg(10, now - 30, replies: [msg(14, now - 5), msg(15, now - 7200)]),
                    msg(11, now - 20),
                    msg(12, now - 3600), // from a computer that was offline
                    msg(13, now - 10), // brought back from Trash
                    msg(16, (now - 60) * 1000), // a stamp in milliseconds
                    ["id": 17, "author": 1, "text": "mine", "ts": .int(now)],
                ]],
                // a conversation restored now (or newly shared with me)
                ["id": 2, "messages": [msg(20, now - 400), msg(21, now - 60)]],
                ["id": 3, "messages": [msg(30, now - 86_000)]],
            ],
            "extra": ["sync": meta.json],
        ]
        let fresh = SyncClient.newMessages(before: before, after: after, me: 1, now: now)
        XCTAssertEqual(fresh.map(\.id), [14, 11, 16, 21])
        XCTAssertEqual(fresh.first?.parent, 10)
    }

    func testAdminAnswersAreMatchedToTheirRequests() {
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        let before = [Person(member: 1, role: .owner), Person(member: 3, role: .member, devices: 1, account: true)]
        let after = [Person(member: 1, role: .owner), Person(member: 3, role: .admin, devices: 1, account: true)]

        c.asks = [.setRole(member: 3, role: .admin), .accessList]
        XCTAssertEqual(c.adminPending, 2)
        // the oldest request wants a list: the next list answers it, whatever
        // it shows (another admin's change can already be in it)
        c.receiveForTests(.access(people: before, role: .owner))
        XCTAssertEqual(c.asks, [.accessList])
        XCTAssertEqual(c.people, before, "the list is taken")
        // refusals of pushed changes aren't answers
        c.receiveForTests(.error(code: "denied", message: FakeServer.putBackMessage, putBack: true))
        c.receiveForTests(.error(code: "denied", message: SyncClient.updateNeeded))
        XCTAssertEqual(c.asks, [.accessList])
        XCTAssertNil(c.adminError)
        XCTAssertEqual(c.notice, SyncClient.updateNeeded)
        c.receiveForTests(.access(people: after, role: .owner))
        XCTAssertEqual(c.adminPending, 0)
        // lists sent unasked after someone's change, with nothing waiting
        c.receiveForTests(.access(people: after, role: .owner))
        XCTAssertEqual(c.adminPending, 0)
        XCTAssertEqual(c.people, after)
        // answers come in order: a list can't answer a request waiting
        // behind one that wants something else
        c.asks = [.backups, .revoke(member: 3)]
        c.receiveForTests(.access(people: after, role: .owner))
        XCTAssertEqual(c.asks, [.backups, .revoke(member: 3)])
        c.receiveForTests(.backups([]))
        XCTAssertEqual(c.asks, [.revoke(member: 3)])
        c.receiveForTests(.access(people: after, role: .owner))
        XCTAssertEqual(c.adminPending, 0)

        // a refusal belongs to the oldest request; with none waiting it's a notice
        c.clearNotice()
        c.asks = [.revoke(member: 1), .backups]
        c.receiveForTests(.error(code: "denied", message: "The owner's access can't be removed."))
        XCTAssertEqual(c.adminError, "The owner's access can't be removed.")
        XCTAssertEqual(c.asks, [.backups])
        c.receiveForTests(.backups([]))
        XCTAssertEqual(c.adminPending, 0)
        c.receiveForTests(.error(code: "denied", message: "Something else"))
        XCTAssertEqual(c.notice, "Something else")
        XCTAssertEqual(c.adminError, "The owner's access can't be removed.", "not mixed up with the admin request")

        // a password change and an admin request waiting: each gets its own answer
        c.asks = [.password, .invite(member: 2)]
        c.receiveForTests(.error(code: "denied", message: "Your current password isn't right."))
        if case .failed(let m) = c.accountRequest { XCTAssertEqual(m, "Your current password isn't right.") } else { XCTFail("password refusal not shown") }
        XCTAssertEqual(c.asks, [.invite(member: 2)])

        // an answer only the asker gets settles the ones before it
        c.asks = [.revoke(member: 3), .invite(member: 2)]
        c.receiveForTests(.invite(member: 2, code: "AAAA-BBBB-CCCC", emailed: nil, mailError: nil))
        XCTAssertEqual(c.adminPending, 0)
        XCTAssertEqual(c.lastInvite?.code, "AAAA-BBBB-CCCC")
    }

    func testAdminRequestsOnTheServerSettleAndRefusalsLandRight() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let owner = await signedIn(server, url, member: 1)
        let alex = await signedIn(server, url, member: 2)
        owner.requestAccessList()
        await until("listed") { owner.people != nil && owner.adminPending == 0 }
        // Alex (a plain member) gets access lists after the owner's changes
        owner.setRole(member: 3, role: .admin)
        await until("role changed") { owner.adminPending == 0 && owner.people?.first { $0.member == 3 }?.role == .admin }
        alex.requestBackups()
        await until("alex turned down") { alex.adminError != nil }
        XCTAssertEqual(alex.adminError, "Only the owner can see and restore backups.")
        XCTAssertEqual(alex.adminPending, 0)
        XCTAssertNil(alex.notice)
        owner.revoke(member: 1)
        await until("owner's refusal") { owner.adminError != nil }
        XCTAssertEqual(owner.adminError, "The owner's access can't be removed.")
        XCTAssertEqual(owner.adminPending, 0)
    }

    func testRestoreSendsTheFilesBackToTheServer() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let cache = folder()
        let picture = Data("a picture".utf8), video = Data("a video".utf8), still = Data("its still".utf8)
        let (ps, vs, ss) = (FileAPI.sha256(picture), FileAPI.sha256(video), FileAPI.sha256(still))
        // kept here when they were sent; the server cleaned them up since
        FileAPI.keep(picture, sha: ps, ext: "jpg", folder: cache)
        FileAPI.keep(video, sha: vs, ext: "mov", folder: cache)
        FileAPI.keep(still, sha: ss, ext: "jpg", folder: cache)
        XCTAssertEqual(FileAPI.localCopy(vs, folder: cache)?.lastPathComponent, vs + ".mov")
        XCTAssertNil(FileAPI.localCopy(String(repeating: "a", count: 64), folder: cache))

        let c = device(url)
        c.fileCache = cache
        c.setMe(1)
        await until("connected") { c.canEdit && c.state.isConnected }
        c.edit { d, _ in
            d.updateItem(at: "tasks", id: 202) { t in
                t["attachments"] = [
                    FileAPI.attachment(sha: ps, name: "p.jpg", isVideo: false, bytes: picture.count, w: 1, h: 1, secs: 0),
                    FileAPI.attachment(sha: vs, name: "v.mov", isVideo: true, bytes: video.count, w: 1, h: 1, secs: 1, thumb: ss),
                ]
            }
        }
        XCTAssertTrue(c.trashTask(202))
        XCTAssertEqual(FileAPI.files(inPayload: c.trash[0].payload), [ps, vs, ss])
        XCTAssertNil(server.storedFile(ps))

        XCTAssertEqual(c.restoreFromTrash(c.trash[0].id), .restored)
        let sent = await c.lastReupload?.value
        XCTAssertEqual(sent, 3)
        XCTAssertEqual(server.storedFile(ps), picture)
        XCTAssertEqual(server.storedFile(vs), video)
        XCTAssertEqual(server.storedFile(ss), still)
        XCTAssertEqual(c.settings.reuploads, [])
        let size = try await FileAPI.remoteSize(server: url, auth: nil, sha: vs)
        XCTAssertEqual(size, video.count)

        // the server has them now: asked again, nothing goes up
        XCTAssertTrue(c.trashTask(202))
        XCTAssertEqual(c.restoreFromTrash(c.trash[0].id), .restored)
        let again = await c.lastReupload?.value
        XCTAssertEqual(again, 0)
    }

    func testARestoreMadeOfflineSendsItsFilesAfterTheNextConnect() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        let url = "ws://127.0.0.1:\(port)"
        let cache = folder()
        let picture = Data("offline picture".utf8)
        let sha = FileAPI.sha256(picture)
        FileAPI.keep(picture, sha: sha, ext: "jpg", folder: cache)
        let c = device(url)
        c.fileCache = cache
        c.setMe(1)
        await until("connected") { c.canEdit && c.state.isConnected }
        c.edit { d, _ in
            d.updateItem(at: "tasks", id: 202) { t in
                t["attachments"] = [FileAPI.attachment(sha: sha, name: "p.jpg", isVideo: false, bytes: picture.count, w: 1, h: 1, secs: 0)]
            }
        }
        XCTAssertTrue(c.trashTask(202))
        await until("deleted on the server") { !(server.current?.serialized().contains(sha) ?? true) }
        server.stop()
        await until("offline") { !c.state.isConnected }
        XCTAssertEqual(c.restoreFromTrash(c.trash[0].id), .restored)
        _ = await c.lastReupload?.value
        XCTAssertEqual(c.settings.reuploads, [sha], "waits for the server")

        let back = FakeServer(snapshot: server.current)
        _ = try await back.start(port: port)
        defer { back.stop() }
        await until("sent after reconnecting", timeout: 10) { back.storedFile(sha) == picture && c.settings.reuploads.isEmpty }
    }
}
