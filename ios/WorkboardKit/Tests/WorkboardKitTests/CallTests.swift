import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

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
final class CallTests: FakeServerTestCase {
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

/// Hands and reactions read and written the way the Mac app does (Signal in
/// crates/wb-voice/src/lib.rs).
final class HuddleSignalTests: XCTestCase {
    func testMessagesMatchTheMacApp() {
        XCTAssertEqual(HuddleSignal.topic, "workboard-huddle")
        XCTAssertEqual(String(data: HuddleSignal.hand(true).data, encoding: .utf8), "hand:1")
        XCTAssertEqual(String(data: HuddleSignal.hand(false).data, encoding: .utf8), "hand:0")
        XCTAssertEqual(String(data: HuddleSignal.react("\u{1F44F}").data, encoding: .utf8), "react:\u{1F44F}")
        XCTAssertEqual(HuddleSignal(data: Data("hand:1".utf8)), .hand(true))
        XCTAssertEqual(HuddleSignal(data: Data("hand:0".utf8)), .hand(false))
        XCTAssertEqual(HuddleSignal(data: Data("react:\u{2764}\u{FE0F}".utf8)), .react("\u{2764}\u{FE0F}"))
        // anything else is ignored
        for bad in ["hand:2", "react:", "react:this is far too long", "wave", "", "x:y"] {
            XCTAssertNil(HuddleSignal(data: Data(bad.utf8)), bad)
        }
        XCTAssertEqual(HuddleSignal.reactions.count, 8)
    }

    func testHandsAndReactionsComeAndGo() {
        var s = HuddleSignals()
        let t0 = Date(timeIntervalSince1970: 1000)
        s.take(.hand(true), from: "2-dev")
        XCTAssertTrue(s.handUp("2-dev"))
        XCTAssertFalse(s.handUp(HuddleSignals.me))
        s.take(.hand(false), from: "2-dev")
        XCTAssertFalse(s.handUp("2-dev"))
        s.take(.hand(true), from: "2-dev")
        s.left("2-dev")
        XCTAssertFalse(s.handUp("2-dev"), "someone who left has no hand up")

        s.take(.react("\u{1F44D}"), from: "2-dev", at: t0)
        s.take(.react("\u{1F389}"), from: HuddleSignals.me, at: t0)
        XCTAssertEqual(s.reactions(of: "2-dev").map(\.emoji), ["\u{1F44D}"])
        XCTAssertEqual(s.reactions(of: HuddleSignals.me).count, 1)
        // a burst shows six at most
        for _ in 0..<10 { s.take(.react("\u{1F44D}"), from: "2-dev", at: t0) }
        XCTAssertEqual(s.reactions(of: "2-dev").count, 6)
        // and they go after a few seconds
        s.prune(t0.addingTimeInterval(HuddleSignals.reactionSeconds + 0.1))
        XCTAssertTrue(s.reactions.isEmpty)
    }
}
