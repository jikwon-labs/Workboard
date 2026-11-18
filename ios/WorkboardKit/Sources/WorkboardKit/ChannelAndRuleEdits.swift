// Channel settings (name, topic, who is in it) and status automations, as
// the Mac app's Channel Settings and Automations dialogs change them
// (src/app/menus.rs).

import Foundation

extension Edits {
    // MARK: channel settings

    /// Whoever made the channel, an owner or admin (or, for a channel from
    /// before channels kept their maker, anyone in it) may change who else
    /// is in it; anyone may join or leave (may_manage_channel).
    public static func mayManageChannel(_ data: JSON, channel: UInt64, me: UInt64, role: String?) -> Bool {
        guard let c = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == channel }) else { return false }
        let admin = role.map { $0 == "owner" || $0 == "admin" || $0 == "Owner" || $0 == "Admin" } ?? false
        let creator = c["creator"]?.uint ?? 0
        let members = (c["members"]?.array ?? []).compactMap(\.uint)
        return admin || creator == me || (creator == 0 && members.contains(me))
    }

    /// Puts someone in the channel or takes them out.
    @discardableResult
    public static func setChannelMember(_ data: inout JSON, channel: UInt64, member: UInt64, _ on: Bool) -> Bool {
        var changed = false
        data.updateItem(at: "channels", id: channel) { c in
            var m = (c["members"]?.array ?? []).compactMap(\.uint)
            if on, !m.contains(member) { m.append(member) } else if !on, m.contains(member) { m.removeAll { $0 == member } } else { return }
            c["members"] = .array(m.map(JSON.id))
            changed = true
        }
        return changed
    }

    @discardableResult
    public static func setChannelTopic(_ data: inout JSON, channel: UInt64, to topic: String) -> Bool {
        data.updateItem(at: "channels", id: channel) { $0["topic"] = .string(topic.trimmingCharacters(in: .whitespacesAndNewlines)) }
    }

    /// Channel names are lower case with dashes, as create_channel writes them.
    @discardableResult
    public static func renameChannel(_ data: inout JSON, channel: UInt64, to name: String) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased().replacingOccurrences(of: " ", with: "-")
        guard !clean.isEmpty else { return false }
        return data.updateItem(at: "channels", id: channel) { c in
            guard c["kind"]?.string == "Channel" else { return }
            c["name"] = .string(clean)
        }
    }

    // MARK: automations (Automation in src/model.rs)

    /// "When status changes to X, then assign / priority / comment / move."
    public static let automationActions: [(value: String, label: String)] = [("assign", "Assign to"), ("priority", "Set priority"), ("comment", "Comment"), ("move", "Move to List")]

    @discardableResult
    public static func addAutomation(_ data: inout JSON, trigger: String, action: String, value: String, list: UInt64?, id: UInt64) -> Bool {
        let v = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trigger.isEmpty, !v.isEmpty, automationActions.contains(where: { $0.value == action }) else { return false }
        var rules = data.at("extra.automations")?.array ?? []
        rules.append(["id": .id(id), "enabled": true, "trigger_status": .string(trigger), "action": .string(action), "value": .string(v), "list": list.map(JSON.id) ?? .null])
        data.set("extra.automations", .array(rules))
        return true
    }

    @discardableResult
    public static func setAutomationEnabled(_ data: inout JSON, automation: UInt64, _ on: Bool) -> Bool {
        data.updateItem(at: "extra.automations", id: automation) { $0["enabled"] = .bool(on) }
    }

    @discardableResult
    public static func deleteAutomation(_ data: inout JSON, automation: UInt64) -> Bool {
        var rules = data.at("extra.automations")?.array ?? []
        let before = rules.count
        rules.removeAll { $0["id"]?.uint == automation }
        guard rules.count != before else { return false }
        data.set("extra.automations", .array(rules))
        return true
    }
}

// MARK: saving, posts, tasks and reminders from messages (src/app/chat.rs)

extension Edits {
    /// Changes a message or a thread reply wherever it is.
    static func changeMessage(_ data: inout JSON, channel: UInt64, message: UInt64, _ change: @escaping (inout JSON) -> Void) -> Bool {
        var done = false
        data.updateItem(at: "channels", id: channel) { c in
            if c.updateItem(at: "messages", id: message, change) { done = true; return }
            var msgs = c["messages"]?.array ?? []
            for i in msgs.indices where !done {
                if msgs[i].updateItem(at: "replies", id: message, change) { done = true }
            }
            if done { c["messages"] = .array(msgs) }
        }
        return done
    }

