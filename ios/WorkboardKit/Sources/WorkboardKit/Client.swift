// The connection to workboard-server and the workspace copy kept on this
// device.
//
// The copy is a raw JSON tree (see JSON.swift). On `welcome` and `update` the
// server's snapshot is merged into it with Sync.merge, the same way the Mac
// app merges a teammate's copy, so a local edit that has not reached the
// server yet is kept until it does.
//
// Edits work offline too: each one changes the tree and stamps it; while
// connected the whole copy is pushed shortly after, and after a reconnect
// the copy goes up right after `welcome`. Every stamp made here is remembered
// as "waiting" until a snapshot from the server carries it (or something
// newer), which is how the app can say "3 changes waiting".
//
// Signing in: hello carries the device token saved for this server address
// (Keychain). When the server refuses with error code "auth" (a new device,
// a token it no longer knows, access removed) it closes the connection and
// the app shows the sign-in screen with the server's message. Each sign-in
// attempt (email + password, invite code + new password, reset code + new
// password) is a fresh connection whose hello carries it; the welcome then
// brings a new token to keep and the account email, if any.

import Foundation
import Observation

public enum ConnectionState: Equatable, Sendable {
    /// No server address yet, or disconnected on purpose.
    case idle
    case connecting
    case connected
    /// The server refused to sign this device in (error code "auth").
    case signIn
    /// Lost the server; the next try is at `retryAt`.
    case waiting(retryAt: Date, attempt: Int)

    public var isConnected: Bool { self == .connected }
}

/// What this device remembers between launches (never shared).
public struct DeviceSettings: Codable, Equatable, Sendable {
    public var server: String = ""
    public var device: String = UUID().uuidString.lowercased()
    public var tag: UInt64?
    public var lastId: UInt64 = 0
    /// Who I am in the workspace: chosen by hand (phase 1) or given by the
    /// server with the sign-in (phase 2).
    public var me: UInt64?
    public var meFromServer = false
    /// Stamps and tombstones made here that the server has not shown back yet.
    public var waitingStamps: [UInt64: Int64] = [:]
    public var waitingTombs: [UInt64: Int64] = [:]
    /// The APNs device token, sent to the server after every welcome.
    public var pushToken: String?
    public var pushEnv = "development"
    /// The account email for `me` on this server (nil: no password set yet).
    public var accountEmail: String?
    /// The role the server gave ("member", "admin", "owner").
    public var role: String?

    public init() {}

    enum CodingKeys: String, CodingKey { case server, device, tag, lastId, me, meFromServer, waitingStamps, waitingTombs, pushToken, pushEnv, accountEmail, role }

    // every field optional, so settings from an older build still load
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        server = try c.decodeIfPresent(String.self, forKey: .server) ?? ""
        device = try c.decodeIfPresent(String.self, forKey: .device) ?? UUID().uuidString.lowercased()
        tag = try c.decodeIfPresent(UInt64.self, forKey: .tag)
        lastId = try c.decodeIfPresent(UInt64.self, forKey: .lastId) ?? 0
        me = try c.decodeIfPresent(UInt64.self, forKey: .me)
        meFromServer = try c.decodeIfPresent(Bool.self, forKey: .meFromServer) ?? false
        waitingStamps = try c.decodeIfPresent([UInt64: Int64].self, forKey: .waitingStamps) ?? [:]
        waitingTombs = try c.decodeIfPresent([UInt64: Int64].self, forKey: .waitingTombs) ?? [:]
        pushToken = try c.decodeIfPresent(String.self, forKey: .pushToken)
        pushEnv = try c.decodeIfPresent(String.self, forKey: .pushEnv) ?? "development"
        accountEmail = try c.decodeIfPresent(String.self, forKey: .accountEmail)
        role = try c.decodeIfPresent(String.self, forKey: .role)
    }
}

/// Files kept in one folder: settings.json and snapshot.json.
public struct Storage: Sendable {
    public let folder: URL

