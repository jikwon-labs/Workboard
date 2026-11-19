import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// "Someone is typing..." in a conversation (Typing.swift), as on the Mac.
@MainActor
final class TypingTests: FakeServerTestCase {
    override var devicePushDelay: TimeInterval? { 0.05 }

    func signedIn(_ server: FakeServer, _ url: String, member: UInt64) async -> SyncClient {
        let c = device(url)
        await until("sign-in screen") { c.needsSignIn }
        c.signIn(.invite(code: server.invite(member: member), email: nil, newPassword: nil))
        await until("signed in as \(member)") { c.canEdit && c.state.isConnected && c.me == member }
        return c
    }

    func testTypingMessagesHaveTheRealServersShape() {
        XCTAssertEqual(ClientMessage.typing(channel: 301).json, ["type": "typing", "channel": 301])
        XCTAssertEqual(ClientMessage(text: #"{"type":"typing","channel":301}"#), .typing(channel: 301))
        XCTAssertEqual(ServerMessage(text: #"{"type":"typing","channel":301,"member":2}"#), .typing(channel: 301, member: 2))
        XCTAssertEqual(ServerMessage(text: ServerMessage.typing(channel: 5, member: 6).text), .typing(channel: 5, member: 6))
        XCTAssertEqual(ServerMessage(text: #"{"type":"typing","channel":301}"#), .unknown("typing without member"))
        // in a thread: the message whose thread it is goes along
        XCTAssertEqual(ClientMessage.typing(channel: 301, thread: 311).json, ["type": "typing", "channel": 301, "thread": 311])
        XCTAssertEqual(ClientMessage(text: #"{"type":"typing","channel":301,"thread":311}"#), .typing(channel: 301, thread: 311))
        XCTAssertEqual(ServerMessage(text: #"{"type":"typing","channel":301,"member":2,"thread":311}"#), .typing(channel: 301, member: 2, thread: 311))
    }

    func testTypingInAThreadShowsOnlyInThatThread() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let a = await signedIn(server, url, member: 1)
        let b = await signedIn(server, url, member: 2)

        // A answers in the thread of message 311 in #general
        a.typingNow(channel: 301, thread: 311)
        await until("B sees A typing in the thread") { b.typing(in: 301, thread: 311) == [1] }
        XCTAssertEqual(b.typing(in: 301), [], "not under #general's own box")

        // right after, typing in #general itself is another place: it goes out
        a.typingNow(channel: 301)
        await until("B sees A typing in #general") { b.typing(in: 301) == [1] }
        XCTAssertEqual(b.typing(in: 301, thread: 311), [], "one place at a time")

        // back in the thread; A's reply arrives: done typing there
        a.typingNow(channel: 301, thread: 311)
        await until("B sees A in the thread again") { b.typing(in: 301, thread: 311) == [1] }
        XCTAssertTrue(a.edit { d, me in
            Edits.sendReply(&d, channel: 301, to: 311, text: "on it", me: me, id: a.newId(), ts: nowTs())
        })
        await until("A's reply at B") {
            b.view.channels.first { $0.id == 301 }!.messages.first { $0.id == 311 }!.replies.last?.text == "on it"
        }
        XCTAssertEqual(b.typing(in: 301, thread: 311), [])
    }

    func testTypingShowsToTheOthersInTheConversationUntilTheirMessageComes() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let a = await signedIn(server, url, member: 1)
        let b = await signedIn(server, url, member: 2)
        let c = await signedIn(server, url, member: 3)

        // A types in #general: B and C see it, A doesn't see itself
        a.typingNow(channel: 301)
        await until("B sees A typing") { b.typing(in: 301) == [1] }
        await until("C sees A typing") { c.typing(in: 301) == [1] }
        XCTAssertEqual(a.typing(in: 301), [])

        // in the DM between A and B only B hears it
        a.typingNow(channel: 303)
        await until("B sees A typing in their DM") { b.typing(in: 303) == [1] }
        try await Task.sleep(nanoseconds: 200_000_000)
        XCTAssertEqual(c.typing(in: 303), [], "C isn't in that DM")

        // more keys right after aren't sent again
        let heard = b.typists.first { $0.channel == 303 }!.heard
        a.typingNow(channel: 303)
        try await Task.sleep(nanoseconds: 200_000_000)
        XCTAssertEqual(b.typists.first { $0.channel == 303 }!.heard, heard)

        // A's message arrives: done typing
        XCTAssertTrue(a.edit { d, me in
            Edits.sendMessage(&d, channel: 301, text: "typed it", me: me, id: a.newId(), ts: nowTs())
        })
        await until("A's message at B") { b.view.channels.first { $0.id == 301 }!.messages.last?.text == "typed it" }
        XCTAssertEqual(b.typing(in: 301), [])
        XCTAssertEqual(b.typing(in: 303), [1], "still typing in the DM")

        // a few seconds without word: gone
        XCTAssertEqual(b.typing(in: 303, at: Date().addingTimeInterval(TypingAPI.shows)), [])

        // not connected: nothing goes out
        a.disconnect()
        a.typedAt = nil
        a.typingNow(channel: 301)
        XCTAssertNil(a.typedAt)
    }
}
