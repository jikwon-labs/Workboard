// "Someone is typing..." in a conversation, as on the Mac (src/app/typing.rs):
//
//   app -> server  typing {channel, thread?}           at most every few seconds while typing
//   server -> app  typing {channel, member, thread?}   to the others who can see that conversation
//
// `thread` is the message whose thread is being answered: that typing shows
// only in the thread, not under the conversation's own box. Nothing is
// saved. Someone shows as typing for a few seconds after the server last
// said so, or until their message arrives.

import Foundation

/// Someone the server said is typing in a conversation.
public struct Typist: Equatable, Sendable {
    public let channel: UInt64
    public let member: UInt64
    /// The message whose thread they're answering; nil in the conversation.
    public var thread: UInt64? = nil
    public var heard: Date
    /// Their newest message in that conversation (or reply in that thread)
    /// when it was heard: a newer one means they sent what they were typing.
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
    /// The id of `member`'s newest message in conversation `channel`, or
    /// newest reply in the thread of message `thread` there.
    func newestMessage(in channel: UInt64, by member: UInt64, thread: UInt64? = nil) -> UInt64? {
        guard let c = view.channels.first(where: { $0.id == channel }) else { return nil }
        if let thread {
            return c.messages.first { $0.id == thread }?.replies.last { $0.author == member }?.id
        }
        return c.messages.last { $0.author == member }?.id
    }

    /// The server says `member` is typing in `channel` (in the thread of
    /// message `thread`). One place at a time: it replaces where they were.
    func heardTyping(channel: UInt64, member: UInt64, thread: UInt64? = nil) {
        guard member != me else { return }
        let now = Date()
        var list = typists.filter { !($0.channel == channel && $0.member == member) && now.timeIntervalSince($0.heard) < TypingAPI.shows }
        list.append(Typist(channel: channel, member: member, thread: thread, heard: now,
                           lastMessage: newestMessage(in: channel, by: member, thread: thread)))
        typists = list
    }

    /// This person typed in the message box of `channel` (or the reply box
    /// of message `thread`'s thread): tell the server, at most every
    /// `TypingAPI.every` (only while connected; it isn't kept for later).
    public func typingNow(channel: UInt64, thread: UInt64? = nil) {
        guard state.isConnected, let ws = socket else { return }
        if let t = typedAt, t.channel == channel, t.thread == thread, Date().timeIntervalSince(t.at) < TypingAPI.every { return }
        typedAt = (channel, thread, Date())
        send(.typing(channel: channel, thread: thread), on: ws)
    }

    /// Who is typing in `channel` (or in the thread of message `thread`
    /// there) at `now`, oldest first: heard lately and not having sent a
    /// message there since. Typing in a thread isn't counted for the channel.
    public func typing(in channel: UInt64, thread: UInt64? = nil, at now: Date = Date()) -> [UInt64] {
        var out: [UInt64] = []
        for t in typists where t.channel == channel && t.thread == thread && now.timeIntervalSince(t.heard) < TypingAPI.shows {
            if newestMessage(in: channel, by: t.member, thread: thread) != t.lastMessage || out.contains(t.member) { continue }
            out.append(t.member)
        }
        return out
    }
}