    /// Saved for later (`saved` is each person's own: change it with
    /// SyncClient.editLocal).
    @discardableResult
    public static func toggleSaved(_ data: inout JSON, channel: UInt64, message: UInt64) -> Bool {
        changeMessage(&data, channel: channel, message: message) { $0["saved"] = .bool(!($0["saved"]?.bool ?? false)) }
    }

    /// "Create task": the first line (80 characters at most) as its name, the
    /// whole message as its description, for me, linked from the message.
    @discardableResult
    public static func messageToTask(_ data: inout JSON, channel: UInt64, message: UInt64, list: UInt64, me: UInt64, id: UInt64, ts: Int64) -> Bool {
        guard let m = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == channel })?["messages"]?.array?.first(where: { $0["id"]?.uint == message }),
              rawLists(data)[list] != nil else { return false }
        let text = m["text"]?.string ?? ""
        let name = String((text.split(separator: "\n", omittingEmptySubsequences: false).first.map(String.init) ?? "").prefix(80))
        addTask(&data, list: list, name: name, assignees: [me], me: me, id: id, ts: ts)
        data.updateItem(at: "tasks", id: id) { $0["description"] = .string(text) }
        _ = changeMessage(&data, channel: channel, message: message) { $0["task"] = .id(id) }
        return true
    }

    /// A post (an announcement with a title) in a channel.
    @discardableResult
    public static func sendPost(_ data: inout JSON, channel: UInt64, title: String, text: String, picked: [UInt64] = [], me: UInt64, id: UInt64, ts: Int64) -> Bool {
        let t = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let body = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty || !body.isEmpty else { return false }
        let mentions = Mentions.ids(in: body, data: data, picked: picked)
        return data.updateItem(at: "channels", id: channel) { c in
            var m = message(id: id, author: me, text: body, ts: ts)
            m["post_title"] = .string(t)
            m["post_type"] = "Announcement"
            Mentions.put(mentions, on: &m)
            var msgs = c["messages"]?.array ?? []
            msgs.append(m)
            c["messages"] = .array(msgs)
            c["last_read"] = .int(ts)
        }
    }

    /// "Remind me" on a message: a reminder pointing at it.
    @discardableResult
    public static func remindAboutMessage(_ data: inout JSON, channel: UInt64, message: UInt64, at: Int64, me: UInt64, id: UInt64) -> Bool {
        guard let m = (data["channels"]?.array ?? []).first(where: { $0["id"]?.uint == channel })?["messages"]?.array?.first(where: { $0["id"]?.uint == message }) else { return false }
        let text = String((m["text"]?.string ?? "").prefix(80))
        guard addReminder(&data, text: text.isEmpty ? "Message" : text, dueTs: at, me: me, id: id) else { return false }
        var rs = data.at("extra.reminders")?.array ?? []
        if let i = rs.indices.last { rs[i]["message"] = [.id(channel), .id(message)] }
        data.set("extra.reminders", .array(rs))
        return true
    }
}

extension WorkspaceView {
    /// My saved messages, newest first.
    public func savedMessages(_ data: JSON, me: UInt64) -> [(channel: ChannelView, message: MessageView)] {
        let saved = Set((data["channels"]?.array ?? []).flatMap { c in
            ((c["messages"]?.array ?? []) + (c["messages"]?.array ?? []).flatMap { $0["replies"]?.array ?? [] })
                .filter { $0["saved"]?.bool == true }.compactMap { $0["id"]?.uint }
        })
        return visibleChannels(me: me).flatMap { c in (c.messages + c.messages.flatMap(\.replies)).filter { saved.contains($0.id) }.map { (c, $0) } }
            .sorted { $0.1.ts > $1.1.ts }
    }

    /// Every post in the channels I can see, newest first.
    public func posts(me: UInt64) -> [(channel: ChannelView, message: MessageView)] {
        visibleChannels(me: me).filter { $0.kind == .channel }.flatMap { c in c.messages.filter { $0.postTitle != nil }.map { (c, $0) } }
            .sorted { $0.1.ts > $1.1.ts }
    }
}
