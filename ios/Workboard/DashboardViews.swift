// Dashboards and their cards (More tab).

import SwiftUI
import WorkboardKit

struct DashboardsView: View {
    @Environment(SyncClient.self) private var client
    @State private var adding = false

    var body: some View {
        let me = client.me ?? 0
        let ds = client.data.map { client.view.dashboards($0, me: me) } ?? []
        List {
            NavigationLink { DashboardDetailView(dashboardId: nil) } label: {
                Label("Workspace overview", systemImage: "chart.bar")
            }
            .accessibilityIdentifier("dashboard-overview")
            ForEach(ds) { d in
                NavigationLink { DashboardDetailView(dashboardId: d.id) } label: {
                    Label(d.name, systemImage: "rectangle.3.group")
                }
                .deleteSwipe(client.canEdit) { client.edit { data, _ in Edits.deleteDashboard(&data, dashboard: d.id) } }
                .accessibilityIdentifier("dashboard-\(d.id)")
            }
        }
        .navigationTitle("Dashboards")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    ForEach([("simple", "Simple Dashboard"), ("time", "Time Tracking"), ("project", "Project Management"), ("", "Every card")], id: \.0) { k, label in
                        Button(label) {
                            let id = client.newId()
                            client.edit { d, me in Edits.addDashboard(&d, name: label, kind: k, space: nil, me: me, id: id) }
                        }
                    }
                } label: { Image(systemName: "plus") }
                    .disabled(!client.canEdit)
                    .accessibilityLabel("New dashboard")
                    .accessibilityIdentifier("newDashboardButton")
            }
        }
    }
}

struct DashboardDetailView: View {
    @Environment(SyncClient.self) private var client
    /// nil: the built-in Workspace overview (every card, every space).
    let dashboardId: UInt64?

    var body: some View {
        let v = client.view
        let me = client.me ?? 0
        let d = dashboardId.flatMap { id in client.data.flatMap { v.dashboards($0, me: me).first { $0.id == id } } }
        let cards = d?.cards ?? DashboardView.defaults("")
        let n = v.dashNumbers(space: d?.space, me: me, now: nowTs())
        List {
            ForEach(Array(cards.enumerated()), id: \.offset) { _, c in
                Section(L(c.title)) { card(c.kind, n) }
            }
        }
        .navigationTitle(d?.name ?? "Workspace overview")
        .navigationBarTitleDisplayMode(.inline)
    }

    @ViewBuilder func card(_ kind: String, _ n: DashNumbers) -> some View {
        switch kind {
        case "stats":
            HStack {
                tile("Total tasks", n.total, .primary)
                tile("Open", n.open, Color(rgb: [0x5f, 0x55, 0xee]))
                tile("Overdue", n.overdue.count, .red)
                tile("Completed", n.total - n.open, Color(rgb: [0x00, 0x88, 0x44]))
            }
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("card-stats")
        case "status":
            let max = n.byStatus.map(\.1).max() ?? 1
            ForEach(n.byStatus, id: \.0.name) { st, c in bar(st.name.uppercased(), Double(c), Double(max), Color(rgb: st.color), "\(c)") }
        case "priority":
            let max = n.byPriority.map(\.1).max() ?? 1
            ForEach(n.byPriority, id: \.0) { p, c in
                bar(Edits.priorityLabels[p], Double(c), Double(max), p == 1 ? .red : p == 2 ? .orange : p == 3 ? .blue : .gray, "\(c)")
            }
        case "workload":
            let max = n.workload.map(\.1).max() ?? 1
            ForEach(n.workload, id: \.0.id) { m, c in bar(m.name, Double(c), Double(max), Color(rgb: m.color), "\(c)") }
        case "time":
            let max = n.timePerDay.max() ?? 1
            ForEach(0..<7, id: \.self) { i in
                bar(["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][i], Double(n.timePerDay[i]), Double(max), .teal, TimeSection.hm(n.timePerDay[i]))
            }
        case "time_people":
            let max = n.timePerPerson.map(\.1).max() ?? 1
            ForEach(n.timePerPerson, id: \.0.id) { m, s in bar(m.name, Double(s), Double(max), Color(rgb: m.color), TimeSection.hm(s)) }
        case "overdue":
            if n.overdue.isEmpty { Text("Nothing overdue").foregroundStyle(.secondary) }
            ForEach(n.overdue) { t in NavigationLink(value: Nav.Route.task(t.id)) { TaskRow(task: t, showList: true) } }
        default:
            Text(kind).foregroundStyle(.secondary)
        }
    }

    func tile(_ label: String, _ v: Int, _ c: Color) -> some View {
        VStack(spacing: 2) {
            Text("\(v)").font(.title2.bold()).foregroundStyle(c).monospacedDigit()
            Text(L(label)).font(.caption2).foregroundStyle(.secondary).lineLimit(1).minimumScaleFactor(0.7)
        }
        .frame(maxWidth: .infinity)
    }

    func bar(_ label: String, _ v: Double, _ max: Double, _ c: Color, _ value: String) -> some View {
        HStack(spacing: 8) {
            Text(L(label)).font(.subheadline).frame(width: 110, alignment: .leading).lineLimit(1)
            GeometryReader { g in
                Capsule().fill(c.opacity(0.85)).frame(width: max > 0 ? g.size.width * v / max : 0)
            }
            .frame(height: 10)
            Text(value).font(.caption).monospacedDigit().foregroundStyle(.secondary).frame(width: 52, alignment: .trailing)
        }
        .accessibilityElement(children: .combine)
    }
}
