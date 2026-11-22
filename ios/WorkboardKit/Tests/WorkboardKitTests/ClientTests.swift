import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

@MainActor
final class ClientTests: FakeServerTestCase {
    func testTwoClientsSeeEachOthersChanges() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let url = "ws://127.0.0.1:\(port)"
        let a = client(url, me: 1)
        let b = client(url, me: 2)
        await until("both connected") { a.canEdit && b.canEdit }
        XCTAssertNotEqual(a.settings.tag, b.settings.tag, "server gives each device its own tag")

        // A sends a message, B sees it
        XCTAssertTrue(a.edit { d, me in Edits.sendMessage(&d, channel: 301, text: "from the phone", me: me, id: a.newId(), ts: nowTs()) })
        await until("message at B") { b.view.channels.first { $0.id == 301 }?.messages.last?.text == "from the phone" }

        // B replies in the thread and changes a task status, A sees both
        b.edit { d, me in
            Edits.sendReply(&d, channel: 301, to: 313, text: "reviewing now", me: me, id: b.newId(), ts: nowTs())
            Edits.setStatus(&d, task: 202, to: "in progress", me: me, ts: nowTs())
        }
        await until("reply and status at A") {
            a.view.tasks.first { $0.id == 202 }?.status == "in progress"
                && a.view.channels.first { $0.id == 301 }?.messages.first { $0.id == 313 }?.replies.last?.text == "reviewing now"
        }

        // A edits a doc, B sees it
        a.edit { d, _ in Edits.setDocBody(&d, doc: 401, to: "Rewritten on the phone", ts: nowTs()) }
        await until("doc at B") { b.view.docs.first { $0.id == 401 }?.body == "Rewritten on the phone" }

