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