    public init(folder: URL) { self.folder = folder }

    public static var standard: Storage {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return Storage(folder: base.appendingPathComponent("Workboard", isDirectory: true))
    }

    var settingsURL: URL { folder.appendingPathComponent("settings.json") }
    var snapshotURL: URL { folder.appendingPathComponent("snapshot.json") }

    public func loadSettings() -> DeviceSettings {
        guard let d = try? Data(contentsOf: settingsURL), let s = try? JSONDecoder().decode(DeviceSettings.self, from: d) else { return DeviceSettings() }
        return s
    }

    public func save(_ s: DeviceSettings) {
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        if let d = try? JSONEncoder().encode(s) { try? d.write(to: settingsURL, options: .atomic) }
    }

    public func loadSnapshot() -> JSON? {
        guard let d = try? Data(contentsOf: snapshotURL) else { return nil }
        return try? JSON(parsing: d)
    }

    public func save(snapshot: JSON) {
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? snapshot.data().write(to: snapshotURL, options: .atomic)
    }
}

/// Why the sign-in screen is up.
public enum SignInReason: Equatable, Sendable {
    /// The server wants a sign-in and this device had nothing to show.
    case needed
    /// The saved token was refused or access was removed.
    case signedOut
    /// A sign-in attempt was refused.
    case failed
}

/// A set_password request from settings.
public enum AccountRequest: Equatable, Sendable {
    case idle
    case sending
    case done
    case failed(String)
}

/// A chat message or reply from someone else that arrived from the server.
public struct IncomingMessage: Equatable, Sendable {
    public let channel: UInt64
    public let id: UInt64
    /// The message this is a thread reply to.
    public let parent: UInt64?
    public let author: UInt64
    public let text: String
}

@MainActor
@Observable
public final class SyncClient {
    public private(set) var state: ConnectionState = .idle
    /// This device's copy of the workspace, nil until the first snapshot.
    public private(set) var data: JSON?
    /// Read-only view of `data` for the screens.
    public private(set) var view: WorkspaceView = .empty
    public private(set) var rev: Int64 = 0
    public private(set) var settings: DeviceSettings
    public private(set) var lastError: String?
    /// The sign-in screen is up: set when the server refuses with "auth",
    /// cleared by a welcome. Stays up while a sign-in attempt connects.
    public private(set) var needsSignIn = false
    /// Why (with the server's message, shown as is).
    public private(set) var signInReason: SignInReason?
    public private(set) var signInMessage: String?
    /// A sign-in attempt is on its way.
    public var signingIn: Bool { pendingSignIn != nil }
    /// A request the server turned down (error code "denied"), shown briefly.
    public private(set) var notice: String?
    public private(set) var accountRequest: AccountRequest = .idle
    /// Bumped every time a snapshot from the server is merged in.
    public private(set) var updatesReceived = 0
    /// Bumped every time a copy is pushed.
    public private(set) var pushesSent = 0

    public var me: UInt64? { settings.me }
    /// Changes made here that the server has not shown back yet.
    public var waitingCount: Int { settings.waitingStamps.count + settings.waitingTombs.count }
    public var hasToken: Bool { !(tokens.token(for: settings.server) ?? "").isEmpty }
    /// Editing needs a copy and a "me"; it is off only while signed out.
    public var canEdit: Bool { data != nil && settings.me != nil && !needsSignIn }

    /// Called with messages from others as they arrive (not on the first load).
    @ObservationIgnored public var onIncoming: ([IncomingMessage]) -> Void = { _ in }

    @ObservationIgnored let storage: Storage
    @ObservationIgnored let tokens: TokenStore
    @ObservationIgnored private let session: URLSession
    @ObservationIgnored private var socket: URLSessionWebSocketTask?
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var pushTask: Task<Void, Never>?
    @ObservationIgnored private var pingTask: Task<Void, Never>?
    @ObservationIgnored private var retryTask: Task<Void, Never>?
    @ObservationIgnored private var lastHeard = Date()
    @ObservationIgnored private var attempt = 0
    @ObservationIgnored private var pendingSignIn: SignInRequest?
    /// What the hello on the current connection carried.
    @ObservationIgnored private var helloUsedToken = false