        // nothing the phone does not know about went missing on the server
        let snap = try XCTUnwrap(server.current)
        XCTAssertEqual(snap["mac_only_top"], ["nested": [1, 2, 3]])
        XCTAssertEqual(snap["tasks"]!.array!.first { $0["id"] == 201 }!["mac_only_task_field"], ["kept": true])
        XCTAssertEqual(snap.at("extra.sync.hashes"), .object([:]))
        XCTAssertNotNil(SyncMeta(of: snap).stamps[401], "the doc edit is stamped for the server's merge")
    }

    func testOfflineEditsWaitAndGoUpAfterReconnect() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        let url = "ws://127.0.0.1:\(port)"
        let s = storage()
        let a = client(url, me: 1, storage: s)
        await until("connected") { a.state.isConnected }

        server.stop()
        await until("offline") { if case .waiting = a.state { return true } else { return false } }
        XCTAssertTrue(a.canEdit, "edits work offline")
        XCTAssertTrue(a.edit { d, me in Edits.sendMessage(&d, channel: 301, text: "written offline", me: me, id: a.newId(), ts: nowTs()) })
        XCTAssertTrue(a.edit { d, me in Edits.setStatus(&d, task: 205, to: "in progress", me: me, ts: nowTs()) })
        XCTAssertEqual(a.waitingCount, 3, "a new message, a task change and its activity line (which has an id now)")

        // a fresh launch while the server is down shows the saved copy with the offline edits
        // (the app writes it when it goes to the background)
        a.flushSnapshot()
        let relaunch = SyncClient(storage: s, tokens: MemoryTokenStore())
        XCTAssertEqual(relaunch.view.channels.first { $0.id == 301 }?.messages.last?.text, "written offline")
        XCTAssertEqual(relaunch.waitingCount, 3, "waiting marks are saved")

        // the server comes back on the same port; the client finds it alone and sends the edits
        let again = FakeServer(snapshot: server.current)
        _ = try await again.start(port: port)
        defer { again.stop() }
        await until("reconnected", timeout: 8) { a.state.isConnected }
        await until("offline edits reached the server") {
            again.current?["channels"]?[0]?["messages"]?.array?.last?["text"] == "written offline"
                && again.current?["tasks"]?.array?.first { $0["id"] == 205 }?["status"] == "in progress"
        }
        await until("nothing waiting") { a.waitingCount == 0 }
    }

    func testOfflineEditsFromTwoDevicesBothSurvive() async throws {
        // the fake server does not merge, but each device merges the snapshot
        // it gets at welcome before pushing, so both offline edits end up kept
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        let url = "ws://127.0.0.1:\(port)"
        let a = client(url, me: 1), b = client(url, me: 2)
        await until("connected") { a.state.isConnected && b.state.isConnected }
        server.stop()
        await until("offline") { !a.state.isConnected && !b.state.isConnected }
        a.edit { d, me in Edits.rename(&d, task: 201, to: "A renamed offline", ts: nowTs()) }
        b.edit { d, me in Edits.rename(&d, task: 203, to: "B renamed offline", ts: nowTs()) }
        let again = FakeServer(snapshot: server.current)
        _ = try await again.start(port: port)
        defer { again.stop() }
        await until("both back and settled", timeout: 10) {
            a.view.tasks.first { $0.id == 203 }?.name == "B renamed offline"
                && b.view.tasks.first { $0.id == 201 }?.name == "A renamed offline"
        }
    }

    func testInviteCodeWithNewPasswordThenPasswordOnASecondDevice() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let tokens = MemoryTokenStore()
        let c = device(url, tokens: tokens)
        await until("asked to sign in") { c.state == .signIn }
        XCTAssertEqual(c.signInReason, .needed)
        XCTAssertTrue(c.needsSignIn)
        XCTAssertNotNil(c.signInMessage)
        XCTAssertFalse(c.canEdit)

        // checked here before asking the server
        XCTAssertNotNil(c.signIn(.invite(code: "X", email: nil, newPassword: "short")))

        XCTAssertNil(c.signIn(.invite(code: "NOPE-NOPE-NOPE", email: nil, newPassword: "sam-password")))
        await until("bad code reported") { c.signInReason == .failed }
        XCTAssertEqual(c.state, .signIn)
        XCTAssertTrue(c.signInMessage?.contains("invite code doesn't work") == true, c.signInMessage ?? "")
        XCTAssertFalse(c.signingIn)

        // Sam's member record has an email, so the email can be left out
        let code = server.invite(member: 3)
        c.signIn(.invite(code: code.lowercased().replacingOccurrences(of: "-", with: ""), email: nil, newPassword: "sam-password"))
        await until("signed in") { c.state.isConnected }
        XCTAssertEqual(c.me, 3, "the server says who this device is")
        XCTAssertTrue(c.settings.meFromServer)
        XCTAssertEqual(c.settings.accountEmail, "sam@example.com")
        XCTAssertEqual(c.settings.role, "member")
        XCTAssertNotNil(tokens.token(for: url), "token kept in the token store")
        c.setMe(1)
        XCTAssertEqual(c.me, 3, "no picking someone else while signed in")
        XCTAssertTrue(c.edit { d, me in Edits.sendMessage(&d, channel: 301, text: "Sam here", me: me, id: c.newId(), ts: nowTs()) })
        await until("message on server") { server.current?["channels"]?[0]?["messages"]?.array?.last?["author"] == 3 }

        // the same code does not work twice
        let other = device(url)
        await until("other refused") { other.state == .signIn }
        other.signIn(.invite(code: code, email: nil, newPassword: "another-pass"))
        await until("second use refused") { other.signInReason == .failed }

        // a second device signs in with the email and password; wrong first
        other.signIn(.password(email: "sam@example.com", password: "wrong-password"))
        await until("wrong password refused") { other.signInReason == .failed && !other.signingIn }
        XCTAssertEqual(other.signInMessage, "That email and password don't match.")
        other.signIn(.password(email: " Sam@Example.com ", password: "sam-password"))
        await until("second device in") { other.state.isConnected }
        XCTAssertEqual(other.me, 3)
        XCTAssertEqual(other.settings.accountEmail, "sam@example.com")
        XCTAssertNotEqual(other.settings.tag, c.settings.tag)

        // a reconnect signs in again with the saved token
        c.disconnect()
        c.connect()
        await until("back with token") { c.state.isConnected }

        server.revoke(member: 3)
        await until("signed out") { c.state == .signIn }
        XCTAssertEqual(c.signInReason, .signedOut)
        XCTAssertEqual(c.signInMessage, "Your access to this server was removed.")
        XCTAssertNil(tokens.token(for: url))
        XCTAssertNil(c.me)
        XCTAssertFalse(c.canEdit)
        XCTAssertNotNil(c.data, "the saved copy stays readable")
        try? await Task.sleep(nanoseconds: 800_000_000)
        XCTAssertEqual(c.state, .signIn, "no retrying after an auth refusal")
    }

    func testCodeFromTheComputerSignsInOnceWithoutAPassword() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let tokens = MemoryTokenStore()
        let c = device(url, tokens: tokens)
        await until("asked to sign in") { c.state == .signIn }
        XCTAssertNotNil(c.signIn(.link(code: " - ")), "checked here first")

        c.signIn(.link(code: "ZZZZZZ"))
        await until("wrong code refused") { c.signInReason == .failed && !c.signingIn }
        XCTAssertEqual(c.signInMessage, "That code doesn't work. It may be used already or expired (codes last 5 minutes). Show a new one on your computer: Work together > Sign in on your phone.")

        // typed in lower case with a space, as people do
        let code = server.linkCode(member: 3)
        let typed = (code.prefix(3) + " " + code.dropFirst(3)).lowercased()
        c.signIn(.link(code: typed))
        await until("signed in") { c.state.isConnected }
        XCTAssertEqual(c.me, 3, "signed in as the person whose computer showed the code")
        XCTAssertTrue(c.settings.meFromServer)
        XCTAssertNotNil(tokens.token(for: url), "a token for next time")
        c.disconnect()
        c.connect()
        await until("back with the token") { c.state.isConnected }

        // it works once
        let other = device(url)
        await until("other asked") { other.state == .signIn }
        other.signIn(.link(code: code))
        await until("second use refused") { other.signInReason == .failed }
    }

    func testInviteWithoutRecordedEmailNeedsOne() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let c = device(url)
        await until("asked to sign in") { c.state == .signIn }
        let code = server.invite(member: 2) // Alex has no email in the workspace
        c.signIn(.invite(code: code, email: nil, newPassword: "alex-password"))
        await until("refused") { c.signInReason == .failed }
        XCTAssertTrue(c.signInMessage?.contains("email") == true)
        c.signIn(.invite(code: code, email: "alex@example.com", newPassword: "alex-password"))
        await until("in", { c.state.isConnected })
        XCTAssertEqual(c.settings.accountEmail, "alex@example.com")
    }

    func testStoredTokenTheServerDoesNotKnowIsDropped() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let tokens = MemoryTokenStore()
        tokens.setToken("old-token", for: url)
        let c = device(url, tokens: tokens)
        await until("refused") { c.state == .signIn }
        XCTAssertEqual(c.signInReason, .signedOut)
        XCTAssertNil(tokens.token(for: url), "the refused token is forgotten")
    }

    func testForgotPasswordThenResetCode() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let first = device(url)
        await until("asked") { first.state == .signIn }
        first.signIn(.invite(code: server.invite(member: 3), email: nil, newPassword: "first-password"))
        await until("in") { first.state.isConnected }

        let c = device(url)
        await until("asked") { c.state == .signIn }
        // the same answer for an unknown email, and no code made
        let unknownMail = try await AuthAPI.forgot(server: url, email: "nobody@example.com")
        XCTAssertTrue(unknownMail)
        XCTAssertNil(server.resetCode(for: "nobody@example.com"))
        let mail = try await AuthAPI.forgot(server: url, email: "sam@example.com")
        XCTAssertTrue(mail)
        let code = try XCTUnwrap(server.resetCode(for: "sam@example.com"))

        c.signIn(.reset(email: "sam@example.com", code: "WRON-GCOD-EXXX", newPassword: "second-password"))
        await until("bad code refused") { c.signInReason == .failed && !c.signingIn }
        c.signIn(.reset(email: "sam@example.com", code: code, newPassword: "second-password"))
        await until("in with reset code") { c.state.isConnected }
        XCTAssertEqual(c.me, 3)

        // the code is used up, and the new password works
        let d = device(url)
        await until("asked") { d.state == .signIn }
        d.signIn(.reset(email: "sam@example.com", code: code, newPassword: "third-password"))
        await until("used code refused") { d.signInReason == .failed && !d.signingIn }
        d.signIn(.password(email: "sam@example.com", password: "first-password"))
        await until("old password refused") { d.signInReason == .failed && !d.signingIn }
        d.signIn(.password(email: "sam@example.com", password: "second-password"))
        await until("new password works") { d.state.isConnected }

        // a server that can't send mail says so
        server.canMail = false
        let noMail = try await AuthAPI.forgot(server: url, email: "sam@example.com")
        XCTAssertFalse(noMail)
    }

    func testSetAndChangePasswordFromASignedInDevice() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let c = device(url)
        await until("asked") { c.state == .signIn }
        // an invite without a password (older apps): signed in, no account yet
        c.signIn(.invite(code: server.invite(member: 2), email: nil, newPassword: nil))
        await until("in") { c.state.isConnected }
        XCTAssertNil(c.settings.accountEmail)

        XCTAssertNotNil(c.setPassword(email: "alex@example.com", password: "short", oldPassword: nil))
        XCTAssertNil(c.setPassword(email: "alex@example.com", password: "alex-password", oldPassword: nil))
        await until("account made") { c.accountRequest == .done }
        XCTAssertEqual(c.settings.accountEmail, "alex@example.com")
        XCTAssertEqual(server.accountEmail(member: 2), "alex@example.com")

        // changing needs the current password
        XCTAssertNotNil(c.setPassword(email: "alex@example.com", password: "new-password", oldPassword: nil))
        c.setPassword(email: "alex@example.com", password: "new-password", oldPassword: "not-it-at-all")
        await until("wrong old password") { if case .failed = c.accountRequest { return true } else { return false } }
        XCTAssertEqual(c.accountRequest, .failed("Your current password isn't right."))
        XCTAssertTrue(c.state.isConnected, "a denied request keeps the connection")
        c.setPassword(email: "alex@example.com", password: "new-password", oldPassword: "alex-password")
        await until("changed") { c.accountRequest == .done }

        let d = device(url)
        await until("asked") { d.state == .signIn }
        d.signIn(.password(email: "alex@example.com", password: "new-password"))
        await until("new password works") { d.state.isConnected }
        XCTAssertEqual(d.me, 2)
    }

    func testIncomingMessagesAreAnnouncedButNotMine() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let url = "ws://127.0.0.1:\(port)"
        let a = client(url, me: 1), b = client(url, me: 2)
        var heard: [IncomingMessage] = []
        a.onIncoming = { heard += $0 }
        await until("connected") { a.state.isConnected && b.state.isConnected }
        a.edit { d, me in Edits.sendMessage(&d, channel: 301, text: "mine", me: me, id: a.newId(), ts: nowTs()) }
        b.edit { d, me in Edits.sendReply(&d, channel: 301, to: 313, text: "theirs", me: me, id: b.newId(), ts: nowTs()) }
        await until("reply heard") { heard.contains { $0.text == "theirs" } }
        XCTAssertFalse(heard.contains { $0.text == "mine" })
        XCTAssertEqual(heard.first { $0.text == "theirs" }?.parent, 313)
        XCTAssertEqual(heard.first { $0.text == "theirs" }?.author, 2)
    }

    /// Back after being away, the messages the welcome brings were pushed to
    /// this phone already (the server pushes only to phones not connected),
    /// so they don't notify again; ones heard while connected still do. A
    /// phone the server has no token for got no pushes, so its welcome
    /// still notifies.
    func testAWelcomeAfterBeingAwayIsMarkedAsPushedOnlyWhenTheServerHasTheToken() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let url = "ws://127.0.0.1:\(port)"
        let s = storage()
        let pre = SyncClient(storage: s, tokens: MemoryTokenStore())
        pre.setPushToken("a1b2c3", env: "development")
        let a = client(url, me: 1, storage: s)
        await until("token registered") { server.registeredPushTokens[a.settings.device] == "a1b2c3" }
        XCTAssertFalse(a.pushedWhileAway, "the server didn't have the token before this welcome")

        // read it where the app does, on each update: the update that
        // answers this phone's own push right after the welcome resets it
        var marks: [Bool] = []
        a.disconnect()
        a.onUpdate = { [weak a] in marks.append(a?.pushedWhileAway ?? false) }
        a.connect()
        await until("welcomed again") { a.state.isConnected && marks.contains(true) }
        a.onUpdate = {}

        let b = client(url, me: 2)
        await until("connected") { b.state.isConnected }
        let seen = a.updatesReceived
        b.edit { d, me in Edits.sendMessage(&d, channel: 303, text: "hello", me: me, id: b.newId(), ts: nowTs()) }
        await until("update merged") { a.updatesReceived > seen }
        XCTAssertFalse(a.pushedWhileAway, "heard while connected: the server didn't push it")

        let c = client(url, me: 2)
        await until("c connected") { c.state.isConnected }
        c.disconnect()
        c.connect()
        await until("c welcomed again") { c.state.isConnected && c.updatesReceived >= 2 }
        XCTAssertFalse(c.pushedWhileAway, "no token, no pushes")
    }

    /// A welcome's changes (what happened while this phone wasn't
    /// connected) are a catch-up, which the app doesn't announce; changes
    /// heard while connected are not.
    func testAWelcomeIsACatchUpAndALiveUpdateIsNot() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let url = "ws://127.0.0.1:\(port)"
        let a = client(url, me: 1)
        var marks: [Bool] = []
        a.onUpdate = { [weak a] in marks.append(a?.catchingUp ?? false) }
        await until("welcomed") { a.state.isConnected && !marks.isEmpty }
        XCTAssertEqual(marks.first, true, "no push token needed")

        let b = client(url, me: 2)
        await until("connected") { b.state.isConnected }
        let seen = a.updatesReceived
        b.edit { d, me in Edits.sendMessage(&d, channel: 303, text: "hello", me: me, id: b.newId(), ts: nowTs()) }
        await until("update merged") { a.updatesReceived > seen }
        a.onUpdate = {}
        XCTAssertFalse(a.catchingUp, "heard while connected")
    }

    func testPushTokenIsSentAfterWelcome() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let s = storage()
        let pre = SyncClient(storage: s, tokens: MemoryTokenStore())
        pre.setPushToken("a1b2c3", env: "development") // before any connection
        let c = client("ws://127.0.0.1:\(port)", me: 1, storage: s)
        await until("token registered") { server.registeredPushTokens[c.settings.device] == "a1b2c3" }
        c.setPushToken("ffee", env: "development")
        await until("new token registered") { server.registeredPushTokens[c.settings.device] == "ffee" }
    }

    /// An edit the server turns down (a space shared to view only): the
    /// server's words show, the team's version comes back and nothing is
    /// left waiting to go up.
    func testAnEditTheServerPutsBackGoesBackAndSaysWhy() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        server.putBack = [204]
        let a = client("ws://127.0.0.1:\(port)", me: 1)
        await until("connected") { a.canEdit }
        let before = a.view.tasks.first { $0.id == 204 }?.name
        XCTAssertNotNil(before)
        a.edit { d, _ in Edits.rename(&d, task: 204, to: "Not allowed here", ts: nowTs()) }
        await until("told why") { a.notice == FakeServer.putBackMessage }
        await until("the team's version is back") { a.view.tasks.first { $0.id == 204 }?.name == before }
        await until("nothing left waiting") { a.waitingCount == 0 }
        XCTAssertTrue(a.canEdit, "still connected")
    }

    func testEditNotYetOnServerSurvivesAnOlderUpdate() async throws {
        let sample = SampleWorkspace.make()
        let server = FakeServer(snapshot: sample)
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let a = client("ws://127.0.0.1:\(port)", me: 1)
        await until("connected") { a.canEdit }
        a.pushDelay = 3 // hold the push back
        a.edit { d, _ in Edits.rename(&d, task: 204, to: "Renamed before push", ts: nowTs()) }
        let seen = a.updatesReceived
        server.replace(sample) // someone else's copy without this edit
        await until("update merged") { a.updatesReceived > seen }
        XCTAssertEqual(a.view.tasks.first { $0.id == 204 }?.name, "Renamed before push")
    }
}
