// The fake server's plain HTTP on the WebSocket port: /health, the
// handshake hand-off to WebSocket, /auth/forgot, deleting a workspace,
// files, and test-only /admin calls (see FakeServer.swift).

import CryptoKit
import Foundation
import Network
import WorkboardKit

extension FakeServer {
    func readHTTP(_ p: Peer) {
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

        if headers["upgrade"]?.lowercased() == "websocket", let until = offlineUntil, until > Date() {
            return respond(p, 503, ["error": "offline"])
        }
        if deleted, path.hasPrefix("/w/") {
            return respond(p, 410, ["error": .string(WorkspaceAPI.goneMessage)])
        }
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
        case ("POST", "/auth/workspaces"):
            listWorkspaces(p, body: body)
        case ("DELETE", _) where path.hasPrefix("/w/"):
            deleteWorkspace(p, headers: headers)
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
                let id = UInt64(Date().timeIntervalSince1970 * 1000) * 1024 + 1023
                Edits.sendMessage(&snap, channel: ch, text: q("text") ?? "Hello", me: author, id: id, ts: nowTs())
                if let key = q("imported") {
                    // as a Mac's Slack import writes it (Message::imported)
                    var chans = snap["channels"]?.array ?? []
                    for i in chans.indices where chans[i]["id"]?.uint == ch {
                        var msgs = chans[i]["messages"]?.array ?? []
                        for j in msgs.indices where msgs[j]["id"]?.uint == id { msgs[j]["imported"] = .string(key) }
                        chans[i]["messages"] = .array(msgs)
                    }
                    snap["channels"] = .array(chans)
                }
                snapshot = snap
                rev += 1
                broadcast(.update(data: snap, rev: rev))
                log("admin: message in \(ch) from \(author)")
                respond(p, 200, ["ok": true])
            case "/admin/typing":
                // someone else typing in a conversation, as if their app said so
                let q = { (n: String) in comps?.queryItems?.first { $0.name == n }?.value }
                let ch = q("channel").flatMap { UInt64($0) } ?? 0
                let who = q("member").flatMap { UInt64($0) } ?? 2
                broadcast(.typing(channel: ch, member: who))
                log("admin: \(who) typing in \(ch)")
                respond(p, 200, ["ok": true])
            case "/admin/reset-code":
                let email = comps?.queryItems?.first { $0.name == "email" }?.value?.lowercased() ?? ""
                respond(p, 200, ["code": lastResetCode[email].map(JSON.string) ?? .null])
            case "/admin/drop-account":
                accounts[member] = nil
                log("admin: dropped the account of member \(member)")
                respond(p, 200, ["ok": true])
            case "/admin/backup":
                // a backup of the current snapshot, for the owner to restore
                let name = comps?.queryItems?.first { $0.name == "name" }?.value ?? "hourly-20261116-03.json"
                backupFiles.removeAll { $0.name == name }
                backupFiles.insert((name, snapshot ?? [:]), at: 0)
                log("admin: backup \(name)")
                respond(p, 200, ["ok": true])
            case "/admin/offline":
                let secs = comps?.queryItems?.first { $0.name == "secs" }?.value.flatMap { Double($0) } ?? 30
                offlineOnQueue(secs)
                respond(p, 200, ["ok": true])
            case "/admin/workspaces":
                // what POST /auth/workspaces lists: list=ID:NAME,ID:NAME (ID
                // "" is the server's first); no list goes back to answering 404
                let list = comps?.queryItems?.first { $0.name == "list" }?.value
                workspaceList = list.map { l in
                    l.split(separator: ",").map { e in
                        let parts = e.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false).map(String.init)
                        return (id: parts[0], name: parts.count > 1 ? parts[1] : "")
                    }
                }
                log("admin: workspaces \(list ?? "none")")
                respond(p, 200, ["ok": true])
            case "/admin/online":
                offlineUntil = nil
                log("online")
                respond(p, 200, ["ok": true])
            case "/admin/find":
                // whether a message or reply with this text is on the server
                let text = comps?.queryItems?.first { $0.name == "text" }?.value ?? ""
                let found = WorkspaceView(snapshot ?? [:]).channels.contains { c in
                    c.messages.contains { $0.text == text || $0.replies.contains { $0.text == text } }
                }
                respond(p, 200, ["found": .bool(found)])
            case "/admin/late-answer":
                // the next access or backup request is answered after secs
                let secs = comps?.queryItems?.first { $0.name == "secs" }?.value.flatMap(Double.init) ?? 30
                lateAnswer = secs
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

