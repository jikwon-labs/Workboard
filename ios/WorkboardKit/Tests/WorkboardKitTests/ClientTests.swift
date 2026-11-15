import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

final class ProtocolTests: XCTestCase {
    func testClientMessagesRoundTrip() {
        let msgs: [ClientMessage] = [
            .hello(Hello(device: "d1", tag: 5)), .hello(Hello(device: "d2", token: "t")),
            .hello(Hello(device: "d3", invite: "ABCD-EFGH-JKMN", email: "a@b.c", newPassword: "12345678")),
            .hello(Hello(device: "d4", email: "a@b.c", password: "pw")),
            .hello(Hello(device: "d5", email: "a@b.c", newPassword: "12345678", resetCode: "QQQQ-WWWW-EEEE")),
            .push(data: ["tasks": []]), .setPassword(email: "a@b.c", password: "12345678", oldPassword: nil),
            .setPassword(email: "a@b.c", password: "12345678", oldPassword: "old-pass"),
            .pushToken(token: "ab12", platform: "ios", env: "development"), .callJoin(channel: 301), .ping, .pong,
        ]
        for m in msgs { XCTAssertEqual(ClientMessage(text: m.text), m) }
    }

    /// The exact shape the real server reads (wb_net::Msg): absent fields are
    /// left out, never sent as null or "".
    func testWireShapeMatchesTheRealServer() {
        XCTAssertEqual(ClientMessage.hello(Hello(device: "d1", tag: 3)).text, #"{"device":"d1","tag":3,"type":"hello"}"#)
        XCTAssertEqual(ClientMessage.hello(Hello(device: "d1", token: "")).text, #"{"device":"d1","type":"hello"}"#)
        XCTAssertEqual(ClientMessage.hello(Hello(device: "d", invite: "C", email: "e", newPassword: "p")).json,
                       ["type": "hello", "device": "d", "invite": "C", "email": "e", "new_password": "p"])
        XCTAssertEqual(ClientMessage.hello(Hello(device: "d", email: "e", newPassword: "p", resetCode: "R")).json,
                       ["type": "hello", "device": "d", "email": "e", "new_password": "p", "reset_code": "R"])
        XCTAssertEqual(ClientMessage.push(data: ["a": 1]).json, ["type": "push", "data": ["a": 1]])
        XCTAssertEqual(ClientMessage.setPassword(email: "e", password: "p", oldPassword: nil).json, ["type": "set_password", "email": "e", "password": "p"])
        XCTAssertEqual(ClientMessage.setPassword(email: "e", password: "p", oldPassword: "o").json["old_password"], "o")
    }

    func testServerMessagesRoundTrip() {
        let msgs: [ServerMessage] = [
            .welcome(Welcome(tag: 3, snapshot: ["a": 1], rev: 9)),
            .welcome(Welcome(tag: 4, snapshot: nil, rev: 0, member: 2, role: "member", token: "tok", email: "a@b.c")),
            .update(data: ["b": 2], rev: 10), .account(email: "a@b.c"), .ping, .pong,
            .error(code: "auth", message: "no"), .error(code: nil, message: "Say hello first."),
            .callToken(channel: 301, url: "ws://x:7880", token: "jwt"), .error(code: "denied", message: "no", channel: 301),
        ]
        for m in msgs { XCTAssertEqual(ServerMessage(text: m.text), m) }
        // as the real server writes them (serde, Option as null, absent token)
        XCTAssertEqual(ServerMessage(text: #"{"type":"welcome","tag":7,"snapshot":null,"rev":0,"member":3,"role":"admin"}"#),
                       .welcome(Welcome(tag: 7, snapshot: nil, rev: 0, member: 3, role: "admin")))
        XCTAssertEqual(ServerMessage(text: #"{"type":"update","data":{"x":1},"rev":2}"#), .update(data: ["x": 1], rev: 2))
        XCTAssertEqual(ServerMessage(text: #"{"type":"error","message":"m"}"#), .error(code: nil, message: "m"))
        XCTAssertEqual(ServerMessage(text: #"{"type":"access","people":[],"role":"owner"}"#), .access(people: [], role: .owner))
        XCTAssertEqual(ServerMessage(text: "garbage"), .unknown("unreadable"))
    }

    func testAddressesPeopleTypeBecomeWebSocketURLs() {
        XCTAssertEqual(SyncClient.url(from: "ws://mac-mini.example.ts.net:8787")?.absoluteString, "ws://mac-mini.example.ts.net:8787/ws")
        XCTAssertEqual(SyncClient.url(from: "ws://h:9000/ws")?.absoluteString, "ws://h:9000/ws")
        XCTAssertEqual(SyncClient.url(from: "macmini")?.absoluteString, "ws://macmini:8787/ws")
        XCTAssertEqual(SyncClient.url(from: " mac-mini:9000 ")?.absoluteString, "ws://mac-mini:9000/ws")
        XCTAssertEqual(SyncClient.url(from: "http://h:1")?.absoluteString, "ws://h:1/ws")
        XCTAssertEqual(SyncClient.url(from: "wss://host")?.absoluteString, "wss://host/ws")
        XCTAssertNil(SyncClient.url(from: ""))
        XCTAssertNil(SyncClient.url(from: "ftp://host"))
        XCTAssertNil(SyncClient.url(from: "two words"))
        XCTAssertEqual(ServerAddress.http("ws://h:1/ws", path: "/auth/forgot")?.absoluteString, "http://h:1/auth/forgot")
        XCTAssertEqual(ServerAddress.http("wss://h", path: "/auth/forgot")?.absoluteString, "https://h/auth/forgot")
        XCTAssertTrue(ServerAddress.same("macmini", "ws://macmini:8787/ws"))
        XCTAssertFalse(ServerAddress.same("macmini", "ws://other:8787"))
    }
}

final class AccountTests: XCTestCase {
    func testInviteLinkIsRead() {
        let l = InviteLink.parse("workboard://join?server=mac-mini.tail1.ts.net:8787&code=ABCD-EFGH-JKMN&email=sam%2Bwork%40example.com")
        XCTAssertEqual(l, InviteLink(server: "ws://mac-mini.tail1.ts.net:8787", code: "ABCD-EFGH-JKMN", email: "sam+work@example.com"))
        XCTAssertEqual(InviteLink.parse("workboard://join?server=10.0.0.5&code=X")?.server, "ws://10.0.0.5")
        XCTAssertEqual(InviteLink.parse("workboard://join?code=X"), InviteLink(server: nil, code: "X", email: nil))
        XCTAssertNil(InviteLink.parse("workboard://join?server=h"), "no code")
        XCTAssertNil(InviteLink.parse("workboard://other?code=X"))
        XCTAssertNil(InviteLink.parse("ABCD-EFGH-JKMN"))
        // the server's web page link
        XCTAssertEqual(InviteLink.parse("http://mac-mini:8787/join?code=ABCD-EFGH-JKMN&email=a%40b.c"),
                       InviteLink(server: "ws://mac-mini:8787", code: "ABCD-EFGH-JKMN", email: "a@b.c"))
        XCTAssertEqual(InviteLink.parse("https://wb.example.com/join?code=C")?.server, "wss://wb.example.com")
        XCTAssertNil(InviteLink.parse("https://example.com/pricing?code=C"))
    }

    func testInviteLinkIsFoundInAPastedEmail() {
        let mail = """
        Hi Sam,

        Jamie invited you to Sample Team on Workboard.
        Open this on your iPhone: <workboard://join?server=mac-mini.tail1.ts.net%3A8787&code=QWER-TYUP-ASDF&email=sam%40example.com>.
        Or visit https://example.com/help for help.
        """
        XCTAssertEqual(InviteLink.find(in: mail), InviteLink(server: "ws://mac-mini.tail1.ts.net:8787", code: "QWER-TYUP-ASDF", email: "sam@example.com"))
        XCTAssertEqual(InviteLink.find(in: "join here: http://10.1.2.3:8787/join?code=AAAA-BBBB-CCCC (7 days)")?.code, "AAAA-BBBB-CCCC")
        XCTAssertNil(InviteLink.find(in: "AAAA-BBBB-CCCC"))
        XCTAssertNil(InviteLink.find(in: "see https://example.com/x"))
    }

    func testInviteLinkRoundTrips() {
        let l = InviteLink(server: "ws://host:8787", code: "ABCD-EFGH-JKMN", email: "a+b@c.d")
        let url = try! XCTUnwrap(l.url)
        XCTAssertTrue(url.absoluteString.hasPrefix("workboard://join?server=host:8787&code=ABCD-EFGH-JKMN"))
        XCTAssertEqual(InviteLink.parse(url.absoluteString), l)
    }

    func testSignInRequestsAreCheckedBeforeSending() {
        XCTAssertNotNil(SignInRequest.password(email: "", password: "x").problem)
        XCTAssertNil(SignInRequest.password(email: "a@b.c", password: "x").problem, "old passwords are the server's business")
        XCTAssertNotNil(SignInRequest.invite(code: "C", email: nil, newPassword: "short").problem)
        XCTAssertNil(SignInRequest.invite(code: "C", email: nil, newPassword: "12345678").problem)
        XCTAssertNil(SignInRequest.invite(code: "C", email: nil, newPassword: nil).problem)
        XCTAssertNotNil(SignInRequest.reset(email: "a@b.c", code: "", newPassword: "12345678").problem)
        XCTAssertNotNil(SignInRequest.reset(email: "a@b.c", code: "R", newPassword: "1234567").problem)
        var h = Hello(device: "d")
        SignInRequest.invite(code: "C", email: "", newPassword: "12345678").fill(&h)
        XCTAssertNil(h.email, "an empty email is left out, so the server uses the member record")
    }
}

@MainActor
final class ClientTests: XCTestCase {
    var dirs: [URL] = []

    override func tearDown() async throws {
        for d in dirs { try? FileManager.default.removeItem(at: d) }
    }

    func storage() -> Storage {
        let d = FileManager.default.temporaryDirectory.appendingPathComponent("wbkit-\(UUID().uuidString)")
        dirs.append(d)
        return Storage(folder: d)
    }

    func client(_ url: String, me: UInt64, storage s: Storage? = nil) -> SyncClient {
        let c = SyncClient(storage: s ?? storage(), tokens: MemoryTokenStore())
        c.pushDelay = 0.05
        c.maxRetryDelay = 0.3
        c.setMe(me)
        c.setServer(url)
        return c
    }

    func until(_ what: String, timeout: TimeInterval = 5, _ cond: @escaping () -> Bool) async {
        let end = Date().addingTimeInterval(timeout)
        while !cond() {
            if Date() > end { XCTFail("timed out waiting for \(what)"); return }
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
    }

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

    func signInServer(_ server: FakeServer) async throws -> String {
        let port = try await server.start(port: 0)
        return "ws://127.0.0.1:\(port)"
    }

    func device(_ url: String, tokens: MemoryTokenStore = MemoryTokenStore()) -> SyncClient {
        let c = SyncClient(storage: storage(), tokens: tokens)
        c.maxRetryDelay = 0.3
        c.setServer(url)
        return c
    }

    func testInviteCodeWithNewPasswordThenPasswordOnASecondDevice() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await signInServer(server)
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

    func testInviteWithoutRecordedEmailNeedsOne() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await signInServer(server)
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
        let url = try await signInServer(server)
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
        let url = try await signInServer(server)
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
        let url = try await signInServer(server)
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

final class HTTPTests: XCTestCase {
    func testHealthAndLinkCall() async throws {
        XCTAssertTrue(CallAPI.linkCall().hasPrefix("https://meet.jit.si/Workboard-"))
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let (data, resp) = try await URLSession.shared.data(from: URL(string: "http://127.0.0.1:\(port)/health")!)
        XCTAssertEqual((resp as? HTTPURLResponse)?.statusCode, 200)
        XCTAssertEqual(try JSON(parsing: data)["ok"], true)
    }
}

/// Huddles over the WebSocket (call_join -> call_token), as on the real server.
@MainActor
final class CallTests: XCTestCase {
    func storage() -> Storage {
        Storage(folder: FileManager.default.temporaryDirectory.appendingPathComponent("wbkit-\(UUID().uuidString)"))
    }

    func until(_ what: String, timeout: TimeInterval = 5, _ cond: @escaping () -> Bool) async {
        let end = Date().addingTimeInterval(timeout)
        while !cond() {
            if Date() > end { XCTFail("timed out waiting for \(what)"); return }
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
    }

    func testCallJoinGivesATokenForTheRoom() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true, livekit: .dev)
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let code = server.invite(member: 2)
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        c.setServer("ws://127.0.0.1:\(port)")
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: code, email: "alex@example.test", newPassword: "alex-password-1"))
        await until("signed in") { c.canEdit && c.state.isConnected }

        let grant = try await c.requestCall(channel: 301)
        XCTAssertEqual(grant.url, "ws://127.0.0.1:7880")
        XCTAssertEqual(grant.room, "wb-301")
        var b64 = String(grant.token.split(separator: ".")[1]).replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        while b64.count % 4 != 0 { b64 += "=" }
        let claims = try JSON(parsing: Data(base64Encoded: b64)!)
        XCTAssertEqual(claims.at("video.room"), "wb-301")
        let identity = try XCTUnwrap(claims["sub"]?.string)
        XCTAssertEqual(identity, "2-\(c.settings.device)", "<member>-<device>, as the real server names people")
        XCTAssertEqual(CallAPI.member(inIdentity: identity), 2)
        XCTAssertEqual(CallAPI.member(inIdentity: "member-7"), 7)

        // a conversation the server doesn't have: turned down, connection stays
        do {
            _ = try await c.requestCall(channel: 999)
            XCTFail("expected denied")
        } catch let e as CallError {
            XCTAssertEqual(e, .denied("That conversation isn't on the server yet. Try again in a moment."))
        }
        XCTAssertTrue(c.state.isConnected)
        XCTAssertNil(c.notice, "a call refusal is the huddle's business, not a notice")
    }

    func testServerWithoutLiveKitMeansLinkCall() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        c.setMe(1)
        c.setServer("ws://127.0.0.1:\(port)")
        await until("connected") { c.state.isConnected }
        do {
            _ = try await c.requestCall(channel: 301)
            XCTFail("expected notAvailable")
        } catch let e as CallError { XCTAssertEqual(e, .notAvailable) }

        c.disconnect()
        do {
            _ = try await c.requestCall(channel: 301)
            XCTFail("expected offline")
        } catch let e as CallError { XCTAssertEqual(e, .offline) }
        XCTAssertEqual(CallAPI.huddleChannel("workboard-huddle:301"), 301)
        XCTAssertNil(CallAPI.huddleChannel("https://meet.jit.si/x"))
    }
}

/// Round 2: stamps in milliseconds, moving to another server, files, empty servers.
@MainActor
final class Round2ClientTests: XCTestCase {
    func storage() -> Storage {
        Storage(folder: FileManager.default.temporaryDirectory.appendingPathComponent("wbkit-\(UUID().uuidString)"))
    }

    func until(_ what: String, timeout: TimeInterval = 5, _ cond: @escaping () -> Bool) async {
        let end = Date().addingTimeInterval(timeout)
        while !cond() {
            if Date() > end { XCTFail("timed out waiting for \(what)"); return }
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
    }

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

/// Round 3: push tokens taken back, managing access and backups.
@MainActor
final class Round3ClientTests: XCTestCase {
    var dirs: [URL] = []

    override func tearDown() async throws {
        for d in dirs { try? FileManager.default.removeItem(at: d) }
    }

    func storage() -> Storage {
        let d = FileManager.default.temporaryDirectory.appendingPathComponent("wbkit-\(UUID().uuidString)")
        dirs.append(d)
        return Storage(folder: d)
    }

    func until(_ what: String, timeout: TimeInterval = 5, _ cond: @escaping () -> Bool) async {
        let end = Date().addingTimeInterval(timeout)
        while !cond() {
            if Date() > end { XCTFail("timed out waiting for \(what)"); return }
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
    }

    func client(_ url: String, me: UInt64) -> SyncClient {
        let c = SyncClient(storage: storage(), tokens: MemoryTokenStore())
        c.pushDelay = 0.05
        c.maxRetryDelay = 0.3
        c.setMe(me)
        c.setServer(url)
        return c
    }

    func signInServer(_ server: FakeServer) async throws -> String {
        "ws://127.0.0.1:\(try await server.start(port: 0))"
    }

    func device(_ url: String, tokens: MemoryTokenStore = MemoryTokenStore()) -> SyncClient {
        let c = SyncClient(storage: storage(), tokens: tokens)
        c.maxRetryDelay = 0.3
        c.setServer(url)
        return c
    }

    /// Signs a new device in as `member` with a fresh invite code.
    func signedIn(_ server: FakeServer, _ url: String, member: UInt64, tokens: MemoryTokenStore = MemoryTokenStore()) async -> SyncClient {
        let c = device(url, tokens: tokens)
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: server.invite(member: member), email: nil, newPassword: nil))
        await until("signed in as \(member)") { c.canEdit && c.state.isConnected }
        return c
    }

    func testSignOutStopsPushesFromThatServer() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await signInServer(server)
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

    func testSwitchingServersStopsPushesFromTheOldOne() async throws {
        let a = FakeServer(snapshot: SampleWorkspace.make())
        let b = FakeServer(snapshot: SampleWorkspace.make())
        let pa = try await a.start(port: 0), pb = try await b.start(port: 0)
        defer { a.stop(); b.stop() }
        let c = client("ws://127.0.0.1:\(pa)", me: 1)
        c.setPushToken("ffee", env: "development")
        await until("registered on a") { a.registeredPushTokens[c.settings.device] == "ffee" }
        c.setServer("ws://127.0.0.1:\(pb)")
        c.setMe(1)
        await until("removed from a") { a.registeredPushTokens[c.settings.device] == nil }
        await until("registered on b") { b.registeredPushTokens[c.settings.device] == "ffee" }
        XCTAssertEqual(c.settings.pushSentTo, "ws://127.0.0.1:\(pb)")

        // switching while disconnected goes over a connection of its own
        c.disconnect()
        c.setServer("ws://127.0.0.1:\(pa)")
        await until("removed from b") { b.registeredPushTokens[c.settings.device] == nil }
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
        let url = try await signInServer(server)
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
        let url = try await signInServer(server)
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

/// Deltas (wb_sync::delta, wb-net's session): after the first whole copy
/// only what changed travels, both ways.
@MainActor
final class DeltaTests: XCTestCase {
    func ws(_ tasks: JSON, _ channels: JSON) -> JSON {
        ["workspace": "W", "spaces": [], "tasks": tasks, "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": channels, "docs": [], "extra": ["sync": SyncMeta().json]]
    }

    func save(_ v: inout JSON, _ now: Int64) {
        var m = SyncMeta(of: v)
        Sync.stamp(v, &m, now: now)
        v.set("extra.sync", m.json)
    }

    /// The Rust test a_delta_holds_only_what_changed_and_merges_like_the_whole_copy.
    func testADeltaHoldsOnlyWhatChangedAndMergesLikeTheWholeCopy() {
        var a = ws([["id": 1, "name": "one"], ["id": 2, "name": "two"]],
                   [["id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [["id": 11, "author": 1, "text": "hi", "ts": 5]]]])
        a["spaces"] = [["id": 100, "name": "S", "folders": [], "lists": [["id": 101, "name": "L"]]], ["id": 200, "name": "T", "folders": [], "lists": []]]
        save(&a, 10)
        let other = a
        var sum = SyncSummary(of: Sync.outgoing(a))
        XCTAssertNil(Sync.delta(since: sum, Sync.outgoing(a)), "nothing changed")
        var chans = a["channels"]!.array!
        chans[0]["messages"] = .array(chans[0]["messages"]!.array! + [["id": 12, "author": 1, "text": "new", "ts": 6]])
        a["channels"] = .array(chans)
        a["tasks"] = [["id": 1, "name": "one!"]]
        a["spaces"] = [["id": 100, "name": "S", "folders": [], "lists": []], ["id": 200, "name": "T", "folders": [], "lists": [["id": 101, "name": "L"]]]]
        a["workspace"] = "Renamed"
        save(&a, 20)
        let d = Sync.delta(since: sum, Sync.outgoing(a))!
        XCTAssertEqual(d["tasks"], [["id": 1, "name": "one!"]])
        XCTAssertEqual(d["channels"]?[0]?["messages"]?.array?.count, 1, "only the new message")
        XCTAssertEqual(d["workspace"], "Renamed")
        XCTAssertNotNil(d.at("extra.sync.tombs.2"))
        XCTAssertNil(d["members"])
        var x = other, y = other
        Sync.merge(&x, [d])
        Sync.merge(&y, [Sync.outgoing(a)])
        XCTAssertEqual(Sync.stripSync(x), Sync.stripSync(y))
        XCTAssertEqual(SyncMeta(of: x).stamps, SyncMeta(of: y).stamps)
        sum.absorb(d)
        XCTAssertNil(Sync.delta(since: sum, Sync.outgoing(a)), "once the server has it, nothing is left")
    }

    func testDeltaMessagesRoundTrip() {
        let m = ServerMessage.delta(data: ["tasks": []], rev: 5, base: 4, hidden: [9])
        XCTAssertEqual(ServerMessage(text: m.text), m)
        XCTAssertEqual(ServerMessage(text: #"{"type":"delta","data":{"x":1},"rev":3,"base":2}"#), .delta(data: ["x": 1], rev: 3, base: 2))
        XCTAssertEqual(ClientMessage(text: ClientMessage.resync.text), .resync)
        var h = Hello(device: "d")
        h.deltas = true
        XCTAssertEqual(ClientMessage.hello(h).json["deltas"], true)
        guard case let .welcome(w) = ServerMessage(text: #"{"type":"welcome","tag":1,"snapshot":null,"rev":0,"deltas":true}"#) else { return XCTFail() }
        XCTAssertTrue(w.deltas)
    }

    func client(_ url: String, me: UInt64) -> SyncClient {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("wb-delta-\(UUID().uuidString)")
        let c = SyncClient(storage: Storage(folder: dir), tokens: MemoryTokenStore())
        c.pushDelay = 0.05
        c.maxRetryDelay = 0.3
        c.setMe(me)
        c.setServer(url)
        return c
    }

    func until(_ what: String, timeout: TimeInterval = 5, _ cond: @escaping () -> Bool) async {
        let end = Date().addingTimeInterval(timeout)
        while !cond() {
            if Date() > end { XCTFail("timed out waiting for \(what)"); return }
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
    }

    final class Pushes: @unchecked Sendable {
        let lock = NSLock()
        var all: [JSON] = []
        func add(_ j: JSON) { lock.lock(); all.append(j); lock.unlock() }
        var last: JSON? { lock.lock(); defer { lock.unlock() }; return all.last }
    }

    func testThePhonePushesAndGetsOnlyWhatChanged() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        server.deltas = true
        let pushes = Pushes()
        server.onPush = { pushes.add($0) }
        let port = try await server.start(port: 0)
        defer { server.stop() }
        let url = "ws://127.0.0.1:\(port)"
        let a = client(url, me: 1), b = client(url, me: 2)
        await until("both connected") { a.canEdit && b.canEdit }
        a.edit { d, _ in Edits.rename(&d, task: 201, to: "renamed on A", ts: nowTs()) }
        await until("the rename at B") { b.view.tasks.first { $0.id == 201 }?.name == "renamed on A" }
        let p = try XCTUnwrap(pushes.last)
        XCTAssertEqual(p["tasks"]?.array?.compactMap { $0["id"]?.uint }, [201], "only the renamed task went up")
        XCTAssertNil(p["channels"])
        // two phones changing different fields of one task at once
        a.edit { d, _ in Edits.rename(&d, task: 202, to: "A's name", ts: nowTs()) }
        b.edit { d, me in Edits.setStatus(&d, task: 202, to: "in progress", me: me, ts: nowTs()) }
        for c in [a, b] {
            await until("both changes") {
                let t = c.view.tasks.first { $0.id == 202 }
                return t?.name == "A's name" && t?.status == "in progress"
            }
        }
        // a lost revision: the next delta doesn't follow, so B asks for everything
        server.dropNextDelta()
        a.edit { d, _ in Edits.rename(&d, task: 203, to: "lost on the way", ts: nowTs()) }
        try await Task.sleep(nanoseconds: 300_000_000)
        a.edit { d, _ in Edits.rename(&d, task: 204, to: "after the loss", ts: nowTs()) }
        await until("B caught up") {
            b.view.tasks.first { $0.id == 203 }?.name == "lost on the way"
                && b.view.tasks.first { $0.id == 204 }?.name == "after the loss"
        }
        XCTAssertGreaterThanOrEqual(server.resyncs, 1)
    }
}

/// Round 4: Trash that leaves with its person, notifications only for news,
/// admin answers matched to their requests, files sent again after a restore.
@MainActor
final class Round4ClientTests: XCTestCase {
    var dirs: [URL] = []

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

    func device(_ url: String, storage s: Storage? = nil, tokens: MemoryTokenStore = MemoryTokenStore()) -> SyncClient {
        let c = SyncClient(storage: s ?? storage(), tokens: tokens)
        c.pushDelay = 0.05
        c.maxRetryDelay = 0.3
        c.setServer(url)
        return c
    }

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
        // someone else's change: an access list nobody here asked for yet
        c.receiveForTests(.access(people: before, role: .owner))
        XCTAssertEqual(c.adminPending, 2, "not what the role change asked for")
        XCTAssertEqual(c.people, before, "the list is taken all the same")
        // refusals of pushed changes aren't answers
        c.receiveForTests(.error(code: "denied", message: FakeServer.putBackMessage, putBack: true))
        c.receiveForTests(.error(code: "denied", message: SyncClient.updateNeeded))
        XCTAssertEqual(c.adminPending, 2)
        XCTAssertNil(c.adminError)
        XCTAssertEqual(c.notice, SyncClient.updateNeeded)
        c.receiveForTests(.access(people: after, role: .owner))
        XCTAssertEqual(c.asks, [.accessList])
        // a plain member's list is never the answer to an owner's request
        c.receiveForTests(.access(people: [], role: .member))
        XCTAssertEqual(c.asks, [.accessList])
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

/// The fake server going away at the same address (what the offline UI test uses).
@MainActor
final class OfflineServerTests: XCTestCase {
    func testAnEditMadeWhileTheServerIsAwayGoesUpWhenItIsBack() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make())
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let c = SyncClient(storage: Storage(folder: FileManager.default.temporaryDirectory.appendingPathComponent("wbkit-\(UUID().uuidString)")), tokens: MemoryTokenStore())
        c.maxRetryDelay = 0.3
        c.setMe(1)
        c.setServer(url)
        func until(_ what: String, _ cond: () -> Bool) async {
            let end = Date().addingTimeInterval(8)
            while !cond() {
                if Date() > end { XCTFail("timed out waiting for \(what)"); return }
                try? await Task.sleep(nanoseconds: 20_000_000)
            }
        }
        await until("connected") { c.state.isConnected && c.canEdit }
        server.goOffline(for: 30)
        await until("offline") { if case .waiting = c.state { return true } else { return false } }
        try await Task.sleep(nanoseconds: 700_000_000)
        XCTAssertFalse(c.state.isConnected, "turned away while offline")
        c.edit { d, me in Edits.sendMessage(&d, channel: 302, text: "Written on the train", me: me, id: c.newId(), ts: nowTs()) }
        XCTAssertEqual(c.waitingCount, 1)
        server.goOnline()
        await until("sent and shown back") { c.state.isConnected && c.waitingCount == 0 }
        XCTAssertTrue(server.current!.serialized().contains("Written on the train"))
    }
}
