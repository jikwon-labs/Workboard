// The workboard-server protocol (wb_net::Msg in crates/wb-net/src/protocol.rs):
// JSON text messages over a WebSocket at ws://host:port/ws, each with a
// snake_case "type".
//
//   app -> server  hello        {device, tag?, token?, invite?, claim?,
//                                email?, password?, new_password?, reset_code?,
//                                link_code?, app?, sync?}   (app version and sync format;
//                                the server takes no pushes from apps with
//                                sync below wb_net::OLDEST_SYNC or none)
//   server -> app  welcome      {tag, snapshot|null, rev, member, role, token?, email?}
//   app -> server  push         {data}   (whole, or only what changed: Sync.delta)
//   server -> all  update       {data, rev, hidden?}   (hidden also in welcome: ids
//                                this person may not see; dropped without tombstones)
//   server -> app  delta        {data, rev, base, hidden?}   (what changed since
//                                revision base; only to apps whose hello said
//                                deltas, from servers whose welcome says deltas)
//   app -> server  resync       {}   (a revision went missing: send the whole
//                                copy again as update)
//   app -> server  set_password {email, password, old_password?}
//   server -> app  account      {email}
//   both           mine         {data}   (my own reading state and notification
//                                choices, Mine.swift; the server sends it after
//                                welcome and when another device of mine changes it,
//                                only to my connections. The app sends it only after
//                                hearing it on that connection.)
//   app -> server  push_token   {token, platform: "ios", env}   (ignored by servers that don't know it)
//   app -> server  push_token   {token: ""}  stops notifications to this device
//                                (sent on sign-out and before leaving for another server)
//   app -> server  create_invite {member, role}   owner or admin
//   server -> app  invite       {member, code, emailed?, mail_error?}
//   app -> server  create_link_code   (the Mac only: a 6-character code that
//                                signs a phone in as the same person)
//   server -> app  link_code    {code, seconds}   (iOS never asks; ignored)
//   app -> server  set_role     {member, role}    owner or admin
//   app -> server  revoke       {member}          owner or admin
//   app -> server  access_list
//   server -> app  access       {people: [{member, role, devices, invite?, account?}], role}
//                                (also sent unasked after any access change)
//   app -> server  backup_list                    owner only
//   server -> app  backups      {backups: [{name, bytes}]}
//   app -> server  restore      {name}            owner only
//   server -> app  restored     {name}
//   app -> server  call_join    {channel}
//   server -> app  call_token   {channel, url, token}   (a LiveKit address and join token)
//   both           ping / pong, every 30 seconds
//   server -> app  error        {message, code?, channel?}   (channel: a call_join turned down)
//
// Optional hello fields are left out when absent: the server treats a field
// that is there as meant. It uses the first that applies: reset_code (with
// email and new_password), invite (with new_password: make the account),
// link_code (the code the person's computer shows as a QR code or six
// letters and digits), email + password, token, claim. iOS never claims: claiming an empty
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
    /// The code the person's computer showed (Work together > Sign in on
    /// your phone), from its QR code or typed.
    public var linkCode: String?
    /// This app's version, and the sync format it writes (Sync.metaV).
    public var app: String?
    public var sync: UInt64?
    /// This app takes `delta` messages.
    public var deltas = false

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
    /// `workspace`: this iPhone's name for the workspace (`SyncClient.pushKey`),
    /// which the server sends back as "ws" in each notification. `prefs`:
    /// this phone's notification choices (NotifyPrefs.forServer).
    case pushToken(token: String, platform: String, env: String, workspace: String? = nil, prefs: JSON? = nil)
    case callJoin(channel: UInt64)
    /// This person is typing in a conversation (wb_net::Msg::Typing, sent
    /// without `member`; see Typing.swift), in the thread of message
    /// `thread` when there is one.
    case typing(channel: UInt64, thread: UInt64? = nil)
    case resync
    case createInvite(member: UInt64, role: Role)
    case setRole(member: UInt64, role: Role)
    case revoke(member: UInt64)
    case accessList
    case backupList
    /// Keep a message to post later (wb_net::ScheduledMsg as JSON).
    case schedule(item: JSON)
    case unschedule(id: UInt64)
    case scheduledList
    /// My reading state and notification choices (wb_net::Msg::Mine,
    /// Mine.swift), sent when they change here.
    case mine(data: JSON)
    /// Webhooks and slash commands (wb_net::HookInfo, wb_net::CommandInfo as JSON).
    case integrations
    case hookAdd(hook: JSON)
    case hookRemove(token: String)
    case commandAdd(command: JSON)
    case commandRemove(name: String)
    case command(channel: UInt64, parent: UInt64?, name: String, text: String)
    case restore(name: String)
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
            put("link_code", h.linkCode)
            put("app", h.app)
            if let v = h.sync { o["sync"] = .id(v) }
            if h.deltas { o["deltas"] = true }
            return .object(o)
        case let .push(data):
            return ["type": "push", "data": data]
        case let .setPassword(email, password, old):
            var o: [String: JSON] = ["type": "set_password", "email": .string(email), "password": .string(password)]
            if let old, !old.isEmpty { o["old_password"] = .string(old) }
            return .object(o)
        case let .pushToken(token, platform, env, workspace, prefs):
            var o: [String: JSON] = ["type": "push_token", "token": .string(token), "platform": .string(platform), "env": .string(env)]
            if let workspace, !workspace.isEmpty { o["workspace"] = .string(workspace) }
            if let prefs { o["prefs"] = prefs }
            return .object(o)
        case let .callJoin(channel):
            return ["type": "call_join", "channel": .id(channel)]
        case let .typing(channel, thread):
            var o: [String: JSON] = ["type": "typing", "channel": .id(channel)]
            if let thread { o["thread"] = .id(thread) }
            return .object(o)
        case .resync: return ["type": "resync"]
        case let .createInvite(member, role):
            return ["type": "create_invite", "member": .id(member), "role": .string(role.rawValue)]
        case let .setRole(member, role):
            return ["type": "set_role", "member": .id(member), "role": .string(role.rawValue)]
        case let .revoke(member):
            return ["type": "revoke", "member": .id(member)]
        case .accessList: return ["type": "access_list"]
        case .backupList: return ["type": "backup_list"]
        case let .schedule(item): return ["type": "schedule", "item": item]
        case let .unschedule(id): return ["type": "unschedule", "id": .id(id)]
        case .scheduledList: return ["type": "scheduled_list"]
        case let .mine(data): return ["type": "mine", "data": data]
        case .integrations: return ["type": "integrations"]
        case let .hookAdd(hook): return ["type": "hook_add", "hook": hook]
        case let .hookRemove(token): return ["type": "hook_remove", "token": .string(token)]
        case let .commandAdd(command): return ["type": "command_add", "command": command]
        case let .commandRemove(name): return ["type": "command_remove", "name": .string(name)]
        case let .command(channel, parent, name, text):
            var o: [String: JSON] = ["type": "command", "channel": .id(channel), "name": .string(name), "text": .string(text)]
            if let parent { o["parent"] = .id(parent) }
            return .object(o)
        case let .restore(name):
            return ["type": "restore", "name": .string(name)]
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
            var h = Hello(device: device, tag: j["tag"]?.uint, token: j["token"]?.string, invite: j["invite"]?.string,
                          claim: j["claim"]?.uint, email: j["email"]?.string, password: j["password"]?.string,
                          newPassword: j["new_password"]?.string, resetCode: j["reset_code"]?.string,
                          app: j["app"]?.string, sync: j["sync"]?.uint)
            h.deltas = j["deltas"]?.bool ?? false
            h.linkCode = j["link_code"]?.string
            self = .hello(h)
        case "push":
            guard let s = j["data"], s.object != nil else { return nil }
            self = .push(data: s)
        case "set_password":
            guard let e = j["email"]?.string, let p = j["password"]?.string else { return nil }
            self = .setPassword(email: e, password: p, oldPassword: j["old_password"]?.string)
        case "push_token":
            self = .pushToken(token: j["token"]?.string ?? "", platform: j["platform"]?.string ?? "", env: j["env"]?.string ?? "", workspace: j["workspace"]?.string, prefs: j["prefs"])
        case "call_join":
            guard let ch = j["channel"]?.uint else { return nil }
            self = .callJoin(channel: ch)
        case "typing":
            guard let ch = j["channel"]?.uint else { return nil }
            self = .typing(channel: ch, thread: j["thread"]?.uint)
        case "resync": self = .resync
        case "create_invite":
            guard let m = j["member"]?.uint else { return nil }
            self = .createInvite(member: m, role: Role(rawValue: j["role"]?.string ?? "") ?? .member)
        case "set_role":
            guard let m = j["member"]?.uint, let r = Role(rawValue: j["role"]?.string ?? "") else { return nil }
            self = .setRole(member: m, role: r)
        case "revoke":
            guard let m = j["member"]?.uint else { return nil }
            self = .revoke(member: m)
        case "access_list": self = .accessList
        case "backup_list": self = .backupList
        case "schedule": self = .schedule(item: j["item"] ?? .null)
        case "unschedule": self = .unschedule(id: j["id"]?.uint ?? 0)
        case "scheduled_list": self = .scheduledList
        case "mine": self = .mine(data: j["data"] ?? [:])
        case "integrations": self = .integrations
        case "hook_add": self = .hookAdd(hook: j["hook"] ?? .null)
        case "hook_remove": self = .hookRemove(token: j["token"]?.string ?? "")
        case "command_add": self = .commandAdd(command: j["command"] ?? .null)
        case "command_remove": self = .commandRemove(name: j["name"]?.string ?? "")
        case "command": self = .command(channel: j["channel"]?.uint ?? 0, parent: j["parent"]?.uint, name: j["name"]?.string ?? "", text: j["text"]?.string ?? "")
        case "restore":
            guard let n = j["name"]?.string else { return nil }
            self = .restore(name: n)
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
    /// The server sends deltas and takes pushes of only what changed.
    public var deltas = false

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
    /// What changed from revision `base` to `rev`.
    case delta(data: JSON, rev: Int64, base: Int64, hidden: [UInt64] = [])
    case account(email: String)
    case callToken(channel: UInt64, url: String, token: String)
    /// `member` is typing in `channel`, in the thread of message `thread`
    /// when there is one (see Typing.swift).
    case typing(channel: UInt64, member: UInt64, thread: UInt64? = nil)
    /// A new invite code (answer to create_invite); `emailed` is the address
    /// the server mailed it to, `mailError` why mailing failed.
    case invite(member: UInt64, code: String, emailed: String? = nil, mailError: String? = nil)
    /// Everyone's access as the asker may see it (empty for plain members)
    /// and the asker's own role. `answer`: true when it answers this
    /// connection's own access_list, set_role or revoke, false when it was
    /// sent unasked after a change; nil from servers older than that.
    case access(people: [Person], role: Role, answer: Bool? = nil)
    case backups([Backup])
    case restored(name: String)
    /// Who is connected now (members), sent to everyone when it changes.
    case presence([UInt64])
    /// My scheduled messages (wb_net::ScheduledMsg as JSON), soonest first.
    case scheduled([JSON])
    /// My reading state and notification choices as the server keeps them
    /// (after welcome, and when another device of mine changed them).
    case mine(JSON)
    /// The webhooks (owners and admins) and slash commands.
    case integrationList(hooks: [JSON], commands: [JSON])
    case ping
    case pong
    /// `channel` is set when a call_join for it was turned down; `putBack`
    /// when changes pushed from here were turned down (not an answer to a
    /// request, so never mixed up with one). `gone`: the workspace was
    /// deleted from the server (code "auth", message WorkspaceAPI.goneMessage).
    case error(code: String?, message: String, channel: UInt64? = nil, putBack: Bool = false, gone: Bool = false)
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
            if w.deltas { o["deltas"] = true }
            return .object(o)
        case let .update(data, rev, hidden):
            var o: [String: JSON] = ["type": "update", "data": data, "rev": .int(rev)]
            if !hidden.isEmpty { o["hidden"] = .array(hidden.map(JSON.id)) }
            return .object(o)
        case let .delta(data, rev, base, hidden):
            var o: [String: JSON] = ["type": "delta", "data": data, "rev": .int(rev), "base": .int(base)]
            if !hidden.isEmpty { o["hidden"] = .array(hidden.map(JSON.id)) }
            return .object(o)
        case let .account(email):
            return ["type": "account", "email": .string(email)]
        case let .callToken(channel, url, token):
            return ["type": "call_token", "channel": .id(channel), "url": .string(url), "token": .string(token)]
        case let .typing(channel, member, thread):
            var o: [String: JSON] = ["type": "typing", "channel": .id(channel), "member": .id(member)]
            if let thread { o["thread"] = .id(thread) }
            return .object(o)
        case let .invite(member, code, emailed, mailError):
            var o: [String: JSON] = ["type": "invite", "member": .id(member), "code": .string(code)]
            if let emailed { o["emailed"] = .string(emailed) }
            if let mailError { o["mail_error"] = .string(mailError) }
            return .object(o)
        case let .access(people, role, answer):
            var o: [String: JSON] = ["type": "access", "people": .array(people.map(\.json)), "role": .string(role.rawValue)]
            if let answer { o["answer"] = .bool(answer) }
            return .object(o)
        case let .backups(list):
            return ["type": "backups", "backups": .array(list.map(\.json))]
        case let .restored(name):
            return ["type": "restored", "name": .string(name)]
        case let .presence(online):
            return ["type": "presence", "online": .array(online.map(JSON.id))]
        case let .scheduled(items):
            return ["type": "scheduled", "items": .array(items)]
        case let .mine(data):
            return ["type": "mine", "data": data]
        case let .integrationList(hooks, commands):
            return ["type": "integration_list", "hooks": .array(hooks), "commands": .array(commands)]
        case .ping: return ["type": "ping"]
        case .pong: return ["type": "pong"]
        case let .error(code, m, channel, putBack, gone):
            var o: [String: JSON] = ["type": "error", "message": .string(m)]
            if let code { o["code"] = .string(code) }
            if let channel { o["channel"] = .id(channel) }
            if putBack { o["put_back"] = true }
            if gone { o["gone"] = true }
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
            var w = Welcome(tag: j["tag"]?.uint ?? 0, snapshot: snap?.object != nil ? snap : nil, rev: j["rev"]?.int ?? 0,
                            member: j["member"]?.uint, role: j["role"]?.string, token: j["token"]?.string, email: j["email"]?.string,
                            hidden: (j["hidden"]?.array ?? []).compactMap(\.uint))
            w.deltas = j["deltas"]?.bool ?? false
            self = .welcome(w)
        case "update":
            if let s = j["data"], s.object != nil {
                self = .update(data: s, rev: j["rev"]?.int ?? 0, hidden: (j["hidden"]?.array ?? []).compactMap(\.uint))
            } else {
                self = .unknown("update without data")
            }
        case "delta":
            if let s = j["data"], s.object != nil {
                self = .delta(data: s, rev: j["rev"]?.int ?? 0, base: j["base"]?.int ?? 0, hidden: (j["hidden"]?.array ?? []).compactMap(\.uint))
            } else {
                self = .unknown("delta without data")
            }
        case "account":
            self = .account(email: j["email"]?.string ?? "")
        case "call_token":
            self = .callToken(channel: j["channel"]?.uint ?? 0, url: j["url"]?.string ?? "", token: j["token"]?.string ?? "")
        case "typing":
            if let ch = j["channel"]?.uint, let m = j["member"]?.uint {
                self = .typing(channel: ch, member: m, thread: j["thread"]?.uint)
            } else {
                self = .unknown("typing without member")
            }
        case "invite":
            self = .invite(member: j["member"]?.uint ?? 0, code: j["code"]?.string ?? "", emailed: j["emailed"]?.string, mailError: j["mail_error"]?.string)
        case "access":
            self = .access(people: (j["people"]?.array ?? []).compactMap(Person.init), role: Role(rawValue: j["role"]?.string ?? "") ?? .member,
                           answer: j["answer"]?.bool)
        case "backups":
            self = .backups((j["backups"]?.array ?? []).compactMap(Backup.init))
        case "restored":
            self = .restored(name: j["name"]?.string ?? "")
        case "presence":
            self = .presence((j["online"]?.array ?? []).compactMap(\.uint))
        case "scheduled":
            self = .scheduled(j["items"]?.array ?? [])
        case "mine":
            self = .mine(j["data"] ?? [:])
        case "integration_list":
            self = .integrationList(hooks: j["hooks"]?.array ?? [], commands: j["commands"]?.array ?? [])
        case "ping": self = .ping
        case "pong": self = .pong
        case "error": self = .error(code: j["code"]?.string, message: j["message"]?.string ?? "", channel: j["channel"]?.uint, putBack: j["put_back"]?.bool ?? false,
                                           gone: j["gone"]?.bool ?? false)
        default: self = .unknown(type)
        }
    }
}

