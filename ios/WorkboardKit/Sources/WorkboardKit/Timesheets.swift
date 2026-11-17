// Timesheets (timesheet_grid and all_timesheets in src/app/hubs.rs): a week
// from Sunday, my time per task and day, and everyone's total for the week,
// counted by whose time it is rather than who the task is assigned to.

import Foundation

public struct TimesheetRow: Identifiable, Hashable, Sendable {
    public let task: TaskView
    /// Seconds on each day of the week, Sunday first.
    public let days: [Int64]
    public var id: UInt64 { task.id }
    public var total: Int64 { days.reduce(0, +) }
}

public enum Timesheet {
    /// The Sunday a week starts on, at local midnight.
    public static func weekStart(of d: Date, calendar: Calendar = .current) -> Date {
        let day = calendar.startOfDay(for: d)
        let back = calendar.component(.weekday, from: day) - 1
        return calendar.date(byAdding: .day, value: -back, to: day) ?? day
    }

    static func dayIndex(_ ts: Int64, from start: Date, calendar: Calendar) -> Int? {
        let day = calendar.startOfDay(for: Date(timeIntervalSince1970: TimeInterval(ts)))
        guard let i = calendar.dateComponents([.day], from: start, to: day).day, (0..<7).contains(i) else { return nil }
        return i
    }

    /// My rows for the week starting `start`: tasks with an entry of mine
    /// that week, or my timer running (counted up to `now`).
    public static func mine(_ view: WorkspaceView, me: UInt64, weekOf start: Date, now: Int64, calendar: Calendar = .current) -> [TimesheetRow] {
        var rows: [TimesheetRow] = []
        for t in view.tasks {
            var days = [Int64](repeating: 0, count: 7)
            var any = false
            for e in t.timeEntries {
                let owner = view.entryOwner(t, e)
                guard owner == me || (owner == 0 && view.solo), let i = dayIndex(e.start, from: start, calendar: calendar) else { continue }
                days[i] += e.secs
                any = true
            }
            if let st = view.myTimerStart(t, me: me), let i = dayIndex(now, from: start, calendar: calendar) {
                days[i] += max(0, now - st)
                any = true
            }
            if any { rows.append(TimesheetRow(task: t, days: days)) }
        }
        return rows.sorted { $0.task.name.localizedCaseInsensitiveCompare($1.task.name) == .orderedAscending }
    }

    /// Everyone's total for the week; 0 is old entries nobody owns.
    public static func everyone(_ view: WorkspaceView, me: UInt64, weekOf start: Date, calendar: Calendar = .current) -> [UInt64: Int64] {
        var per: [UInt64: Int64] = [:]
        for t in view.tasks {
            for e in t.timeEntries where dayIndex(e.start, from: start, calendar: calendar) != nil {
                var owner = view.entryOwner(t, e)
                if owner == 0 && view.solo { owner = me }
                per[owner, default: 0] += e.secs
            }
        }
        return per
    }
}
