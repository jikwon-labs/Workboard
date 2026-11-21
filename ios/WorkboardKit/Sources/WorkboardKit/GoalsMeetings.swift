// Goals and Meetings (Goal, GoalTarget, Meeting in src/model.rs; the pages
// in src/app/more.rs and src/app/hubs.rs). Both are shared with the team
// (`extra.goals`, `extra.meetings`).

import Foundation

public struct GoalTargetView: Hashable, Sendable {
    public enum Kind: Hashable, Sendable {
        case number(start: Double, target: Double, current: Double, unit: String)
        case trueFalse(done: Bool)
        case tasks(ids: [UInt64])
    }

    /// So two people changing different targets at once both keep theirs
    /// (Sync.fillGoalIds); 0 until given one.
    public let id: UInt64
    public let name: String
    public let kind: Kind

    /// TargetKind is written by serde as {"Number": {...}}, {"TrueFalse":
    /// {...}} or {"Tasks": {...}}.
    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        let k = j["kind"]
        if let n = k?["Number"] {
            kind = .number(start: n["start"]?.double ?? 0, target: n["target"]?.double ?? 0, current: n["current"]?.double ?? 0, unit: n["unit"]?.string ?? "")
        } else if let t = k?["TrueFalse"] {
            kind = .trueFalse(done: t["done"]?.bool ?? false)
        } else {
            kind = .tasks(ids: (k?["Tasks"]?["ids"]?.array ?? []).compactMap(\.uint))
        }
    }

    var json: JSON {
        let k: JSON
        switch kind {
        case .number(let s, let t, let c, let u): k = ["Number": ["start": .double(s), "target": .double(t), "current": .double(c), "unit": .string(u)]]
        case .trueFalse(let d): k = ["TrueFalse": ["done": .bool(d)]]
        case .tasks(let ids): k = ["Tasks": ["ids": .array(ids.map(JSON.id))]]
        }
        var j: JSON = ["name": .string(name), "kind": k]
        if id != 0 { j["id"] = .id(id) }
        return j
    }

    public init(name: String, kind: Kind) {
        self.id = 0
        self.name = name
        self.kind = kind
    }

    /// A number from a value to the same value: nothing to measure, so it
    /// can't be added (it would read as met from the start), as on the Mac.
    public var startsWhereItEnds: Bool {
        if case .number(let s, let t, _, _) = kind { return abs(t - s) < .ulpOfOne }
        return false
    }
}

public struct GoalView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let color: [Int]
    public let due: String?
    public let owner: UInt64
    public let targets: [GoalTargetView]

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        color = (j["color"]?.array ?? []).compactMap { $0.int.map(Int.init) }
        due = j["due"]?.string
        owner = j["owner"]?.uint ?? 0
        targets = (j["targets"]?.array ?? []).map(GoalTargetView.init)
    }
}

public struct MeetingView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let title: String
    public let date: String
    public let time: String
    public let attendees: [UInt64]
    public let agenda: String
    public let notes: String

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        title = j["title"]?.string ?? ""
        date = j["date"]?.string ?? ""
        time = j["time"]?.string ?? ""
        attendees = (j["attendees"]?.array ?? []).compactMap(\.uint)
        agenda = j["agenda"]?.string ?? ""
        notes = j["notes"]?.string ?? ""
    }

    /// Lines of the notes starting with "[ ]" (action items).
    public var actionItems: [String] {
        notes.components(separatedBy: "\n").compactMap { l in
            let t = l.trimmingCharacters(in: .whitespaces)
            guard t.hasPrefix("[ ]") else { return nil }
            let s = t.dropFirst(3).trimmingCharacters(in: .whitespaces)
            return s.isEmpty ? nil : s
        }
    }
}

extension WorkspaceView {
    public func goals(_ data: JSON) -> [GoalView] { (data.at("extra.goals")?.array ?? []).map(GoalView.init) }

    public func meetings(_ data: JSON) -> [MeetingView] {
        (data.at("extra.meetings")?.array ?? []).map(MeetingView.init).sorted { ($0.date, $0.time) < ($1.date, $1.time) }
    }

    /// goal_progress in src/app/more/goals.rs: the average of its targets' progress.
    public func progress(of g: GoalView) -> Double {
        guard !g.targets.isEmpty else { return 0 }
        return g.targets.map { t -> Double in
            switch t.kind {
            case .number(let s, let target, let c, _):
                return abs(target - s) < .ulpOfOne ? 1 : min(1, max(0, (c - s) / (target - s)))
            case .trueFalse(let d): return d ? 1 : 0
            case .tasks(let ids):
                // tasks deleted since they were picked don't count either
                // way; archived ones still do, as on the Mac
                let (done, total) = goalTaskCounts(ids)
                return total == 0 ? 0 : Double(done) / Double(total)
            }
        }.reduce(0, +) / Double(g.targets.count)
    }