/// What someone may do on the server (wb_net::Role). The owner set the
/// workspace up; owners and admins manage access.
public enum Role: String, Comparable, Sendable, CaseIterable {
    case member, admin, owner

    /// May invite people, change roles and remove access.
    public var manages: Bool { self >= .admin }

    public var label: String {
        switch self {
        case .member: return "Member"
        case .admin: return "Admin"
        case .owner: return "Owner"
        }
    }

    var rank: Int { Role.allCases.firstIndex(of: self) ?? 0 }
    public static func < (a: Role, b: Role) -> Bool { a.rank < b.rank }
}

/// One person's access (wb_net::Person).
public struct Person: Equatable, Identifiable, Sendable {
    public var member: UInt64
    public var role: Role
    /// Devices signed in as this person.
    public var devices: Int
    /// An invite code not used yet (only the ones the asker may see).
    public var invite: String?
    /// Has set up an account (email and password).
    public var account: Bool

    public var id: UInt64 { member }

    public init(member: UInt64, role: Role, devices: Int = 0, invite: String? = nil, account: Bool = false) {
        self.member = member
        self.role = role
        self.devices = devices
        self.invite = invite
        self.account = account
    }

    init?(_ j: JSON) {
        guard let m = j["member"]?.uint else { return nil }
        self.init(member: m, role: Role(rawValue: j["role"]?.string ?? "") ?? .member, devices: Int(j["devices"]?.int ?? 0),
                  invite: j["invite"]?.string, account: j["account"]?.bool ?? false)
    }

