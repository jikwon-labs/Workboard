// The workboard-server protocol (wb_net::Msg in crates/wb-net/src/lib.rs):
// JSON text messages over a WebSocket at ws://host:port/ws, each with a
// snake_case "type".
//
//   app -> server  hello        {device, tag?, token?, invite?, claim?,
//                                email?, password?, new_password?, reset_code?}
//   server -> app  welcome      {tag, snapshot|null, rev, member, role, token?, email?}
//   app -> server  push         {data}
//   server -> all  update       {data, rev}
//   app -> server  set_password {email, password, old_password?}
//   server -> app  account      {email}
//   app -> server  push_token   {token, platform: "ios", env}   (ignored by servers that don't know it)
//   both           ping / pong, every 30 seconds
//   server -> app  error        {message, code?}
//
// Optional hello fields are left out when absent: the server treats a field
// that is there as meant. It uses the first that applies: reset_code (with
// email and new_password), invite (with new_password: make the account),
// email + password, token, claim (iOS never claims).
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

    public init(device: String, tag: UInt64? = nil, token: String? = nil, invite: String? = nil, claim: UInt64? = nil,
                email: String? = nil, password: String? = nil, newPassword: String? = nil, resetCode: String? = nil) {
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
            return .object(o)
        case let .push(data):
            return ["type": "push", "data": data]
        case let .setPassword(email, password, old):
            var o: [String: JSON] = ["type": "set_password", "email": .string(email), "password": .string(password)]
            if let old, !old.isEmpty { o["old_password"] = .string(old) }
            return .object(o)
        case let .pushToken(token, platform, env):
            return ["type": "push_token", "token": .string(token), "platform": .string(platform), "env": .string(env)]
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
                                newPassword: j["new_password"]?.string, resetCode: j["reset_code"]?.string))
        case "push":
            guard let s = j["data"], s.object != nil else { return nil }
            self = .push(data: s)
        case "set_password":
            guard let e = j["email"]?.string, let p = j["password"]?.string else { return nil }
            self = .setPassword(email: e, password: p, oldPassword: j["old_password"]?.string)
        case "push_token":
            self = .pushToken(token: j["token"]?.string ?? "", platform: j["platform"]?.string ?? "", env: j["env"]?.string ?? "")
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

    public init(tag: UInt64, snapshot: JSON?, rev: Int64, member: UInt64? = nil, role: String? = nil, token: String? = nil, email: String? = nil) {
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
    case update(data: JSON, rev: Int64)
    case account(email: String)
    case ping
    case pong
    case error(code: String?, message: String)
    case unknown(String)

    public var json: JSON {
        switch self {
        case let .welcome(w):
            var o: [String: JSON] = ["type": "welcome", "tag": .id(w.tag), "snapshot": w.snapshot ?? .null, "rev": .int(w.rev)]
            if let m = w.member { o["member"] = .id(m) }
            if let r = w.role { o["role"] = .string(r) }
            if let t = w.token { o["token"] = .string(t) }
            if let e = w.email { o["email"] = .string(e) }
            return .object(o)
        case let .update(data, rev):
            return ["type": "update", "data": data, "rev": .int(rev)]
        case let .account(email):
            return ["type": "account", "email": .string(email)]
        case .ping: return ["type": "ping"]
        case .pong: return ["type": "pong"]
        case let .error(code, m):
            var o: [String: JSON] = ["type": "error", "message": .string(m)]
            if let code { o["code"] = .string(code) }
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
                                    member: j["member"]?.uint, role: j["role"]?.string, token: j["token"]?.string, email: j["email"]?.string))
        case "update":
            if let s = j["data"], s.object != nil {
                self = .update(data: s, rev: j["rev"]?.int ?? 0)
            } else {
                self = .unknown("update without data")
            }
        case "account":
            self = .account(email: j["email"]?.string ?? "")
        case "ping": self = .ping
        case "pong": self = .pong
        case "error": self = .error(code: j["code"]?.string, message: j["message"]?.string ?? "")
        default: self = .unknown(type)
        }
    }
}
