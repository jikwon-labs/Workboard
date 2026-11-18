// Timesheets: a week of tracked time (More tab).

import SwiftUI
import WorkboardKit

struct TimesheetsView: View {
    @Environment(SyncClient.self) private var client
    @State private var week = Timesheet.weekStart(of: Date())
    @State private var everyone = false
    @State private var adding = false

    var body: some View {
        let v = client.view
        let me = client.me ?? 0
        let cal = Calendar.current
        let end = cal.date(byAdding: .day, value: 6, to: week) ?? week
        List {
            Section {
                HStack {
                    Button { week = cal.date(byAdding: .day, value: -7, to: week) ?? week } label: { Image(systemName: "chevron.left") }
                        .accessibilityLabel("Previous week").accessibilityIdentifier("prevWeek")
                    Spacer()
                    Text("\(week.formatted(.dateTime.month(.abbreviated).day())) - \(end.formatted(.dateTime.month(.abbreviated).day()))")
                        .font(.headline).accessibilityIdentifier("weekTitle")
                    Spacer()
                    Button { week = cal.date(byAdding: .day, value: 7, to: week) ?? week } label: { Image(systemName: "chevron.right") }
                        .accessibilityLabel("Next week").accessibilityIdentifier("nextWeek")
                }
                .buttonStyle(.borderless)
                Picker("Whose", selection: $everyone) {
                    Text("My timesheet").tag(false)
                    Text("All timesheets").tag(true)
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("timesheetTabs")
            }
            if everyone {
                let per = Timesheet.everyone(v, me: me, weekOf: week)
                Section {
                    ForEach(v.members) { m in
                        HStack(spacing: 10) {
                            Avatar(member: m, size: 24)
                            Text(m.name)
                            Spacer()
                            Text(TimeSection.hm(per[m.id] ?? 0)).monospacedDigit().foregroundStyle(.secondary)
                        }
                        .accessibilityIdentifier("sheetPerson-\(m.id)")
                    }
                    let rest = per.filter { k, _ in !v.members.contains { $0.id == k } }.values.reduce(0, +)
                    if rest > 0 { LabeledContent("Unknown", value: TimeSection.hm(rest)) }
                }
            } else {
                TimelineView(.periodic(from: .now, by: 60)) { ctx in
                    let rows = Timesheet.mine(v, me: me, weekOf: week, now: Int64(ctx.date.timeIntervalSince1970))
                    Section {
                        if rows.isEmpty {
                            Text("No time tracked this week.").foregroundStyle(.secondary)
                        }
                        ForEach(rows) { r in
                            NavigationLink(value: Nav.Route.task(r.task.id)) {
                                VStack(alignment: .leading, spacing: 4) {
                                    HStack {
                                        Text(r.task.name).lineLimit(1)
                                        Spacer()
                                        Text(TimeSection.hm(r.total)).monospacedDigit().fontWeight(.semibold)
                                    }
                                    HStack(spacing: 0) {
                                        ForEach(0..<7, id: \.self) { i in
                                            VStack(spacing: 1) {
                                                Text(["S", "M", "T", "W", "T", "F", "S"][i]).font(.caption2).foregroundStyle(.tertiary)
                                                Text(r.days[i] > 0 ? TimeSection.hm(r.days[i]) : "-").font(.caption2).monospacedDigit().foregroundStyle(.secondary)
                                            }
                                            .frame(maxWidth: .infinity)
                                        }
                                    }
                                }
                            }
                            .accessibilityIdentifier("sheetRow-\(r.task.id)")
                        }
                    } footer: {
                        Text("Total \(TimeSection.hm(rows.reduce(0) { $0 + $1.total }))").accessibilityIdentifier("sheetTotal")
                    }
                }
            }
        }
        .navigationTitle("Timesheets")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("Add time", id: "sheetAddTime") { adding = true }
            }
        }
        .sheet(isPresented: $adding) { SheetAddTime() }
    }
}

/// Add time to a task picked from the timesheet.
private struct SheetAddTime: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    @State private var task: TaskView?
    @State private var query = ""

    var body: some View {
        if let task {
            AddTimeSheet(task: task)
        } else {
            NavigationStack {
                List(client.view.tasks.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) }) { t in
                    Button { task = t } label: { TaskRow(task: t, showList: true) }
                        .foregroundStyle(.primary)
                        .accessibilityIdentifier("sheetPick-\(t.id)")
                }
                .searchable(text: $query)
                .navigationTitle("Add time to")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { CancelButton { dismiss() } }
            }
        }
    }
}
