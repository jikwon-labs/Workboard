// A test-only stand-in for workboard-server (server/src/lib.rs). It speaks
// the same messages (wb_net::Msg, see Protocol.swift) but does not merge: it
// keeps the last pushed copy as the snapshot and sends it straight back to
// everyone as an `update`. Real merging is the server's job. With `deltas`
// it behaves like the real server does with apps that take deltas: it
// merges pushes (whole or partial) with Sync.merge and sends `delta`s.
//
// Like the real axum server, WebSocket and plain HTTP share one port:
//   GET  /health       -> {"ok":true,"rev":N,"workspace":bool}
//   POST /auth/forgot  -> {"ok":true,"mail":true}; the reset code is logged
//                         (printed by wb-fake-server) instead of mailed
//   PUT/GET /files/<sha256>  pictures and videos (server/src/files.rs), with
//                         Authorization: Bearer <device>:<token>
// so the WebSocket handshake and frames are done by hand on a TCP listener.
// Huddles go over the WebSocket like the real server: call_join {channel}
// -> call_token {channel, url, token}, or error {code: "denied", channel}.
//
// Optional sign-in (`requireSignIn`), like the real server: hello must carry
// a reset code, an invite code, an email and password, or a device token,
// else the answer is error {code: "auth"} and the connection closes.
// Accounts (email + password) are kept in memory. invite(member:) makes a
// one-time code, revoke(member:) signs that member's devices out.
// Without `requireSignIn` every hello is welcomed without a member, and the
// app lets the person pick who they are.
//
// Listens on 127.0.0.1 only unless told otherwise.

import CryptoKit
import Foundation
import Network
import WorkboardKit

public struct LiveKitConfig: Sendable {
    public var url: String
    public var key: String
    public var secret: String
    public init(url: String, key: String, secret: String) {
        self.url = url
        self.key = key
        self.secret = secret
    }
    /// What `livekit-server --dev` uses.
    public static let dev = LiveKitConfig(url: "ws://127.0.0.1:7880", key: "devkey", secret: "secret")
}

public final class FakeServer: @unchecked Sendable {
    private let queue = DispatchQueue(label: "workboard.fake-server")
    private var listener: NWListener?
    private var peers: [ObjectIdentifier: Peer] = [:]
    private var snapshot: JSON?
    private var rev: Int64 = 0
    /// device -> tag, so a device keeps its tag across reconnects.
    private var tags: [String: UInt64] = [:]
    /// normalized code (letters and digits only, upper case) -> member
    private var invites: [String: UInt64] = [:]
    /// token -> (member, device)
    private var tokens: [String: (member: UInt64, device: String)] = [:]
    private var pushTokens: [String: String] = [:]
    /// member -> account
    private var accounts: [UInt64: Account] = [:]
    /// normalized reset code -> (email, made at)
    private var resets: [String: (email: String, made: Date)] = [:]
    private var lastResetCode: [String: String] = [:]
    /// sha256 -> (contents, content type)
    private var files: [String: (data: Data, type: String)] = [:]

    struct Account {
        var email: String
        var password: String
    }

    /// Both run on the server's queue; they must not call back into the server.
    public var log: (String) -> Void = { _ in }
    public var onPush: (JSON) -> Void = { _ in }
    public let requireSignIn: Bool
    public let livekit: LiveKitConfig?
    /// Merge pushes and send deltas to apps that take them.
    public var deltas = false
    /// What the snapshot held at the last broadcast (deltas mode).
    private var summary: SyncSummary?
    private var loseNextDelta = false
    private var resyncCount = 0
    /// Tests: the next delta goes to nobody, as if it were lost.
    public func dropNextDelta() { queue.sync { loseNextDelta = true } }
    /// Tests: how many resync requests came in.
    public var resyncs: Int { queue.sync { resyncCount } }
    /// What /auth/forgot answers for "mail" (false: this server can't send email).
    public var canMail = true
    /// Test-only HTTP admin endpoints (POST /admin/invite?member=N,
    /// /admin/revoke?member=N, /admin/say?channel=C&author=A&text=T,
    /// /admin/reset-code?email=E, /admin/drop-account?member=N), so UI
    /// tests in the simulator can drive sign-in and incoming messages.
    public var adminHTTP = false

