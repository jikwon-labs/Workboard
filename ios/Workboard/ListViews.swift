import SwiftUI
import WorkboardKit

// A list's Board and Calendar views, as on the Mac: the Board has a column
// per status (drag a card to another column, or use its menu), the Calendar
// a month with each task on the days from its start to its due date, the
// day's tasks below it, and the tasks without a date.

enum ListMode: String, CaseIterable, Identifiable {
    case list = "List", board = "Board", calendar = "Calendar", gantt = "Gantt", timeline = "Timeline", table = "Table"
    var id: Self { self }
    var icon: String {
        switch self {
        case .list: return "list.bullet"
        case .board: return "rectangle.split.3x1"
        case .calendar: return "calendar"
        case .gantt: return "chart.bar.doc.horizontal"
        case .timeline: return "timeline.selection"
        case .table: return "tablecells"
        }
    }
}

struct BoardView: View {
    @Environment(SyncClient.self) private var client
    let listId: UInt64

    var body: some View {
        let v = client.view
        let tasks = v.tasks(inList: listId)
        ScrollView(.horizontal) {
            LazyHStack(alignment: .top, spacing: 12) {
                ForEach(v.statuses(forList: listId), id: \.name) { st in
                    BoardColumn(listId: listId, status: st, tasks: tasks.filter { v.status(of: $0).name == st.name })
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
        }
        .background(Color(.systemGroupedBackground))
    }
}

struct BoardColumn: View {
    @Environment(SyncClient.self) private var client
    let listId: UInt64
    let status: StatusView
    let tasks: [TaskView]
    @State private var adding = false
    @State private var newName = ""
    @State private var targeted = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                StatusDot(status: status, size: 10)
                Text(status.name.uppercased()).font(.caption.weight(.bold))
                Text("\(tasks.count)").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button { adding = true } label: { Image(systemName: "plus") }
                    .disabled(!client.canEdit)
                    .accessibilityLabel("Add task to \(status.name)")
                    .accessibilityIdentifier("boardAdd-\(status.name)")
            }
            .padding(.horizontal, 4)
            ScrollView {
                LazyVStack(spacing: 8) {
                    ForEach(tasks) { t in
                        NavigationLink(value: Nav.Route.task(t.id)) { BoardCard(task: t) }
                            .buttonStyle(.plain)
                            .draggable(String(t.id))
                            .accessibilityIdentifier("card-\(t.id)")
                            .overlay(alignment: .topTrailing) {
                                if client.canEdit { statusMenu(t) }
                            }
                    }
                    if tasks.isEmpty {
                        Text("No tasks").font(.footnote).foregroundStyle(.tertiary).frame(maxWidth: .infinity).padding(.vertical, 24)
                    }
                }
                .padding(.bottom, 12)
            }
        }
        .padding(10)
        .frame(width: 280)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Color(rgb: status.color).opacity(targeted ? 0.22 : 0.08)))
        .dropDestination(for: String.self) { items, _ in
            guard let id = items.first.flatMap(UInt64.init) else { return false }
            return client.edit { d, me in Edits.setStatus(&d, task: id, to: status.name, me: me, ts: nowTs()) }
        } isTargeted: { targeted = $0 }
        .alert("New task in \(status.name.uppercased())", isPresented: $adding) {
            TextField("Task name", text: $newName).accessibilityIdentifier("boardNewTaskField")
            Button("Cancel", role: .cancel) { newName = "" }
            Button("Add") {
                let name = newName
                newName = ""
                guard !name.trimmingCharacters(in: .whitespaces).isEmpty else { return }
                client.edit { d, me in Edits.addTask(&d, list: listId, name: name, assignees: [me], status: status.name, me: me, id: client.newId(), ts: nowTs()) }
            }
        }
    }

    /// The card's status, changed without dragging.
    func statusMenu(_ t: TaskView) -> some View {
        Menu {
            Section("Move to") {
                ForEach(client.view.statuses(forList: listId), id: \.name) { st in
                    Button(st.name.uppercased()) {
                        client.edit { d, me in Edits.setStatus(&d, task: t.id, to: st.name, me: me, ts: nowTs()) }
                    }
                    .disabled(st.name == t.status)
                }
            }
        } label: {
            Image(systemName: "ellipsis").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                .frame(width: 32, height: 28).contentShape(Rectangle())
        }
        .accessibilityLabel("Move \(t.name)")
        .accessibilityIdentifier("cardMenu-\(t.id)")
    }
}

