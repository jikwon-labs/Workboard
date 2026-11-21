// The task window's other fields, as the Mac app changes them (src/app.rs
// Action::Replace, src/app/taskfields.rs, src/app/parity.rs, src/model.rs):
// start date and times of day, tags, watchers, relationships, the parent,
// repeat, the time estimate, timers and time entries, custom fields, the
// task type, checklist names and people, duplicating and archiving. History
// lines are written in English with the Mac app's words, since teammates on
// Macs read the same lines.

import Foundation

extension Edits {
    // MARK: dates and times

    /// Start date as "YYYY-MM-DD" or nil, logged like the Mac app; its time
    /// of day goes with it when it is cleared (Task::tidy_times).
    @discardableResult
    public static func setStart(_ data: inout JSON, task: UInt64, to day: String?, me: UInt64, ts: Int64, today: Date = Date()) -> Bool {
        data.updateItem(at: "tasks", id: task) { j in
            let before = j["start"]?.string
            guard before != day else { return }
            j["start"] = day.map(JSON.string) ?? .null
            if day == nil || before == nil, let t = j["start_time"], !t.isNull {
                j["start_time"] = .null
            }
            log(&j, me: me, text: "changed start date to \(day.map { macWhen($0, time: j["start_time"]?.string, today: today) } ?? "none")", ts: ts)
            j["updated"] = .int(ts)
        }
    }

    /// The time of day on the due date ("14:30", or nil to remove it). Only
    /// with a due date; a removed time is written as null, never left out.
    @discardableResult
    public static func setDueTime(_ data: inout JSON, task: UInt64, to hhmm: String?, me: UInt64, ts: Int64, today: Date = Date()) -> Bool {
        setTime(&data, task: task, dateKey: "due", timeKey: "due_time", what: "due date", to: hhmm, me: me, ts: ts, today: today)
    }

    @discardableResult
    public static func setStartTime(_ data: inout JSON, task: UInt64, to hhmm: String?, me: UInt64, ts: Int64, today: Date = Date()) -> Bool {
        setTime(&data, task: task, dateKey: "start", timeKey: "start_time", what: "start date", to: hhmm, me: me, ts: ts, today: today)
    }

