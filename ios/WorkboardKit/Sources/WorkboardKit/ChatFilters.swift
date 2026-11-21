// Narrowing a conversation's messages, as the Mac app's channel panel does
// (Replies, Pinned messages, Saved, Files, Links, Turned into tasks), the
// conversation's files gathered in one place, and the operators quick find
// reads in a search (has:file, has:link, from:name).

import Foundation

/// What a conversation shows (the channel panel's filters on the Mac).
public enum MessageFilter: String, CaseIterable, Sendable {
    case all, replies, pinned, saved, files, links, tasks

    /// Whether the message (with its thread) is shown under this filter.
    public func keeps(_ m: MessageView) -> Bool {
        switch self {
        case .all: return true
        case .replies: return !m.replies.isEmpty
        case .pinned: return m.pinned
        case .saved: return m.saved || m.replies.contains { $0.saved }
        case .files: return m.hasFiles || m.replies.contains { $0.hasFiles }
        case .links: return m.hasLink || m.replies.contains { $0.hasLink }
        case .tasks: return m.task != nil
        }
    }
}

extension MessageView {
    /// Pictures, videos or other files were sent in it (has_files on the Mac).
    public var hasFiles: Bool { !attachments.isEmpty }

    /// It has a link preview or a web address in its text, not counting
    /// code (has_links on the Mac).
    public var hasLink: Bool { preview != nil || ChatFormat.firstURL(text) != nil }
}

/// One file sent in a conversation: in a message, or in a reply in its thread.
public struct ConversationFile: Hashable, Sendable {
    public let attachment: AttachmentView
    /// The message or reply it was sent in.
    public let message: MessageView
    /// The message whose thread the reply is in (nil when sent in the conversation).
    public let parent: UInt64?
}

extension ChannelView {
    /// Every file sent in the conversation and its threads, newest first.
    public var files: [ConversationFile] {
        var out: [ConversationFile] = []
        for m in messages {
            for a in m.attachments { out.append(ConversationFile(attachment: a, message: m, parent: nil)) }
            for r in m.replies {
                for a in r.attachments { out.append(ConversationFile(attachment: a, message: r, parent: m.id)) }
            }
        }
        // newest first; within one message in the order sent
        return out.enumerated().sorted { a, b in
            a.element.message.ts != b.element.message.ts ? a.element.message.ts > b.element.message.ts : a.offset < b.offset
        }.map(\.element)
    }
}

/// A quick-find query with its operators taken out: `has:file`, `has:link`
/// and `from:name` (several `from:` mean any of them). What is left is the
/// text, matched as before.
public struct SearchQuery: Equatable, Sendable {
    public var text: String
    public var hasFile = false
    public var hasLink = false
    /// Lowercased, as typed after `from:`.
    public var from: [String] = []

    public init(_ raw: String) {
        var words: [String] = []
        for token in raw.split(whereSeparator: \.isWhitespace).map(String.init) {
            let low = token.lowercased()
            if low == "has:file" || low == "has:files" {
                hasFile = true
            } else if low == "has:link" || low == "has:links" {
                hasLink = true
            } else if low.hasPrefix("from:"), low.count > 5 {
                from.append(String(low.dropFirst(5)))
            } else {
                words.append(token)
            }
        }
        text = words.joined(separator: " ")
    }

    /// Any operator was typed: only messages are searched, and the text may be empty.
    public var hasOperators: Bool { hasFile || hasLink || !from.isEmpty }

    /// Whether a message (or a thread reply) matches the operators and the text.
    public func matches(_ m: MessageView, members: [MemberView], me: UInt64) -> Bool {
        if hasFile && !m.hasFiles { return false }
        if hasLink && !m.hasLink { return false }
        if !from.isEmpty && !from.contains(where: { Self.isFrom(m, $0, members: members, me: me) }) { return false }
        return text.isEmpty || m.text.localizedCaseInsensitiveContains(text)
    }

    /// `from:who` (lowercased): `me`, or the start of a name or of any word
    /// in it; a bot's message goes by the bot's name.
    static func isFrom(_ m: MessageView, _ who: String, members: [MemberView], me: UInt64) -> Bool {
        if let bot = m.bot { return nameStarts(bot, who) }
        if who == "me" && m.author == me { return true }
        guard let p = members.first(where: { $0.id == m.author }) else { return false }
        return nameStarts(p.fullName, who) || nameStarts(p.displayName, who)
    }

    /// The name without spaces starts with `who`, or one of its words does.
    static func nameStarts(_ name: String, _ who: String) -> Bool {
        let low = name.lowercased()
        guard !low.isBlank else { return false }
        if low.filter({ !$0.isWhitespace }).hasPrefix(who) { return true }
        return low.split(whereSeparator: \.isWhitespace).contains { $0.hasPrefix(who) }
    }
}
