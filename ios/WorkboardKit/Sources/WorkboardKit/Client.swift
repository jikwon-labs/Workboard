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
    /// The server the push token was last sent to (the registration a
    /// sign-out or a server switch takes back).
    public var pushSentTo: String?
    /// Servers that still have to be told to stop notifying this device
    /// (they couldn't be reached at sign-out); their device tokens wait in
    /// the token store under `SyncClient.unregisterKey`.
    public var pushUnregister: [String] = []

    public init() {}

    enum CodingKeys: String, CodingKey { case server, device, tag, lastId, me, meFromServer, waitingStamps, waitingTombs, pushToken, pushEnv, accountEmail, role, pushSentTo, pushUnregister }

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
        pushSentTo = try c.decodeIfPresent(String.self, forKey: .pushSentTo)
        pushUnregister = try c.decodeIfPresent([String].self, forKey: .pushUnregister) ?? []
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
    public internal(set) var settings: DeviceSettings
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
    /// This iPhone's Trash (personal, never shared; see Trash.swift).
    public internal(set) var trash: [TrashItem] = []

    // owners and admins (see Admin.swift)
    /// Everyone's access, from the server's last `access` (nil: not asked yet).
    public internal(set) var people: [Person]?
    /// The server's backups (owner only; nil: not asked yet).
    public internal(set) var backups: [Backup]?
    /// The newest invite code made here, with where it was emailed.
    public internal(set) var lastInvite: InviteResult?
    /// An access or backup request is on its way.
    public internal(set) var adminPending = 0
    /// The last access or backup request the server turned down.
    public internal(set) var adminError: String?
    /// Said after a restore ("Restored the team's workspace to ...").
    public internal(set) var adminNotice: String?

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
    @ObservationIgnored let session: URLSession
    @ObservationIgnored private(set) var socket: URLSessionWebSocketTask?
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var pushTask: Task<Void, Never>?
    @ObservationIgnored private var pingTask: Task<Void, Never>?
    @ObservationIgnored private var retryTask: Task<Void, Never>?
    @ObservationIgnored private var lastHeard = Date()
    @ObservationIgnored private var attempt = 0
    @ObservationIgnored private var pendingSignIn: SignInRequest?
    /// What the hello on the current connection carried.
    @ObservationIgnored private var helloUsedToken = false
    /// call_join requests waiting for the server's answer, by channel.
    @ObservationIgnored private var pendingCalls: [UInt64: [CheckedContinuation<CallGrant, Error>]] = [:]
    /// How long to wait for a call_token.
    @ObservationIgnored public var callTimeout: TimeInterval = 15

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
        if var snap = storage.loadSnapshot() {
            // a copy saved by an older build: give activity lines their ids
            // and take today's fingerprints as the baseline (every edit made
            // here was stamped when it was made), so nothing reads as edited
            if Sync.fillActivityIds(&snap) || SyncMeta(of: snap).v < 1 {
                var meta = SyncMeta(of: snap)
                meta.hashes = Sync.fingerprints(snap)
                meta.valueHashes = Sync.valueHashes(snap)
                meta.v = 1
                snap.set("extra.sync", meta.json)
            }
            self.data = snap
            self.view = WorkspaceView(snap)
        }
        self.trash = storage.loadTrash()
        storage.save(settings)
    }

    // MARK: settings

    /// Whether moving to `address` means leaving for a different server
    /// (and so starting from a fresh copy of that server's workspace).
    public func isDifferentServer(_ address: String) -> Bool {
        let a = address.trimmingCharacters(in: .whitespacesAndNewlines)
        return !settings.server.isEmpty && !a.isEmpty && !ServerAddress.same(a, settings.server)
    }

    /// Use another server address. `freshCopy` drops this device's copy of
    /// the workspace (and edits still waiting), so one workspace is never
    /// merged into another; by default that happens whenever the address
    /// points at a different server than before (the same server typed
    /// another way keeps the copy). Ask the person first when there are
    /// edits waiting (`waitingCount`).
    public func setServer(_ address: String, freshCopy: Bool? = nil) {
        let a = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard a != settings.server else { return }
        let freshCopy = freshCopy ?? isDifferentServer(a)
        // the server left behind stops sending this device notifications
        if isDifferentServer(a) { stopPushes(from: settings.server, token: tokens.token(for: settings.server)) }
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
            // people are numbered per workspace: pick again on the new one
            settings.me = nil
            settings.meFromServer = false
            settings.waitingStamps = [:]
            settings.waitingTombs = [:]
            try? FileManager.default.removeItem(at: storage.snapshotURL)
            // Trash holds the other workspace's items
            setTrash([])
        }
        settings.accountEmail = nil
        settings.role = nil
        clearAdmin()
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
        if state.isConnected, let ws = socket { sendPushToken(on: ws) }
    }

    private func sendPushToken(on ws: URLSessionWebSocketTask) {
        guard let t = settings.pushToken else { return }
        send(.pushToken(token: t, platform: "ios", env: settings.pushEnv), on: ws)
        settings.pushSentTo = settings.server
        storage.save(settings)
    }

    /// Where a device token waits until its server is told to stop pushes.
    nonisolated static func unregisterKey(_ server: String) -> String { "push-unregister|" + server }

    /// Tell `server` to stop notifying this device (push_token with an
    /// empty token, which the server takes as "forget it") and forget that
    /// it was registered there. Over the open connection when there is
    /// one; else, and when that fails, over a short connection of its own
    /// signed in with `token`, tried again on every connect until it works.
    private func stopPushes(from server: String, token: String?) {
        guard let sent = settings.pushSentTo, ServerAddress.same(sent, server) else { return }
        settings.pushSentTo = nil
        storage.save(settings)
        guard state.isConnected, let ws = socket, ServerAddress.same(settings.server, server) else {
            return queueUnregister(server, token: token)
        }
        // keep this socket out of closeSocket() until the message is out
        socket = nil
        ws.send(.string(ClientMessage.pushToken(token: "", platform: "ios", env: settings.pushEnv).text)) { [weak self] error in
            ws.cancel(with: .goingAway, reason: nil)
            if error != nil { Task { @MainActor in self?.queueUnregister(server, token: token) } }
        }
    }

    private func queueUnregister(_ server: String, token: String?) {
        if !settings.pushUnregister.contains(server) { settings.pushUnregister.append(server) }
        tokens.setToken(token ?? "", for: Self.unregisterKey(server))
        storage.save(settings)
        flushUnregisters()
    }

    /// Servers whose unregister is running now.
    @ObservationIgnored private var unregistering: Set<String> = []

    /// Try the waiting unregisters (see stopPushes).
    func flushUnregisters() {
        for server in settings.pushUnregister where !unregistering.contains(server) {
            guard let url = Self.url(from: server) else {
                finishUnregister(server)
                continue
            }
            unregistering.insert(server)
            let token = tokens.token(for: Self.unregisterKey(server)).flatMap { $0.isEmpty ? nil : $0 }
            let hello = ClientMessage.hello(Hello(device: settings.device, token: token)).text
            let stop = ClientMessage.pushToken(token: "", platform: "ios", env: settings.pushEnv).text
            let session = self.session
            Task { [weak self] in
                let done = await Self.unregister(url: url, hello: hello, stop: stop, session: session)
                guard let self else { return }
                self.unregistering.remove(server)
                guard done else { return }
                self.finishUnregister(server)
                // signed in there again meanwhile: that sign-in's registration stands
                if self.state.isConnected, ServerAddress.same(self.settings.server, server), let ws = self.socket { self.sendPushToken(on: ws) }
            }
        }
    }

    private func finishUnregister(_ server: String) {
        settings.pushUnregister.removeAll { $0 == server }
        tokens.setToken(nil, for: Self.unregisterKey(server))
        storage.save(settings)
    }

    /// One unregister connection: hello, wait for welcome, push_token "".
    /// True when it's done (or there's nothing left to do: the server
    /// doesn't know this device any more); false to try again later.
    nonisolated static func unregister(url: URL, hello: String, stop: String, session: URLSession) async -> Bool {
        let ws = session.webSocketTask(with: url)
        ws.maximumMessageSize = 64 * 1024 * 1024
        ws.resume()
        defer { ws.cancel(with: .goingAway, reason: nil) }
        do {
            try await ws.send(.string(hello))
            let deadline = Date().addingTimeInterval(20)
            while Date() < deadline {
                let text: String
                switch try await ws.receive() {
                case .string(let s): text = s
                case .data(let d): text = String(decoding: d, as: UTF8.self)
                @unknown default: continue
                }
                switch ServerMessage(text: text) {
                case .welcome:
                    try await ws.send(.string(stop))
                    // a ping answered means the server has read the message before it
                    try await withCheckedThrowingContinuation { (c: CheckedContinuation<Void, Error>) in
                        ws.sendPing { e in if let e { c.resume(throwing: e) } else { c.resume() } }
                    }
                    return true
                case let .error(code, _, _):
                    // signed out there for good (token unknown, access removed): the
                    // server dropped this device's notifications with it
                    return code == ServerErrorCode.auth.rawValue
                default:
                    continue
                }
            }
        } catch {}
        return false
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
        stopPushes(from: settings.server, token: tokens.token(for: settings.server))
        tokens.setToken(nil, for: settings.server)
        if settings.meFromServer {
            settings.me = nil
            settings.meFromServer = false
        }
        settings.accountEmail = nil
        settings.role = nil
        clearAdmin()
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
        flushUnregisters()
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
        failCalls()
        pingTask?.cancel()
        pingTask = nil
        pushTask?.cancel()
        pushTask = nil
        socket?.cancel(with: .goingAway, reason: nil)
        socket = nil
    }

    func send(_ m: ClientMessage, on ws: URLSessionWebSocketTask) {
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
            if let snapshot = w.snapshot { take(snapshot, announce: false, hidden: w.hidden) }
            state = .connected
            // signed in here again: this welcome's registration replaces the old one
            if settings.pushUnregister.contains(where: { ServerAddress.same($0, settings.server) }) && !unregistering.contains(settings.server) {
                for old in settings.pushUnregister where ServerAddress.same(old, settings.server) { finishUnregister(old) }
            }
            sendPushToken(on: ws)
            // after (re)connecting the whole copy goes up, as the server design says
            if data != nil { pushNow() }
        case let .update(snapshot, rev, hidden):
            self.rev = rev
            take(snapshot, announce: true, hidden: hidden)
        case let .account(email):
            settings.accountEmail = email
            storage.save(settings)
            accountRequest = .done
        case let .callToken(channel, url, token):
            finishCall(channel, .success(CallGrant(url: url, token: token, room: CallAPI.room(channel: channel))))
        case .invite, .access, .backups, .restored:
            handleAdmin(m)
        case .ping:
            send(.pong, on: ws)
        case .pong:
            break
        case let .error(code, message, channel):
            if let channel, pendingCalls[channel] != nil {
                // a call_join turned down; the connection stays
                finishCall(channel, .failure(CallAPI.meansNotSetUp(message) ? CallError.notAvailable : CallError.denied(message)))
            } else {
                handleError(code.flatMap(ServerErrorCode.init(rawValue:)), message: message)
            }
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
            // a server that signed this device out dropped its notifications too
            if let sent = settings.pushSentTo, ServerAddress.same(sent, settings.server) { settings.pushSentTo = nil }
            clearAdmin()
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
            } else if adminPending > 0 {
                adminPending -= 1
                adminError = m
            } else {
                notice = m
            }
        case nil:
            // no code: like losing the connection
            lost(generation, message.isEmpty ? "The server reported an error." : message)
        }
    }

    /// Merge a snapshot from the server into this device's copy.
    private func take(_ snapshot: JSON, announce: Bool, hidden: [UInt64] = []) {
        let before = data
        if var local = data {
            Sync.merge(&local, [snapshot])
            data = local
        } else {
            var fresh = snapshot
            Sync.fillActivityIds(&fresh)
            var meta = SyncMeta(of: fresh)
            meta.hashes = Sync.fingerprints(fresh)
            meta.valueHashes = Sync.valueHashes(fresh)
            meta.v = 1
            fresh.set("extra.sync", meta.json)
            data = fresh
        }
        if !hidden.isEmpty, var d = data {
            // out of this copy without a tombstone: nothing is deleted for the others
            let gone = Sync.hide(&d, ids: hidden)
            if !gone.isEmpty {
                var meta = SyncMeta(of: d)
                meta.hashes = Sync.fingerprints(d)
                d.set("extra.sync", meta.json)
                data = d
                settings.waitingStamps = settings.waitingStamps.filter { !gone.contains($0.key) }
                storage.save(settings)
            }
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
        adminPending = 0
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

    // MARK: huddles

    /// Ask the server for a way into a conversation's huddle (call_join).
    /// Throws CallError.notAvailable when the server has no LiveKit,
    /// .denied with the server's words, .offline or .noAnswer.
    public func requestCall(channel: UInt64) async throws -> CallGrant {
        guard state.isConnected, let ws = socket else { throw CallError.offline }
        // a conversation made a moment ago goes to the server first, so the
        // server knows it (the Mac app does the same)
        if pushTask != nil {
            pushTask?.cancel()
            pushTask = nil
            pushNow()
        }
        send(.callJoin(channel: channel), on: ws)
        let timeout = callTimeout
        let gen = generation
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(timeout * 1_000_000_000))
            guard let self, self.generation == gen else { return }
            self.finishCall(channel, .failure(CallError.noAnswer))
        }
        return try await withCheckedThrowingContinuation { cont in
            pendingCalls[channel, default: []].append(cont)
        }
    }

    private func finishCall(_ channel: UInt64, _ result: Result<CallGrant, Error>) {
        guard let waiting = pendingCalls.removeValue(forKey: channel) else { return }
        for c in waiting { c.resume(with: result) }
    }

    private func failCalls() {
        let all = pendingCalls
        pendingCalls = [:]
        for (_, waiting) in all {
            for c in waiting { c.resume(throwing: CallError.offline) }
        }
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
        // activity lines are written without ids; give them theirs before
        // stamping, as the Mac app does before it saves
        Sync.fillActivityIds(&d)
        let old = SyncMeta(of: d)
        var meta = old
        Sync.stamp(d, &meta, now: nowMs())
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