    var json: JSON {
        var o: [String: JSON] = ["member": .id(member), "role": .string(role.rawValue), "devices": .int(Int64(devices))]
        if let invite { o["invite"] = .string(invite) }
        if account { o["account"] = true }
        return .object(o)
    }
}

/// A backup of the team's copy on the server (wb_net::Backup).
public struct Backup: Equatable, Identifiable, Sendable {
    /// File name in the server's backups folder, e.g. hourly-20261115-03.json
    public var name: String
    public var bytes: UInt64

    public var id: String { name }

    public init(name: String, bytes: UInt64) {
        self.name = name
        self.bytes = bytes
    }

    init?(_ j: JSON) {
        guard let n = j["name"]?.string else { return nil }
        self.init(name: n, bytes: j["bytes"]?.uint ?? 0)
    }

    var json: JSON { ["name": .string(name), "bytes": .id(bytes)] }

    /// "2026-11-15 03:00 (hourly)" from the file name (Backup::label).
    public var label: String {
        let stem = name.hasSuffix(".json") ? String(name.dropLast(5)) : name
        let kinds = [("hourly-", "hourly"), ("daily-", "daily"), ("before-restore-", "before a restore")]
        guard let (prefix, kind) = kinds.first(where: { stem.hasPrefix($0.0) }) else { return name }
        let rest = String(stem.dropFirst(prefix.count))
        let d = Array(rest.filter { $0.isASCII && $0.isNumber })
        func cut(_ a: Int, _ b: Int) -> String { String(d[a..<b]) }
        let date = d.count >= 8 ? "\(cut(0, 4))-\(cut(4, 6))-\(cut(6, 8))" : rest
        let time: String
        switch d.count {
        case 10: time = " \(cut(8, 10)):00"
        case 14: time = " \(cut(8, 10)):\(cut(10, 12)):\(cut(12, 14))"
        default: time = ""
        }
        return "\(date)\(time) (\(kind))"
    }
}