    /// Timings, shortened in tests.
    @ObservationIgnored public var pingInterval: TimeInterval = 30
    @ObservationIgnored public var silenceLimit: TimeInterval = 75
    @ObservationIgnored public var pushDelay: TimeInterval = 0.25
    @ObservationIgnored public var maxRetryDelay: TimeInterval = 30

    public init(storage: Storage = .standard, tokens: TokenStore = KeychainTokenStore()) {
        self.storage = storage
        self.tokens = tokens
        self.settings = storage.loadSettings()
        let cfg = URLSessionConfiguration.default
        cfg.waitsForConnectivity = false
        cfg.timeoutIntervalForRequest = 15
        self.session = URLSession(configuration: cfg)
        if let snap = storage.loadSnapshot() {
            self.data = snap
            self.view = WorkspaceView(snap)
        }
        storage.save(settings)
    }

    // MARK: settings

    /// Use another server address. `freshCopy` drops this device's copy of
    /// the workspace (and edits still waiting), for moving to a different
    /// server, so one workspace is never merged into another.
    public func setServer(_ address: String, freshCopy: Bool = false) {
        let a = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard a != settings.server else { return }
        settings.server = a
        if settings.meFromServer {
            settings.me = nil
            settings.meFromServer = false
            // that server wanted a sign-in; stay on the sign-in screen until
            // this one says otherwise
            needsSignIn = true
            signInReason = .needed
        }
        if freshCopy {
            data = nil
            view = .empty
            rev = 0
            settings.waitingStamps = [:]
            settings.waitingTombs = [:]
            try? FileManager.default.removeItem(at: storage.snapshotURL)
        }
        settings.accountEmail = nil
        settings.role = nil
        pendingSignIn = nil
        signInMessage = nil
        storage.save(settings)
        disconnect()
        connect()
    }

    /// Choose who I am by hand (only while the server does not say it).
    public func setMe(_ id: UInt64?) {
        guard !settings.meFromServer else { return }
        settings.me = id
        storage.save(settings)
    }

    /// The APNs token for this install; sent now if connected, else after the next welcome.
    public func setPushToken(_ token: String, env: String) {
        settings.pushToken = token
        settings.pushEnv = env
        storage.save(settings)
        if state.isConnected, let ws = socket { send(.pushToken(token: token, platform: "ios", env: env), on: ws) }
    }

    /// The device token for the current server (for calls).
    public var deviceToken: String? { tokens.token(for: settings.server) }

    // MARK: sign-in

    /// Try to sign in (a fresh connection whose hello carries `request`).
    /// Returns what is wrong with it without asking the server, or nil.
    @discardableResult
    public func signIn(_ request: SignInRequest) -> String? {
        let r = request.cleaned
        if let why = r.problem { return why }
        guard Self.url(from: settings.server) != nil else { return "Type the server's address first." }
        pendingSignIn = r
        signInMessage = nil
        lastError = nil
        connect()
        return nil
    }

    /// Forget the token and who I am (the "Sign out" button).
    public func signOut() {
        tokens.setToken(nil, for: settings.server)
        if settings.meFromServer {
            settings.me = nil
            settings.meFromServer = false
        }
        settings.accountEmail = nil
        settings.role = nil
        storage.save(settings)
        pendingSignIn = nil
        needsSignIn = true
        signInReason = .needed
        signInMessage = nil
        disconnect()
        connect()
    }

