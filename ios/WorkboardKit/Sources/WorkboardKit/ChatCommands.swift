// Slash commands, webhooks and workflows, the Mac app's way
// (src/app/integrations.rs, src/app/workflows.rs, server/src/hooks.rs):
// "/shrug", "/me", "/topic", "/mute", "/unmute", "/remind" and "/poll" work
// in the app; the commands an owner or admin added go to the team server, which
// posts their answer. Workflows (automations with `trigger` "message") run
// in the app of whoever posts the message, right after sending.

import Foundation

/// A webhook (wb_net::HookInfo).
public struct HookItem: Identifiable, Hashable, Sendable {
    public var id: String { token }
    public let token: String
    public let kind: String
    public let channel: UInt64
    public let name: String
    public let url: String
    public let trigger: String

    init?(_ j: JSON) {
        guard let t = j["token"]?.string, let c = j["channel"]?.uint else { return nil }
        token = t; channel = c
        kind = j["kind"]?.string ?? ""
        name = j["name"]?.string ?? ""
        url = j["url"]?.string ?? ""
        trigger = j["trigger"]?.string ?? ""
    }

    /// A new webhook as hook_add sends it; the server fills in the token
    /// and creator.
    static func request(kind: String, channel: UInt64, name: String, url: String = "", trigger: String = "") -> JSON {
        var h: [String: JSON] = ["kind": .string(kind), "channel": .id(channel), "name": .string(name.trimmed)]
        if !url.trimmed.isEmpty { h["url"] = .string(url.trimmed) }
        if !trigger.trimmed.isEmpty { h["trigger"] = .string(trigger.trimmed) }
        return .object(h)
    }
}

/// A slash command (wb_net::CommandInfo).
public struct CommandItem: Identifiable, Hashable, Sendable {
    public var id: String { name }
    public let name: String
    public let description: String
    public let url: String

    init?(_ j: JSON) {
        guard let n = j["name"]?.string, !n.isEmpty else { return nil }
        name = n
        description = j["description"]?.string ?? ""
        url = j["url"]?.string ?? ""
    }

    /// A new command as command_add sends it.
    static func request(name: String, url: String, description: String) -> JSON {
        ["name": .string(name.trimmed), "url": .string(url.trimmed), "description": .string(description.trimmed)]
    }
}

public enum SlashCommands {
    /// The commands that work in the app: (name, what it does).
    public static let builtIn: [(String, String)] = [
        ("shrug", "Add ¯\\_(ツ)_/¯ to your message"),
        ("me", "Say what you're doing, in italics"),
        ("topic", "Set this channel's topic"),
        ("mute", "Turn off notifications for this channel"),
        ("unmute", "Turn notifications for this channel back on"),
        ("remind", "Remind yourself: /remind call Sam in 30 minutes"),
        ("poll", "Ask the conversation a question with choices"),
    ]

    /// What a "/" message is.
    public enum Parsed: Equatable, Sendable {
        /// not a command ("/usr/bin is full")
        case plain
        case shrug(String)
        case me(String)
        case topic(String)
        case mute
        case unmute
        case remind(String, seconds: Int64)
        /// /remind without a time it understands
        case remindHelp
        /// /poll [question]: the New poll window, with the question filled in
        case poll(String)
        /// a server's command, or one not known (`known` false)
        case server(name: String, text: String, known: Bool)
    }

    public static func parse(_ text: String, commands: [CommandItem]) -> Parsed {
        guard text.hasPrefix("/") else { return .plain }
        let body = String(text.dropFirst())
        let name: String, rest: String
        if let sp = body.firstIndex(where: \.isWhitespace) {
            name = String(body[..<sp]); rest = body[sp...].trimmingCharacters(in: .whitespaces)
        } else {
            name = body; rest = ""
        }
        if name.isEmpty || name.contains("/") { return .plain }
        switch name.lowercased() {
        case "shrug": return .shrug(rest.isEmpty ? "¯\\_(ツ)_/¯" : rest + " ¯\\_(ツ)_/¯")
        case "me" where !rest.isEmpty: return .me("*\(rest)*")
        case "topic": return .topic(rest)
        case "mute": return .mute
        case "unmute": return .unmute
        case "remind":
            guard let (what, secs) = remindParts(rest) else { return .remindHelp }
            return .remind(what, seconds: secs)
        case "poll": return .poll(rest)
        default:
            let lower = name.lowercased()
            return .server(name: lower, text: rest, known: commands.contains { $0.name == lower })
        }
    }