struct BoardCard: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView

    var body: some View {
        let v = client.view
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .top, spacing: 6) {
                if task.milestone { Image(systemName: "diamond.fill").font(.caption2).foregroundStyle(.secondary) }
                Text(task.name).font(.subheadline.weight(.medium)).foregroundStyle(.primary).multilineTextAlignment(.leading)
                Spacer(minLength: 24)
            }
            if !task.tags.isEmpty {
                Text(task.tags.joined(separator: " · ")).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
            }
            HStack(spacing: 6) {
                ForEach(task.assignees.prefix(3), id: \.self) { a in Avatar(member: v.member(a), size: 18) }
                Spacer()
                if task.priority > 0 {
                    Image(systemName: "flag.fill").font(.caption2).foregroundStyle(task.priority == 1 ? .red : task.priority == 2 ? .orange : task.priority == 3 ? .blue : .secondary)
                }
                if let due = Fmt.due(task) {
                    Text(due).font(.caption2)
                        .foregroundStyle(task.isOverdue(today: dayString(), now: timeString()) && !v.status(of: task).isClosed ? .red : .secondary)
                }
                let subs = v.subtasks(of: task.id).count
                if subs > 0 { Label("\(subs)", systemImage: "arrow.turn.down.right").font(.caption2).foregroundStyle(.secondary) }
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color(.secondarySystemGroupedBackground)))
    }
}

/// The days a task covers on a calendar: start to due, or just its due day.
func calendarSpan(_ t: TaskView) -> (String, String)? {
    guard let due = t.due else { return nil }
    if let s = t.start, s <= due { return (s, due) }
    return (due, due)
}

struct CalendarListView: View {
    @Environment(SyncClient.self) private var client
    let listId: UInt64
    @State private var month = CalendarListView.firstOfMonth(Date())
    @State private var selected = dayString()
    @State private var newName = ""
    @FocusState private var adding: Bool

    static func firstOfMonth(_ d: Date) -> Date {
        let cal = Calendar(identifier: .gregorian)
        return cal.date(from: cal.dateComponents([.year, .month], from: d)) ?? d
    }

    var body: some View {
        let v = client.view
        let tasks = v.tasks(inList: listId)
        let cal = Calendar(identifier: .gregorian)
        let lead = cal.component(.weekday, from: month) - 1 // Sunday first, as on the Mac
        let days = cal.range(of: .day, in: .month, for: month)?.count ?? 30
        let cells: [String?] = Array(repeating: nil, count: lead) + (0..<days).map { dayString(cal.date(byAdding: .day, value: $0, to: month)!) }
        let onDay = { (day: String) in tasks.filter { t in calendarSpan(t).map { $0.0 <= day && day <= $0.1 } ?? false } }
        List {
            Section {
                VStack(spacing: 8) {
                    HStack {
                        Button { month = cal.date(byAdding: .month, value: -1, to: month)! } label: { Image(systemName: "chevron.left") }
                            .accessibilityLabel("Previous month").accessibilityIdentifier("prevMonth")
                        Spacer()
                        Text(month.formatted(.dateTime.year().month(.wide))).font(.headline).accessibilityIdentifier("monthTitle")
                        Spacer()
                        Button { month = cal.date(byAdding: .month, value: 1, to: month)! } label: { Image(systemName: "chevron.right") }
                            .accessibilityLabel("Next month").accessibilityIdentifier("nextMonth")
                    }
                    .buttonStyle(.borderless)
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 2), count: 7), spacing: 4) {
                        ForEach(["S", "M", "T", "W", "T", "F", "S"].indices, id: \.self) { i in
                            Text(["S", "M", "T", "W", "T", "F", "S"][i]).font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
                        }
                        ForEach(cells.indices, id: \.self) { i in
                            if let day = cells[i] {
                                DayCell(day: day, tasks: onDay(day), selected: day == selected, today: day == dayString())
                                    .onTapGesture { selected = day }
                                    .accessibilityAddTraits(.isButton)
                                    .accessibilityIdentifier("day-\(day)")
                            } else {
                                Color.clear.frame(height: 40)
                            }
                        }
                    }
                }
                .padding(.vertical, 4)
            }
            Section {
                ForEach(onDay(selected)) { t in
                    NavigationLink(value: Nav.Route.task(t.id)) { TaskRow(task: t) }
                        .accessibilityIdentifier("task-\(t.id)")
                }
                if client.canEdit {
                    TextField("Add task on this day", text: $newName)
                        .focused($adding)
                        .submitLabel(.done)
                        .onSubmit(add)
                        .accessibilityIdentifier("calendarNewTaskField")
                }
            } header: {
                Text(Fmt.due(selected)).accessibilityIdentifier("selectedDay")
            }
            let undated = tasks.filter { $0.due == nil }
            if !undated.isEmpty {
                Section("No date") {
                    ForEach(undated) { t in
                        NavigationLink(value: Nav.Route.task(t.id)) { TaskRow(task: t) }
                            .swipeActions(edge: .leading) {
                                if client.canEdit {
                                    Button("Due \(Fmt.due(selected))") {
                                        client.edit { d, me in Edits.setDue(&d, task: t.id, to: selected, me: me, ts: nowTs()) }
                                    }
                                    .tint(.orange)
                                }
                            }
                    }
                }
            }
        }
    }

    func add() {
        let name = newName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { adding = false; return }
        let day = selected
        if client.edit({ d, me in Edits.addTask(&d, list: listId, name: name, assignees: [me], due: day, me: me, id: client.newId(), ts: nowTs()) }) {
            newName = ""
            adding = true
        }
    }
}