    /// Set up an account for this signed-in device (first time) or change
    /// its password (`oldPassword` needed when there is an account already).
    /// The answer shows in `accountRequest`. Returns a problem found before
    /// asking the server, or nil.
    @discardableResult
    public func setPassword(email: String, password: String, oldPassword: String?) -> String? {
        let e = email.trimmingCharacters(in: .whitespacesAndNewlines)
        if e.isEmpty { return "Type your email." }
        if let why = passwordProblem(password) { return why }
        if settings.accountEmail != nil, (oldPassword ?? "").isEmpty { return "Type your current password." }
        guard state.isConnected, let ws = socket else { return "Connect to the server first." }
        accountRequest = .sending
        send(.setPassword(email: e, password: password, oldPassword: oldPassword), on: ws)
        return nil
    }

    public func resetAccountRequest() { accountRequest = .idle }

    public func clearNotice() { notice = nil }

    // MARK: connection

    public func connect() {
        retryTask?.cancel()
        retryTask = nil
        guard let url = Self.url(from: settings.server) else {
            state = .idle
            lastError = settings.server.isEmpty ? nil : "That address doesn't look right."
            return
        }
        closeSocket()
        generation += 1
        let gen = generation
        state = .connecting
        let ws = session.webSocketTask(with: url)
        ws.maximumMessageSize = 64 * 1024 * 1024
        socket = ws
        ws.resume()
        lastHeard = Date()
        sendHello(on: ws)
        Task { await self.receiveLoop(ws, gen) }
        pingTask = Task { [weak self] in
            while !Task.isCancelled {
                let interval = self?.pingInterval ?? 30
                try? await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
                guard let self, !Task.isCancelled, self.generation == gen else { return }
                if Date().timeIntervalSince(self.lastHeard) > self.silenceLimit {
                    self.lost(gen, "No answer from the server")
                    return
                }
                self.send(.ping, on: ws)
            }
        }
    }

    private func sendHello(on ws: URLSessionWebSocketTask) {
        var h = Hello(device: settings.device, tag: settings.tag)
        if let attempt = pendingSignIn {
            attempt.fill(&h)
            helloUsedToken = false
        } else {
            h.token = tokens.token(for: settings.server).flatMap { $0.isEmpty ? nil : $0 }
            helloUsedToken = h.token != nil
        }
        send(.hello(h), on: ws)
    }

    /// Stop on purpose (no retry).
    public func disconnect() {
        retryTask?.cancel()
        retryTask = nil
        closeSocket()
        generation += 1
        state = .idle
    }

    nonisolated static func url(from address: String) -> URL? {
        ServerAddress.websocket(address)
    }

    private func closeSocket() {
        pingTask?.cancel()
        pingTask = nil
        pushTask?.cancel()
        pushTask = nil
        socket?.cancel(with: .goingAway, reason: nil)
        socket = nil
    }

    private func send(_ m: ClientMessage, on ws: URLSessionWebSocketTask) {
        ws.send(.string(m.text)) { _ in }
    }

    private func receiveLoop(_ ws: URLSessionWebSocketTask, _ gen: Int) async {
        while generation == gen {
            do {
                let msg = try await ws.receive()
                guard generation == gen else { return }
                lastHeard = Date()
                switch msg {
                case .string(let s): handle(ServerMessage(text: s), ws)
                case .data(let d): handle(ServerMessage(text: String(decoding: d, as: UTF8.self)), ws)
                @unknown default: break
                }
            } catch {
                lost(gen, error.localizedDescription)
                return
            }
        }
    }

