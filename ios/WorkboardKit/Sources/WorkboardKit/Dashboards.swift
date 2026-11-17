// Dashboards (Dashboard and DashCard in src/model.rs, the cards in
// src/app/pages.rs dash_card_body): a space's tasks, or every task, as
// totals, status and priority bars, workload, this week's time and the
// overdue list.

import Foundation

public struct DashboardView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let space: UInt64?
    public let owner: UInt64
    public let kind: String
    public let isPrivate: Bool
    /// The cards in order (the template's set until customised).
    public let cards: [(kind: String, title: String)]

    public static func == (a: DashboardView, b: DashboardView) -> Bool { a.id == b.id && a.name == b.name && a.cards.map(\.kind) == b.cards.map(\.kind) }
    public func hash(into h: inout Hasher) { h.combine(id) }

    /// DASH_CARDS in src/app/more.rs.
    public static let standard: [(kind: String, title: String)] = [
        ("stats", "Task totals"), ("status", "Tasks by status"), ("priority", "Tasks by priority"),
        ("workload", "Workload by assignee"), ("time", "Time tracked this week"),
        ("time_people", "Time tracked this week by person"), ("overdue", "Overdue tasks"),
    ]

    /// default_dash_cards: the template's cards.
    public static func defaults(_ kind: String) -> [(kind: String, title: String)] {
        standard.filter { c in
            switch kind {
            case "simple": return ["stats", "status", "priority", "overdue"].contains(c.kind)
            case "time": return ["time", "time_people"].contains(c.kind)
            case "project": return ["stats", "status", "workload", "overdue"].contains(c.kind)
            default: return c.kind != "time_people"
            }
        }
    }

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        space = j["space"]?.uint
        owner = j["owner"]?.uint ?? 0
        kind = j["kind"]?.string ?? ""
        isPrivate = j["private"]?.bool ?? false
        if let cs = j["cards"]?.array {
            cards = cs.map { c in
                let k = c["kind"]?.string ?? ""
                let t = c["title"]?.string ?? ""
                return (k, t.isEmpty ? (Self.standard.first { $0.kind == k }?.title ?? k) : t)
            }
        } else {
            cards = Self.defaults(kind)
        }
    }
}

public struct DashNumbers: Sendable {
    public let total: Int
    public let open: Int
    public let overdue: [TaskView]
    public let byStatus: [(StatusView, Int)]
    public let byPriority: [(Int, Int)]
    public let workload: [(MemberView, Int)]
    public let timePerDay: [Int64]
    public let timePerPerson: [(MemberView, Int64)]
}

extension WorkspaceView {
    public func dashboards(_ data: JSON, me: UInt64) -> [DashboardView] {
        (data.at("extra.dashboards")?.array ?? []).map(DashboardView.init).filter { !$0.isPrivate || $0.owner == me }
    }

    /// The numbers a dashboard's cards show, for a space (nil: everything I
    /// can see).
    public func dashNumbers(space: UInt64?, me: UInt64, now: Int64, calendar: Calendar = .current) -> DashNumbers {
        let lists: Set<UInt64> = Set(space.flatMap { s in spaces.first { $0.id == s }?.allListIds } ?? visibleSpaces(me: me).flatMap(\.allListIds))
        let ts = tasks.filter { lists.contains($0.listId) }
        let closed = { (t: TaskView) in self.status(of: t).isClosed }
        let today = dayString(), hm = timeString()
        let overdue = ts.filter { !closed($0) && $0.isOverdue(today: today, now: hm) }
        var sts: [StatusView] = []
        for s in spaces { for st in s.statuses where !sts.contains(where: { $0.name == st.name }) { sts.append(st) } }
        if sts.isEmpty { sts = StatusView.defaults }
        let byStatus = sts.map { st in (st, ts.filter { status(of: $0).name == st.name }.count) }
        let byPriority = [1, 2, 3, 4, 0].map { p in (p, ts.filter { $0.priority == p && !closed($0) }.count) }
        let workload = members.map { m in (m, ts.filter { $0.assignees.contains(m.id) && !closed($0) }.count) }
        let start = Timesheet.weekStart(of: Date(timeIntervalSince1970: TimeInterval(now)), calendar: calendar)
        var perDay = [Int64](repeating: 0, count: 7)
        var perPerson: [UInt64: Int64] = [:]
        for t in ts {
            for e in t.timeEntries {
                guard let i = Timesheet.dayIndex(e.start, from: start, calendar: calendar) else { continue }
                perDay[i] += e.secs
                var o = entryOwner(t, e)
                if o == 0 && solo { o = me }
                perPerson[o, default: 0] += e.secs
            }
            for r in t.timers {
                guard let i = Timesheet.dayIndex(r.start, from: start, calendar: calendar) else { continue }
                perDay[i] += max(0, now - r.start)
                if let by = r.member ?? (solo ? me : nil) { perPerson[by, default: 0] += max(0, now - r.start) }
            }
        }
        return DashNumbers(total: ts.count, open: ts.filter { !closed($0) }.count, overdue: overdue, byStatus: byStatus,
                           byPriority: byPriority, workload: workload, timePerDay: perDay,
                           timePerPerson: members.map { ($0, perPerson[$0.id] ?? 0) })
    }
}

