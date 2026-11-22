// What the message box does with what is typed, apart from drawing it:
// "/" commands, sending to a channel (workflows and link previews too), and
// the unsent drafts kept on this iPhone.

import SwiftUI
import WorkboardKit

/// What a "/" message came to (SlashCommands, as on the Mac).
enum SlashOutcome { case notCommand, done, keep, post(String) }

/// Run a "/" message written in `channel` (thread `parent`): the app's own
/// commands here, the server's sent to it; an unknown one stays in the box.
@MainActor
func runSlash(_ client: SyncClient, notifier: Notifier, text: String, channel: UInt64, parent: UInt64?, hasFiles: Bool) -> SlashOutcome {
    guard text.hasPrefix("/"), !hasFiles else { return .notCommand }
    switch SlashCommands.parse(text.trimmed, commands: client.commands) {
    case .plain: return .notCommand
    case .shrug(let t), .me(let t): return .post(t)
    case .topic(let t):
        client.edit { d, _ in Edits.setChannelTopic(&d, channel: channel, to: t) }
        return .done
    case .mute:
        notifier.prefs.setChannelMode(channel, "nothing")
        return .done
    case .unmute:
        notifier.prefs.setChannelMode(channel, notifier.prefs.channelDefault == "nothing" ? "mentions" : notifier.prefs.channelDefault)
        return .done
    case let .remind(what, secs):
        let id = client.newId(), me = client.me ?? 0, at = nowTs() + secs
        client.editLocal { d in Edits.addReminder(&d, text: what, dueTs: at, me: me, id: id) }
        ReminderAlerts.schedule(id: id, text: what, at: at)
        return .done
    case .remindHelp:
        client.showNotice(String(localized: "Try: /remind call Sam in 30 minutes"))
        return .keep
    case let .server(name, rest, known):
        guard known else {
            client.showNotice(String(localized: "There's no /\(name) command."))
            return .keep
        }
        return client.runCommand(name, text: rest, channel: channel, parent: parent) ? .done : .keep
    }
}

/// Send `text` (and `files`) to `channel` as its message box does: a "/"
/// command runs instead, workflows the message starts run, and its first
/// web address gets a preview. True when the box can be cleared. The quick
/// New message sheet sends through here too, so it works the same.
@MainActor
func sendChannelMessage(_ client: SyncClient, notifier: Notifier, channel: UInt64, text: String, files: [JSON] = [], picked: [UInt64] = []) -> Bool {
    var text = text
    switch runSlash(client, notifier: notifier, text: text, channel: channel, parent: nil, hasFiles: !files.isEmpty) {
    case .notCommand: break
    case .done: return true
    case .keep: return false
    case .post(let t): text = t
    }
    let id = client.newId()
    let sent = client.edit { d, me in
        Edits.sendMessage(&d, channel: channel, text: text, attachments: files, picked: picked, online: client.online, me: me, id: id, ts: nowTs())
        // workflows this message starts (ChatCommands.swift)
        Edits.runWorkflows(&d, channel: channel, message: id, me: me, ts: nowTs()) { client.newId() }
    }
    if sent { fetchPreview(client, channel: channel, message: id, text: text) }
    return sent
}

/// Fetch the preview of the first web address of a message I just sent or
/// changed, and put it on the message (as the Mac app does after sending).
@MainActor
func fetchPreview(_ client: SyncClient, channel: UInt64, message: UInt64, text: String) {
    guard let url = ChatFormat.firstURL(text) else { return }
    Task {
        guard let p = await LinkPreviews.fetch(url) else { return }
        _ = client.edit { d, me in Edits.setPreview(&d, channel: channel, message: message, preview: p, me: me) }
    }
}

/// Unsent messages, kept on this iPhone per server, conversation and thread
/// ("channel|thread", thread 0 for the conversation itself), like the Mac's
/// Drafts.
enum ChatDrafts {
    static func prefix(_ server: String) -> String { "chatDraft|\(server)|" }

    static func save(_ text: String, key: String, server: String) {
        let k = prefix(server) + key
        if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            UserDefaults.standard.removeObject(forKey: k)
        } else {
            UserDefaults.standard.set(text, forKey: k)
        }
    }

    static func load(key: String, server: String) -> String {
        UserDefaults.standard.string(forKey: prefix(server) + key) ?? ""
    }

    /// Forget every draft on every server (starting from nothing).
    static func removeAll() {
        for k in UserDefaults.standard.dictionaryRepresentation().keys where k.hasPrefix("chatDraft|") {
            UserDefaults.standard.removeObject(forKey: k)
        }
    }

    /// Every draft on `server`: (channel, thread or nil, text).
    static func all(server: String) -> [(UInt64, UInt64?, String)] {
        let p = prefix(server)
        return UserDefaults.standard.dictionaryRepresentation().compactMap { k, v in
            guard k.hasPrefix(p), let text = v as? String else { return nil }
            let parts = k.dropFirst(p.count).split(separator: "|")
            guard parts.count == 2, let ch = UInt64(parts[0]), let t = UInt64(parts[1]) else { return nil }
            return (ch, t == 0 ? nil : t, text)
        }.sorted { ($0.0, $0.1 ?? 0) < ($1.0, $1.1 ?? 0) }
    }
}