    private func handle(_ m: ServerMessage, _ ws: URLSessionWebSocketTask) {
        switch m {
        case let .welcome(w):
            attempt = 0
            lastError = nil
            needsSignIn = false
            signInReason = nil
            signInMessage = nil
            pendingSignIn = nil
            settings.tag = w.tag
            if let t = w.token, !t.isEmpty { tokens.setToken(t, for: settings.server) }
            if let member = w.member {
                settings.me = member
                settings.meFromServer = true
                settings.accountEmail = w.email
                settings.role = w.role
            } else if settings.meFromServer {
                // the server stopped saying who we are (no sign-in any more)
                settings.meFromServer = false
                settings.accountEmail = nil
                settings.role = nil
            }
            storage.save(settings)
            self.rev = w.rev
            if let snapshot = w.snapshot { take(snapshot, announce: false) }
            state = .connected
            if let t = settings.pushToken { send(.pushToken(token: t, platform: "ios", env: settings.pushEnv), on: ws) }
            // after (re)connecting the whole copy goes up, as the server design says
            if data != nil { pushNow() }
        case let .update(snapshot, rev):
            self.rev = rev
            take(snapshot, announce: true)
        case let .account(email):
            settings.accountEmail = email
            storage.save(settings)
            accountRequest = .done
        case .ping:
            send(.pong, on: ws)
        case .pong:
            break
        case let .error(code, message):
            handleError(code.flatMap(ServerErrorCode.init(rawValue:)), message: message)
        case .unknown:
            break
        }
    }

    private func handleError(_ code: ServerErrorCode?, message: String) {
        switch code {
        case .auth:
            // the server closes the connection now; don't retry on our own
            let wasAttempt = pendingSignIn != nil
            let wasIn = state.isConnected
            generation += 1
            closeSocket()
            retryTask?.cancel()
            retryTask = nil
            pendingSignIn = nil
            if wasIn || helloUsedToken { tokens.setToken(nil, for: settings.server) }
            if settings.meFromServer {
                settings.me = nil
                settings.meFromServer = false
            }
            settings.accountEmail = nil
            settings.role = nil
            storage.save(settings)
            signInReason = wasAttempt ? .failed : (wasIn || helloUsedToken ? .signedOut : .needed)
            signInMessage = message.isEmpty ? "The server didn't let this device in." : message
            needsSignIn = true
            state = .signIn
            attempt = 0
        case .denied:
            let m = message.isEmpty ? "The server turned that down." : message
            if accountRequest == .sending {
                accountRequest = .failed(m)
            } else {
                notice = m
            }
        case nil:
            // no code: like losing the connection
            lost(generation, message.isEmpty ? "The server reported an error." : message)
        }
    }

    /// Merge a snapshot from the server into this device's copy.
    private func take(_ snapshot: JSON, announce: Bool) {
        let before = data
        if var local = data {
            Sync.merge(&local, [snapshot])
            data = local
        } else {
            var fresh = snapshot
            var meta = SyncMeta(of: fresh)
            meta.hashes = Sync.fingerprints(fresh)
            fresh.set("extra.sync", meta.json)
            data = fresh
        }
        clearWaiting(seenIn: SyncMeta(of: snapshot))
        updatesReceived += 1
        refresh()
        if announce, let before, let after = data {
            let fresh = Self.newMessages(before: before, after: after, me: settings.me)
            if !fresh.isEmpty { onIncoming(fresh) }
        }
    }

    /// Drop the waiting marks the server has now shown back (or overtaken).
    private func clearWaiting(seenIn s: SyncMeta) {
        let before = (settings.waitingStamps, settings.waitingTombs)
        settings.waitingStamps = settings.waitingStamps.filter { id, t in
            if let st = s.stamps[id], st >= t { return false }
            if let tomb = s.tombs[id], (s.revived[id] ?? 0) < tomb { return false } // deleted by someone: nothing to wait for
            return true
        }
        settings.waitingTombs = settings.waitingTombs.filter { id, t in
            if let tomb = s.tombs[id], tomb >= t { return false }
            return true
        }
        if before != (settings.waitingStamps, settings.waitingTombs) { storage.save(settings) }
    }