extension Edits {
    /// A dashboard from a template ("simple", "time", "project", or "").
    @discardableResult
    public static func addDashboard(_ data: inout JSON, name: String, kind: String, space: UInt64?, me: UInt64, id: UInt64) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        var ds = data.at("extra.dashboards")?.array ?? []
        var d: JSON = ["id": .id(id), "name": .string(clean.isEmpty ? "Dashboard" : clean), "space": space.map(JSON.id) ?? .null, "owner": .id(me)]
        if !kind.isEmpty { d["kind"] = .string(kind) }
        ds.append(d)
        data.set("extra.dashboards", .array(ds))
        return true
    }

    @discardableResult
    public static func renameDashboard(_ data: inout JSON, dashboard: UInt64, to name: String) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        var ds = data.at("extra.dashboards")?.array ?? []
        guard !clean.isEmpty, let i = ds.firstIndex(where: { $0["id"]?.uint == dashboard }) else { return false }
        ds[i]["name"] = .string(clean)
        data.set("extra.dashboards", .array(ds))
        return true
    }

    @discardableResult
    public static func deleteDashboard(_ data: inout JSON, dashboard: UInt64) -> Bool {
        var ds = data.at("extra.dashboards")?.array ?? []
        let n = ds.count
        ds.removeAll { $0["id"]?.uint == dashboard }
        guard ds.count != n else { return false }
        data.set("extra.dashboards", .array(ds))
        return true
    }
}

// MARK: teams (Team in src/model.rs; the Teams hub in src/app/hubs.rs)

public struct TeamView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let members: [UInt64]
    public let color: [Int]

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        members = (j["members"]?.array ?? []).compactMap(\.uint)
        color = (j["color"]?.array ?? []).compactMap { $0.int.map(Int.init) }
    }
}

extension WorkspaceView {
    public func teams(_ data: JSON) -> [TeamView] { (data.at("extra.teams")?.array ?? []).map(TeamView.init) }
}

extension Edits {
    /// "Create Team": "Team N" with me in it.
    @discardableResult
    public static func addTeam(_ data: inout JSON, me: UInt64, id: UInt64) -> Bool {
        var ts = data.at("extra.teams")?.array ?? []
        let n = ts.count
        ts.append(["id": .id(id), "name": .string("Team \(n + 1)"), "members": [.id(me)], "color": .array(statusPalette[1 + (n * 3) % 13].map { .int(Int64($0)) })])
        data.set("extra.teams", .array(ts))
        return true
    }

    private static func changeTeam(_ data: inout JSON, _ id: UInt64, _ change: (inout JSON) -> Void) -> Bool {
        var ts = data.at("extra.teams")?.array ?? []
        guard let i = ts.firstIndex(where: { $0["id"]?.uint == id }) else { return false }
        change(&ts[i])
        data.set("extra.teams", .array(ts))
        return true
    }

    @discardableResult
    public static func renameTeam(_ data: inout JSON, team: UInt64, to name: String) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return false }
        return changeTeam(&data, team) { $0["name"] = .string(clean) }
    }

    @discardableResult
    public static func toggleTeamMember(_ data: inout JSON, team: UInt64, member: UInt64) -> Bool {
        changeTeam(&data, team) { t in
            var m = (t["members"]?.array ?? []).compactMap(\.uint)
            if let i = m.firstIndex(of: member) { m.remove(at: i) } else { m.append(member) }
            t["members"] = .array(m.map(JSON.id))
        }
    }

    @discardableResult
    public static func deleteTeam(_ data: inout JSON, team: UInt64) -> Bool {
        var ts = data.at("extra.teams")?.array ?? []
        let n = ts.count
        ts.removeAll { $0["id"]?.uint == team }
        guard ts.count != n else { return false }
        data.set("extra.teams", .array(ts))
        return true
    }
}
