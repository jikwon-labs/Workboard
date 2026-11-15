// The workboard-server protocol (wb_net::Msg in crates/wb-net/src/lib.rs):
// JSON text messages over a WebSocket at ws://host:port/ws, each with a
// snake_case "type".
//
//   app -> server  hello        {device, tag?, token?, invite?, claim?,
//                                email?, password?, new_password?, reset_code?,
//                                app?, sync?}   (app version and sync format;
//                                the server takes no pushes from apps with
//                                sync below wb_net::OLDEST_SYNC or none)
//   server -> app  welcome      {tag, snapshot|null, rev, member, role, token?, email?}
//   app -> server  push         {data}
//   server -> all  update       {data, rev, hidden?}   (hidden also in welcome: ids
//                                this person may not see; dropped without tombstones)
//   app -> server  set_password {email, password, old_password?}
//   server -> app  account      {email}
//   app -> server  push_token   {token, platform: "ios", env}   (ignored by servers that don't know it)
//   app -> server  call_join    {channel}
//   server -> app  call_token   {channel, url, token}   (a LiveKit address and join token)
//   both           ping / pong, every 30 seconds
//   server -> app  error        {message, code?, channel?}   (channel: a call_join turned down)
//
// Optional hello fields are left out when absent: the server treats a field
// that is there as meant. It uses the first that applies: reset_code (with
// email and new_password), invite (with new_password: make the account),
// email + password, token, claim. iOS never claims: claiming an empty
// server means bringing a workspace, which only the Mac app has; the sign-in
// screen asks /health and says to set the server up from the Mac instead.
//
// Error codes: "auth" = signing in was refused and the server closes the
// connection; "denied" = one request was refused, the connection stays; no
// code = treat as a lost connection.

import Foundation

/// What the first message says about who this device is.
public struct Hello: Equatable, Sendable {
    public var device: String
    public var tag: UInt64?
    public var token: String?
    public var invite: String?
    public var claim: UInt64?
    public var email: String?
    public var password: String?
    public var newPassword: String?
    public var resetCode: String?
    /// This app's version, and the sync format it writes (Sync.metaV).
    public var app: String?
    public var sync: UInt64?

    public init(device: String, tag: UInt64? = nil, token: String? = nil, invite: String? = nil, claim: UInt64? = nil,
                email: String? = nil, password: String? = nil, newPassword: String? = nil, resetCode: String? = nil,
                app: String? = nil, sync: UInt64? = nil) {
        self.app = app
        self.sync = sync
        self.device = device
        self.tag = tag
        self.token = token
        self.invite = invite
        self.claim = claim
        self.email = email
        self.password = password
        self.newPassword = newPassword
        self.resetCode = resetCode
    }
}

public enum ClientMessage: Equatable, Sendable {
    case hello(Hello)
    case push(data: JSON)
    case setPassword(email: String, password: String, oldPassword: String?)
    case pushToken(token: String, platform: String, env: String)
    case callJoin(channel: UInt64)
    case ping
    case pong

    public var json: JSON {
        switch self {
        case let .hello(h):
            var o: [String: JSON] = ["type": "hello", "device": .string(h.device)]
            if let t = h.tag { o["tag"] = .id(t) }
            func put(_ k: String, _ v: String?) { if let v, !v.isEmpty { o[k] = .string(v) } }
            put("token", h.token)
            put("invite", h.invite)
            if let c = h.claim { o["claim"] = .id(c) }
            put("email", h.email)
            put("password", h.password)
            put("new_password", h.newPassword)
            put("reset_code", h.resetCode)
            put("app", h.app)
            if let v = h.sync { o["sync"] = .id(v) }
            return .object(o)
        case let .push(data):
            return ["type": "push", "data": data]
        case let .setPassword(email, password, old):
            var o: [String: JSON] = ["type": "set_password", "email": .string(email), "password": .string(password)]
            if let old, !old.isEmpty { o["old_password"] = .string(old) }
            return .object(o)
        case let .pushToken(token, platform, env):
            return ["type": "push_token", "token": .string(token), "platform": .string(platform), "env": .string(env)]
        case let .callJoin(channel):
            return ["type": "call_join", "channel": .id(channel)]
        case .ping: return ["type": "ping"]
        case .pong: return ["type": "pong"]
        }
    }

    public var text: String { json.serialized() }

    public init?(text: String) {
        guard let j = try? JSON(parsing: text), let type = j["type"]?.string else { return nil }
        switch type {
        case "hello":
            guard let device = j["device"]?.string else { return nil }
            self = .hello(Hello(device: device, tag: j["tag"]?.uint, token: j["token"]?.string, invite: j["invite"]?.string,
                                claim: j["claim"]?.uint, email: j["email"]?.string, password: j["password"]?.string,
                                newPassword: j["new_password"]?.string, resetCode: j["reset_code"]?.string,
                                app: j["app"]?.string, sync: j["sync"]?.uint))
        case "push":
            guard let s = j["data"], s.object != nil else { return nil }
            self = .push(data: s)
        case "set_password":
            guard let e = j["email"]?.string, let p = j["password"]?.string else { return nil }
            self = .setPassword(email: e, password: p, oldPassword: j["old_password"]?.string)
        case "push_token":
            self = .pushToken(token: j["token"]?.string ?? "", platform: j["platform"]?.string ?? "", env: j["env"]?.string ?? "")
        case "call_join":
            guard let ch = j["channel"]?.uint else { return nil }
            self = .callJoin(channel: ch)
        case "ping": self = .ping
        case "pong": self = .pong
        default: return nil
        }
    }
}

