// Edits on the raw workspace tree. Each one changes only the fields it means
// to change, so unknown fields stay as they were. New items are written with
// every field the Mac app's structs require (src/model.rs), because the Mac
// app drops a merged copy it cannot read.

import Foundation

extension JSON {
    /// Finds the item with this id in the array at `path` and changes it.
    /// Returns false when there is no such item.
    @discardableResult
    mutating func updateItem(at path: String, id: UInt64, _ change: (inout JSON) -> Void) -> Bool {
        guard var arr = at(path)?.array, let i = arr.firstIndex(where: { $0["id"]?.uint == id }) else { return false }
        change(&arr[i])
        set(path, .array(arr))
        return true
    }

    /// Like `updateItem`, but `change` may refuse: when it returns false the
    /// array is left as it was and this returns false too.
    @discardableResult
    mutating func tryUpdateItem(at path: String, id: UInt64, _ change: (inout JSON) -> Bool) -> Bool {
        guard var arr = at(path)?.array, let i = arr.firstIndex(where: { $0["id"]?.uint == id }), change(&arr[i]) else { return false }
        set(path, .array(arr))
        return true
    }

    mutating func append(at path: String, _ item: JSON) {
        var arr = at(path)?.array ?? []
        arr.append(item)
        set(path, .array(arr))
    }
}

public enum Edits {
    static func message(id: UInt64, author: UInt64, text: String, ts: Int64) -> JSON {
        [
            "id": .id(id), "author": .id(author), "text": .string(text), "ts": .int(ts),
            "reactions": [], "replies": [], "saved": false, "task": nil,
            "post_title": nil, "post_type": "", "pinned": false, "call": nil,
        ]
    }

    // MARK: chat

    /// `attachments` are Attachment objects ({file, name, kind, bytes, w, h,
    /// secs}); a message may be only pictures, with no text. `picked`: the
    /// people chosen from the @ list, so a name several people share
    /// mentions the one picked (kept in `mentions`, as the Mac app does).
    @discardableResult
    public static func sendMessage(_ data: inout JSON, channel: UInt64, text: String, attachments: [JSON] = [], picked: [UInt64] = [], me: UInt64, id: UInt64, ts: Int64) -> Bool {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty || !attachments.isEmpty else { return false }
        let mentions = Mentions.ids(in: text, data: data, picked: picked)
        return data.updateItem(at: "channels", id: channel) { c in
            var msgs = c["messages"]?.array ?? []
            var m = message(id: id, author: me, text: text, ts: ts)
            if !attachments.isEmpty { m["attachments"] = .array(attachments) }
            Mentions.put(mentions, on: &m)
            msgs.append(m)
            c["messages"] = .array(msgs)
            c["last_read"] = .int(ts)
        }
    }

    @discardableResult
    public static func sendReply(_ data: inout JSON, channel: UInt64, to parent: UInt64, text: String, attachments: [JSON] = [], picked: [UInt64] = [], me: UInt64, id: UInt64, ts: Int64) -> Bool {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty || !attachments.isEmpty else { return false }
        let mentions = Mentions.ids(in: text, data: data, picked: picked)
        var done = false
        data.updateItem(at: "channels", id: channel) { c in
            done = c.updateItem(at: "messages", id: parent) { m in
                var reps = m["replies"]?.array ?? []
                var r = message(id: id, author: me, text: text, ts: ts)
                if !attachments.isEmpty { r["attachments"] = .array(attachments) }
                Mentions.put(mentions, on: &r)
                reps.append(r)
                m["replies"] = .array(reps)
            }
        }
        return done
    }

    /// Like start_call in src/app/together.rs: a "Started a call" message
    /// carrying the link others join from.
    @discardableResult
    public static func postCall(_ data: inout JSON, channel: UInt64, link: String, me: UInt64, id: UInt64, ts: Int64) -> Bool {
        data.updateItem(at: "channels", id: channel) { c in
            var msgs = c["messages"]?.array ?? []
            var m = message(id: id, author: me, text: "Started a call", ts: ts)
            m["call"] = .string(link)
            msgs.append(m)
            c["messages"] = .array(msgs)
            c["last_read"] = .int(ts)
        }
    }

