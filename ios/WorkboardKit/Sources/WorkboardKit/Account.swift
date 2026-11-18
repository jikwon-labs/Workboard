// Signing in on workboard-server: the ways a hello can carry who this person
// is, the password rule, invite links, and the "forgot password" request.
//
//   POST http(s)://HOST:PORT/auth/forgot   {"email": "..."}
//   -> 200 {"ok": true, "mail": true|false}
//
// The answer is the same whether the account exists or not. mail: false
// means the server cannot send email; the person asks the owner or an admin
// for a new invite code instead. With mail: true a reset code
// (XXXX-XXXX-XXXX, valid 30 minutes) arrives by email and goes into a hello
// with reset_code, email and new_password.
//
// Invite link: workboard://join?server=HOST[:PORT]&code=XXXX-XXXX-XXXX&email=...

import Foundation

/// The shortest password the server takes.
public let minPasswordLength = 8

/// One sign-in attempt; each one is a fresh connection whose hello carries it.
public enum SignInRequest: Equatable, Sendable {
    /// Account sign-in on any device.
    case password(email: String, password: String)
    /// A one-time invite code. With a new password the account is made (or
    /// replaced); `email` may be left out when the workspace already has the
    /// person's email. Without a password it only signs this device in.
    case invite(code: String, email: String?, newPassword: String?)
    /// The code mailed by /auth/forgot.
    case reset(email: String, code: String, newPassword: String)

    /// What is wrong before asking the server, or nil.
    public var problem: String? {
        switch self {
        case let .password(email, password):
            if email.isEmpty { return "Type your email." }
            if password.isEmpty { return "Type your password." }
        case let .invite(code, _, newPassword):
            if code.isEmpty { return "Type the invite code." }
            if let p = newPassword, let why = passwordProblem(p) { return why }
        case let .reset(email, code, newPassword):
            if email.isEmpty { return "Type your email." }
            if code.isEmpty { return "Type the code from the email." }
            if let why = passwordProblem(newPassword) { return why }
        }
        return nil
    }

    /// The hello fields for this attempt (device and tag come from the client).
    func fill(_ h: inout Hello) {
        switch self {
        case let .password(email, password):
            h.email = email
            h.password = password
        case let .invite(code, email, newPassword):
            h.invite = code
            h.email = email.flatMap { $0.isEmpty ? nil : $0 }
            h.newPassword = newPassword
        case let .reset(email, code, newPassword):
            h.email = email
            h.resetCode = code
            h.newPassword = newPassword
        }
    }

    /// Trimmed (passwords are kept as typed).
    var cleaned: SignInRequest {
        func t(_ s: String) -> String { s.trimmingCharacters(in: .whitespacesAndNewlines) }
        switch self {
        case let .password(e, p): return .password(email: t(e), password: p)
        case let .invite(c, e, p): return .invite(code: t(c), email: e.map(t), newPassword: p)
        case let .reset(e, c, p): return .reset(email: t(e), code: t(c), newPassword: p)
        }
    }
}

/// Why a new password won't do, or nil.
public func passwordProblem(_ p: String) -> String? {
    p.count < minPasswordLength ? "Use at least \(minPasswordLength) characters for the password." : nil
}

/// What an invite link says.
public struct InviteLink: Equatable, Sendable {
    /// As typed in the server field: ws://host[:port] (or wss://, from an https link).
    public var server: String?
    public var code: String
    public var email: String?

    public init(server: String?, code: String, email: String?) {
        self.server = server
        self.code = code
        self.email = email
    }

    /// Finds an invite link in `text`: the link itself, or a whole email that
    /// contains one. Also takes the server's web page link
    /// (http://host:port/join?code=...&email=...). Nil when there is none.
    public static func find(in text: String) -> InviteLink? {
        for prefix in ["workboard://join", "https://", "http://"] {
            var rest = text[...]
            while let r = rest.range(of: prefix, options: .caseInsensitive) {
                let tail = rest[r.lowerBound...]
                let end = tail.firstIndex { $0.isWhitespace || "\"'<>()[]".contains($0) } ?? tail.endIndex
                if let link = parse(String(tail[..<end])) { return link }
                rest = rest[r.upperBound...]
            }
        }
        return nil
    }