/// Error codes (wb_net::CODE_AUTH, CODE_DENIED).
public enum ServerErrorCode: String, Sendable {
    /// Signing in was refused (bad code or password, token unknown or
    /// removed, too many tries). The server closes the connection.
    case auth
    /// One request was refused; the connection stays.
    case denied
}

public struct Welcome: Equatable, Sendable {
    public var tag: UInt64
    public var snapshot: JSON?
    public var rev: Int64
    /// Who this device signs in as. The real server always says; the fake
    /// server without sign-in leaves it out and the person picks.
    public var member: UInt64?
    public var role: String?
    /// A new device token to keep.
    public var token: String?
    /// The account email of this member, when they have a password.
    public var email: String?
    /// Items the server hides from this person (newer servers; see Sync.hide).
    public var hidden: [UInt64]

    public init(tag: UInt64, snapshot: JSON?, rev: Int64, member: UInt64? = nil, role: String? = nil, token: String? = nil, email: String? = nil, hidden: [UInt64] = []) {
        self.hidden = hidden
        self.tag = tag
        self.snapshot = snapshot
        self.rev = rev
        self.member = member
        self.role = role
        self.token = token
        self.email = email
    }
}

public enum ServerMessage: Equatable, Sendable {
    case welcome(Welcome)
    case update(data: JSON, rev: Int64, hidden: [UInt64] = [])
    case account(email: String)
    case callToken(channel: UInt64, url: String, token: String)
    case ping
    case pong
    /// `channel` is set when a call_join for it was turned down.
    case error(code: String?, message: String, channel: UInt64? = nil)
    case unknown(String)

    public var json: JSON {
        switch self {
        case let .welcome(w):
            var o: [String: JSON] = ["type": "welcome", "tag": .id(w.tag), "snapshot": w.snapshot ?? .null, "rev": .int(w.rev)]
            if let m = w.member { o["member"] = .id(m) }
            if let r = w.role { o["role"] = .string(r) }
            if let t = w.token { o["token"] = .string(t) }
            if let e = w.email { o["email"] = .string(e) }
            if !w.hidden.isEmpty { o["hidden"] = .array(w.hidden.map(JSON.id)) }
            return .object(o)
        case let .update(data, rev, hidden):
            var o: [String: JSON] = ["type": "update", "data": data, "rev": .int(rev)]
            if !hidden.isEmpty { o["hidden"] = .array(hidden.map(JSON.id)) }
            return .object(o)
        case let .account(email):
            return ["type": "account", "email": .string(email)]
        case let .callToken(channel, url, token):
            return ["type": "call_token", "channel": .id(channel), "url": .string(url), "token": .string(token)]
        case .ping: return ["type": "ping"]
        case .pong: return ["type": "pong"]
        case let .error(code, m, channel):
            var o: [String: JSON] = ["type": "error", "message": .string(m)]
            if let code { o["code"] = .string(code) }
            if let channel { o["channel"] = .id(channel) }
            return .object(o)
        case let .unknown(t): return ["type": .string(t)]
        }
    }

    public var text: String { json.serialized() }

    public init(text: String) {
        guard let j = try? JSON(parsing: text), let type = j["type"]?.string else {
            self = .unknown("unreadable")
            return
        }
        switch type {
        case "welcome":
            let snap = j["snapshot"]
            self = .welcome(Welcome(tag: j["tag"]?.uint ?? 0, snapshot: snap?.object != nil ? snap : nil, rev: j["rev"]?.int ?? 0,
                                    member: j["member"]?.uint, role: j["role"]?.string, token: j["token"]?.string, email: j["email"]?.string,
                                    hidden: (j["hidden"]?.array ?? []).compactMap(\.uint)))
        case "update":
            if let s = j["data"], s.object != nil {
                self = .update(data: s, rev: j["rev"]?.int ?? 0, hidden: (j["hidden"]?.array ?? []).compactMap(\.uint))
            } else {
                self = .unknown("update without data")
            }
        case "account":
            self = .account(email: j["email"]?.string ?? "")
        case "call_token":
            self = .callToken(channel: j["channel"]?.uint ?? 0, url: j["url"]?.string ?? "", token: j["token"]?.string ?? "")
        case "ping": self = .ping
        case "pong": self = .pong
        case "error": self = .error(code: j["code"]?.string, message: j["message"]?.string ?? "", channel: j["channel"]?.uint)
        default: self = .unknown(type)
        }
    }
}