    /// Like post_huddle_card in src/app/huddle.rs: "Started a huddle" with
    /// the conversation's huddle, so others can join from the message.
    @discardableResult
    public static func postHuddle(_ data: inout JSON, channel: UInt64, me: UInt64, id: UInt64, ts: Int64) -> Bool {
        data.updateItem(at: "channels", id: channel) { c in
            var msgs = c["messages"]?.array ?? []
            var m = message(id: id, author: me, text: "Started a huddle", ts: ts)
            m["call"] = .string(CallAPI.huddlePrefix + String(channel))
            msgs.append(m)
            c["messages"] = .array(msgs)
            c["last_read"] = .int(ts)
        }
    }

    /// Read position is personal (a LOCAL_KEY), so this never reaches others.
    public static func markRead(_ data: inout JSON, channel: UInt64, ts: Int64) {
        data.updateItem(at: "channels", id: channel) { c in
            if (c["last_read"]?.int ?? 0) < ts { c["last_read"] = .int(ts) }
        }
    }

    // MARK: tasks

    static func log(_ t: inout JSON, me: UInt64, text: String, ts: Int64) {
        var act = t["activity"]?.array ?? []
        act.append(["ts": .int(ts), "author": .id(me), "text": .string(text), "comment": false, "post": false])
        t["activity"] = .array(act)
    }

    /// Like Action::Replace with a new status: logs the change, keeps Date
    /// done / Date closed in step, runs the status automations (assign,
    /// priority, move, comment, as in src/app.rs) and rolls a repeating task
    /// on when it closes (roll_recurring in src/app/parity.rs).
    @discardableResult
    public static func setStatus(_ data: inout JSON, task: UInt64, to status: String, me: UInt64, ts: Int64, today: Date = Date()) -> Bool {
        let view = WorkspaceView(data)
        guard let t = view.tasks.first(where: { $0.id == task }), t.status != status else { return false }
        let sts = view.statuses(forList: t.listId)
        let was = view.status(of: t).kind
        let now = sts.first { $0.name == status }?.kind ?? sts[0].kind
        let isDone = { (k: StatusKind) in k == .done || k == .closed }
        let space = view.space(ofList: t.listId)?.id
        let rules = (data.at("extra.automations")?.array ?? []).filter { a in
            guard a["enabled"]?.bool == true, a["trigger_status"]?.string == status else { return false }
            guard let l = a["list"]?.uint else { return a["list"] == nil || a["list"] == .null }
            return l == t.listId || l == space
        }
        // Apps > Multiple Assignees (on unless turned off)
        let multiple = data.at("extra.apps.multiple_assignees")?.bool ?? true
        let members = Mentions.members(of: data)
        // a status name that closes, by the list's statuses (like `closes` in roll_recurring)
        let closes = { (name: String) in sts.contains { $0.name == name && isDone($0.kind) } }
        let lists = rawLists(data)
        var moveTo: UInt64?
        let changed = data.updateItem(at: "tasks", id: task) { j in
            log(&j, me: me, text: "changed status from \(t.status.uppercased()) to \(status.uppercased())", ts: ts)
            j["status"] = .string(status)
            if isDone(now) && !isDone(was) {
                j["done_at"] = .int(ts)
            } else if !isDone(now) {
                j["done_at"] = .null
            }
            if now == .closed && was != .closed {
                j["closed_at"] = .int(ts)
            } else if now != .closed {
                j["closed_at"] = .null
            }
            for a in rules {
                let value = a["value"]?.string ?? ""
                let what: String
                switch a["action"]?.string ?? "" {
                case "assign":
                    if let uid = UInt64(value) {
                        var asg = (j["assignees"]?.array ?? []).compactMap(\.uint)
                        if !multiple {
                            asg = [uid]
                        } else if !asg.contains(uid) {
                            asg.append(uid)
                            var act = j["activity"]?.array ?? []
                            act.append(["ts": .int(ts), "author": .id(me), "text": .string("assigned to \(view.member(uid)?.name ?? "")"),
                                        "comment": false, "post": false, "target": .id(uid)])
                            j["activity"] = .array(act)
                        }
                        j["assignees"] = .array(asg.map(JSON.id))
                    }
                    what = "assign to \(UInt64(value).flatMap { view.member($0)?.name } ?? "")"
                case "priority":
                    if let p = UInt8(value) { j["priority"] = .int(Int64(min(p, 4))) }
                    what = "set priority to \(Int(value).flatMap { $0 >= 0 && $0 < 5 ? priorityLabels[$0] : nil } ?? "")"
                case "move":
                    // moved below, with subtasks and statuses, as Move does
                    if let l = UInt64(value), lists[l] != nil { moveTo = l }
                    what = "move to \(UInt64(value).flatMap { lists[$0]?.name } ?? "List")"
                default:
                    var act = j["activity"]?.array ?? []
                    var line: JSON = ["ts": .int(ts), "author": .id(me), "text": .string(value), "comment": true, "post": false]
                    Mentions.put(Mentions.ids(in: value, members: members), on: &line)
                    act.append(line)
                    j["activity"] = .array(act)
                    what = "comment \"\(value)\""
                }
                log(&j, me: me, text: "Automation ran: \(what)", ts: ts)
            }
            rollRecurring(&j, wasClosing: closes(t.status), closes: closes, first: sts[0].name, me: me, ts: ts, today: today)
            j["updated"] = .int(ts)
        }
        if changed, let l = moveTo { moveTasks(&data, ids: [task], to: l, me: me, ts: ts) }
        return changed
    }

