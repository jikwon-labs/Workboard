// "Someone is typing..." in a conversation, as on the Mac (src/app/typing.rs):
//
//   app -> server  typing {channel}            at most every few seconds while typing
//   server -> app  typing {channel, member}    to the others who can see that conversation
//
// Nothing is saved. Someone shows as typing for a few seconds after the
// server last said so, or until their message arrives.

import Foundation

/// Someone the server said is typing in a conversation.
public struct Typist: Equatable, Sendable {
    public let channel: UInt64
    public let member: UInt64
    public var heard: Date
    /// Their newest message in that conversation when it was heard: a newer
    /// one means they sent what they were typing.
    public let lastMessage: UInt64?
}

public enum TypingAPI {
    /// How often the app says its person is still typing (TYPING_EVERY).
    public static let every: TimeInterval = 3
    /// How long someone shows as typing after the server last said so
    /// (TYPING_SHOWS), a bit more than `every`.
    public static let shows: TimeInterval = 5
}

extension SyncClient {
    /// The id of `member`'s newest message in conversation `channel`.
    func newestMessage(in channel: UInt64, by member: UInt64) -> UInt64? {
        view.channels.first { $0.id == channel }?.messages.last { $0.author == member }?.id
    }

    /// The server says `member` is typing in `channel`.
    func heardTyping(channel: UInt64, member: UInt64) {
        guard member != me else { return }
        let now = Date()
        var list = typists.filter { !($0.channel == channel && $0.member == member) && now.timeIntervalSince($0.heard) < TypingAPI.shows }
        list.append(Typist(channel: channel, member: member, heard: now, lastMessage: newestMessage(in: channel, by: member)))
        typists = list
    }

    /// This person typed in the message box of `channel`: tell the server, at
    /// most every `TypingAPI.every` (only while connected; it isn't kept for later).
    public func typingNow(channel: UInt64) {
        guard state.isConnected, let ws = socket else { return }
        if let t = typedAt, t.channel == channel, Date().timeIntervalSince(t.at) < TypingAPI.every { return }
        typedAt = (channel, Date())
        send(.typing(channel: channel), on: ws)
    }

    /// Who is typing in `channel` at `now`, oldest first: heard lately and
    /// not having sent a message there since.
    public func typing(in channel: UInt64, at now: Date = Date()) -> [UInt64] {
        var out: [UInt64] = []
        for t in typists where t.channel == channel && now.timeIntervalSince(t.heard) < TypingAPI.shows {
            if newestMessage(in: channel, by: t.member) != t.lastMessage || out.contains(t.member) { continue }
            out.append(t.member)
        }
        return out
    }
}
