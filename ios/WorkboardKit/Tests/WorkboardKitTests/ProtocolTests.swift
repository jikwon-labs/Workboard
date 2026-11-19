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
            .pushToken(token: "ab12", platform: "ios", env: "development"),
            .pushToken(token: "ab12", platform: "ios", env: "production", workspace: "k1"), .callJoin(channel: 301), .ping, .pong,
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
        // a workspace other than the server's first, below /w/<id>
        XCTAssertEqual(ServerAddress.websocket("https://wb.example.com/w/ab12")?.absoluteString, "wss://wb.example.com/w/ab12/ws")
        XCTAssertEqual(ServerAddress.websocket("macmini/w/ab12/ws")?.absoluteString, "ws://macmini:8787/w/ab12/ws")
        XCTAssertEqual(ServerAddress.http("https://wb.example.com/w/ab12", path: "/files/abc")?.absoluteString, "https://wb.example.com/w/ab12/files/abc")
        XCTAssertEqual(ServerAddress.http("macmini/w/ab12", path: "/auth/forgot")?.absoluteString, "http://macmini:8787/w/ab12/auth/forgot")
        XCTAssertEqual(ServerAddress.workspaceID("https://wb.example.com/w/ab12"), "ab12")
        XCTAssertNil(ServerAddress.workspaceID("https://wb.example.com"))
        XCTAssertFalse(ServerAddress.same("macmini/w/ab12", "macmini/w/cd34"))
        XCTAssertTrue(ServerAddress.same("macmini/w/ab12", "ws://macmini:8787/w/ab12/ws"))
        XCTAssertEqual(InviteLink.parse("https://wb.example.com/w/ab12/join?code=C")?.server, "wss://wb.example.com/w/ab12")
        XCTAssertEqual(InviteLink.parse("workboard://join?server=wb.example.com%2Fw%2Fab12&code=C")?.server, "ws://wb.example.com/w/ab12")
        XCTAssertEqual(CallAPI.room(channel: 10), "wb-10")
        XCTAssertEqual(CallAPI.room(channel: 10, workspace: "ab12"), "wb-ab12-10")
    }
}