    /// Every list in the workspace (archived ones and those in archived
    /// spaces too, like space_of_list in src/model.rs): its name and the
    /// statuses of its space.
    static func rawLists(_ data: JSON) -> [UInt64: (name: String, statuses: [StatusView])] {
        var out: [UInt64: (name: String, statuses: [StatusView])] = [:]
        for s in data["spaces"]?.array ?? [] {
            let st = (s["statuses"]?.array ?? []).map(StatusView.init)
            let statuses = st.isEmpty ? StatusView.defaults : st
            let all = (s["lists"]?.array ?? []) + (s["folders"]?.array ?? []).flatMap { $0["lists"]?.array ?? [] }
            for l in all {
                guard let id = l["id"]?.uint, out[id] == nil else { continue }
                out[id] = (l["name"]?.string ?? "", statuses)
            }
        }
        return out
    }

    /// move_tasks in src/app/parity.rs: tasks (and their subtasks) go to
    /// another list; statuses that list doesn't have fall back to its first
    /// status. A moved subtask whose parent stays becomes a task of its own.
    @discardableResult
    public static func moveTasks(_ data: inout JSON, ids: [UInt64], to list: UInt64, me: UInt64, ts: Int64) -> Bool {
        guard let target = rawLists(data)[list] else { return false }
        var tasks = data["tasks"]?.array ?? []
        var all = ids
        var i = 0
        while i < all.count {
            let k = all[i]
            all += tasks.filter { $0["parent"]?.uint == k && !all.contains($0["id"]?.uint ?? 0) }.compactMap { $0["id"]?.uint }
            i += 1
        }
        var moved = false
        for ti in tasks.indices {
            guard let id = tasks[ti]["id"]?.uint, all.contains(id), tasks[ti]["list_id"]?.uint != list else { continue }
            var t = tasks[ti]
            t["list_id"] = .id(list)
            if !target.statuses.contains(where: { $0.name == t["status"]?.string }) {
                t["status"] = .string(target.statuses[0].name)
            }
            if ids.contains(id) {
                log(&t, me: me, text: "moved this task to \(target.name)", ts: ts)
                if let p = t["parent"]?.uint, !all.contains(p) { t["parent"] = .null }
            }
            t["updated"] = .int(ts)
            tasks[ti] = t
            moved = true
        }
        if moved { data["tasks"] = .array(tasks) }
        return moved
    }

    /// roll_recurring in src/app/parity.rs: a repeating task that just closed
    /// opens again with its dates moved on.
    static func rollRecurring(_ j: inout JSON, wasClosing: Bool, closes: (String) -> Bool, first: String, me: UInt64, ts: Int64, today: Date) {
        guard let r = j["recur"]?.string, !wasClosing, closes(j["status"]?.string ?? "") else { return }
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = DayFormat.timeZone
        guard let base = j["due"]?.string.flatMap(DayFormat.date(fromDay:)) ?? DayFormat.date(fromDay: DayFormat.day(today)) else { return }
        let next: Date
        switch r {
        case "daily": next = cal.date(byAdding: .day, value: 1, to: base) ?? base
        case "weekly": next = cal.date(byAdding: .day, value: 7, to: base) ?? base
        case "monthly": next = cal.date(byAdding: .month, value: 1, to: base) ?? base
        case "yearly": next = cal.date(byAdding: .month, value: 12, to: base) ?? base
        default: next = base
        }
        let shift = cal.dateComponents([.day], from: base, to: next).day ?? 0
        let nextDay = DayFormat.day(next)
        j["due"] = .string(nextDay)
        if let s = j["start"]?.string.flatMap(DayFormat.date(fromDay:)), let moved = cal.date(byAdding: .day, value: shift, to: s) {
            j["start"] = .string(DayFormat.day(moved))
        }
        j["status"] = .string(first)
        // it's open again: no Date done / Date closed until it next closes
        j["done_at"] = .null
        j["closed_at"] = .null
        log(&j, me: me, text: "completed a \(r) repeat; next due \(macWhen(nextDay, time: j["due_time"]?.string, today: today))", ts: ts)
        if var items = j["checklist"]?.array {
            for i in items.indices where items[i].object != nil { items[i]["done"] = false }
            j["checklist"] = .array(items)
        }
    }