private struct DayCell: View {
    @Environment(SyncClient.self) private var client
    let day: String
    let tasks: [TaskView]
    let selected: Bool
    let today: Bool

    var body: some View {
        VStack(spacing: 3) {
            Text(String(Int(day.suffix(2)) ?? 0))
                .font(.subheadline.weight(today ? .bold : .regular))
                .foregroundStyle(selected ? Color.white : today ? Color.accentColor : Color.primary)
                .frame(width: 30, height: 30)
                .background(Circle().fill(selected ? Color.accentColor : Color.clear))
            HStack(spacing: 2) {
                ForEach(tasks.prefix(3)) { t in
                    Circle().fill(Color(rgb: client.view.status(of: t).color)).frame(width: 5, height: 5)
                }
            }
            .frame(height: 5)
        }
        .frame(maxWidth: .infinity, minHeight: 40)
        .contentShape(Rectangle())
        .accessibilityLabel("\(day), \(tasks.count) tasks")
    }
}

// MARK: table

/// The list as a table: a row per task and a column per field, scrolled
/// sideways; tapping a row opens the task.
struct TableListView: View {
    @Environment(SyncClient.self) private var client
    let listId: UInt64

    var body: some View {
        let v = client.view
        let tasks = v.tasks(inList: listId)
        let fields = v.apps.customFields ? v.fields : []
        ScrollView([.horizontal, .vertical]) {
            Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 0) {
                GridRow {
                    ForEach(["Name", "Status", "Assignee", "Due date", "Priority", "Tags", "Estimate"] + fields.map(\.name), id: \.self) { h in
                        Text(h.uppercased()).font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
                    }
                }
                .padding(.vertical, 8)
                Divider()
                ForEach(tasks) { t in
                    let st = v.status(of: t)
                    GridRow {
                        NavigationLink(value: Nav.Route.task(t.id)) {
                            Text(t.name).lineLimit(1).frame(width: 200, alignment: .leading)
                        }
                        .accessibilityIdentifier("tableRow-\(t.id)")
                        HStack(spacing: 4) { StatusDot(status: st, size: 8); Text(st.name.uppercased()).font(.caption) }
                        Text(t.assignees.compactMap { v.member($0)?.name }.joined(separator: ", ")).font(.caption).lineLimit(1).frame(maxWidth: 140, alignment: .leading)
                        Text(Fmt.due(t) ?? "-").font(.caption)
                        Text(t.priority == 0 ? "-" : Edits.priorityLabels[t.priority]).font(.caption)
                        Text(t.tags.joined(separator: ", ")).font(.caption).lineLimit(1).frame(maxWidth: 120, alignment: .leading)
                        Text(t.estimateMin.map(Edits.fmtEstimate) ?? "-").font(.caption)
                        ForEach(fields) { f in Text(t.custom[f.id] ?? "-").font(.caption).lineLimit(1) }
                    }
                    .padding(.vertical, 10)
                    Divider()
                }
            }
            .padding(.horizontal)
        }
        .overlay { if tasks.isEmpty { EmptyNote(title: "No tasks yet", systemImage: "tablecells") } }
    }
}

// MARK: gantt and timeline

/// Tasks as bars on a day axis (src/app/timebar.rs): Gantt has a row per
/// task, Timeline a lane per assignee (a task with several shows in each).
/// Dragging a bar sideways moves its dates by whole days.
struct BarsView: View {
    @Environment(SyncClient.self) private var client
    let listId: UInt64
    let lanes: Bool
    @State private var dayWidth: CGFloat = 36
    @State private var drag: (id: UInt64, dx: CGFloat)?

