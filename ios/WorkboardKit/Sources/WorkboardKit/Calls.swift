// Huddles. Joining asks workboard-server over the open WebSocket, as the Mac
// app does (wb_net::Msg::CallJoin):
//
//   app -> server  call_join  {channel}
//   server -> app  call_token {channel, url, token}     the LiveKit address and a join token
//                  error {code: "denied", channel, message}   turned down
//
// The server names the room "wb-<channel>" and the person "<member>-<device>"
// (server/src/lib.rs call_token). A server without LiveKit turns the request
// down with "Voice huddles aren't set up on this server yet."; then the app
// falls back to the Mac app's link call (a Jitsi link in a message).

import Foundation

public struct CallGrant: Equatable, Sendable {
    public let url: String
    public let token: String
    public let room: String

    public init(url: String, token: String, room: String) {
        self.url = url
        self.token = token
        self.room = room
    }
}

public enum CallError: Error, Equatable, Sendable {
    /// No in-app calls on this server; post a link call instead.
    case notAvailable
    /// Not connected to the server right now.
    case offline
    /// The server didn't answer in time.
    case noAnswer
    /// The server turned it down; its words.
    case denied(String)
}

public enum CallAPI {
    /// The room the server makes for a conversation (Hub::room in
    /// server/src/lib.rs): the workspace id goes in for a workspace other
    /// than the server's first, as channel ids repeat between workspaces.
    public static func room(channel: UInt64, workspace: String? = nil) -> String {
        workspace.map { "wb-\($0)-\(channel)" } ?? "wb-\(channel)"
    }

    /// How the server words "this server has no LiveKit".
    public static func meansNotSetUp(_ message: String) -> Bool {
        message.localizedCaseInsensitiveContains("aren't set up") || message.localizedCaseInsensitiveContains("not set up")
    }

    /// The member in a LiveKit identity: "<member>-<device>" from the real
    /// server ("member-<id>" from older fake servers).
    public static func member(inIdentity s: String) -> UInt64? {
        if s.hasPrefix("member-") { return UInt64(s.dropFirst(7)) }
        guard let dash = s.firstIndex(of: "-") else { return UInt64(s) }
        return UInt64(s[..<dash])
    }

    /// The link call the Mac app makes when there is no call server
    /// (call_link in src/app/together.rs).
    public static func linkCall() -> String {
        let n = UInt64.random(in: 0...UInt64.max)
        return "https://meet.jit.si/Workboard-" + String(format: "%016llx", n)
    }

    /// What a "Started a huddle" message carries (HUDDLE_PREFIX in src/app/huddle.rs).
    public static let huddlePrefix = "workboard-huddle:"

    /// The conversation a huddle card is for, if `call` is one.
    public static func huddleChannel(_ call: String) -> UInt64? {
        guard call.hasPrefix(huddlePrefix) else { return nil }
        return UInt64(call.dropFirst(huddlePrefix.count))
    }
}

/// Raised hands and reactions in a huddle travel as small data messages in
/// the call, the same as the Mac app's (Signal in crates/wb-voice/src/lib.rs):
/// topic "workboard-huddle", text "hand:1", "hand:0" or "react:<emoji>".
public enum HuddleSignal: Equatable, Sendable {
    case hand(Bool)
    case react(String)

    public static let topic = "workboard-huddle"

    /// The reactions to pick from (REACTIONS in src/app/huddle/panel.rs).
    public static let reactions = ["\u{1F44D}", "\u{1F44F}", "\u{1F602}", "\u{2764}\u{FE0F}", "\u{1F389}", "\u{1F62E}", "\u{1F64C}", "\u{1F44B}"]

    public init?(data: Data) {
        guard let text = String(data: data, encoding: .utf8), let colon = text.firstIndex(of: ":") else { return nil }
        let kind = text[..<colon]
        let value = String(text[text.index(after: colon)...])
        switch (kind, value) {
        case ("hand", "1"): self = .hand(true)
        case ("hand", "0"): self = .hand(false)
        // a reaction is one emoji, a few characters at most
        case ("react", _) where !value.isEmpty && value.unicodeScalars.count <= 8: self = .react(value)
        default: return nil
        }
    }

    public var data: Data {
        switch self {
        case .hand(let up): return Data("hand:\(up ? 1 : 0)".utf8)
        case .react(let e): return Data("react:\(e)".utf8)
        }
    }
}

/// Who has a hand up and which reactions are showing, from the messages
/// that came (and your own). `me` is the key for yours.
public struct HuddleSignals: Equatable, Sendable {
    public struct Reaction: Equatable, Sendable, Identifiable {
        public let id: Int
        public let who: String
        public let emoji: String
        public let at: Date
    }

    /// How long a reaction shows over its sender's tile.
    public static let reactionSeconds: TimeInterval = 3.5
    public static let me = ""

    public private(set) var hands: [String: Bool] = [:]
    public private(set) var reactions: [Reaction] = []
    private var next = 0

    public init() {}

    public func handUp(_ who: String) -> Bool { hands[who] ?? false }

    /// A message from `who` (an identity, or `me`).
    public mutating func take(_ s: HuddleSignal, from who: String, at now: Date = Date()) {
        switch s {
        case .hand(let up):
            hands[who] = up
        case .react(let e):
            prune(now)
            // a burst of taps shows a few, not a column
            guard reactions.filter({ $0.who == who }).count < 6 else { return }
            next += 1
            reactions.append(Reaction(id: next, who: who, emoji: e, at: now))
        }
    }

    /// Someone left: their hand goes down.
    public mutating func left(_ who: String) {
        hands[who] = nil
    }

    /// Drop reactions that have finished showing.
    public mutating func prune(_ now: Date = Date()) {
        reactions.removeAll { now.timeIntervalSince($0.at) >= Self.reactionSeconds }
    }

    public func reactions(of who: String) -> [Reaction] {
        reactions.filter { $0.who == who }
    }
}