    /// Like Action::DeleteTask: the task and its subtasks go. Others see it
    /// gone too (the sync tombstones them).
    @discardableResult
    public static func deleteTask(_ data: inout JSON, task: UInt64) -> Bool {
        var tasks = data["tasks"]?.array ?? []
        var kill: [UInt64] = [task]
        var i = 0
        while i < kill.count {
            let k = kill[i]
            kill += tasks.filter { $0["parent"]?.uint == k }.compactMap { $0["id"]?.uint }
            i += 1
        }
        let before = tasks.count
        tasks.removeAll { kill.contains($0["id"]?.uint ?? 0) }
        guard tasks.count != before else { return false }
        data["tasks"] = .array(tasks)
        return true
    }

    /// Moves a task to its list's first closed status (or back to the first status).
    @discardableResult
    public static func setDone(_ data: inout JSON, task: UInt64, done: Bool, me: UInt64, ts: Int64, today: Date = Date()) -> Bool {
        let view = WorkspaceView(data)
        guard let t = view.tasks.first(where: { $0.id == task }) else { return false }
        let sts = view.statuses(forList: t.listId)
        let target = done ? (sts.first { $0.kind == .closed } ?? sts.first { $0.isClosed } ?? sts[sts.count - 1]) : sts[0]
        return setStatus(&data, task: task, to: target.name, me: me, ts: ts, today: today)
    }

    /// Logged like the Mac app's typed fields (my line from the last five
    /// minutes is updated instead of adding another). `me` nil: not logged.
    @discardableResult
    public static func rename(_ data: inout JSON, task: UInt64, to name: String, me: UInt64? = nil, ts: Int64) -> Bool {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { return false }
        return data.updateItem(at: "tasks", id: task) { j in
            guard j["name"]?.string != name else { return }
            j["name"] = .string(name)
            if let me { logEdit(&j, me: me, like: "renamed this task to ", text: "renamed this task to \"\(name)\"", ts: ts) }
            j["updated"] = .int(ts)
        }
    }

    @discardableResult
    public static func setDescription(_ data: inout JSON, task: UInt64, to text: String, me: UInt64? = nil, ts: Int64) -> Bool {
        data.updateItem(at: "tasks", id: task) { j in
            guard j["description"]?.string != text else { return }
            j["description"] = .string(text)
            if let me { logEdit(&j, me: me, like: "updated the description", text: "updated the description", ts: ts) }
            j["updated"] = .int(ts)
        }
    }

    /// Like Action::Add: the list's first status, "created this task" logged,
    /// and "assigned to <name>" (with the person's id in "target", which the
    /// Mac app's Inbox goes by) for everyone else put on it. With `parent` it
    /// is a subtask, in its parent's list (the Mac app's "Add subtask").
    /// `status`: added from a Board column, the task starts in it (a name
    /// the list doesn't have falls back to the first status).
    public static func addTask(_ data: inout JSON, list: UInt64, name: String, assignees: [UInt64], parent: UInt64? = nil, status: String? = nil, due: String? = nil, me: UInt64, id: UInt64, ts: Int64) {
        let view = WorkspaceView(data)
        var activity: [JSON] = [["ts": .int(ts), "author": .id(me), "text": "created this task", "comment": false, "post": false]]
        for who in assignees where who != me {
            let name = view.member(who)?.name ?? ""
            activity.append(["ts": .int(ts), "author": .id(me), "text": .string("assigned to \(name)"), "comment": false, "post": false, "target": .id(who)])
        }
        let sts = view.statuses(forList: list)
        let status = sts.first { $0.name == status }?.name ?? sts[0].name
        let order = ((data["tasks"]?.array ?? []).compactMap { $0["order"]?.int }.max() ?? 0) + 1
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let task: JSON = [
            "id": .id(id), "list_id": .id(list), "parent": parent.map(JSON.id) ?? .null,
            "name": .string(clean.isEmpty ? "Untitled task" : clean),
            "description": "", "status": .string(status), "priority": 0,
            "assignees": .array(assignees.map(JSON.id)),
            "start": nil, "due": due.map(JSON.string) ?? .null, "tags": [], "checklist": [],
            "activity": .array(activity),
            "time_entries": [], "timer_started": nil, "estimate_min": nil,
            "created": .int(ts), "updated": .int(ts), "order": .int(order),
            "custom": [:], "recur": nil, "archived": false, "done_at": nil, "closed_at": nil,
        ]
        data.append(at: "tasks", task)
    }