    private static func setTime(_ data: inout JSON, task: UInt64, dateKey: String, timeKey: String, what: String, to hhmm: String?, me: UInt64, ts: Int64, today: Date) -> Bool {
        let clean = hhmm.flatMap(TaskView.hhmm)
        guard hhmm == nil || clean != nil else { return false }
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            guard let day = j[dateKey]?.string else { return }
            let before = j[timeKey]?.string.flatMap(TaskView.hhmm)
            guard before != clean else { return }
            if let clean {
                j[timeKey] = .string(clean)
            } else if j[timeKey] != nil {
                j[timeKey] = .null
            }
            log(&j, me: me, text: "changed \(what) to \(macWhen(day, time: clean, today: today))", ts: ts)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    // MARK: tags

    /// Tags are kept in lower case, as the Mac tag field writes them.
    @discardableResult
    public static func addTag(_ data: inout JSON, task: UInt64, tag: String, me: UInt64, ts: Int64) -> Bool {
        let tag = tag.trimmed.lowercased()
        guard !tag.isEmpty else { return false }
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var tags = (j["tags"]?.array ?? []).compactMap(\.string)
            guard !tags.contains(tag) else { return }
            tags.append(tag)
            j["tags"] = .array(tags.map(JSON.string))
            log(&j, me: me, text: "added tag \(tag)", ts: ts)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    @discardableResult
    public static func removeTag(_ data: inout JSON, task: UInt64, tag: String, me: UInt64, ts: Int64) -> Bool {
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var tags = (j["tags"]?.array ?? []).compactMap(\.string)
            guard let i = tags.firstIndex(of: tag) else { return }
            tags.remove(at: i)
            j["tags"] = .array(tags.map(JSON.string))
            log(&j, me: me, text: "removed tag \(tag)", ts: ts)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    // MARK: watchers

    /// Someone turns the task's updates on or off (Task::set_watching; no
    /// history line, as on the Mac). Assignees and the maker get them until
    /// they turn them off, which puts them in `unwatched`; anyone else is
    /// added to `watchers`. `unwatched` stays once written, even empty, so
    /// taking the last one off reaches the other copies.
    @discardableResult
    public static func toggleWatcher(_ data: inout JSON, task: UInt64, member: UInt64, ts: Int64) -> Bool {
        data.updateItem(at: "tasks", id: task) { j in
            let on = !Inbox.watches(j, me: member)
            let own = Inbox.isMine(j, me: member)
            var w = (j["watchers"]?.array ?? []).compactMap(\.uint).filter { $0 != member }
            var u = j["unwatched"]?.array.map { $0.compactMap(\.uint).filter { $0 != member } }
            if on && !own { w.append(member) }
            if !on && own { u = (u ?? []) + [member] }
            j["watchers"] = .array(w.map(JSON.id))
            if let u { j["unwatched"] = .array(u.map(JSON.id)) }
            j["updated"] = .int(ts)
        }
    }

    // MARK: relationships (TaskPick in src/app/taskfields.rs)

    public enum Relation: String, CaseIterable, Sendable {
        case waitingOn = "Waiting on", blocking = "Blocking", linked = "Linked"
    }

    /// Waiting on is stored on the waiting task, so Blocking is written on
    /// the other one; a link is stored on this one.
    @discardableResult
    public static func addRelation(_ data: inout JSON, task: UInt64, _ rel: Relation, other: UInt64, me: UInt64, ts: Int64) -> Bool {
        let names = taskNames(data)
        guard task != other, names[task] != nil, names[other] != nil else { return false }
        let (on, target, key, text): (UInt64, UInt64, String, String)
        switch rel {
        case .waitingOn: (on, target, key, text) = (task, other, "waiting_on", "is waiting on \(names[other] ?? "")")
        case .blocking: (on, target, key, text) = (other, task, "waiting_on", "is waiting on \(names[task] ?? "")")
        case .linked: (on, target, key, text) = (task, other, "linked", "linked \(names[other] ?? "")")
        }
        var changed = false
        data.updateItem(at: "tasks", id: on) { j in
            var ids = (j[key]?.array ?? []).compactMap(\.uint)
            if !ids.contains(target) { ids.append(target) }
            j[key] = .array(ids.map(JSON.id))
            log(&j, me: me, text: text, ts: ts)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    /// Takes away a relationship of either direction (no history line).
    @discardableResult
    public static func removeRelation(_ data: inout JSON, task: UInt64, _ rel: Relation, other: UInt64, ts: Int64) -> Bool {
        var tasks = data["tasks"]?.array ?? []
        var changed = false
        for i in tasks.indices {
            guard let id = tasks[i]["id"]?.uint else { continue }
            let (key, drop): (String, UInt64)
            switch rel {
            case .waitingOn where id == task: (key, drop) = ("waiting_on", other)
            case .blocking where id == other: (key, drop) = ("waiting_on", task)
            case .linked where id == task: (key, drop) = ("linked", other)
            case .linked where id == other: (key, drop) = ("linked", task)
            default: continue
            }
            let before = (tasks[i][key]?.array ?? []).compactMap(\.uint)
            let after = before.filter { $0 != drop }
            guard after.count != before.count else { continue }
            tasks[i][key] = .array(after.map(JSON.id))
            tasks[i]["updated"] = .int(ts)
            changed = true
        }
        if changed { data["tasks"] = .array(tasks) }
        return changed
    }

    /// The task becomes a subtask of `parent` (in its list), or with nil a
    /// task of its own again. A task can't go under itself or one of its
    /// own subtasks.
    @discardableResult
    public static func setParent(_ data: inout JSON, task: UInt64, to parent: UInt64?, me: UInt64, ts: Int64) -> Bool {
        let tasks = data["tasks"]?.array ?? []
        guard let parent else {
            return data.updateItem(at: "tasks", id: task) { j in
                guard let p = j["parent"], !p.isNull else { return }
                j["parent"] = .null
                log(&j, me: me, text: "made this a top-level task", ts: ts)
                j["updated"] = .int(ts)
            }
        }
        guard let target = tasks.first(where: { $0["id"]?.uint == parent }), !descendants(of: task, in: tasks).contains(parent) else { return false }
        let list = target["list_id"] ?? .null
        let name = target["name"]?.string ?? ""
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            guard j["parent"]?.uint != parent else { return }
            j["parent"] = .id(parent)
            j["list_id"] = list
            log(&j, me: me, text: "made this a subtask of \(name)", ts: ts)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    /// The task and every subtask under it.
    static func descendants(of task: UInt64, in tasks: [JSON]) -> [UInt64] {
        var all = [task]
        var i = 0
        while i < all.count {
            let k = all[i]
            all += tasks.filter { $0["parent"]?.uint == k }.compactMap { $0["id"]?.uint }
            i += 1
        }
        return all
    }

    static func taskNames(_ data: JSON) -> [UInt64: String] {
        var out: [UInt64: String] = [:]
        for t in data["tasks"]?.array ?? [] {
            if let id = t["id"]?.uint { out[id] = t["name"]?.string ?? "" }
        }
        return out
    }

    // MARK: repeat

    /// RECUR in src/widgets.rs.
    public static let repeats: [(value: String?, label: String)] = [(nil, "Never"), ("daily", "Daily"), ("weekly", "Weekly"), ("monthly", "Monthly"), ("yearly", "Yearly")]

    /// A repeating task needs a due date: without one it gets today's (the
    /// Repeat menu in src/app/parity.rs).
    @discardableResult
    public static func setRecur(_ data: inout JSON, task: UInt64, to rule: String?, me: UInt64, ts: Int64, today: Date = Date()) -> Bool {
        var changed = data.updateItem(at: "tasks", id: task) { j in
            guard j["recur"]?.string != rule else { return }
            j["recur"] = rule.map(JSON.string) ?? .null
            j["updated"] = .int(ts)
        }
        let hasDue = (data["tasks"]?.array ?? []).first { $0["id"]?.uint == task }?["due"]?.string != nil
        if changed, rule != nil, !hasDue {
            changed = setDue(&data, task: task, to: dayString(today), me: me, ts: ts, today: today)
        }
        return changed
    }

    // MARK: time estimate

    /// parse_estimate in src/app/taskfields.rs: minutes from "2h 30m",
    /// "1.5h", "45m", "1d" (8 hours), "2:30" or a bare number (hours).
    public static func parseEstimate(_ text: String) -> UInt32? {
        let s = text.trimmed.lowercased()
        guard !s.isEmpty else { return nil }
        if let colon = s.firstIndex(of: ":") {
            guard let h = UInt32(s[..<colon].trimmingCharacters(in: .whitespaces)),
                  let m = UInt32(s[s.index(after: colon)...].trimmingCharacters(in: .whitespaces)) else { return nil }
            return h * 60 + m
        }
        var total = 0.0
        var num = ""
        var units = 0
        let chars = Array(s)
        var i = 0
        while i < chars.count {
            let c = chars[i]
            if c.isASCII && (c.isNumber || c == ".") { num.append(c); i += 1; continue }
            if c.isWhitespace { i += 1; continue }
            guard c.isLetter else { return nil }
            var unit = ""
            while i < chars.count, chars[i].isLetter { unit.append(chars[i]); i += 1 }
            guard let v = Double(num) else { return nil }
            num = ""
            let mult: Double
            switch unit {
            case "h", "hr", "hrs", "hour", "hours", "\u{c2dc}\u{ac04}": mult = 60
            case "m", "min", "mins", "minute", "minutes", "\u{bd84}": mult = 1
            case "d", "day", "days": mult = 480
            case "w", "wk", "week", "weeks": mult = 2400
            default: return nil
            }
            total += v * mult
            units += 1
        }
        if !num.isEmpty {
            guard let v = Double(num) else { return nil }
            // "2h 30": a trailing bare number after a unit is minutes
            total += v * (units > 0 ? 1 : 60)
        }
        guard total >= 0, total < Double(UInt32.max) else { return nil }
        return UInt32(total.rounded())
    }

    /// fmt_estimate: "2h 30m", "2h", "45m".
    public static func fmtEstimate(_ min: UInt32) -> String {
        switch (min / 60, min % 60) {
        case (0, let m): return "\(m)m"
        case (let h, 0): return "\(h)h"
        case (let h, let m): return "\(h)h \(m)m"
        }
    }

    /// fmt_duration in src/widgets.rs, in English (history lines).
    static func fmtDuration(_ secs: Int64) -> String {
        let h = secs / 3600, m = (secs % 3600) / 60, s = secs % 60
        if h > 0 { return "\(h)h \(m)m" }
        if m > 0 { return "\(m)m \(s)s" }
        return "\(s)s"
    }

    /// Logged like the Mac app's typed fields: my own line about the
    /// estimate from the last five minutes is updated instead of adding one.
    @discardableResult
    public static func setEstimate(_ data: inout JSON, task: UInt64, minutes: UInt32?, me: UInt64, ts: Int64) -> Bool {
        data.updateItem(at: "tasks", id: task) { j in
            let before = j["estimate_min"]?.uint.map(UInt32.init)
            guard before != minutes else { return }
            j["estimate_min"] = minutes.map { .int(Int64($0)) } ?? .null
            let text = minutes.map { "set the time estimate to \(fmtDuration(Int64($0) * 60))" } ?? "removed the time estimate"
            logEdit(&j, me: me, like: "time estimate", text: text, ts: ts)
            j["updated"] = .int(ts)
        }
    }

    /// log_edit in src/app.rs.
    static func logEdit(_ j: inout JSON, me: UInt64, like: String, text: String, ts: Int64) {
        var act = j["activity"]?.array ?? []
        if let i = act.indices.last, act[i]["comment"]?.bool != true, act[i]["author"]?.uint == me,
           ts - (act[i]["ts"]?.int ?? 0) < 300, (act[i]["text"]?.string ?? "").contains(like) {
            act[i]["text"] = .string(text)
            act[i]["ts"] = .int(ts)
            j["activity"] = .array(act)
            return
        }
        log(&j, me: me, text: text, ts: ts)
    }

    // MARK: timers and time entries (src/model.rs)

    /// Several people can run a timer on one task: the first in
    /// `timer_started` / `timer_by` (what older apps read), the rest in
    /// `timers`. `solo`: I'm the only member, so an old timer without an
    /// owner is mine.
    @discardableResult
    public static func startTimer(_ data: inout JSON, task: UInt64, me: UInt64, ts: Int64) -> Bool {
        let solo = (data["members"]?.array ?? []).count <= 1
        return data.updateItem(at: "tasks", id: task) { j in
            // mine already runs (the Mac only offers Start when it doesn't)
            let by = j["timer_by"]?.uint
            if j["timer_started"]?.int != nil, by == me || (by == nil && solo) { return }
            if j["timer_started"]?.int == nil {
                j["timer_started"] = .int(ts)
                j["timer_by"] = .id(me)
            } else {
                var timers = j["timers"]?.array ?? []
                guard !timers.contains(where: { $0["member"]?.uint == me }) else { return }
                timers.append(["member": .id(me), "start": .int(ts)])
                j["timers"] = .array(timers)
            }
        }
    }

    /// Stops my timer on the task, keeping the time as my entry
    /// (stop_my_timer).
    @discardableResult
    public static func stopTimer(_ data: inout JSON, task: UInt64, me: UInt64, ts: Int64) -> Bool {
        let solo = (data["members"]?.array ?? []).count <= 1
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var entries = j["time_entries"]?.array ?? []
            var timers = j["timers"]?.array ?? []
            let by = j["timer_by"]?.uint
            if let st = j["timer_started"]?.int, by == me || (by == nil && solo) {
                entries.append(["start": .int(st), "end": .int(ts), "member": .id(me)])
                j["timer_started"] = .null
                j["timer_by"] = nil
                // the next person's timer moves into the field older apps read
                if !timers.isEmpty {
                    let r = timers.removeFirst()
                    j["timer_started"] = r["start"] ?? .null
                    j["timer_by"] = r["member"]
                }
            } else if let k = timers.firstIndex(where: { $0["member"]?.uint == me }) {
                let r = timers.remove(at: k)
                entries.append(["start": r["start"] ?? .int(ts), "end": .int(ts), "member": .id(me)])
            } else {
                return
            }
            j["time_entries"] = .array(entries)
            if timers.isEmpty { j["timers"] = nil } else { j["timers"] = .array(timers) }
            changed = true
        }
        return changed
    }

    /// "Add time": an entry of `minutes` ending now (or this time yesterday).
    @discardableResult
    public static func addTimeEntry(_ data: inout JSON, task: UInt64, minutes: UInt32, yesterday: Bool, me: UInt64, ts: Int64) -> Bool {
        guard minutes > 0 else { return false }
        let end = ts - (yesterday ? 86400 : 0)
        return data.updateItem(at: "tasks", id: task) { j in
            var entries = j["time_entries"]?.array ?? []
            entries.append(["start": .int(end - Int64(minutes) * 60), "end": .int(end), "member": .id(me)])
            j["time_entries"] = .array(entries)
        }
    }

    /// Removes one entry, found by its person and start.
    @discardableResult
    public static func removeTimeEntry(_ data: inout JSON, task: UInt64, member: UInt64, start: Int64) -> Bool {
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var entries = j["time_entries"]?.array ?? []
            guard let i = entries.firstIndex(where: { ($0["member"]?.uint ?? 0) == member && $0["start"]?.int == start }) else { return }
            entries.remove(at: i)
            j["time_entries"] = .array(entries)
            changed = true
        }
        return changed
    }

    // MARK: custom fields

    /// A custom field's value as text ("true" for a ticked checkbox, a date
    /// as "YYYY-MM-DD"); an empty value takes the field off the task.
    @discardableResult
    public static func setCustom(_ data: inout JSON, task: UInt64, field: UInt64, to value: String, ts: Int64) -> Bool {
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var custom = j["custom"]?.object ?? [:]
            let key = String(field)
            let clean = value.trimmed
            guard (custom[key]?.string ?? "") != clean else { return }
            if clean.isEmpty { custom[key] = nil } else { custom[key] = .string(clean) }
            j["custom"] = .object(custom)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    // MARK: task type

    @discardableResult
    public static func setMilestone(_ data: inout JSON, task: UInt64, _ on: Bool, ts: Int64) -> Bool {
        data.updateItem(at: "tasks", id: task) { j in
            guard (j["milestone"]?.bool ?? false) != on else { return }
            j["milestone"] = on ? .bool(true) : nil
            j["updated"] = .int(ts)
        }
    }

    // MARK: checklist names and people

    @discardableResult
    public static func renameCheckItem(_ data: inout JSON, task: UInt64, index: Int, to text: String, ts: Int64) -> Bool {
        let clean = text.trimmed
        guard !clean.isEmpty else { return false }
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var items = j["checklist"]?.array ?? []
            guard items.indices.contains(index), items[index]["text"]?.string != clean else { return }
            items[index]["text"] = .string(clean)
            j["checklist"] = .array(items)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    /// The one person a checklist item is for, or nobody.
    @discardableResult
    public static func setCheckAssignee(_ data: inout JSON, task: UInt64, index: Int, to member: UInt64?, ts: Int64) -> Bool {
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var items = j["checklist"]?.array ?? []
            guard items.indices.contains(index), items[index]["assignee"]?.uint != member else { return }
            items[index]["assignee"] = member.map(JSON.id)
            j["checklist"] = .array(items)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    /// "Convert to subtask": the item leaves the checklist and becomes a
    /// subtask, for the person it was for.
    @discardableResult
    public static func checkItemToSubtask(_ data: inout JSON, task: UInt64, index: Int, me: UInt64, id: UInt64, ts: Int64) -> Bool {
        guard let t = (data["tasks"]?.array ?? []).first(where: { $0["id"]?.uint == task }),
              let items = t["checklist"]?.array, items.indices.contains(index),
              let list = t["list_id"]?.uint else { return false }
        let item = items[index]
        guard removeCheckItem(&data, task: task, index: index, ts: ts) else { return false }
        addTask(&data, list: list, name: item["text"]?.string ?? "", assignees: item["assignee"]?.uint.map { [$0] } ?? [], parent: task, me: me, id: id, ts: ts)
        return true
    }

    // MARK: duplicate and archive (src/app/parity.rs)

    /// A copy of the task with its subtasks, named "<name> (copy)", without
    /// its time; returns the copy's id.
    @discardableResult
    public static func duplicateTask(_ data: inout JSON, task: UInt64, me: UInt64, ts: Int64, newId: () -> UInt64) -> UInt64? {
        let tasks = data["tasks"]?.array ?? []
        let names = Dictionary(Mentions.members(of: data).map { ($0.id, $0.name) }, uniquingKeysWith: { a, _ in a })
        var added: [JSON] = []
        func copy(_ id: UInt64, parent: UInt64?, top: Bool) -> UInt64? {
            guard var t = tasks.first(where: { $0["id"]?.uint == id }) else { return nil }
            let src = t["name"]?.string ?? ""
            let nid = newId()
            t["id"] = .id(nid)
            t["parent"] = parent.map(JSON.id) ?? .null
            if top { t["name"] = .string("\(src) (copy)") }
            t["created"] = .int(ts)
            t["updated"] = .int(ts)
            t["timer_started"] = .null
            t["timer_by"] = nil
            t["timers"] = nil
            t["time_entries"] = []
            var act: [JSON] = [["ts": .int(ts), "author": .id(me), "text": .string("duplicated this task from \(src)"), "comment": false, "post": false]]
            for a in (t["assignees"]?.array ?? []).compactMap(\.uint) where a != me {
                act.append(["ts": .int(ts), "author": .id(me), "text": .string("assigned to \(names[a] ?? "")"), "comment": false, "post": false, "target": .id(a)])
            }
            t["activity"] = .array(act)
            added.append(t)
            for s in tasks where s["parent"]?.uint == id {
                if let sid = s["id"]?.uint { _ = copy(sid, parent: nid, top: false) }
            }
            return nid
        }
        guard let nid = copy(task, parent: tasks.first { $0["id"]?.uint == task }?["parent"]?.uint, top: true) else { return nil }
        data["tasks"] = .array(tasks + added)
        return nid
    }

    /// Archived tasks stay out of every view until unarchived.
    @discardableResult
    public static func setArchived(_ data: inout JSON, task: UInt64, _ archived: Bool, me: UInt64, ts: Int64) -> Bool {
        data.updateItem(at: "tasks", id: task) { j in
            guard (j["archived"]?.bool ?? false) != archived else { return }
            j["archived"] = .bool(archived)
            log(&j, me: me, text: archived ? "archived this task" : "unarchived this task", ts: ts)
            j["updated"] = .int(ts)
        }
    }
}

// MARK: moving dates (the Gantt and Timeline bars, src/app/timebar.rs)

extension Edits {
    /// Moves a task's start and due dates by `days` (dragging its bar);
    /// times of day stay. A task with only one of the two moves that one.
    @discardableResult
    public static func shiftDates(_ data: inout JSON, task: UInt64, days: Int, me: UInt64, ts: Int64, today: Date = Date()) -> Bool {
        guard days != 0, let t = (data["tasks"]?.array ?? []).first(where: { $0["id"]?.uint == task }) else { return false }
        let cal = Calendar(identifier: .gregorian)
        let move = { (s: String?) -> String? in s.flatMap(DayFormat.date(fromDay:)).flatMap { cal.date(byAdding: .day, value: days, to: $0) }.map { DayFormat.day($0) } }
        var changed = false
        if let s = move(t["start"]?.string) { changed = setStart(&data, task: task, to: s, me: me, ts: ts, today: today) || changed }
        if let d = move(t["due"]?.string) { changed = setDue(&data, task: task, to: d, me: me, ts: ts, today: today) || changed }
        return changed
    }
}

// MARK: attachments (attach_ready_task_files in src/app/taskfields.rs)

extension Edits {
    /// A picture or video now on the server joins the task, logged
    /// "attached <name>"; the same file twice is kept once.
    @discardableResult
    public static func addTaskAttachment(_ data: inout JSON, task: UInt64, attachment: JSON, me: UInt64, ts: Int64) -> Bool {
        guard let file = attachment["file"]?.string, !file.isEmpty else { return false }
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var atts = j["attachments"]?.array ?? []
            guard !atts.contains(where: { $0["file"]?.string == file }) else { return }
            atts.append(attachment)
            j["attachments"] = .array(atts)
            log(&j, me: me, text: "attached \(attachment["name"]?.string ?? "")", ts: ts)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }

    @discardableResult
    public static func removeTaskAttachment(_ data: inout JSON, task: UInt64, file: String, ts: Int64) -> Bool {
        var changed = false
        data.updateItem(at: "tasks", id: task) { j in
            var atts = j["attachments"]?.array ?? []
            let n = atts.count
            atts.removeAll { $0["file"]?.string == file }
            guard atts.count != n else { return }
            j["attachments"] = .array(atts)
            j["updated"] = .int(ts)
            changed = true
        }
        return changed
    }
}