    final class Peer {
        let conn: NWConnection
        var buffer = Data()
        var isWebSocket = false
        var device: String?
        var member: UInt64?
        var token: String?
        var welcomed = false
        var fragments = Data()
        var deltas = false
        init(_ c: NWConnection) { conn = c }
    }

    /// Runs on the server queue only.
    final class Once: @unchecked Sendable { var done = false }

    public init(snapshot: JSON?, requireSignIn: Bool = false, livekit: LiveKitConfig? = nil) {
        self.snapshot = snapshot
        self.requireSignIn = requireSignIn
        self.livekit = livekit
    }

    /// Starts listening; returns the port (pass 0 to pick a free one).
    public func start(host: String = "127.0.0.1", port: UInt16) async throws -> UInt16 {
        let params = NWParameters.tcp
        params.allowLocalEndpointReuse = true
        params.requiredLocalEndpoint = .hostPort(host: NWEndpoint.Host(host), port: NWEndpoint.Port(rawValue: port) ?? .any)
        let l = try NWListener(using: params)
        listener = l
        l.newConnectionHandler = { [weak self] c in self?.accept(c) }
        return try await withCheckedThrowingContinuation { cont in
            let once = Once()
            l.stateUpdateHandler = { st in
                guard !once.done else { return }
                switch st {
                case .ready:
                    once.done = true
                    cont.resume(returning: l.port?.rawValue ?? port)
                case .failed(let e):
                    once.done = true
                    cont.resume(throwing: e)
                default: break
                }
            }
            l.start(queue: queue)
        }
    }

    /// Stops listening and drops every connection (the server going away).
    public func stop() {
        queue.sync {
            listener?.cancel()
            listener = nil
            for p in peers.values { p.conn.cancel() }
            peers.removeAll()
        }
    }

    public var current: JSON? { queue.sync { snapshot } }
    public var currentRev: Int64 { queue.sync { rev } }
    public var connectionCount: Int { queue.sync { peers.values.filter(\.welcomed).count } }
    public var registeredPushTokens: [String: String] { queue.sync { pushTokens } }

    /// Replace the snapshot (as if another app pushed) and tell everyone.
    public func replace(_ s: JSON) {
        queue.async {
            self.snapshot = s
            self.rev += 1
            if self.deltas { self.summary = SyncSummary(of: s) }
            self.broadcast(.update(data: s, rev: self.rev))
        }
    }

    /// A one-time invite code for a member ("workboard-server invite").
    public func invite(member: UInt64) -> String {
        let code = makeCode()
        queue.sync { invites[Self.normalize(code)] = member }
        return code
    }

    /// XXXX-XXXX-XXXX without look-alike letters, like the real server.
    private func makeCode() -> String {
        let alphabet = Array("ABCDEFGHJKMNPQRSTUVWXYZ23456789")
        func four() -> String { String((0..<4).map { _ in alphabet.randomElement()! }) }
        return four() + "-" + four() + "-" + four()
    }

    /// What people type: any case, with or without dashes and spaces.
    static func normalize(_ code: String) -> String {
        String(code.uppercased().filter { $0.isASCII && ($0.isLetter || $0.isNumber) })
    }

    /// The account email of a member, if they have one.
    public func accountEmail(member: UInt64) -> String? { queue.sync { accounts[member]?.email } }

    /// The newest reset code made by /auth/forgot for an email (tests).
    public func resetCode(for email: String) -> String? { queue.sync { lastResetCode[email.lowercased()] } }

    /// Remove a member's account (tests: a device signed in before passwords).
    public func dropAccount(member: UInt64) { queue.sync { _ = accounts.removeValue(forKey: member) } }

    /// Cut off every device signed in as this member ("workboard-server revoke").
    public func revoke(member: UInt64) {
        queue.sync { revokeOnQueue(member) }
    }

    private func revokeOnQueue(_ member: UInt64) {
        for (t, v) in tokens where v.member == member { tokens[t] = nil }
        for p in peers.values where p.member == member {
            refuse(p, "Your access to this server was removed.")
        }
    }