    /// DELETE /w/<id>: an owner's device deletes the workspace, like
    /// registry.rs; its apps are told and stop.
    func deleteWorkspace(_ p: Peer, headers: [String: String]) {
        let auth = headers["authorization"] ?? ""
        let rest = auth.hasPrefix("Bearer ") ? String(auth.dropFirst(7)) : ""
        let parts = rest.split(separator: ":", maxSplits: 1).map(String.init)
        guard parts.count == 2, let t = tokens[parts[1]], t.device == parts[0] else {
            return respond(p, 401, ["error": "Sign in to this workspace first."])
        }
        guard roleOf(t.member) == .owner else {
            return respond(p, 403, ["error": "Only an owner of this workspace can delete it."])
        }
        deleted = true
        tokens = [:]
        log("workspace deleted by member \(t.member)")
        for peer in peers.values where peer.isWebSocket {
            refuse(peer, WorkspaceAPI.goneMessage)
        }
        respond(p, 200, ["deleted": "fake"])
    }

    /// POST /auth/workspaces: every workspace on this server, for an
    /// account whose email and password match (registry.rs list_route).
    func listWorkspaces(_ p: Peer, body: Data) {
        guard let list = workspaceList else { return respond(p, 404, ["error": "not_found"]) }
        let j = try? JSON(parsing: body)
        let email = (j?["email"]?.string ?? "").trimmed.lowercased()
        let password = j?["password"]?.string ?? ""
        guard accounts.values.contains(where: { $0.email == email && $0.password == password }) else {
            return respond(p, 401, ["error": "That email and password don't match."])
        }
        respond(p, 200, ["workspaces": .array(list.map { ["id": .string($0.id), "name": .string($0.name)] })])
    }

    /// POST /auth/forgot: always ok; a code only when the account exists.
    func forgot(_ p: Peer, body: Data) {
        let email = ((try? JSON(parsing: body))?["email"]?.string ?? "").trimmed.lowercased()
        if canMail, accounts.values.contains(where: { $0.email == email }) {
            let code = makeCode()
            resets = resets.filter { $0.value.email != email }
            resets[Self.normalize(code)] = (email, Date())
            lastResetCode[email] = code
            log("reset code for \(email): \(code)")
        }
        respond(p, 200, ["ok": true, "mail": .bool(canMail)])
    }

    func respond(_ p: Peer, _ status: Int, _ body: JSON) {
        let text = body.serialized()
        let reason = [200: "OK", 201: "Created", 401: "Unauthorized", 403: "Forbidden", 404: "Not Found", 410: "Gone", 400: "Bad Request", 501: "Not Implemented", 503: "Service Unavailable"][status] ?? "Error"
        let resp = "HTTP/1.1 \(status) \(reason)\r\nContent-Type: application/json\r\nContent-Length: \(text.utf8.count)\r\nConnection: close\r\n\r\n\(text)"
        p.conn.send(content: Data(resp.utf8), completion: .contentProcessed { _ in p.conn.cancel() })
    }

    /// call_join, like Hub::call in server/src/lib.rs.
    func callJoin(_ ch: UInt64, from p: Peer) {
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
    func file(_ p: Peer, method: String, name: String, headers: [String: String], body: Data) {
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
}