    /// Reads one link (workboard://join?..., or http(s)://host/join?...).
    public static func parse(_ s: String) -> InviteLink? {
        guard let c = URLComponents(string: s.trimmingCharacters(in: .whitespacesAndNewlines.union(CharacterSet(charactersIn: ".,;")))),
              let scheme = c.scheme?.lowercased() else { return nil }
        func q(_ name: String) -> String? {
            c.queryItems?.first { $0.name == name }?.value?.trimmingCharacters(in: .whitespacesAndNewlines).nilIfEmpty
        }
        guard let code = q("code") else { return nil }
        var server: String?
        switch scheme {
        case "workboard":
            // workboard://join?... (host "join"), or workboard:join?...
            guard c.host?.lowercased() == "join" || c.path.lowercased().hasSuffix("join") else { return nil }
            server = q("server").map { a in a.contains("://") ? a : "ws://" + a }
        case "http", "https":
            guard c.path.hasSuffix("/join"), let host = c.host else { return nil }
            let shown = host.contains(":") ? "[\(host)]" : host
            // the join page of a workspace other than the server's first: /w/<id>/join
            let below = ServerAddress.workspaceID(inPath: String(c.path.dropLast(5))).map { "/w/" + $0 } ?? ""
            server = (scheme == "https" ? "wss://" : "ws://") + shown + (c.port.map { ":\($0)" } ?? "") + below
        default:
            return nil
        }
        return InviteLink(server: server, code: code, email: q("email"))
    }

    /// The link for these values (percent-encoded).
    public var url: URL? {
        var c = URLComponents()
        c.scheme = "workboard"
        c.host = "join"
        var items: [URLQueryItem] = []
        if let s = server {
            let bare = s.replacingOccurrences(of: "ws://", with: "")
            items.append(URLQueryItem(name: "server", value: bare))
        }
        items.append(URLQueryItem(name: "code", value: code))
        if let e = email { items.append(URLQueryItem(name: "email", value: e)) }
        c.queryItems = items
        // "+" in an email must stay a plus, not become a space on the other side
        c.percentEncodedQuery = c.percentEncodedQuery?.replacingOccurrences(of: "+", with: "%2B")
        return c.url
    }
}

extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
}

public enum AuthError: Error, Equatable, Sendable {
    case badAddress
    case failed(String)
}

public enum AuthAPI {
    /// Ask for a password reset code. Returns whether the server can mail it.
    public static func forgot(server: String, email: String, session: URLSession = .shared) async throws -> Bool {
        guard let url = ServerAddress.http(server, path: "/auth/forgot") else { throw AuthError.badAddress }
        var req = URLRequest(url: url, timeoutInterval: 15)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = JSON.object(["email": .string(email.trimmingCharacters(in: .whitespacesAndNewlines))]).data()
        let (data, resp): (Data, URLResponse)
        do {
            (data, resp) = try await session.data(for: req)
        } catch {
            throw AuthError.failed(error.localizedDescription)
        }
        let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
        let j = try? JSON(parsing: data)
        if status == 200, let j { return j["mail"]?.bool ?? false }
        // the server's reason ("error"; "message" from older servers): a
        // deleted workspace (410), one that never was (404)
        if let why = j?["error"]?.string ?? j?["message"]?.string { throw AuthError.failed(why) }
        throw AuthError.failed(status == 404 ? "This server can't reset passwords. Ask the owner or an admin for a new invite code." : "The server answered \(status).")
    }

    /// Whether the server has a workspace yet (GET /health "workspace"); nil
    /// when it can't be asked or doesn't say. An empty server is set up by
    /// the first Mac app that connects (it brings its workspace with a
    /// `claim`); the iPhone has no workspace of its own to bring.
    public static func hasWorkspace(server: String, session: URLSession = .shared) async -> Bool? {
        guard let url = ServerAddress.http(server, path: "/health") else { return nil }
        guard let (data, resp) = try? await session.data(for: URLRequest(url: url, timeoutInterval: 10)),
              (resp as? HTTPURLResponse)?.statusCode == 200, let j = try? JSON(parsing: data) else { return nil }
        return j["workspace"]?.bool
    }

    /// What to say on the sign-in screen when the server is still empty.
    public static let emptyServerNote = "This server has no workspace yet. Set it up from Workboard on the Mac first (it brings its workspace and makes you the owner), then make an invite code there or set a password, and sign in here."
}