    /// A comment on a task (activity with comment = true; activity only grows,
    /// so comments from several devices are all kept by the merge).
    @discardableResult
    public static func addComment(_ data: inout JSON, task: UInt64, text: String, picked: [UInt64] = [], me: UInt64, ts: Int64) -> Bool {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return false }
        let mentions = Mentions.ids(in: text, data: data, picked: picked)
        return data.updateItem(at: "tasks", id: task) { j in
            var act = j["activity"]?.array ?? []
            var line: JSON = ["ts": .int(ts), "author": .id(me), "text": .string(text), "comment": true, "post": false]
            Mentions.put(mentions, on: &line)
            act.append(line)
            j["activity"] = .array(act)
            j["updated"] = .int(ts)
        }
    }

    /// PRIORITIES labels in src/model.rs (0 = none ... 4 = low).
    public static let priorityLabels = ["No Priority", "Urgent", "High", "Normal", "Low"]

    @discardableResult
    public static func setPriority(_ data: inout JSON, task: UInt64, to p: Int, me: UInt64, ts: Int64) -> Bool {
        let p = max(0, min(4, p))
        return data.updateItem(at: "tasks", id: task) { j in
            guard (j["priority"]?.int ?? 0) != Int64(p) else { return }
            j["priority"] = .int(Int64(p))
            log(&j, me: me, text: "set priority to \(priorityLabels[p])", ts: ts)
            j["updated"] = .int(ts)
        }
    }

    /// Due date as "YYYY-MM-DD" or nil; logged like the Mac app. The due
    /// time (`due_time`, set on the Mac) stays when the day moves, and goes
    /// with the date when it is cleared, like Task::tidy_times: a removed time
    /// is written as null, never left out, so the merge doesn't bring it back.
    @discardableResult
    public static func setDue(_ data: inout JSON, task: UInt64, to day: String?, me: UInt64, ts: Int64, today: Date = Date()) -> Bool {
        data.updateItem(at: "tasks", id: task) { j in
            let before = j["due"]?.string
            guard before != day else { return }
            j["due"] = day.map(JSON.string) ?? .null
            if day == nil || before == nil, let t = j["due_time"], !t.isNull {
                j["due_time"] = .null
            }
            log(&j, me: me, text: "changed due date to \(day.map { macWhen($0, time: j["due_time"]?.string, today: today) } ?? "none")", ts: ts)
            j["updated"] = .int(ts)
        }
    }

    /// A time of day as the Mac app writes it in history: "14:30" -> "2:30 PM".
    public static func macTime(_ hhmm: String) -> String {
        let p = hhmm.split(separator: ":").compactMap { Int($0) }
        guard p.count >= 2 else { return hhmm }
        let h = p[0] % 12 == 0 ? 12 : p[0] % 12
        return String(format: "%d:%02d %@", h, p[1], p[0] < 12 ? "AM" : "PM")
    }

    /// A date with its time like the Mac app's history: "Tomorrow at 2:30 PM".
    static func macWhen(_ day: String, time: String?, today: Date) -> String {
        let d = macDate(day, today: today)
        guard let t = time.flatMap(TaskView.hhmm) else { return d }
        return "\(d) at \(macTime(t))"
    }

    /// Adds or removes one assignee, logged like the Mac app.
    @discardableResult
    public static func toggleAssignee(_ data: inout JSON, task: UInt64, member: UInt64, me: UInt64, ts: Int64) -> Bool {
        let name = WorkspaceView(data).member(member)?.name ?? ""
        return data.updateItem(at: "tasks", id: task) { j in
            var a = (j["assignees"]?.array ?? []).compactMap(\.uint)
            if let i = a.firstIndex(of: member) {
                a.remove(at: i)
                log(&j, me: me, text: "removed assignee \(name)", ts: ts)
            } else {
                a.append(member)
                log(&j, me: me, text: "assigned to \(name)", ts: ts)
            }
            j["assignees"] = .array(a.map(JSON.id))
            j["updated"] = .int(ts)
        }
    }

    /// fmt_date in src/widgets.rs: Today / Tomorrow / Yesterday / weekday / "Mar 3" / "3/3/27".
    public static func macDate(_ day: String, today: Date = Date()) -> String {
        guard let d = DayFormat.date(fromDay: day), let t = DayFormat.date(fromDay: DayFormat.day(today)) else { return day }
        let diff = Int((d.timeIntervalSince(t) / 86400).rounded())
        let out: DateFormatter
        switch diff {
        case 0: return "Today"
        case 1: return "Tomorrow"
        case -1: return "Yesterday"
        case 2...6: out = macWeekday
        default:
            let cal = Calendar(identifier: .gregorian)
            out = cal.component(.year, from: d) == cal.component(.year, from: t) ? macMonthDay : macShortDate
        }
        return out.string(from: d)
    }

    private static let macWeekday = DayFormat.posix("EEE")
    private static let macMonthDay = DayFormat.posix("MMM d")
    private static let macShortDate = DayFormat.posix("M/d/yy")

    // MARK: reactions

    /// Like ChatAction::React in src/app/chat.rs: my reaction on or off.
    @discardableResult
    public static func toggleReaction(_ data: inout JSON, channel: UInt64, message: UInt64, emoji: String, me: UInt64) -> Bool {
        func flip(_ m: inout JSON) {
            var rs = m["reactions"]?.array ?? []
            if let i = rs.firstIndex(where: { $0["emoji"]?.string == emoji }) {
                var users = (rs[i]["users"]?.array ?? []).compactMap(\.uint)
                if let j = users.firstIndex(of: me) { users.remove(at: j) } else { users.append(me) }
                if users.isEmpty { rs.remove(at: i) } else { rs[i]["users"] = .array(users.map(JSON.id)) }
            } else {
                rs.append(["emoji": .string(emoji), "users": [.id(me)]])
            }
            m["reactions"] = .array(rs)
        }
        var done = false
        data.updateItem(at: "channels", id: channel) { c in
            if c.updateItem(at: "messages", id: message, flip) { done = true; return }
            // a thread reply
            var msgs = c["messages"]?.array ?? []
            for i in msgs.indices where !done {
                if msgs[i].updateItem(at: "replies", id: message, flip) { done = true }
            }
            if done { c["messages"] = .array(msgs) }
        }
        return done
    }

    // MARK: message actions (src/app/chat.rs)

    /// Changes one of my messages or replies (ChatAction::SaveEdit), with
    /// its `mentions` worked out again from the new text.
    @discardableResult
    public static func editMessage(_ data: inout JSON, channel: UInt64, message: UInt64, text: String, me: UInt64, ts: Int64) -> Bool {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return false }
        let members = Mentions.members(of: data)
        var done = false
        func change(_ m: inout JSON) {
            // a huddle or call card isn't edited (the Mac app offers no Edit for it)
            guard m["author"]?.uint == me, m["text"]?.string != text, m["call"]?.string == nil else { return }
            let picked = (m["mentions"]?.array ?? []).compactMap(\.uint)
            Mentions.put(Mentions.ids(in: text, members: members, picked: picked), on: &m)
            m["text"] = .string(text)
            m["edited"] = .int(ts)
            done = true
        }
        data.updateItem(at: "channels", id: channel) { c in
            if c.updateItem(at: "messages", id: message, change) { return }
            var msgs = c["messages"]?.array ?? []
            for i in msgs.indices { if msgs[i].updateItem(at: "replies", id: message, change) { break } }
            c["messages"] = .array(msgs)
        }
        return done
    }

    /// Deletes one of my messages (with its thread) or replies (ChatAction::Delete).
    @discardableResult
    public static func deleteMessage(_ data: inout JSON, channel: UInt64, message: UInt64, me: UInt64) -> Bool {
        var done = false
        data.updateItem(at: "channels", id: channel) { c in
            var msgs = c["messages"]?.array ?? []
            if let i = msgs.firstIndex(where: { $0["id"]?.uint == message && $0["author"]?.uint == me }) {
                msgs.remove(at: i)
                done = true
            } else {
                for i in msgs.indices {
                    var reps = msgs[i]["replies"]?.array ?? []
                    if let k = reps.firstIndex(where: { $0["id"]?.uint == message && $0["author"]?.uint == me }) {
                        reps.remove(at: k)
                        msgs[i]["replies"] = .array(reps)
                        done = true
                        break
                    }
                }
            }
            if done { c["messages"] = .array(msgs) }
        }
        return done
    }

    /// Pins or unpins a message (ChatAction::Pin; not thread replies).
    @discardableResult
    public static func togglePin(_ data: inout JSON, channel: UInt64, message: UInt64) -> Bool {
        var done = false
        data.updateItem(at: "channels", id: channel) { c in
            done = c.updateItem(at: "messages", id: message) { m in m["pinned"] = .bool(!(m["pinned"]?.bool ?? false)) }
        }
        return done
    }

    /// Like create_channel in src/app/parity.rs (without "Add a List"): a
    /// public channel has everyone in it, a private one only me.
    @discardableResult
    public static func createChannel(_ data: inout JSON, name: String, isPrivate: Bool, me: UInt64, id: UInt64, ts: Int64) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased().replacingOccurrences(of: " ", with: "-")
        guard !clean.isEmpty else { return false }
        let members: [UInt64] = isPrivate ? [me] : (data["members"]?.array ?? []).compactMap { $0["id"]?.uint }
        data.append(at: "channels", [
            "id": .id(id), "name": .string(clean), "kind": "Channel", "topic": "",
            "members": .array(members.map(JSON.id)), "messages": [], "last_read": .int(ts),
            "private": .bool(isPrivate), "list": nil, "creator": .id(me),
        ])
        return true
    }

    /// Like ChatAction::Dm in src/app/chat.rs: the direct message I already
    /// have with this person, or a new one with the two of us. Of several
    /// copies (two devices that started it while apart) it is the one the Mac
    /// app shows: one with messages, then the oldest. Returns its id, or nil
    /// for myself or someone not in the workspace.
    public static func openDM(_ data: inout JSON, with other: UInt64, me: UInt64, id: UInt64, ts: Int64) -> UInt64? {
        guard other != me, (data["members"]?.array ?? []).contains(where: { $0["id"]?.uint == other }) else { return nil }
        let people = [me, other].sorted()
        let existing = (data["channels"]?.array ?? [])
            .filter { c in
                c["kind"]?.string == "Direct"
                    && Array(Set((c["members"]?.array ?? []).compactMap(\.uint))).sorted() == people
            }
            .min { a, b in
                let ae = (a["messages"]?.array ?? []).isEmpty, be = (b["messages"]?.array ?? []).isEmpty
                return ae != be ? !ae : (a["id"]?.uint ?? 0) < (b["id"]?.uint ?? 0)
            }
        if let found = existing?["id"]?.uint { return found }
        // no "creator": the Mac app leaves it out for a direct message (0)
        data.append(at: "channels", [
            "id": .id(id), "name": "", "kind": "Direct", "topic": "",
            "members": .array([me, other].map(JSON.id)), "messages": [], "last_read": .int(ts),
            "private": false, "list": nil,
        ])
        return id
    }

    // MARK: checklist

    /// Changes a task's checklist (CheckItem {text, done, assignee?} in
    /// src/model.rs) and marks the task updated, as the Mac task window does.
    /// The Mac app logs no history line for checklist changes.
    private static func changeChecklist(_ data: inout JSON, task: UInt64, ts: Int64, _ change: (inout [JSON]) -> Bool) -> Bool {
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var items = j["checklist"]?.array ?? []
            guard change(&items) else { return }
            j["checklist"] = .array(items)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    /// "+ Add item": a new unticked item at the end.
    @discardableResult
    public static func addCheckItem(_ data: inout JSON, task: UInt64, text: String, ts: Int64) -> Bool {
        let clean = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return false }
        return changeChecklist(&data, task: task, ts: ts) { items in
            items.append(["text": .string(clean), "done": false])
            return true
        }
    }

    /// Ticks or unticks item `index`; other fields (assignee) stay.
    @discardableResult
    public static func setCheckItem(_ data: inout JSON, task: UInt64, index: Int, done: Bool, ts: Int64) -> Bool {
        changeChecklist(&data, task: task, ts: ts) { items in
            guard items.indices.contains(index), items[index]["done"]?.bool != done else { return false }
            items[index]["done"] = .bool(done)
            return true
        }
    }

    @discardableResult
    public static func removeCheckItem(_ data: inout JSON, task: UInt64, index: Int, ts: Int64) -> Bool {
        changeChecklist(&data, task: task, ts: ts) { items in
            guard items.indices.contains(index) else { return false }
            items.remove(at: index)
            return true
        }
    }

    // MARK: docs

    /// Like new_doc in src/app/menus.rs: a public doc at the top of the list.
    public static func addDoc(_ data: inout JSON, title: String, space: UInt64?, me: UInt64, id: UInt64, ts: Int64) {
        let t = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let doc: JSON = [
            "id": .id(id), "space": space.map(JSON.id) ?? .null, "owner": .id(me), "private": false,
            "archived": false, "meeting_note": false, "parent": nil,
            "title": .string(t.isEmpty ? "Untitled" : t), "body": "", "created": .int(ts), "updated": .int(ts),
        ]
        var docs = data["docs"]?.array ?? []
        docs.insert(doc, at: 0)
        data["docs"] = .array(docs)
    }

    @discardableResult
    public static func setDocBody(_ data: inout JSON, doc: UInt64, to body: String, ts: Int64) -> Bool {
        data.updateItem(at: "docs", id: doc) { j in
            guard j["body"]?.string != body else { return }
            j["body"] = .string(body)
            j["updated"] = .int(ts)
        }
    }

    @discardableResult
    public static func setDocTitle(_ data: inout JSON, doc: UInt64, to title: String, ts: Int64) -> Bool {
        data.updateItem(at: "docs", id: doc) { j in
            guard j["title"]?.string != title else { return }
            j["title"] = .string(title)
            j["updated"] = .int(ts)
        }
    }

    /// Like trash_doc in src/app/docs.rs: a page and its sub pages go.
    @discardableResult
    public static func deleteDoc(_ data: inout JSON, doc: UInt64) -> Bool {
        var docs = data["docs"]?.array ?? []
        var kill: [UInt64] = [doc]
        var i = 0
        while i < kill.count {
            let k = kill[i]
            kill += docs.filter { $0["parent"]?.uint == k }.compactMap { $0["id"]?.uint }
            i += 1
        }
        let before = docs.count
        docs.removeAll { kill.contains($0["id"]?.uint ?? 0) }
        guard docs.count != before else { return false }
        data["docs"] = .array(docs)
        return true
    }
}

