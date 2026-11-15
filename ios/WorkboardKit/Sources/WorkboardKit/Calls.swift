// Asking workboard-server for a call: POST /call-token on the same host and
// port as the WebSocket (docs/server-design.md phase 3). The answer says
// where the LiveKit server is and carries a join token for one room.
//
//   POST http://HOST:PORT/call-token
//   Authorization: Bearer <device token>     (phase 2; empty in phase 1)
//   {"channel": 301}
//   -> 200 {"url": "ws://mac-mini...:7880", "token": "<jwt>", "room": "workboard-301"}
//   -> 404 / 501   the server has no calls: the app falls back to a link call
//   -> 401         sign in again

import Foundation

public struct CallGrant: Equatable, Sendable {
    public let url: String
    public let token: String
    public let room: String
}

public enum CallError: Error, Equatable, Sendable {
    /// No in-app calls on this server; post a link call instead.
    case notAvailable
    case unauthorized
    case badAddress
    case failed(String)
}

public enum CallAPI {
    /// ws://host:port/... -> http://host:port/call-token (wss -> https).
    public static func endpoint(forServer address: String) -> URL? {
        ServerAddress.http(address, path: "/call-token")
    }

    public static func requestToken(server: String, deviceToken: String?, channel: UInt64, session: URLSession = .shared) async throws -> CallGrant {
        guard let url = endpoint(forServer: server) else { throw CallError.badAddress }
        var req = URLRequest(url: url, timeoutInterval: 10)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if let t = deviceToken, !t.isEmpty { req.setValue("Bearer \(t)", forHTTPHeaderField: "Authorization") }
        req.httpBody = JSON.object(["channel": .id(channel)]).data()
        let (data, resp): (Data, URLResponse)
        do {
            (data, resp) = try await session.data(for: req)
        } catch {
            throw CallError.failed(error.localizedDescription)
        }
        let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
        switch status {
        case 200:
            guard let j = try? JSON(parsing: data), let u = j["url"]?.string, let t = j["token"]?.string else { throw CallError.failed("unreadable answer") }
            return CallGrant(url: u, token: t, room: j["room"]?.string ?? "")
        case 401, 403: throw CallError.unauthorized
        case 404, 405, 501: throw CallError.notAvailable
        default: throw CallError.failed("server answered \(status)")
        }
    }

    /// The link call the Mac app makes when there is no call server
    /// (call_link in src/app/together.rs).
    public static func linkCall() -> String {
        let n = UInt64.random(in: 0...UInt64.max)
        return "https://meet.jit.si/Workboard-" + String(format: "%016llx", n)
    }
}