    // MARK: connections

    private func accept(_ c: NWConnection) {
        let p = Peer(c)
        peers[ObjectIdentifier(c)] = p
        c.stateUpdateHandler = { [weak self] st in
            switch st {
            case .failed, .cancelled: self?.peers[ObjectIdentifier(c)] = nil
            default: break
            }
        }
        c.start(queue: queue)
        read(p)
    }

    private func drop(_ p: Peer) {
        p.conn.cancel()
        peers[ObjectIdentifier(p.conn)] = nil
    }

    private func read(_ p: Peer) {
        p.conn.receive(minimumIncompleteLength: 1, maximumLength: 1 << 20) { [weak self] data, _, done, err in
            guard let self else { return }
            if let data { p.buffer.append(data) }
            if p.isWebSocket { self.readFrames(p) } else { self.readHTTP(p) }
            if err != nil || done {
                self.drop(p)
                return
            }
            if self.peers[ObjectIdentifier(p.conn)] != nil { self.read(p) }
        }
    }

    // MARK: HTTP

    private func readHTTP(_ p: Peer) {
        guard let end = p.buffer.range(of: Data("\r\n\r\n".utf8)) else { return }
        let head = String(decoding: p.buffer[..<end.lowerBound], as: UTF8.self)
        var lines = head.components(separatedBy: "\r\n")
        let request = lines.removeFirst().split(separator: " ").map(String.init)
        var headers: [String: String] = [:]
        for l in lines {
            if let i = l.firstIndex(of: ":") {
                headers[l[..<i].lowercased()] = l[l.index(after: i)...].trimmingCharacters(in: .whitespaces)
            }
        }
        let length = Int(headers["content-length"] ?? "0") ?? 0
        let bodyStart = end.upperBound
        guard p.buffer.endIndex - bodyStart >= length else { return }
        let body = Data(p.buffer[bodyStart..<(bodyStart + length)])
        p.buffer.removeSubrange(..<(bodyStart + length))
        let method = request.first ?? ""
        let path = request.count > 1 ? request[1] : "/"

        if headers["upgrade"]?.lowercased() == "websocket", let key = headers["sec-websocket-key"] {
            let accept = Data(Insecure.SHA1.hash(data: Data((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").utf8))).base64EncodedString()
            let resp = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: \(accept)\r\n\r\n"
            p.conn.send(content: Data(resp.utf8), completion: .contentProcessed { _ in })
            p.isWebSocket = true
            readFrames(p)
            return
        }
        switch (method, path) {
        case ("GET", "/health"):
            respond(p, 200, ["ok": true, "rev": .int(rev), "workspace": .bool(snapshot != nil), "devices": .int(Int64(tokens.count))])
        case ("POST", "/auth/forgot"):
            forgot(p, body: body)
        case (_, _) where path.hasPrefix("/files/"):
            file(p, method: method, name: String(path.dropFirst(7)), headers: headers, body: body)
        case ("POST", _) where adminHTTP && path.hasPrefix("/admin/"):
            let comps = URLComponents(string: path)
            let member = comps?.queryItems?.first { $0.name == "member" }?.value.flatMap { UInt64($0) } ?? 0
            switch comps?.path {
            case "/admin/invite":
                let code = makeCode()
                invites[Self.normalize(code)] = member
                log("admin: invite for member \(member): \(code)")
                respond(p, 200, ["code": .string(code)])
            case "/admin/say":
                // a message from someone else, as if their app had pushed it
                let q = { (n: String) in comps?.queryItems?.first { $0.name == n }?.value }
                let ch = q("channel").flatMap { UInt64($0) } ?? 0
                let author = q("author").flatMap { UInt64($0) } ?? 2
                var snap = snapshot ?? [:]
                Edits.sendMessage(&snap, channel: ch, text: q("text") ?? "Hello", me: author, id: UInt64(Date().timeIntervalSince1970 * 1000) * 1024 + 1023, ts: nowTs())
                snapshot = snap
                rev += 1
                broadcast(.update(data: snap, rev: rev))
                log("admin: message in \(ch) from \(author)")
                respond(p, 200, ["ok": true])
            case "/admin/reset-code":
                let email = comps?.queryItems?.first { $0.name == "email" }?.value?.lowercased() ?? ""
                respond(p, 200, ["code": lastResetCode[email].map(JSON.string) ?? .null])
            case "/admin/drop-account":
                accounts[member] = nil
                log("admin: dropped the account of member \(member)")
                respond(p, 200, ["ok": true])
            case "/admin/revoke":
                revokeOnQueue(member)
                log("admin: revoked member \(member)")
                respond(p, 200, ["ok": true])
            default:
                respond(p, 404, ["error": "not_found"])
            }
        default:
            respond(p, 404, ["error": "not_found"])
        }
    }

    /// POST /auth/forgot: always ok; a code only when the account exists.
    private func forgot(_ p: Peer, body: Data) {
        let email = ((try? JSON(parsing: body))?["email"]?.string ?? "").trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if canMail, accounts.values.contains(where: { $0.email == email }) {
            let code = makeCode()
            resets = resets.filter { $0.value.email != email }
            resets[Self.normalize(code)] = (email, Date())
            lastResetCode[email] = code
            log("reset code for \(email): \(code)")
        }
        respond(p, 200, ["ok": true, "mail": .bool(canMail)])
    }

    private func respond(_ p: Peer, _ status: Int, _ body: JSON) {
        let text = body.serialized()
        let reason = [200: "OK", 201: "Created", 401: "Unauthorized", 404: "Not Found", 400: "Bad Request", 501: "Not Implemented"][status] ?? "Error"
        let resp = "HTTP/1.1 \(status) \(reason)\r\nContent-Type: application/json\r\nContent-Length: \(text.utf8.count)\r\nConnection: close\r\n\r\n\(text)"
        p.conn.send(content: Data(resp.utf8), completion: .contentProcessed { _ in p.conn.cancel() })
    }

    /// call_join, like Hub::call in server/src/lib.rs.
    private func callJoin(_ ch: UInt64, from p: Peer) {
        func deny(_ why: String) { send(.error(code: ServerErrorCode.denied.rawValue, message: why, channel: ch), to: p) }
        guard let lk = livekit else { return deny("Voice huddles aren't set up on this server yet.") }
        guard let snap = snapshot else { return deny("There's no workspace on the server yet.") }
        let view = WorkspaceView(snap)
        guard let c = view.channels.first(where: { $0.id == ch }) else { return deny("That conversation isn't on the server yet. Try again in a moment.") }
        let member = p.member ?? 0
        if requireSignIn, !c.members.isEmpty, !c.members.contains(member) { return deny("You're not in that conversation.") }
        let identity = "\(member)-\(p.device ?? "device")"
        let name = view.member(member)?.name ?? ""
        let jwt = Self.livekitToken(key: lk.key, secret: lk.secret, identity: identity, name: name, room: CallAPI.room(channel: ch))
        log("call token for \(identity) in \(CallAPI.room(channel: ch))")
        send(.callToken(channel: ch, url: lk.url, token: jwt), to: p)
    }

    /// PUT/GET/HEAD /files/<sha256>, like server/src/files.rs (no ranges).
    private func file(_ p: Peer, method: String, name: String, headers: [String: String], body: Data) {
        guard name.count == 64, name.allSatisfy({ $0.isHexDigit && !$0.isUppercase }) else { return respond(p, 404, ["error": "no such file"]) }
        if requireSignIn {
            let auth = headers["authorization"] ?? ""
            let rest = auth.hasPrefix("Bearer ") ? String(auth.dropFirst(7)) : ""
            let parts = rest.split(separator: ":", maxSplits: 1).map(String.init)
            guard parts.count == 2, let t = tokens[parts[1]], t.device == parts[0] else { return respond(p, 401, ["error": "sign in first"]) }
        }
        switch method {
        case "PUT":
            if files[name] != nil { return respond(p, 200, ["ok": "already here"]) }
            let sha = SHA256.hash(data: body).map { String(format: "%02x", $0) }.joined()
            guard sha == name else { return respond(p, 400, ["error": "contents don't match the name"]) }
            files[name] = (body, headers["content-type"] ?? "application/octet-stream")
            log("stored file \(name.prefix(8)) (\(body.count) bytes)")
            respond(p, 201, ["ok": "stored"])
        case "GET", "HEAD":
            guard let f = files[name] else { return respond(p, 404, ["error": "no such file"]) }
            let head = "HTTP/1.1 200 OK\r\nContent-Type: \(f.type)\r\nContent-Length: \(f.data.count)\r\nConnection: close\r\n\r\n"
            let payload = method == "GET" ? Data(head.utf8) + f.data : Data(head.utf8)
            p.conn.send(content: payload, completion: .contentProcessed { _ in p.conn.cancel() })
        default:
            respond(p, 404, ["error": "not_found"])
        }
    }

    /// A file put here (tests).
    public func storedFile(_ sha: String) -> Data? { queue.sync { files[sha]?.data } }

    /// A LiveKit access token (HS256 JWT with a video grant).
    public static func livekitToken(key: String, secret: String, identity: String, name: String, room: String, now: Date = Date()) -> String {
        func b64(_ d: Data) -> String {
            d.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        }
        let t = Int64(now.timeIntervalSince1970)
        let header: JSON = ["alg": "HS256", "typ": "JWT"]
        let claims: JSON = [
            "iss": .string(key), "sub": .string(identity), "name": .string(name),
            "nbf": .int(t - 10), "exp": .int(t + 6 * 3600),
            "video": ["room": .string(room), "roomJoin": true, "canPublish": true, "canSubscribe": true],
        ]
        let signing = b64(header.data()) + "." + b64(claims.data())
        let mac = HMAC<SHA256>.authenticationCode(for: Data(signing.utf8), using: SymmetricKey(data: Data(secret.utf8)))
        return signing + "." + b64(Data(mac))
    }

    // MARK: WebSocket frames

    private func readFrames(_ p: Peer) {
        while true {
            let b = [UInt8](p.buffer.prefix(14))
            guard b.count >= 2 else { return }
            let fin = b[0] & 0x80 != 0
            let opcode = b[0] & 0x0F
            let masked = b[1] & 0x80 != 0
            var len = UInt64(b[1] & 0x7F)
            var at = 2
            if len == 126 {
                guard b.count >= 4 else { return }
                len = UInt64(b[2]) << 8 | UInt64(b[3])
                at = 4
            } else if len == 127 {
                guard b.count >= 10 else { return }
                len = (2..<10).reduce(UInt64(0)) { $0 << 8 | UInt64(b[$1]) }
                at = 10
            }
            let maskLen = masked ? 4 : 0
            guard UInt64(p.buffer.count) >= UInt64(at + maskLen) + len else { return }
            let start = p.buffer.startIndex
            let mask = masked ? [UInt8](p.buffer[(start + at)..<(start + at + 4)]) : []
            var payload = [UInt8](p.buffer[(start + at + maskLen)..<(start + at + maskLen + Int(len))])
            if masked { for i in payload.indices { payload[i] ^= mask[i % 4] } }
            p.buffer.removeSubrange(start..<(start + at + maskLen + Int(len)))
            switch opcode {
            case 0x0, 0x1, 0x2:
                p.fragments.append(contentsOf: payload)
                if fin {
                    let text = String(decoding: p.fragments, as: UTF8.self)
                    p.fragments = Data()
                    handle(text, from: p)
                }
            case 0x8:
                frame(p, opcode: 0x8, Data(payload.prefix(2)))
                drop(p)
                return
            case 0x9:
                frame(p, opcode: 0xA, Data(payload))
            default:
                break
            }
        }
    }

    private func frame(_ p: Peer, opcode: UInt8, _ payload: Data) {
        var head: [UInt8] = [0x80 | opcode]
        let n = payload.count
        if n < 126 {
            head.append(UInt8(n))
        } else if n <= 0xFFFF {
            head += [126, UInt8(n >> 8), UInt8(n & 0xFF)]
        } else {
            head.append(127)
            for s in stride(from: 56, through: 0, by: -8) { head.append(UInt8((UInt64(n) >> UInt64(s)) & 0xFF)) }
        }
        p.conn.send(content: Data(head) + payload, completion: .contentProcessed { _ in })
    }

    // MARK: protocol

    private func handle(_ text: String, from p: Peer) {
        guard let m = ClientMessage(text: text) else {
            log("unreadable message")
            return
        }
        if !p.welcomed {
            // the first message must be hello
            guard case let .hello(h) = m else {
                send(.error(code: nil, message: "Say hello first."), to: p)
                drop(p)
                return
            }
            hello(h, from: p)
            return
        }
        switch m {
        case .hello:
            break
        case let .push(s):
            onPush(s)
            if deltas, var snap = snapshot {
                Sync.merge(&snap, [s])
                snapshot = snap
                rev += 1
                log("push from \(p.device?.prefix(8) ?? "?") merged -> rev \(rev)")
                let part = summary.flatMap { Sync.delta(since: $0, snap) } ?? [:]
                summary = SyncSummary(of: snap)
                let lost = loseNextDelta
                loseNextDelta = false
                for q in peers.values where q.welcomed && !(lost && q.deltas) {
                    send(q.deltas ? .delta(data: part, rev: rev, base: rev - 1) : .update(data: snap, rev: rev), to: q)
                }
                return
            }
            snapshot = s
            rev += 1
            log("push from \(p.device?.prefix(8) ?? "?") -> rev \(rev)")
            broadcast(.update(data: s, rev: rev))
        case .resync:
            resyncCount += 1
            if let snap = snapshot { send(.update(data: snap, rev: rev), to: p) }
        case let .setPassword(email, password, old):
            setPassword(email: email, password: password, old: old, from: p)
        case let .pushToken(token, platform, env):
            if let d = p.device { pushTokens[d] = token }
            log("push token from \(p.device?.prefix(8) ?? "?") (\(platform), \(env))")
        case let .callJoin(ch):
            callJoin(ch, from: p)
        case .ping:
            send(.pong, to: p)
        case .pong:
            break
        }
    }

    /// Check a hello the way the real server does (first that applies:
    /// reset code, invite, email + password, token).
    private func hello(_ h: Hello, from p: Peer) {
        p.device = h.device
        var newToken: String?
        if requireSignIn {
            let email = h.email?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            let member: UInt64
            if let code = h.resetCode {
                guard let r = resets[Self.normalize(code)], Date().timeIntervalSince(r.made) < 30 * 60, r.email == email,
                      let m = accounts.first(where: { $0.value.email == r.email })?.key else {
                    return refuse(p, "That reset code doesn't work. It may be used already or expired (codes last 30 minutes).")
                }
                guard let pw = h.newPassword, pw.count >= minPasswordLength else { return refuse(p, "Use at least 8 characters for the password.") }
                resets[Self.normalize(code)] = nil
                accounts[m]?.password = pw
                member = m
                newToken = addDevice(h.device, member: m)
            } else if let code = h.invite {
                guard let m = invites[Self.normalize(code)] else {
                    return refuse(p, "That invite code doesn't work. It may be used already or expired (codes last 7 days). Ask for a new one.")
                }
                if let pw = h.newPassword {
                    guard pw.count >= minPasswordLength else { return refuse(p, "Use at least 8 characters for the password.") }
                    let recorded = memberEmail(m)
                    guard let e = email ?? recorded else {
                        return refuse(p, "Type the email you want to sign in with. The workspace has none for you.")
                    }
                    if accounts.contains(where: { $0.key != m && $0.value.email == e }) {
                        return refuse(p, "Someone else on this server already uses that email.")
                    }
                    accounts[m] = Account(email: e, password: pw)
                }
                invites[Self.normalize(code)] = nil
                member = m
                newToken = addDevice(h.device, member: m)
            } else if let e = email, let pw = h.password {
                guard let (m, _) = accounts.first(where: { $0.value.email == e && $0.value.password == pw }) else {
                    return refuse(p, "That email and password don't match.")
                }
                member = m
                newToken = addDevice(h.device, member: m)
            } else if let token = h.token, let t = tokens[token] {
                member = t.member
                p.token = token
            } else if h.token != nil {
                return refuse(p, "This device isn't signed in to this server anymore. Sign in again.")
            } else {
                return refuse(p, "This server is private. Sign in, or use the invite code you were given.")
            }
            if let t = newToken { p.token = t }
            p.member = member
        }
        let tag = assignTag(device: h.device, wanted: h.tag)
        p.welcomed = true
        p.deltas = deltas && h.deltas
        if deltas, summary == nil, let snap = snapshot { summary = SyncSummary(of: snap) }
        log("hello from \(h.device.prefix(8)) -> tag \(tag)" + (p.member.map { " as member \($0)" } ?? ""))
        let role = p.member.map { $0 == 1 ? "owner" : "member" }
        var w = Welcome(tag: tag, snapshot: snapshot, rev: rev, member: p.member, role: role, token: newToken,
                        email: p.member.flatMap { accounts[$0]?.email })
        w.deltas = deltas
        send(.welcome(w), to: p)
    }

    private func addDevice(_ device: String, member: UInt64) -> String {
        let token = (0..<4).map { _ in String(format: "%08x", UInt32.random(in: 0...UInt32.max)) }.joined()
        tokens[token] = (member, device)
        return token
    }

    private func memberEmail(_ m: UInt64) -> String? {
        let e = (WorkspaceView(snapshot ?? [:]).member(m)?.email ?? "").trimmingCharacters(in: .whitespaces).lowercased()
        return e.isEmpty ? nil : e
    }

    /// error {code: "auth"} and close, like the real server.
    private func refuse(_ p: Peer, _ why: String) {
        log("refused \(p.device?.prefix(8) ?? "?"): \(why)")
        p.welcomed = false
        p.member = nil
        p.token = nil
        send(.error(code: ServerErrorCode.auth.rawValue, message: why), to: p)
        frame(p, opcode: 0x8, Data([0x03, 0xE8]))
        let c = p.conn
        peers[ObjectIdentifier(c)] = nil
        // let the error and close frames go out first
        c.send(content: nil, contentContext: .finalMessage, isComplete: true, completion: .contentProcessed { _ in c.cancel() })
    }

    private func setPassword(email: String, password: String, old: String?, from p: Peer) {
        guard let m = p.member else {
            send(.error(code: ServerErrorCode.denied.rawValue, message: "This server has no sign-in."), to: p)
            return
        }
        let e = email.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard e.contains("@") else { return send(.error(code: ServerErrorCode.denied.rawValue, message: "That doesn't look like an email."), to: p) }
        guard password.count >= minPasswordLength else {
            return send(.error(code: ServerErrorCode.denied.rawValue, message: "Use at least 8 characters for the password."), to: p)
        }
        if let a = accounts[m], a.password != (old ?? "") {
            return send(.error(code: ServerErrorCode.denied.rawValue, message: "Your current password isn't right."), to: p)
        }
        if accounts.contains(where: { $0.key != m && $0.value.email == e }) {
            return send(.error(code: ServerErrorCode.denied.rawValue, message: "Someone else on this server already uses that email."), to: p)
        }
        accounts[m] = Account(email: e, password: password)
        log("account for member \(m): \(e)")
        send(.account(email: e), to: p)
    }

    private func assignTag(device: String, wanted: UInt64?) -> UInt64 {
        if let t = tags[device] { return t }
        let used = Set(tags.values)
        var t = wanted.map { $0 % 1024 } ?? 1
        if t == 0 || used.contains(t) {
            t = (1..<1024).first { !used.contains(UInt64($0)) }.map(UInt64.init) ?? 1
        }
        tags[device] = t
        return t
    }

    private func broadcast(_ m: ServerMessage) {
        for p in peers.values where p.welcomed { send(m, to: p) }
    }

    private func send(_ m: ServerMessage, to p: Peer) {
        frame(p, opcode: 0x1, Data(m.text.utf8))
    }
}