    /// A tasks target's tasks closed and in all, counted as its progress
    /// is (archived ones too, deleted ones not).
    public func goalTaskCounts(_ ids: [UInt64]) -> (done: Int, total: Int) {
        let live = ids.compactMap { goalTaskClosed[$0] }
        return (live.filter { $0 }.count, live.count)
    }

    /// pct_of in src/app/more/goals.rs: the progress as a whole percent,
    /// rounded, but 100 only once every target is met (99.5% reads 99).
    public func percent(of g: GoalView) -> Int {
        let p = progress(of: g)
        let n = Int(min(100, max(0, (p * 100).rounded())))
        return p < 1 ? min(n, 99) : n
    }
}

extension Edits {
    // MARK: goals

    @discardableResult
    public static func addGoal(_ data: inout JSON, name: String, me: UInt64, id: UInt64) -> Bool {
        let clean = name.trimmed
        var goals = data.at("extra.goals")?.array ?? []
        goals.append(["id": .id(id), "name": .string(clean.isEmpty ? "New goal" : clean), "color": .array(statusPalette[1].map { .int(Int64($0)) }), "due": nil, "owner": .id(me), "targets": []])
        data.set("extra.goals", .array(goals))
        return true
    }

    private static func changeGoal(_ data: inout JSON, _ id: UInt64, _ change: (inout JSON) -> Bool) -> Bool {
        data.tryUpdateItem(at: "extra.goals", id: id, change)
    }

    @discardableResult
    public static func renameGoal(_ data: inout JSON, goal: UInt64, to name: String) -> Bool {
        let clean = name.trimmed
        guard !clean.isEmpty else { return false }
        return changeGoal(&data, goal) { $0["name"] = .string(clean); return true }
    }

    @discardableResult
    public static func setGoalDue(_ data: inout JSON, goal: UInt64, to day: String?) -> Bool {
        changeGoal(&data, goal) { $0["due"] = day.map(JSON.string) ?? .null; return true }
    }

    @discardableResult
    public static func deleteGoal(_ data: inout JSON, goal: UInt64) -> Bool {
        var goals = data.at("extra.goals")?.array ?? []
        let n = goals.count
        goals.removeAll { $0["id"]?.uint == goal }
        guard goals.count != n else { return false }
        data.set("extra.goals", .array(goals))
        return true
    }

    @discardableResult
    public static func addTarget(_ data: inout JSON, goal: UInt64, _ target: GoalTargetView) -> Bool {
        guard !target.name.isBlank, !target.startsWhereItEnds else { return false }
        return changeGoal(&data, goal) { g in
            var t = g["targets"]?.array ?? []
            t.append(target.json)
            g["targets"] = .array(t)
            return true
        }
    }

    /// Replaces target `index` (its current number, done, or tasks). It
    /// keeps the target's id, so the change merges as that target's.
    @discardableResult
    public static func setTarget(_ data: inout JSON, goal: UInt64, index: Int, to target: GoalTargetView) -> Bool {
        changeGoal(&data, goal) { g in
            var t = g["targets"]?.array ?? []
            guard t.indices.contains(index) else { return false }
            var j = target.json
            if j["id"] == nil, let old = t[index]["id"] { j["id"] = old }
            t[index] = j
            g["targets"] = .array(t)
            return true
        }
    }

    @discardableResult
    public static func removeTarget(_ data: inout JSON, goal: UInt64, index: Int) -> Bool {
        changeGoal(&data, goal) { g in
            var t = g["targets"]?.array ?? []
            guard t.indices.contains(index) else { return false }
            t.remove(at: index)
            g["targets"] = .array(t)
            return true
        }
    }

    // MARK: meetings

    /// "+ Meeting": today at 10:00 with me attending.
    @discardableResult
    public static func addMeeting(_ data: inout JSON, title: String, date: String, time: String = "10:00", me: UInt64, id: UInt64) -> Bool {
        let clean = title.trimmed
        var ms = data.at("extra.meetings")?.array ?? []
        ms.append(["id": .id(id), "title": .string(clean.isEmpty ? "New meeting" : clean), "date": .string(date), "time": .string(time), "attendees": [.id(me)], "agenda": "", "notes": ""])
        data.set("extra.meetings", .array(ms))
        return true
    }

