import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Round 3: push tokens taken back, managing access and backups.
@MainActor
final class Round3ClientTests: FakeServerTestCase {
    /// Signs a new device in as `member` with a fresh invite code.
    func signedIn(_ server: FakeServer, _ url: String, member: UInt64, tokens: MemoryTokenStore = MemoryTokenStore()) async -> SyncClient {
        let c = device(url, tokens: tokens)
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: server.invite(member: member), email: nil, newPassword: nil))
        await until("signed in as \(member)") { c.canEdit && c.state.isConnected }
        return c
    }

    /// An owner deletes the workspace (not the server's first): everyone's
    /// copy goes, they're signed out with why, and coming back later gets
    /// the same instead of retrying.
    func testOwnerDeletesTheWorkspaceAndEveryCopyGoes() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let port = try await server.start(port: 0)
        defer { server.stop() }
        // the server's first workspace can't be deleted
        server.setRole(member: 3, role: .owner)
        let first = await signedIn(server, "ws://127.0.0.1:\(port)", member: 3)
        XCTAssertFalse(first.canDeleteWorkspace)
        first.disconnect()

        let url = "ws://127.0.0.1:\(port)/w/abc"
        let owner = await signedIn(server, url, member: 3)
        let other = await signedIn(server, url, member: 2)
        XCTAssertTrue(owner.canDeleteWorkspace)
        XCTAssertFalse(other.canDeleteWorkspace, "only an owner")
        XCTAssertNotNil(other.data)

        let failed = await owner.deleteWorkspace()
        XCTAssertNil(failed)
        XCTAssertTrue(server.isDeleted)
        XCTAssertEqual(owner.signInReason, .gone)
        XCTAssertNil(owner.data, "this iPhone's copy is gone")
        XCTAssertFalse(owner.hasToken)

        await until("the other iPhone is told") { other.signInReason == .gone }
        XCTAssertEqual(other.signInMessage, WorkspaceAPI.goneMessage)
        XCTAssertNil(other.data)
        XCTAssertFalse(other.hasToken)
        XCTAssertEqual(other.state, .signIn)

        // one that was away hears it on connecting, and doesn't retry
        let late = device(url)
        await until("late one told") { late.signInReason == .gone }
        try? await Task.sleep(nanoseconds: 800_000_000)
        XCTAssertEqual(late.state, .signIn, "no retrying a deleted workspace")
    }

    func testSignOutStopsPushesFromThatServer() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let c = await signedIn(server, url, member: 3)
        c.setPushToken("a1b2c3", env: "development")
        await until("token registered") { server.registeredPushTokens[c.settings.device] == "a1b2c3" }
        XCTAssertEqual(c.settings.pushSentTo, url)
        c.signOut()
        await until("token removed on the server") { server.registeredPushTokens[c.settings.device] == nil }
        XCTAssertNil(c.settings.pushSentTo, "the local registration is gone too")
        XCTAssertEqual(c.settings.pushUnregister, [])
        XCTAssertEqual(c.settings.pushToken, "a1b2c3", "kept for the next sign-in")

        // signing in again registers the token again
        c.signIn(.invite(code: server.invite(member: 3), email: nil, newPassword: nil))
        await until("registered again") { server.registeredPushTokens[c.settings.device] == "a1b2c3" }
    }

    func testSignOutWhileOfflineStopsPushesOnTheNextConnect() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let port = try await server.start(port: 0)
        let url = "ws://127.0.0.1:\(port)"
        defer { server.stop() }
        let tokens = MemoryTokenStore()
        let c = await signedIn(server, url, member: 3, tokens: tokens)
        c.setPushToken("a1b2c3", env: "development")
        await until("token registered") { server.registeredPushTokens[c.settings.device] == "a1b2c3" }
        server.stop() // the server can't be reached
        await until("lost the server") { !c.state.isConnected }
        c.signOut()
        XCTAssertEqual(c.settings.pushUnregister, [url], "remembered until the server hears it")
        XCTAssertNil(tokens.token(for: url), "signed out here")
        XCTAssertNotNil(tokens.token(for: SyncClient.unregisterKey(url)), "the old token waits to sign the unregister")
        try? await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertEqual(server.registeredPushTokens[c.settings.device], "a1b2c3")

        _ = try await server.start(port: port) // back
        c.connect()
        await until("token removed on the server", timeout: 8) { server.registeredPushTokens[c.settings.device] == nil }
        await until("nothing left waiting") { c.settings.pushUnregister.isEmpty }
        XCTAssertNil(tokens.token(for: SyncClient.unregisterKey(url)))
    }

    func testAWorkspaceLeftKeepsItsPushesUntilItIsRemoved() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make())
        let b = FakeServer(snapshot: SampleWorkspace.make())
        let pa = try await a.start(port: 0), pb = try await b.start(port: 0)
        defer { a.stop(); b.stop() }
        let (ua, ub) = ("ws://127.0.0.1:\(pa)", "ws://127.0.0.1:\(pb)")
        let c = client(ua, me: 1)
        c.setPushToken("ffee", env: "development")
        await until("registered on a") { a.registeredPushTokens[c.settings.device] == "ffee" }
        let keyA = c.pushKey
        XCTAssertEqual(a.registeredPushWorkspaces[c.settings.device], keyA, "with this iPhone's name for it")
        await until("a's copy") { c.data != nil }
        c.setServer(ub)
        c.setMe(1)
        await until("registered on b") { b.registeredPushTokens[c.settings.device] == "ffee" }
        XCTAssertEqual(b.registeredPushWorkspaces[c.settings.device], c.pushKey)
        XCTAssertNotEqual(c.pushKey, keyA)
        XCTAssertEqual(c.settings.pushSentTo, ub)
        try? await Task.sleep(nanoseconds: 300_000_000)
        XCTAssertEqual(a.registeredPushTokens[c.settings.device], "ffee", "a is kept here, so it still notifies")

        // a tap on a's notification finds a; one from b (open) or with no "ws" finds nothing to switch to
        let kept = try XCTUnwrap(c.savedWorkspace(pushKey: keyA))
        XCTAssertEqual(kept.server, ua)
        XCTAssertNil(c.savedWorkspace(pushKey: c.pushKey))
        XCTAssertNil(c.savedWorkspace(pushKey: nil))
        XCTAssertNil(c.savedWorkspace(pushKey: "someOtherKey"))

        // removing a from this iPhone stops its pushes
        c.forget(kept)
        await until("removed from a") { a.registeredPushTokens[c.settings.device] == nil }
        XCTAssertEqual(b.registeredPushTokens[c.settings.device], "ffee")
    }

    func testBackToAKeptWorkspaceItsWelcomeIsMarkedAsPushed() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make())
        let b = FakeServer(snapshot: SampleWorkspace.make())
        let pa = try await a.start(port: 0), pb = try await b.start(port: 0)
        defer { a.stop(); b.stop() }
        let (ua, ub) = ("ws://127.0.0.1:\(pa)", "ws://127.0.0.1:\(pb)")
        let c = client(ua, me: 1)
        c.setPushToken("ffee", env: "development")
        await until("registered on a") { a.registeredPushTokens[c.settings.device] == "ffee" }
        await until("a's copy") { c.data != nil }
        c.setServer(ub)
        c.setMe(1)
        await until("registered on b") { b.registeredPushTokens[c.settings.device] == "ffee" }

        // a tap on a's notification opens a: a pushed what came meanwhile
        var marks: [Bool] = []
        c.onUpdate = { [weak c] in marks.append(c?.pushedWhileAway ?? false) }
        c.switchTo(try XCTUnwrap(c.savedWorkspaces.first))
        XCTAssertEqual(c.settings.pushSentTo, ua, "a's server has the token, not b's")
        await until("welcomed by a") { c.state.isConnected && !marks.isEmpty }
        c.onUpdate = {}
        XCTAssertEqual(marks.first, true, "the welcome's messages were pushed already")
    }

    func testSwitchingToAWorkspaceNotKeptStopsPushesFromTheOldOne() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make())
        let b = FakeServer(snapshot: SampleWorkspace.make())
        let pa = try await a.start(port: 0), pb = try await b.start(port: 0)
        defer { a.stop(); b.stop() }
        let c = client("ws://127.0.0.1:\(pa)", me: 1)
        c.setPushToken("ffee", env: "development")
        await until("registered on a") { a.registeredPushTokens[c.settings.device] == "ffee" }
        // the same workspace's copy goes along (not kept apart): a is left for good
        c.setServer("ws://127.0.0.1:\(pb)", freshCopy: false)
        await until("removed from a") { a.registeredPushTokens[c.settings.device] == nil }
        await until("registered on b") { b.registeredPushTokens[c.settings.device] == "ffee" }
    }

    func testAdminMessagesRoundTrip() {
        let out: [ClientMessage] = [.createInvite(member: 3, role: .member), .setRole(member: 2, role: .admin), .revoke(member: 2),
                                    .accessList, .backupList, .restore(name: "hourly-20261115-03.json")]
        for m in out { XCTAssertEqual(ClientMessage(text: m.text), m) }
        XCTAssertEqual(ClientMessage.createInvite(member: 3, role: .member).json, ["type": "create_invite", "member": 3, "role": "member"])
        XCTAssertEqual(ClientMessage.accessList.text, #"{"type":"access_list"}"#)
        let back: [ServerMessage] = [.invite(member: 3, code: "AAAA-BBBB-CCCC", emailed: "sam@example.com"),
                                     .access(people: [Person(member: 1, role: .owner, devices: 2, account: true), Person(member: 3, role: .member, invite: "X")], role: .owner),
                                     .backups([Backup(name: "daily-20261115.json", bytes: 1234)]), .restored(name: "daily-20261115.json")]
        for m in back { XCTAssertEqual(ServerMessage(text: m.text), m) }
        // as the real server (serde) writes them
        XCTAssertEqual(ServerMessage(text: #"{"type":"access","people":[{"member":2,"role":"admin","devices":1}],"role":"admin"}"#),
                       .access(people: [Person(member: 2, role: .admin, devices: 1)], role: .admin))
        XCTAssertEqual(ServerMessage(text: #"{"type":"invite","member":3,"code":"C","mail_error":"no route"}"#), .invite(member: 3, code: "C", mailError: "no route"))
        // whether a list answers a request (servers from round 6 on say)
        for answer in [true, false] {
            let m = ServerMessage.access(people: [Person(member: 2, role: .admin)], role: .owner, answer: answer)
            XCTAssertEqual(ServerMessage(text: m.text), m)
        }
        XCTAssertEqual(ServerMessage(text: #"{"type":"access","people":[],"role":"owner","answer":true}"#), .access(people: [], role: .owner, answer: true))
        XCTAssertEqual(ServerMessage(text: #"{"type":"access","people":[],"role":"owner"}"#), .access(people: [], role: .owner, answer: nil))
        XCTAssertEqual(Backup(name: "hourly-20261115-03.json", bytes: 0).label, "2026-11-15 03:00 (hourly)")
        XCTAssertEqual(Backup(name: "daily-20261115.json", bytes: 0).label, "2026-11-15 (daily)")
        XCTAssertEqual(Backup(name: "before-restore-20261115-031522.json", bytes: 0).label, "2026-11-15 03:15:22 (before a restore)")
        XCTAssertEqual(Backup(name: "odd.json", bytes: 0).label, "odd.json")
    }

    func testAccessRulesMatchTheServer() {
        let owner = AccessRules(role: .owner, me: 1), admin = AccessRules(role: .admin, me: 2), member = AccessRules(role: .member, me: 3)
        let theOwner = Person(member: 1, role: .owner, account: true)
        let anAdmin = Person(member: 4, role: .admin)
        let newPerson = Person(member: 5, role: .member)
        let withPassword = Person(member: 6, role: .member, account: true)
        XCTAssertTrue(owner.seesBackups)
        XCTAssertFalse(admin.seesBackups)
        XCTAssertFalse(member.seesPeople)
        XCTAssertTrue(admin.seesPeople)
        for r in [owner, admin] {
            XCTAssertFalse(r.mayChangeRole(of: theOwner))
            XCTAssertFalse(r.mayRemoveAccess(of: theOwner))
            XCTAssertTrue(r.mayChangeRole(of: newPerson))
            XCTAssertTrue(r.mayRemoveAccess(of: anAdmin))
            XCTAssertTrue(r.mayInvite(newPerson))
        }
        XCTAssertFalse(owner.mayInvite(theOwner), "not for myself")
        XCTAssertTrue(owner.mayInvite(anAdmin))
        XCTAssertTrue(owner.mayInvite(withPassword))
        XCTAssertFalse(admin.mayInvite(theOwner))
        XCTAssertFalse(admin.mayInvite(anAdmin), "a code signs in as that person")
        XCTAssertFalse(admin.mayInvite(withPassword))
        XCTAssertFalse(admin.mayChangeRole(of: Person(member: 2, role: .admin)), "not my own")
        XCTAssertFalse(member.mayInvite(newPerson))
        XCTAssertFalse(member.mayRemoveAccess(of: newPerson))
    }

    func testOwnerAndAdminManageAccess() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let owner = await signedIn(server, url, member: 1)
        XCTAssertEqual(owner.myRole, .owner)
        owner.requestAccessList()
        await until("people listed") { owner.people != nil }
        XCTAssertEqual(owner.people?.map(\.member), [1, 2, 3])
        XCTAssertEqual(owner.people?.first?.role, .owner)
        XCTAssertEqual(owner.people?.first?.devices, 1)

        // an invite code for Sam, emailed to the address on record
        owner.createInvite(member: 3)
        await until("invite made") { owner.lastInvite != nil }
        XCTAssertEqual(owner.lastInvite?.member, 3)
        XCTAssertEqual(owner.lastInvite?.emailed, "sam@example.com")
        XCTAssertEqual(owner.lastInvite?.code, server.openInvite(member: 3))
        await until("code in the list") { owner.people?.first { $0.member == 3 }?.invite == owner.lastInvite?.code }
        let sam = device(url)
        await until("sam asked to sign in") { sam.needsSignIn }
        sam.signIn(.invite(code: owner.lastInvite!.code, email: nil, newPassword: nil))
        await until("sam in") { sam.canEdit }
        XCTAssertEqual(sam.myRole, .member)
        sam.requestAccessList()
        await until("sam turned down") { sam.adminError != nil }
        XCTAssertEqual(sam.adminError, "Only the owner or an admin can do that.")
        XCTAssertNil(sam.people)

        // make Sam an admin: Sam's own role changes as the server says
        owner.setRole(member: 3, role: .admin)
        await until("sam is admin") { sam.myRole == .admin }
        await until("owner sees it") { owner.people?.first { $0.member == 3 }?.role == .admin }
        XCTAssertNotNil(sam.people, "admins get everyone's access")
        XCTAssertEqual(server.role(of: 3), .admin)

        // what admins may not do is turned down by the server too
        sam.setRole(member: 1, role: .member)
        await until("owner's role kept") { sam.adminError != nil }
        XCTAssertEqual(sam.adminError, "The owner's role can't be changed here.")
        sam.requestBackups()
        await until("no backups for admins") { sam.adminError == "Only the owner can see and restore backups." }
        sam.createInvite(member: 1)
        await until("no owner code from an admin") { sam.adminError == "Only the owner can make a code for the owner." }
        sam.createInvite(member: 2)
        await until("admin makes a code for a new member") { sam.lastInvite?.member == 2 }

        // remove Sam's access: Sam is signed out
        owner.revoke(member: 3)
        await until("sam signed out") { sam.state == .signIn }
        XCTAssertEqual(sam.signInReason, .signedOut)
        XCTAssertNil(sam.people, "nothing of the access list stays after signing out")
        await until("sam back to member") { owner.people?.first { $0.member == 3 }?.role == .member }
        XCTAssertEqual(owner.people?.first { $0.member == 3 }?.devices, 0)
    }

    func testOwnerListsAndRestoresBackups() async throws {
        let old = SampleWorkspace.make()
        var now = SampleWorkspace.make()
        now["tasks"] = .array(now["tasks"]!.array!.filter { $0["id"] != 206 }) // gone since yesterday
        let server = FakeServer(snapshot: now, requireSignIn: true)
        server.addBackup(name: "daily-20261115.json", snapshot: old)
        server.addBackup(name: "hourly-20261116-03.json", snapshot: SampleWorkspace.make())
        let url = try await start(server)
        defer { server.stop() }
        let owner = await signedIn(server, url, member: 1)
        owner.requestBackups()
        await until("backups listed") { owner.backups != nil }
        XCTAssertEqual(owner.backups?.map(\.label), ["2026-11-16 03:00 (hourly)", "2026-11-15 (daily)"])
        owner.restore(backup: "daily-20261115.json")
        await until("restored") { owner.adminNotice != nil }
        XCTAssertEqual(owner.adminNotice, "Restored the team's workspace to 2026-11-15 (daily). Everyone gets it now.")
        await until("the deleted task comes back") { owner.view.tasks.contains { $0.id == 206 } }
        await until("the copy from before is a new backup") { owner.backups?.count == 3 }
        owner.restore(backup: "nope.json")
        await until("unknown backup turned down") { owner.adminError != nil }
    }

    func testTrashIsPersonalAndRestoreBringsItBackForEveryone() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let url = "ws://127.0.0.1:\(port)"
        let s = storage()
        let c = SyncClient(storage: s, tokens: MemoryTokenStore())
        c.pushDelay = 0.05
        c.setMe(1)
        c.setServer(url)
        let other = client(url, me: 2)
        await until("both connected") { c.canEdit && other.canEdit && c.state.isConnected }
        XCTAssertTrue(c.trashTask(202))
        XCTAssertTrue(c.trashDoc(401))
        XCTAssertEqual(c.trash.map(\.kind), ["Task", "Doc"])
        await until("gone for the teammate") { !other.view.tasks.contains { $0.id == 202 } && !other.view.docs.contains { $0.id == 401 } }
        XCTAssertFalse(server.current!.serialized().contains("\"payload\""), "Trash never goes to the server")
        XCTAssertNil(server.current?.at("extra.trash"))

        // kept on this iPhone across launches
        let again = SyncClient(storage: s, tokens: MemoryTokenStore())
        XCTAssertEqual(again.trash, c.trash)

        let taskItem = c.trash[0].id
        XCTAssertEqual(c.restoreFromTrash(taskItem), .restored)
        XCTAssertEqual(c.trash.map(\.kind), ["Doc"])
        await until("back for the teammate") { other.view.tasks.contains { $0.id == 202 } }
        c.deleteFromTrash(c.trash[0].id)
        XCTAssertTrue(c.trash.isEmpty)
        XCTAssertTrue(SyncClient(storage: s, tokens: MemoryTokenStore()).trash.isEmpty)

        // another server's workspace starts with an empty Trash
        XCTAssertTrue(c.trashTask(203))
        let b = FakeServer(snapshot: SampleWorkspace.make())
        let pb = try await b.start(port: 0)
        defer { b.stop() }
        c.setServer("ws://127.0.0.1:\(pb)")
        XCTAssertTrue(c.trash.isEmpty)
    }

    func testVideosGoUpStreamedFromTheirFile() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let video = Data((0..<3_000_123).map { UInt8(($0 * 7) % 251) })
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("wbvideo-\(UUID().uuidString).mov")
        try video.write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }
        XCTAssertEqual(try FileAPI.sha256(file: file, chunk: 4096), FileAPI.sha256(video), "hashed a piece at a time, same answer")
        let sha = try await FileAPI.upload(server: "ws://127.0.0.1:\(port)", auth: nil, file: file, contentType: "video/quicktime")
        XCTAssertEqual(sha, FileAPI.sha256(video))
        XCTAssertEqual(server.storedFile(sha), video)
    }
}