    var body: some View {
        let v = client.view
        let tasks = v.tasks(inList: listId)
        let dated = tasks.filter { calendarSpan($0) != nil }
        let cal = Calendar(identifier: .gregorian)
        let today = cal.startOfDay(for: Date())
        let first = dated.compactMap { calendarSpan($0).flatMap { DayFormat.date(fromDay: $0.0) } }.min().map { min($0, today) } ?? today
        let start = cal.date(byAdding: .day, value: -3, to: first)!
        let last = dated.compactMap { calendarSpan($0).flatMap { DayFormat.date(fromDay: $0.1) } }.max().map { max($0, today) } ?? today
        let days = max(21, (cal.dateComponents([.day], from: start, to: last).day ?? 0) + 8)
        let rows: [(String, [TaskView])] = lanes
            ? (v.members.map { m in (m.name, dated.filter { $0.assignees.contains(m.id) }) } + [("No assignee", dated.filter { $0.assignees.isEmpty })]).filter { !$0.1.isEmpty }
            : dated.map { ($0.name, [$0]) }
        VStack(spacing: 0) {
            HStack {
                Text(lanes ? "By assignee" : "\(dated.count) of \(tasks.count) tasks have dates").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button { dayWidth = max(14, dayWidth / 1.4) } label: { Image(systemName: "minus.magnifyingglass") }.accessibilityLabel("Zoom out")
                Button { dayWidth = min(90, dayWidth * 1.4) } label: { Image(systemName: "plus.magnifyingglass") }.accessibilityLabel("Zoom in")
            }
            .padding(.horizontal).padding(.vertical, 6)
            ScrollView([.horizontal, .vertical]) {
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 0) {
                        ForEach(0..<days, id: \.self) { i in
                            let d = cal.date(byAdding: .day, value: i, to: start)!
                            VStack(spacing: 0) {
                                Text(d.formatted(.dateTime.day())).font(.caption2.weight(cal.isDate(d, inSameDayAs: today) ? .bold : .regular))
                                Text(d.formatted(.dateTime.weekday(.narrow))).font(.system(size: 9)).foregroundStyle(.tertiary)
                            }
                            .foregroundStyle(cal.isDate(d, inSameDayAs: today) ? Color.red : Color.secondary)
                            .frame(width: dayWidth)
                        }
                    }
                    .padding(.vertical, 4)
                    ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                        VStack(alignment: .leading, spacing: 2) {
                            if lanes { Text(row.0).font(.caption.weight(.semibold)).padding(.leading, 4).padding(.top, 6) }
                            ForEach(row.1) { t in bar(t, start: start, cal: cal) }
                        }
                    }
                }
                .background(alignment: .topLeading) {
                    // today's line
                    let x = CGFloat(cal.dateComponents([.day], from: start, to: today).day ?? 0) * dayWidth + dayWidth / 2
                    Rectangle().fill(Color.red.opacity(0.5)).frame(width: 1).offset(x: x)
                }
            }
            let undated = tasks.filter { calendarSpan($0) == nil }
            if !undated.isEmpty {
                Divider()
                ScrollView(.horizontal) {
                    HStack {
                        Text("No dates:").font(.caption).foregroundStyle(.secondary)
                        ForEach(undated) { t in
                            NavigationLink(value: Nav.Route.task(t.id)) { Text(t.name).font(.caption).lineLimit(1) }
                        }
                    }
                    .padding(.horizontal).padding(.vertical, 8)
                }
            }
        }
    }

    func bar(_ t: TaskView, start: Date, cal: Calendar) -> some View {
        let span = calendarSpan(t)!
        let s = DayFormat.date(fromDay: span.0) ?? start, e = DayFormat.date(fromDay: span.1) ?? start
        let x = CGFloat(cal.dateComponents([.day], from: start, to: s).day ?? 0) * dayWidth
        let w = CGFloat((cal.dateComponents([.day], from: s, to: e).day ?? 0) + 1) * dayWidth
        let st = client.view.status(of: t)
        let dx = drag?.id == t.id ? drag!.dx : 0
        return NavigationLink(value: Nav.Route.task(t.id)) {
            HStack(spacing: 4) {
                if t.milestone { Image(systemName: "diamond.fill").font(.system(size: 8)) }
                Text(t.name).font(.caption2.weight(.medium)).lineLimit(1)
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 6)
            .frame(width: max(w - 4, 18), height: 22, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 5).fill(Color(rgb: st.color)))
        }
        .buttonStyle(.plain)
        .offset(x: x + 2 + dx)
        .frame(height: 26, alignment: .leading)
        .highPriorityGesture(
            DragGesture(minimumDistance: 12)
                .onChanged { g in if client.canEdit { drag = (t.id, g.translation.width) } }
                .onEnded { g in
                    let days = Int((g.translation.width / dayWidth).rounded())
                    drag = nil
                    if days != 0 { client.edit { d, me in Edits.shiftDates(&d, task: t.id, days: days, me: me, ts: nowTs()) } }
                }
        )
        .accessibilityIdentifier("bar-\(t.id)")
    }
}