    /// Changes a meeting's title, date, time, agenda or notes (nil leaves it).
    @discardableResult
    public static func updateMeeting(_ data: inout JSON, meeting: UInt64, title: String? = nil, date: String? = nil, time: String? = nil, agenda: String? = nil, notes: String? = nil) -> Bool {
        data.updateItem(at: "extra.meetings", id: meeting) { m in
            if let title, !title.isBlank { m["title"] = .string(title) }
            if let date { m["date"] = .string(date) }
            if let time { m["time"] = .string(time) }
            if let agenda { m["agenda"] = .string(agenda) }
            if let notes { m["notes"] = .string(notes) }
        }
    }

    @discardableResult
    public static func toggleAttendee(_ data: inout JSON, meeting: UInt64, member: UInt64) -> Bool {
        data.updateItem(at: "extra.meetings", id: meeting) { m in
            var a = (m["attendees"]?.array ?? []).compactMap(\.uint)
            if let k = a.firstIndex(of: member) { a.remove(at: k) } else { a.append(member) }
            m["attendees"] = .array(a.map(JSON.id))
        }
    }

    @discardableResult
    public static func deleteMeeting(_ data: inout JSON, meeting: UInt64) -> Bool {
        var ms = data.at("extra.meetings")?.array ?? []
        let n = ms.count
        ms.removeAll { $0["id"]?.uint == meeting }
        guard ms.count != n else { return false }
        data.set("extra.meetings", .array(ms))
        return true
    }

    /// "Create N tasks from action items": each "[ ]" line becomes a task in
    /// `list` for the attendees, and the lines are ticked ("[x]").
    @discardableResult
    public static func meetingActionsToTasks(_ data: inout JSON, meeting: UInt64, list: UInt64, me: UInt64, ts: Int64, newId: () -> UInt64) -> Int {
        guard let m = (data.at("extra.meetings")?.array ?? []).map(MeetingView.init).first(where: { $0.id == meeting }) else { return 0 }
        let members = Set(Mentions.members(of: data).map(\.id))
        let who = m.attendees.filter { members.contains($0) }
        let items = m.actionItems
        for n in items { addTask(&data, list: list, name: n, assignees: who, me: me, id: newId(), ts: ts) }
        if !items.isEmpty { updateMeeting(&data, meeting: meeting, notes: m.notes.replacingOccurrences(of: "[ ]", with: "[x]")) }
        return items.count
    }
}

// MARK: reminders and the notepad (each person's own: `extra.reminders` and
// `extra.notepad` are not shared, so they change with SyncClient.editLocal)

public struct ReminderView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let text: String
    public let dueTs: Int64
    public let done: Bool
    public let task: UInt64?

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        text = j["text"]?.string ?? ""
        dueTs = j["due_ts"]?.int ?? 0
        done = j["done"]?.bool ?? false
        task = j["task"]?.uint
    }
}

extension WorkspaceView {
    /// My reminders, soonest first.
    public func reminders(_ data: JSON, me: UInt64) -> [ReminderView] {
        (data.at("extra.reminders")?.array ?? []).filter { ($0["owner"]?.uint ?? me) == me }.map(ReminderView.init).sorted { $0.dueTs < $1.dueTs }
    }
}

extension Edits {
    /// Reminder in src/model.rs; `task` for "Remind me" on a task.
    @discardableResult
    public static func addReminder(_ data: inout JSON, text: String, dueTs: Int64, task: UInt64? = nil, me: UInt64, id: UInt64) -> Bool {
        let clean = text.trimmed
        guard !clean.isEmpty else { return false }
        var rs = data.at("extra.reminders")?.array ?? []
        var r: JSON = ["id": .id(id), "text": .string(clean), "due_ts": .int(dueTs), "done": false, "owner": .id(me), "alerted": false]
        if let task { r["task"] = .id(task) }
        rs.append(r)
        data.set("extra.reminders", .array(rs))
        return true
    }

    @discardableResult
    public static func setReminderDone(_ data: inout JSON, reminder: UInt64, _ done: Bool) -> Bool {
        data.updateItem(at: "extra.reminders", id: reminder) { $0["done"] = .bool(done) }
    }

    @discardableResult
    public static func deleteReminder(_ data: inout JSON, reminder: UInt64) -> Bool {
        var rs = data.at("extra.reminders")?.array ?? []
        let n = rs.count
        rs.removeAll { $0["id"]?.uint == reminder }
        guard rs.count != n else { return false }
        data.set("extra.reminders", .array(rs))
        return true
    }

    public static func setNotepad(_ data: inout JSON, to text: String) {
        data.set("extra.notepad", .string(text))
    }
}