/// Deleting a workspace other than the server's first, as its owner
/// (server/src/registry.rs):
///
///   DELETE http(s)://HOST:PORT/w/<id>   Authorization: Bearer <device>:<token>
///   -> 200 {"deleted": id}; 403/401 {"error"}; 410 when it is gone already
///
/// Its apps are then told with error {code: "auth", message: goneMessage,
/// gone: true}, and connecting to it answers 410.
public enum WorkspaceAPI {
    /// What the server says to the apps of a deleted workspace (wb_net::WORKSPACE_GONE).
    public static let goneMessage = "This workspace was deleted from the server by its owner."

    /// Delete the workspace at `server` (an address with /w/<id>).
    public static func delete(server: String, device: String, token: String, session: URLSession = .shared) async throws {
        guard ServerAddress.workspaceID(server) != nil else {
            throw AuthError.failed("The server's first workspace can't be deleted.")
        }
        guard let url = ServerAddress.http(server, path: "") else { throw AuthError.badAddress }
        var req = URLRequest(url: url, timeoutInterval: 20)
        req.httpMethod = "DELETE"
        req.setValue("Bearer \(device):\(token)", forHTTPHeaderField: "Authorization")
        let (data, resp): (Data, URLResponse)
        do {
            (data, resp) = try await session.data(for: req)
        } catch {
            throw AuthError.failed(error.localizedDescription)
        }
        let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
        if status == 200 || status == 410 { return }
        let said = (try? JSON(parsing: data))?["error"]?.string
        throw AuthError.failed(said ?? (status == 404 || status == 405 ? "This server can't delete workspaces yet. Update workboard-server first." : "The server answered \(status)."))
    }
}

/// Turning what a person typed into the addresses the app uses, the way the
/// Mac app does (wb_net::server_url): "macmini", "host:9000",
/// "ws://host:8787" and "ws://host:8787/ws" all work.
public enum ServerAddress {
    public static let defaultPort = 8787
    public static let path = "/ws"

    /// The WebSocket address, or nil when it can't be one.
    public static func websocket(_ address: String) -> URL? {
        var a = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !a.isEmpty, !a.contains(where: \.isWhitespace) else { return nil }
        if !a.contains("://") { a = "ws://" + a }
        guard var c = URLComponents(string: a), let scheme = c.scheme?.lowercased(), let host = c.host, !host.isEmpty else { return nil }
        switch scheme {
        case "ws", "http": c.scheme = "ws"
        case "wss", "https": c.scheme = "wss"
        default: return nil
        }
        if c.port == nil, c.scheme == "ws" { c.port = defaultPort }
        if let id = workspaceID(inPath: c.path) {
            // a workspace other than the server's first: its connection is below it
            c.path = "/w/" + id + path
        } else if c.path.isEmpty || c.path == "/" {
            c.path = path
        }
        return c.url
    }

    /// The workspace id in an address path ("/w/<id>" or "/w/<id>/ws"); nil
    /// for the server's first workspace (see registry.rs in the server).
    public static func workspaceID(inPath p: String) -> String? {
        guard p.hasPrefix("/w/") else { return nil }
        var rest = String(p.dropFirst(3))
        if rest.hasSuffix(path) { rest = String(rest.dropLast(path.count)) }
        while rest.hasSuffix("/") { rest.removeLast() }
        return rest.isEmpty || rest.contains("/") ? nil : rest
    }

    /// The workspace id in an address; nil for the server's first workspace.
    public static func workspaceID(_ address: String) -> String? {
        guard let ws = websocket(address), let c = URLComponents(url: ws, resolvingAgainstBaseURL: false) else { return nil }
        return workspaceID(inPath: c.path)
    }

    /// http(s) on the same host and port, below the workspace's /w/<id>
    /// when the address has one.
    public static func http(_ address: String, path: String) -> URL? {
        guard let ws = websocket(address), var c = URLComponents(url: ws, resolvingAgainstBaseURL: false) else { return nil }
        let below = workspaceID(inPath: c.path).map { "/w/" + $0 } ?? ""
        c.scheme = c.scheme == "wss" ? "https" : "http"
        c.path = below + path
        c.query = nil
        return c.url
    }

    /// Whether two typed addresses point at the same server.
    public static func same(_ a: String, _ b: String) -> Bool {
        guard let x = websocket(a), let y = websocket(b) else { return a == b }
        return x.absoluteString.lowercased() == y.absoluteString.lowercased()
    }
}