    /// "in 30 minutes" / "in 2 hours" at the end: (what, seconds).
    public static func remindParts(_ rest: String) -> (String, Int64)? {
        let w = rest.split(whereSeparator: \.isWhitespace).map(String.init)
        let n = w.count
        guard n >= 3, w[n - 3].lowercased() == "in", let count = Int64(w[n - 2]), count > 0 else { return nil }
        var unit = w[n - 1].lowercased()
        while unit.hasSuffix("s") { unit.removeLast() }
        let secs: Int64
        switch unit {
        case "minute", "min", "m": secs = 60
        case "hour", "hr", "h": secs = 3600
        case "day", "d": secs = 86_400
        default: return nil
        }
        var what = w[..<(n - 3)].joined(separator: " ")
        if what.hasPrefix("me ") { what.removeFirst(3) }
        what = what.trimmingCharacters(in: .whitespaces)
        return what.isEmpty ? nil : (what, count * secs)
    }

    /// The commands the "/" list offers for what is typed: (name, description).
    public static func choices(_ typed: String, commands: [CommandItem]) -> [(String, String)] {
        guard typed.hasPrefix("/") else { return [] }
        let q = typed.dropFirst().lowercased()
        guard !q.contains(where: \.isWhitespace) else { return [] }
        return (builtIn + commands.map { ($0.name, $0.description) }).filter { q.isEmpty || $0.0.hasPrefix(q) }
    }
}

extension SyncClient {
    /// A word for the person, shown briefly (an unknown command, say).
    public func showNotice(_ text: String) { notice = text }

    public func askIntegrations() {
        if state.isConnected, let ws = socket { send(.integrations, on: ws) }
    }

    /// Send "/name text" to the server; false when not connected.
    @discardableResult
    public func runCommand(_ name: String, text: String, channel: UInt64, parent: UInt64? = nil) -> Bool {
        guard state.isConnected, let ws = socket else { return false }
        send(.command(channel: channel, parent: parent, name: name, text: text), on: ws)
        return true
    }

    public func addHook(kind: String, channel: UInt64, name: String, url: String = "", trigger: String = "") {
        guard let ws = socket else { return }
        send(.hookAdd(hook: HookItem.request(kind: kind, channel: channel, name: name, url: url, trigger: trigger)), on: ws)
    }

    public func removeHook(_ token: String) {
        if let ws = socket { send(.hookRemove(token: token), on: ws) }
        hooks.removeAll { $0.token == token }
    }

    public func addCommand(name: String, url: String, description: String) {
        guard let ws = socket else { return }
        send(.commandAdd(command: CommandItem.request(name: name, url: url, description: description)), on: ws)
    }

    public func removeCommand(_ name: String) {
        if let ws = socket { send(.commandRemove(name: name), on: ws) }
        commands.removeAll { $0.name == name }
    }

    /// The address an incoming webhook takes posts at (wb_net::http_address).
    public func hookAddress(_ token: String) -> String {
        ServerAddress.httpAddress(settings.server, path: "/hooks/\(token)") ?? ""
    }
}

extension ServerAddress {
    /// `path` on the server (or the server's workspace) `addr` points to:
    /// "https://wb.example.com/w/ab12" + "/hooks/t" (wb_net::http_address).
    public static func httpAddress(_ addr: String, path: String) -> String? {
        var a = addr.trimmingCharacters(in: .whitespaces)
        while a.hasSuffix("/") { a.removeLast() }
        guard !a.isEmpty else { return nil }
        let lower = a.lowercased()
        var scheme = "http", rest = a
        for (p, s) in [("wss://", "https"), ("https://", "https"), ("ws://", "http"), ("http://", "http")] where lower.hasPrefix(p) {
            scheme = s; rest = String(a.dropFirst(p.count)); break
        }
        var host = rest, sub = ""
        if let slash = rest.firstIndex(of: "/") { host = String(rest[..<slash]); sub = String(rest[slash...]) }
        if sub.hasSuffix("/ws") { sub.removeLast(3) }
        if !host.contains(":") && scheme == "http" { host += ":8787" }
        return "\(scheme)://\(host)\(sub)\(path)"
    }
}

// MARK: workflows

