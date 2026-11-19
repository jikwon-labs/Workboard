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
//   DELETE /w/<id>     an owner deletes the workspace (registry.rs): every
//                         app is told and connecting answers 410 afterwards
//                         (reach it at ws://HOST:PORT/w/<id>; any id does)
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
// Access requests like the real server (Hub::request): create_invite,
// set_role, revoke and access_list for owners and admins, backup_list and
// restore for the owner, with the same permission rules; each request gets
// one answer, and after a change every other connection gets its own
// `access` (the real server's access broadcast, which skips the connection
// whose request made the change). Lists say whether they answer
// (`answer`). Member 1 is the owner unless
// setRole(member:role:) says otherwise. Backups are added with
// addBackup(name:snapshot:); a restore puts that snapshot back as is.
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
    let queue = DispatchQueue(label: "workboard.fake-server")
    var listener: NWListener?
    var peers: [ObjectIdentifier: Peer] = [:]
    var snapshot: JSON?
    var rev: Int64 = 0
    /// device -> tag, so a device keeps its tag across reconnects.
    var tags: [String: UInt64] = [:]
    /// normalized code (letters and digits only, upper case) -> member
    var invites: [String: UInt64] = [:]
    /// token -> (member, device)
    var tokens: [String: (member: UInt64, device: String)] = [:]
    /// DELETE /w/<id> came from an owner: the workspace is gone
    var deleted = false
    /// Whether an owner deleted the workspace (tests).
    public var isDeleted: Bool { queue.sync { deleted } }
    var pushTokens: [String: String] = [:]
    /// device -> the phone's name for this workspace that came with its token
    var pushWorkspaces: [String: String] = [:]
    /// The notification choices each device sent with its token.
    var pushPrefs: [String: JSON] = [:]
    /// member -> account
    var accounts: [UInt64: Account] = [:]
    /// normalized reset code -> (email, made at)
    var resets: [String: (email: String, made: Date)] = [:]
    var lastResetCode: [String: String] = [:]
    /// sha256 -> (contents, content type)
    var files: [String: (data: Data, type: String)] = [:]
    /// member -> role; members not listed are plain members, member 1 the owner
    var roles: [UInt64: Role] = [1: .owner]
    /// newest first
    var backupFiles: [(name: String, snapshot: JSON)] = []

    struct Account {
        var email: String
        var password: String
    }

    /// Tasks whose changes this server turns down, as the real server does
    /// in a space shared with someone at a level that doesn't allow them:
    /// the team's version stays with its stamp past the sender's (so their
    /// copy takes it back), and the sender hears why.
    public var putBack: Set<UInt64> {
        get { queue.sync { putBackIds } }
        set { queue.sync { putBackIds = newValue } }
    }
    var putBackIds: Set<UInt64> = []
    public static let putBackMessage =
        "Some changes were put back because that space is shared with you at a level that doesn't allow them."

    /// Both run on the server's queue; they must not call back into the server.
    public var log: (String) -> Void = { _ in }
    public var onPush: (JSON) -> Void = { _ in }
    public let requireSignIn: Bool
    public let livekit: LiveKitConfig?
    /// Merge pushes and send deltas to apps that take them.
    public var deltas = false
    /// What the snapshot held at the last broadcast (deltas mode).
    var summary: SyncSummary?
    var loseNextDelta = false
    var resyncCount = 0
    /// Tests: the next delta goes to nobody, as if it were lost.
    public func dropNextDelta() { queue.sync { loseNextDelta = true } }
    var meddle: (member: UInt64, role: Role)?
    var lateAnswer: TimeInterval?
    /// Tests: another admin gives `member` this role just as the next
    /// set_role or revoke is made, before its answer is worked out (the
    /// real server answers with access_for after letting go of its lock),
    /// so that answer shows the other admin's change instead. The other
    /// admin's list reaches the asker first, unasked.
    public func meddleBeforeNextAnswer(member: UInt64, role: Role) { queue.sync { meddle = (member, role) } }
    /// Tests: the next access or backup request takes `secs` to answer, and
    /// nothing else reaches that connection meanwhile (the real server
    /// handles a connection's messages one at a time).
    public func lateNextAnswer(after secs: TimeInterval) { queue.sync { lateAnswer = secs } }
    /// Tests: how many resync requests came in.
    public var resyncs: Int { queue.sync { resyncCount } }
    /// What /auth/forgot answers for "mail" (false: this server can't send email).
    public var canMail = true
    /// Test-only HTTP admin endpoints (POST /admin/invite?member=N,
    /// /admin/revoke?member=N, /admin/backup?name=FILE, /admin/say?channel=C&author=A&text=T[&imported=KEY],
    /// /admin/reset-code?email=E, /admin/drop-account?member=N,
    /// /admin/offline?secs=N, /admin/online, /admin/find?text=T,
    /// /admin/late-answer?secs=N, /admin/typing?channel=C&member=M), so UI
    /// tests in the simulator can drive sign-in, incoming messages and the
    /// server going away.
    public var adminHTTP = false
    /// Until when the server acts as gone: its WebSocket connections were
    /// dropped and new ones are turned away (HTTP still answers, so a test
    /// can bring it back). The address stays the same.
    var offlineUntil: Date?

    /// Tests: drop every connection and turn new ones away for `secs`.
    public func goOffline(for secs: TimeInterval) { queue.sync { offlineOnQueue(secs) } }

    /// Tests: take connections again.
    public func goOnline() { queue.sync { offlineUntil = nil } }

    func offlineOnQueue(_ secs: TimeInterval) {
        offlineUntil = Date().addingTimeInterval(secs)
        for p in peers.values where p.isWebSocket { drop(p) }
        log("offline for \(Int(secs))s")
    }

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
        /// Messages held back while a late answer is being "worked out".
        var held: [ServerMessage]?
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
    public var registeredPushWorkspaces: [String: String] { queue.sync { pushWorkspaces } }

    /// Replace the snapshot (as if another app pushed) and tell everyone.
    public func replace(_ s: JSON) {
        queue.async {
            self.snapshot = s
            self.rev += 1
            if self.deltas { self.summary = SyncSummary(of: s) }
            self.broadcast(.update(data: s, rev: self.rev))
        }
    }

    public func role(of member: UInt64) -> Role { queue.sync { roleOf(member) } }

    public func setRole(member: UInt64, role: Role) {
        queue.sync { roles[member] = role == .member ? nil : role }
    }

    /// A backup the owner can list and restore (newest first).
    public func addBackup(name: String, snapshot: JSON) {
        queue.sync { backupFiles.insert((name, snapshot), at: 0) }
    }

    /// The open invite code for a member, if any.
    public func openInvite(member: UInt64) -> String? { queue.sync { inviteCode(of: member) } }

    func roleOf(_ member: UInt64) -> Role { roles[member] ?? .member }

    func inviteCode(of member: UInt64) -> String? {
        invites.first { $0.value == member }.map { k, _ in
            // codes are kept normalized; show them as XXXX-XXXX-XXXX
            stride(from: 0, to: k.count, by: 4).map { i in
                String(k[k.index(k.startIndex, offsetBy: i)..<k.index(k.startIndex, offsetBy: min(i + 4, k.count))])
            }.joined(separator: "-")
        }
    }

    /// A one-time invite code for a member ("workboard-server invite").
    public func invite(member: UInt64) -> String {
        let code = makeCode()
        queue.sync { invites[Self.normalize(code)] = member }
        return code
    }

    /// XXXX-XXXX-XXXX without look-alike letters, like the real server.
    func makeCode() -> String {
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

    func revokeOnQueue(_ member: UInt64) {
        for (t, v) in tokens where v.member == member {
            tokens[t] = nil
            pushTokens[v.device] = nil
        }
        for p in peers.values where p.member == member {
            refuse(p, "Your access to this server was removed.")
        }
    }

    // MARK: connections

    func accept(_ c: NWConnection) {
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

    func drop(_ p: Peer) {
        p.conn.cancel()
        peers[ObjectIdentifier(p.conn)] = nil
    }

    func read(_ p: Peer) {
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

    // MARK: protocol

    func handle(_ text: String, from p: Peer) {
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
        case var .push(s):
            if let before = snapshot, keepTeamVersions(&s, before) {
                send(.error(code: "denied", message: Self.putBackMessage, putBack: true), to: p)
            }
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
        case let .pushToken(token, platform, env, workspace, prefs):
            // an empty token stops notifications to this device (set_push_token)
            if let d = p.device {
                pushTokens[d] = token.isEmpty ? nil : token
                pushWorkspaces[d] = token.isEmpty ? nil : workspace
                pushPrefs[d] = token.isEmpty ? nil : prefs
            }
            log("push token from \(p.device?.prefix(8) ?? "?") (\(platform), \(env))\(token.isEmpty ? ": removed" : "")")
        case .createInvite, .setRole, .revoke, .accessList, .backupList, .restore:
            request(m, from: p)
        case let .callJoin(ch):
            callJoin(ch, from: p)
        case let .typing(ch):
            typing(ch, from: p)
        case .ping:
            send(.pong, to: p)
        case .pong:
            break
        }
    }

    /// Check a hello the way the real server does (first that applies:
    /// reset code, invite, email + password, token).
    func hello(_ h: Hello, from p: Peer) {
        p.device = h.device
        var newToken: String?
        if requireSignIn {
            let email = h.email?.trimmed.lowercased()
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
        let role = p.member.map { roleOf($0).rawValue }
        var w = Welcome(tag: tag, snapshot: snapshot, rev: rev, member: p.member, role: role, token: newToken,
                        email: p.member.flatMap { accounts[$0]?.email })
        w.deltas = deltas
        send(.welcome(w), to: p)
    }

    /// Hub::request: access and backup requests with the real server's rules.
    func request(_ m: ClientMessage, from p: Peer) {
        func deny(_ why: String) { send(.error(code: ServerErrorCode.denied.rawValue, message: why), to: p) }
        if let secs = lateAnswer {
            lateAnswer = nil
            log("answering in \(secs)s")
            hold(p, for: secs)
        }
        guard requireSignIn, let member = p.member else { return deny("This server has no sign-in.") }
        let me = roleOf(member)
        switch m {
        case .backupList, .restore:
            guard me == .owner else { return deny("Only the owner can see and restore backups.") }
            if case let .restore(name) = m {
                guard let b = backupFiles.first(where: { $0.name == name }) else { return deny("There's no backup called \(name).") }
                backupFiles.insert(("before-restore-20261116-120000.json", snapshot ?? [:]), at: 0)
                snapshot = b.snapshot
                rev += 1
                broadcast(.update(data: b.snapshot, rev: rev))
                log("restored \(name)")
                send(.restored(name: name), to: p)
            } else {
                send(.backups(backupFiles.map { Backup(name: $0.name, bytes: UInt64($0.snapshot.serialized().utf8.count)) }), to: p)
            }
            return
        default:
            break
        }
        guard me.manages else { return deny("Only the owner or an admin can do that.") }
        switch m {
        case .accessList:
            break
        case let .createInvite(who, role):
            if role == .owner { return deny("There is one owner. Invite them as an admin instead.") }
            guard WorkspaceView(snapshot ?? [:]).member(who) != nil else {
                return deny("That person isn't in the workspace yet. Add them with Invite first.")
            }
            if roleOf(who) == .owner && me != .owner { return deny("Only the owner can make a code for the owner.") }
            if me != .owner && !(roleOf(who) == .member && accounts[who] == nil) {
                return deny("Only the owner can make a code for an admin or for someone who has already signed in. They can sign in with their email and password, or use Forgot password.")
            }
            for (k, v) in invites where v == who { invites[k] = nil }
            let code = makeCode()
            invites[Self.normalize(code)] = who
            let email = canMail ? memberEmail(who) : nil
            log("invite for member \(who): \(code)" + (email.map { " (emailed to \($0))" } ?? ""))
            send(.invite(member: who, code: code, emailed: email), to: p)
            broadcastAccess(except: p)
            return
        case let .setRole(who, role):
            if role == .owner || roleOf(who) == .owner { return deny("The owner's role can't be changed here.") }
            roles[who] = role == .member ? nil : role
        case let .revoke(who):
            if roleOf(who) == .owner { return deny("The owner's access can't be removed.") }
            for (k, v) in invites where v == who { invites[k] = nil }
            accounts[who] = nil
            roles[who] = nil
            revokeOnQueue(who)
        default:
            return
        }
        if m == .accessList { return send(accessFor(member, answer: true), to: p) }
        // someone else's change landing between this one and its answer:
        // their list reaches everyone, this connection too
        if let (who, role) = meddle {
            meddle = nil
            roles[who] = role == .member ? nil : role
            log("another admin made member \(who) \(role.rawValue)")
            broadcastAccess()
        }
        send(accessFor(member, answer: true), to: p)
        // Hub::request tells every other connection
        broadcastAccess(except: p)
    }

    /// Keep what goes to `p` for `secs`, then send it in order.
    func hold(_ p: Peer, for secs: TimeInterval) {
        if p.held == nil { p.held = [] }
        queue.asyncAfter(deadline: .now() + secs) { [weak self] in
            guard let self, let held = p.held else { return }
            p.held = nil
            for m in held { self.send(m, to: p) }
        }
    }

    /// Hub::access_for: everyone's access as `member` may see it.
    func accessFor(_ member: UInt64, answer: Bool = false) -> ServerMessage {
        let viewer = roleOf(member)
        guard viewer.manages else { return .access(people: [], role: viewer, answer: answer) }
        var ids = WorkspaceView(snapshot ?? [:]).members.map(\.id)
        for t in tokens.values where !ids.contains(t.member) { ids.append(t.member) }
        let people = ids.map { m in
            Person(member: m, role: roleOf(m), devices: Set(tokens.values.filter { $0.member == m }.map(\.device)).count,
                   invite: inviteCode(of: m).flatMap { viewer == .owner || (roleOf(m) == .member && accounts[m] == nil) ? $0 : nil },
                   account: accounts[m] != nil)
        }
        return .access(people: people, role: viewer, answer: answer)
    }

    func broadcastAccess(except: Peer? = nil) {
        for q in peers.values where q.welcomed && q !== except {
            if let m = q.member { send(accessFor(m), to: q) }
        }
    }

    func addDevice(_ device: String, member: UInt64) -> String {
        let token = (0..<4).map { _ in String(format: "%08x", UInt32.random(in: 0...UInt32.max)) }.joined()
        tokens[token] = (member, device)
        return token
    }

    func memberEmail(_ m: UInt64) -> String? {
        let e = (WorkspaceView(snapshot ?? [:]).member(m)?.email ?? "").trimmingCharacters(in: .whitespaces).lowercased()
        return e.isEmpty ? nil : e
    }

    /// error {code: "auth"} and close, like the real server.
    func refuse(_ p: Peer, _ why: String) {
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

    func setPassword(email: String, password: String, old: String?, from p: Peer) {
        guard let m = p.member else {
            send(.error(code: ServerErrorCode.denied.rawValue, message: "This server has no sign-in."), to: p)
            return
        }
        let e = email.trimmed.lowercased()
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
        // Hub::set_password tells every connection, this one too
        broadcastAccess()
    }

    func assignTag(device: String, wanted: UInt64?) -> UInt64 {
        if let t = tags[device] { return t }
        let used = Set(tags.values)
        var t = wanted.map { $0 % 1024 } ?? 1
        if t == 0 || used.contains(t) {
            t = (1..<1024).first { !used.contains(UInt64($0)) }.map(UInt64.init) ?? 1
        }
        tags[device] = t
        return t
    }

    /// The tasks in `putBackIds` that a pushed copy changed go back to the
    /// team's version, stamped past the change. True when any did.
    func keepTeamVersions(_ s: inout JSON, _ before: JSON) -> Bool {
        var theirs = SyncMeta(of: s)
        let team = SyncMeta(of: before)
        var tasks = s["tasks"]?.array ?? []
        let teamTasks = before["tasks"]?.array ?? []
        var refused = false
        for id in putBackIds {
            guard let t = teamTasks.first(where: { $0["id"]?.uint == id }) else { continue }
            // their change: the item's stamp or the newest of its field stamps
            let stamp = max(theirs.stamps[id] ?? 0, theirs.fields[id]?.values.max() ?? 0)
            guard stamp > (team.stamps[id] ?? 0) else { continue }
            if let i = tasks.firstIndex(where: { $0["id"]?.uint == id }) { tasks[i] = t } else { tasks.append(t) }
            // without field stamps every field counts as changed at the stamp
            theirs.fields[id] = nil
            theirs.stamps[id] = stamp + 1
            refused = true
        }
        if refused {
            s.set("tasks", .array(tasks))
            s.set("extra.sync", theirs.json)
        }
        return refused
    }

    /// Pass someone's typing on to the others who can see the conversation,
    /// as the real server does (Hub::typing_in): a DM or private channel
    /// with a member list only to its members.
    func typing(_ ch: UInt64, from p: Peer) {
        guard let member = p.member,
              let c = WorkspaceView(snapshot ?? [:]).channels.first(where: { $0.id == ch }) else { return }
        let restricted = (c.kind == .direct || c.isPrivate) && !c.members.isEmpty
        if restricted && !c.members.contains(member) { return }
        for q in peers.values where q.welcomed && q !== p {
            guard let m = q.member, m != member, !restricted || c.members.contains(m) else { continue }
            send(.typing(channel: ch, member: member), to: q)
        }
    }

    func broadcast(_ m: ServerMessage) {
        for p in peers.values where p.welcomed { send(m, to: p) }
    }

    func send(_ m: ServerMessage, to p: Peer) {
        if p.held != nil { return p.held!.append(m) }
        frame(p, opcode: 0x1, Data(m.text.utf8))
    }
}