// MARK: doc pages, privacy and archive (src/app/docs.rs)

extension Edits {
    /// add_page: a sub page "Page N" under `parent`, in its space and as
    /// private as it.
    @discardableResult
    public static func addPage(_ data: inout JSON, parent: UInt64, me: UInt64, id: UInt64, ts: Int64) -> Bool {
        let docs = data["docs"]?.array ?? []
        guard let p = docs.first(where: { $0["id"]?.uint == parent }) else { return false }
        let n = docs.filter { $0["parent"]?.uint == parent }.count + 1
        data.append(at: "docs", [
            "id": .id(id), "space": p["space"] ?? .null, "owner": .id(me), "private": p["private"] ?? false,
            "archived": false, "meeting_note": false, "parent": .id(parent),
            "title": .string("Page \(n)"), "body": "", "created": .int(ts), "updated": .int(ts),
        ])
        return true
    }

    /// Only its owner sees a private doc.
    @discardableResult
    public static func setDocPrivate(_ data: inout JSON, doc: UInt64, _ on: Bool, ts: Int64) -> Bool {
        data.updateItem(at: "docs", id: doc) { j in
            guard (j["private"]?.bool ?? false) != on else { return }
            j["private"] = .bool(on)
            j["updated"] = .int(ts)
        }
    }

    /// Archived docs leave the Docs list until unarchived (Docs > Archived).
    @discardableResult
    public static func setDocArchived(_ data: inout JSON, doc: UInt64, _ on: Bool, ts: Int64) -> Bool {
        data.updateItem(at: "docs", id: doc) { j in
            guard (j["archived"]?.bool ?? false) != on else { return }
            j["archived"] = .bool(on)
            j["updated"] = .int(ts)
        }
    }
}