extension Edits {
    /// The name workflow messages go under.
    public static let workflowBot = "Workflow"

    /// A workflow: when a message is posted in `channel` (with `keyword` in
    /// it, when set), then post `text` in `target`, reply `text` in its
    /// thread, or make a task in List `list`.
    @discardableResult
    public static func addWorkflow(_ data: inout JSON, channel: UInt64, keyword: String, action: String, text: String, target: UInt64?, list: UInt64?, id: UInt64) -> Bool {
        var r: [String: JSON] = ["id": .id(id), "enabled": true, "trigger_status": "", "action": .string(action),
                                 "list": .null, "trigger": "message", "channel": .id(channel)]
        if !keyword.trimmed.isEmpty { r["keyword"] = .string(keyword.trimmed) }
        switch action {
        case "post":
            guard let target, !text.trimmed.isEmpty else { return false }
            r["value"] = .string(text.trimmed); r["target"] = .id(target)
        case "reply":
            guard !text.trimmed.isEmpty else { return false }
            r["value"] = .string(text.trimmed)
        case "task":
            guard let list else { return false }
            r["value"] = .string(String(list))
        default: return false
        }
        var rules = data.at("extra.automations")?.array ?? []
        rules.append(.object(r))
        data.set("extra.automations", .array(rules))
        return true
    }

    /// Run the workflows my message `message` (just posted in `channel`)
    /// starts. `ids` makes the new messages' and tasks' ids.
    public static func runWorkflows(_ data: inout JSON, channel: UInt64, message: UInt64, me: UInt64, ts: Int64, ids: () -> UInt64) {
        guard let c = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == channel }),
              let m = (c["messages"]?.array ?? []).first(where: { $0["id"]?.uint == message }),
              m["bot"]?.string == nil else { return }
        // a post: the keyword may be in its title, and without text its
        // title is the {text} (as on the Mac, workflows.rs); not the default
        // title a post without one gets, which nobody wrote
        let rawTitle = (m["post_title"]?.string ?? "").trimmed
        let title: String? = rawTitle.isEmpty || rawTitle == MessageView.defaultPostTitle ? nil : rawTitle
        let typed = m["text"]?.string ?? ""
        let text = typed.trimmed.isEmpty ? (title ?? typed) : typed
        let rules = (data.at("extra.automations")?.array ?? []).filter { a in
            guard a["enabled"]?.bool == true, a["trigger"]?.string == "message", a["channel"]?.uint == channel else { return false }
            let k = (a["keyword"]?.string ?? "").trimmed.lowercased()
            return k.isEmpty || typed.lowercased().contains(k) || (title?.lowercased().contains(k) ?? false)
        }
        guard !rules.isEmpty else { return }
        let members = data["members"]?.array ?? []
        let user = members.first { $0["id"]?.uint == me }.map(Mentions.shownName) ?? ""
        let place = "#" + (c["name"]?.string ?? "")
        let said = ChatFormat.blocks(text).map { b -> String in
            switch b {
            case .text(let s), .quote(let s), .bullet(let s): return String(ChatFormat.inline(s).characters)
            case .code(let s): return s
            }
        }.joined(separator: "\n")
        func fill(_ t: String) -> String {
            t.replacingOccurrences(of: "{user}", with: user).replacingOccurrences(of: "{text}", with: said).replacingOccurrences(of: "{channel}", with: place)
        }
        for a in rules {
            let value = a["value"]?.string ?? ""
            switch a["action"]?.string {
            case "reply":
                let id = ids()
                sendReply(&data, channel: channel, to: message, text: fill(value), me: me, id: id, ts: ts)
                _ = changeMessage(&data, channel: channel, message: id) { $0["bot"] = .string(workflowBot) }
            case "task":
                if let list = UInt64(value) {
                    let name = String((said.split(separator: "\n").first.map(String.init) ?? "").prefix(120))
                    if !name.trimmed.isEmpty { addTask(&data, list: list, name: name, assignees: [me], me: me, id: ids(), ts: ts) }
                }
            default:
                if let to = a["target"]?.uint {
                    let id = ids()
                    sendMessage(&data, channel: to, text: fill(value), me: me, id: id, ts: ts)
                    _ = changeMessage(&data, channel: to, message: id) { $0["bot"] = .string(workflowBot) }
                }
            }
        }
    }
}