    /// Messages and replies by others that are in `after` but not in `before`.
    nonisolated static func newMessages(before: JSON, after: JSON, me: UInt64?) -> [IncomingMessage] {
        func ids(_ d: JSON) -> Set<UInt64> {
            var s = Set<UInt64>()
            for c in d["channels"]?.array ?? [] {
                for m in c["messages"]?.array ?? [] {
                    if let id = m["id"]?.uint { s.insert(id) }
                    for r in m["replies"]?.array ?? [] { if let id = r["id"]?.uint { s.insert(id) } }
                }
            }
            return s
        }
        let old = ids(before)
        var out: [IncomingMessage] = []
        for c in after["channels"]?.array ?? [] {
            let ch = c["id"]?.uint ?? 0
            for m in c["messages"]?.array ?? [] {
                let mid = m["id"]?.uint ?? 0
                let author = m["author"]?.uint ?? 0
                if !old.contains(mid), author != me { out.append(IncomingMessage(channel: ch, id: mid, parent: nil, author: author, text: m["text"]?.string ?? "")) }
                for r in m["replies"]?.array ?? [] {
                    let rid = r["id"]?.uint ?? 0
                    let ra = r["author"]?.uint ?? 0
                    if !old.contains(rid), ra != me { out.append(IncomingMessage(channel: ch, id: rid, parent: mid, author: ra, text: r["text"]?.string ?? "")) }
                }
            }
        }
        return out
    }

    private func lost(_ gen: Int, _ why: String) {
        guard generation == gen else { return }
        generation += 1
        closeSocket()
        if accountRequest == .sending { accountRequest = .failed(why) }
        if pendingSignIn != nil {
            // a sign-in attempt that couldn't reach the server: say so and
            // let the person try again, rather than retrying it on our own
            pendingSignIn = nil
            signInReason = .failed
            signInMessage = why
            needsSignIn = true
            state = .signIn
            attempt = 0
            return
        }
        lastError = why
        attempt += 1
        let delay = min(maxRetryDelay, pow(2, Double(min(attempt - 1, 10))))
        let at = Date().addingTimeInterval(delay)
        state = .waiting(retryAt: at, attempt: attempt)
        retryTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            guard let self, !Task.isCancelled else { return }
            self.connect()
        }
    }

    /// Try again right away (the "Retry now" button).
    public func retryNow() {
        if case .waiting = state { connect() }
    }

    // MARK: edits

    /// A new id for something made on this device.
    public func newId() -> UInt64 {
        var maker = IdMaker(tag: settings.tag ?? 0, last: settings.lastId)
        let id = maker.next()
        settings.lastId = id
        storage.save(settings)
        return id
    }

    /// Change the copy, stamp what changed and push it (now, or after the
    /// next reconnect). Returns false when editing is not allowed.
    @discardableResult
    public func edit(_ change: (inout JSON, _ me: UInt64) -> Void) -> Bool {
        guard canEdit, var d = data, let me = settings.me else { return false }
        change(&d, me)
        let old = SyncMeta(of: d)
        var meta = old
        Sync.stamp(d, &meta, now: nowTs(), pastSeen: true)
        for (id, t) in meta.stamps where old.stamps[id] != t { settings.waitingStamps[id] = t }
        for (id, t) in meta.tombs where old.tombs[id] != t {
            settings.waitingTombs[id] = t
            settings.waitingStamps[id] = nil
        }
        storage.save(settings)
        d.set("extra.sync", meta.json)
        data = d
        refresh()
        schedulePush()
        return true
    }

    /// Personal-only changes (read position): saved, never stamped as shared.
    public func editLocal(_ change: (inout JSON) -> Void) {
        guard var d = data else { return }
        let before = d
        change(&d)
        guard d != before else { return }
        data = d
        refresh()
    }

    private func schedulePush() {
        guard state.isConnected else { return }
        pushTask?.cancel()
        let delay = pushDelay
        pushTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            guard let self, !Task.isCancelled else { return }
            self.pushNow()
        }
    }

    private func pushNow() {
        guard let ws = socket, state.isConnected, let d = data else { return }
        send(.push(data: Sync.outgoing(d)), on: ws)
        pushesSent += 1
    }

    private func refresh() {
        guard let d = data else { return }
        view = WorkspaceView(d)
        storage.save(snapshot: d)
    }
}
