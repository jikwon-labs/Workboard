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
