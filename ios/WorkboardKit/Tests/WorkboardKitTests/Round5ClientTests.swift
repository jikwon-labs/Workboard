import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

@MainActor
final class Round5ClientTests: FakeServerTestCase {
    override var devicePushDelay: TimeInterval? { 0.05 }

    func signedIn(_ server: FakeServer, _ url: String, member: UInt64) async -> SyncClient {
        let c = device(url)
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: server.invite(member: member), email: nil, newPassword: nil))
        await until("signed in as \(member)") { c.canEdit && c.state.isConnected && c.me == member }
        return c
    }

    /// Messages a Mac brought in from a Slack export are old history: no
    /// notification and no unread count, like the Mac (src/app/notify.rs).
    func testImportedMessagesDontNotifyOrCountAsUnread() {
        let now: Int64 = 1_800_000_000
        func msg(_ id: UInt64, imported: String? = nil, replies: [JSON] = []) -> JSON {
            var m: JSON = ["id": .id(id), "author": 2, "text": .string("m\(id)"), "ts": .int(now - 10), "replies": .array(replies)]
            if let imported { m["imported"] = .string(imported) }
            return m
        }
        let before: JSON = ["channels": [["id": 1, "messages": []]], "extra": ["sync": SyncMeta().json]]
        let after: JSON = [
            "channels": [["id": 1, "last_read": .int(now - 100), "messages": [
                msg(10, imported: "slack:C1:1800000000.000100", replies: [msg(11, imported: "slack:C1:1800000000.000200"), msg(12)]),
                msg(13),
                msg(14, imported: "slack:C1:1800000000.000300"),
                ["id": 15, "author": 2, "text": "null field", "ts": .int(now - 5), "imported": .null],
            ]]],
            "extra": ["sync": SyncMeta().json],
        ]
        let fresh = SyncClient.newMessages(before: before, after: after, me: 1, now: now)
        XCTAssertEqual(fresh.map(\.id), [12, 13, 15], "a new reply under imported history still notifies; imported ones don't")
        let ch = WorkspaceView(after).channels[0]
        XCTAssertEqual(ch.unread(me: 1), 2, "imported history isn't unread")
        XCTAssertEqual(ch.messages.map(\.imported), [true, false, true, false])
        XCTAssertEqual(ch.messages[0].replies.map(\.imported), [true, false])
    }

    /// An imported message arriving from the server doesn't notify, and the
    /// field stays as it came through this iPhone's own changes.
    func testImportedMessageFromTheServerStaysQuietAndKeepsItsField() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let c = device(url)
        c.setMe(1)
        var heard: [IncomingMessage] = []
        c.onIncoming = { heard += $0 }
        await until("connected") { c.state.isConnected && c.data != nil }

        var snap = server.current!
        let key = "slack:C024BE91L:\(nowTs()).000100"
        _ = snap.updateItem(at: "channels", id: 301) { ch in
            var msgs = ch["messages"]?.array ?? []
            msgs.append(["id": .id(990_001), "author": 2, "text": "From Slack", "ts": .int(nowTs()), "pinned": false,
                         "reactions": [], "replies": [], "attachments": [], "imported": .string(key)])
            msgs.append(["id": .id(990_002), "author": 2, "text": "Written now", "ts": .int(nowTs()), "pinned": false,
                         "reactions": [], "replies": [], "attachments": []])
            ch["messages"] = .array(msgs)
        }
        server.replace(snap)
        await until("the new message heard") { heard.contains { $0.text == "Written now" } }
        XCTAssertFalse(heard.contains { $0.text == "From Slack" })

        // a reaction from here goes back with the import mark untouched
        XCTAssertTrue(c.edit { d, me in _ = Edits.toggleReaction(&d, channel: 301, message: 990_001, emoji: "👍", me: me) })
        await until("pushed") { c.waitingCount == 0 && server.current!.serialized().contains("👍") }
        let m = server.current!["channels"]?.array?.first { $0["id"]?.uint == 301 }?["messages"]?.array?.first { $0["id"]?.uint == 990_001 }
        XCTAssertEqual(m?["imported"], .string(key))
        XCTAssertTrue(c.view.channels.first { $0.id == 301 }?.messages.first { $0.id == 990_001 }?.imported ?? false)
    }

    /// Two admins change the same person at once: the answer the server
    /// works out after the change shows the other admin's role. It still
    /// answers the request, so a later refusal lands on its own request.
    func testRoleChangeRacingAnotherAdminStillSettles() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let owner = await signedIn(server, url, member: 1)
        owner.requestAccessList()
        await until("listed") { owner.people != nil && owner.adminPending == 0 }

        // the owner makes Sam an admin; another admin makes Sam a member
        // again just before the answer is worked out
        server.meddleBeforeNextAnswer(member: 3, role: .member)
        owner.setRole(member: 3, role: .admin)
        await until("answered") { owner.adminPending == 0 }
        XCTAssertEqual(owner.people?.first { $0.member == 3 }?.role, .member, "the other admin's change came last")
        XCTAssertNil(owner.adminUnconfirmed)

        // a refusal afterwards is its own request's, nothing left over takes it
        owner.revoke(member: 1)
        await until("refused") { owner.adminError != nil }
        XCTAssertEqual(owner.adminError, "The owner's access can't be removed.")
        XCTAssertEqual(owner.adminPending, 0)
        XCTAssertNil(owner.notice)

        // and the next role change goes through as usual
        owner.setRole(member: 2, role: .admin)
        await until("Alex is an admin") { owner.adminPending == 0 && owner.people?.first { $0.member == 2 }?.role == .admin }
        XCTAssertNil(owner.adminUnconfirmed)
    }

    /// A request whose answer is slow is given up on after a while, with a
    /// gentle note. Its answer still comes (the server answers every
    /// request, in order) and is taken as its own, not a later request's,
    /// so the later request's refusal still lands on it.
    func testSlowRequestIsGivenUpWithANoteAndItsLateAnswerIsItsOwn() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let owner = await signedIn(server, url, member: 1)
        owner.askTimeout = 1
        owner.requestAccessList()
        await until("listed") { owner.people != nil && owner.adminPending == 0 }

        server.lateNextAnswer(after: 1.6)
        owner.setRole(member: 3, role: .admin)
        XCTAssertEqual(owner.adminPending, 1)
        await until("given up", timeout: 3) { owner.adminPending == 0 }
        XCTAssertEqual(owner.adminUnconfirmed, SyncClient.unconfirmedNote)
        XCTAssertNil(owner.adminError)
        XCTAssertEqual(owner.lateAnswersOwed, 1)
        // it comes after all: the note goes
        await until("confirmed late", timeout: 3) { owner.lateAnswersOwed == 0 }
        XCTAssertNil(owner.adminUnconfirmed)
        XCTAssertEqual(owner.people?.first { $0.member == 3 }?.role, .admin)

        server.lateNextAnswer(after: 1.6)
        owner.setRole(member: 3, role: .member)
        await until("given up again", timeout: 3) { owner.adminPending == 0 }
        XCTAssertEqual(owner.lateAnswersOwed, 1)

        // sent before the late answer comes: the late list is the role
        // change's, and the refusal after it is this request's
        owner.revoke(member: 1)
        XCTAssertNil(owner.adminUnconfirmed, "a new request starts afresh")
        await until("refused", timeout: 4) { owner.adminError != nil || owner.notice != nil }
        XCTAssertEqual(owner.adminError, "The owner's access can't be removed.")
        XCTAssertNil(owner.notice)
        XCTAssertEqual(owner.adminPending, 0)
        XCTAssertEqual(owner.lateAnswersOwed, 0)
        XCTAssertEqual(owner.people?.first { $0.member == 3 }?.role, .member, "the late answer's list is taken")

        // invites and restores may take longer (email, a whole workspace)
        owner.asks = [.invite(member: 2), .setRole(member: 3, role: .admin)]
        owner.expireAsks(now: Date().addingTimeInterval(1.5))
        XCTAssertEqual(owner.asks, [.invite(member: 2)])
        owner.expireAsks(now: Date().addingTimeInterval(5))
        XCTAssertEqual(owner.adminPending, 0)
        XCTAssertEqual(owner.lateAnswersOwed, 2)
        // answers that never come are forgotten in the end
        owner.expireAsks(now: Date().addingTimeInterval(11))
        XCTAssertEqual(owner.lateAnswersOwed, 1, "the invite is owed longer")
        owner.expireAsks(now: Date().addingTimeInterval(41))
        XCTAssertEqual(owner.lateAnswersOwed, 0)

        // a password change that gets no answer stops spinning
        owner.asks = [.password]
        owner.accountRequest = .sending
        owner.clearAdminMessages()
        owner.expireAsks(now: Date().addingTimeInterval(1.5))
        XCTAssertEqual(owner.accountRequest, .failed(SyncClient.passwordUnconfirmed))
        XCTAssertNil(owner.adminUnconfirmed, "not an admin request")
    }

    /// Lists say whether they answer: one sent unasked (another admin's
    /// change) never takes a request's place, and a late answer to a request
    /// given up on is taken as that request's.
    func testListsThatSayTheyAnswerSettleOnlyTheirRequests() {
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        c.askTimeout = 1
        let list = [Person(member: 1, role: .owner), Person(member: 2, role: .admin)]

        c.asks = [.setRole(member: 2, role: .admin), .revoke(member: 1)]
        c.receiveForTests(.access(people: list, role: .owner, answer: false))
        XCTAssertEqual(c.asks, [.setRole(member: 2, role: .admin), .revoke(member: 1)], "unasked: no answer")
        XCTAssertEqual(c.people, list, "but the list is taken")
        c.receiveForTests(.access(people: list, role: .owner, answer: true))
        XCTAssertEqual(c.asks, [.revoke(member: 1)])
        c.receiveForTests(.access(people: list, role: .owner, answer: false))
        c.receiveForTests(.error(code: "denied", message: "The owner's access can't be removed."))
        XCTAssertEqual(c.adminError, "The owner's access can't be removed.")
        XCTAssertNil(c.notice)
        XCTAssertEqual(c.adminPending, 0)

        // a late answer: the request given up on takes it, the newer one waits on
        c.clearAdminMessages()
        c.asks = [.setRole(member: 2, role: .member)]
        c.expireAsks(now: Date().addingTimeInterval(1.5))
        XCTAssertEqual(c.adminUnconfirmed, SyncClient.unconfirmedNote)
        c.asked.append(WaitingAsk(ask: .revoke(member: 1), sent: Date()))
        c.receiveForTests(.access(people: list, role: .owner, answer: true))
        XCTAssertEqual(c.asks, [.revoke(member: 1)])
        XCTAssertEqual(c.lateAnswersOwed, 0)
        XCTAssertNil(c.adminUnconfirmed, "confirmed after all")
        c.receiveForTests(.error(code: "denied", message: "The owner's access can't be removed."))
        XCTAssertEqual(c.adminError, "The owner's access can't be removed.")
        XCTAssertNil(c.notice)

        // a late refusal with nothing newer waiting replaces the note
        c.clearAdminMessages()
        c.asks = [.revoke(member: 1)]
        c.expireAsks(now: Date().addingTimeInterval(1.5))
        c.receiveForTests(.error(code: "denied", message: "The owner's access can't be removed."))
        XCTAssertEqual(c.adminError, "The owner's access can't be removed.")
        XCTAssertNil(c.adminUnconfirmed)
        XCTAssertEqual(c.lateAnswersOwed, 0)

        // with a newer request waiting it's shown briefly, not as that one's
        c.clearAdminMessages()
        c.asks = [.revoke(member: 1)]
        c.expireAsks(now: Date().addingTimeInterval(1.5))
        c.asked.append(WaitingAsk(ask: .accessList, sent: Date()))
        c.receiveForTests(.error(code: "denied", message: "The owner's access can't be removed."))
        XCTAssertNil(c.adminError)
        XCTAssertEqual(c.notice, "The owner's access can't be removed.")
        c.receiveForTests(.access(people: list, role: .owner, answer: true))
        XCTAssertEqual(c.adminPending, 0)

        // a late password refusal replaces the "didn't answer" failure
        c.asks = [.password]
        c.accountRequest = .sending
        c.expireAsks(now: Date().addingTimeInterval(1.5))
        c.receiveForTests(.error(code: "denied", message: "Your current password isn't right."))
        XCTAssertEqual(c.accountRequest, .failed("Your current password isn't right."))

        // from a server too old to say, a late list still goes to its request
        c.asks = [.setRole(member: 2, role: .admin)]
        c.expireAsks(now: Date().addingTimeInterval(1.5))
        c.asked.append(WaitingAsk(ask: .revoke(member: 1), sent: Date()))
        c.receiveForTests(.access(people: list, role: .owner))
        XCTAssertEqual(c.asks, [.revoke(member: 1)])
        XCTAssertEqual(c.lateAnswersOwed, 0)
    }

    /// Two requests back to back: each gets its own answer, and the second
    /// one's refusal shows where its request was made, even when another
    /// admin's list arrives just before the first answer.
    func testBackToBackRequestsEachGetTheirOwnAnswer() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let owner = await signedIn(server, url, member: 1)
        owner.requestAccessList()
        await until("listed") { owner.people != nil && owner.adminPending == 0 }

        owner.setRole(member: 2, role: .admin)
        owner.revoke(member: 1)
        XCTAssertEqual(owner.adminPending, 2)
        await until("both answered") { owner.adminPending == 0 }
        XCTAssertEqual(owner.adminError, "The owner's access can't be removed.")
        XCTAssertNil(owner.notice)
        XCTAssertEqual(owner.people?.first { $0.member == 2 }?.role, .admin)

        // another admin's change lands first; its list isn't an answer
        owner.clearAdminMessages()
        server.meddleBeforeNextAnswer(member: 3, role: .admin)
        owner.setRole(member: 2, role: .member)
        owner.revoke(member: 1)
        await until("both answered again") { owner.adminPending == 0 }
        XCTAssertEqual(owner.adminError, "The owner's access can't be removed.")
        XCTAssertNil(owner.notice)
        XCTAssertEqual(owner.people?.first { $0.member == 2 }?.role, .member)
        XCTAssertEqual(owner.people?.first { $0.member == 3 }?.role, .admin)

        // an invite and a role change back to back: the invite brings no extra list
        owner.clearAdminMessages()
        owner.createInvite(member: 3)
        owner.setRole(member: 1, role: .admin)
        await until("answered") { owner.adminPending == 0 }
        XCTAssertNotNil(owner.lastInvite)
        XCTAssertEqual(owner.adminError, "The owner's role can't be changed here.")
        XCTAssertNil(owner.notice)
    }
}
