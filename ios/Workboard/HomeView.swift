import SwiftUI
import WorkboardKit

/// Home, the first screen and the only root: quick find, the places
/// (Inbox, Today, Upcoming, Chat, Docs, Tools), and each space's lists.
/// Settings at the bottom left, the "+" for a new task, message or doc at
/// the bottom right. Everything else is pushed on top of it.
struct HomeView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @AppStorage(Place.upcoming.shownKey) private var showUpcoming = true
    @AppStorage(Place.docs.shownKey) private var showDocs = true
    @AppStorage(Place.tools.shownKey) private var showTools = true
    @State private var creating = false
    @State private var prompt: NamePrompt?

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        let spaces = v.visibleSpaces(me: me)
        List {
            Section {
                Button { nav.path.append(.search) } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "magnifyingglass").font(.system(size: 14, weight: .semibold))
                        Text("Quick find")
                    }
                    .font(.system(size: 15))
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 38)
                    .background(RoundedRectangle(cornerRadius: 11, style: .continuous).fill(Calm.card))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets())
                .accessibilityIdentifier("quickFind")
            }
            Section {
                ForEach(places, id: \.self) { p in
                    Button { nav.path.append(.place(p)) } label: { row(p).contentShape(Rectangle()) }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("place-\(p.rawValue)")
                }
            }
            if spaces.isEmpty {
                EmptyNote(title: "No spaces yet", systemImage: "square.grid.2x2").listRowBackground(Color.clear)
            }
            ForEach(spaces) { s in spaceSection(s) }
            Section {
                Button { prompt = .newSpace } label: {
                    Label("New space", systemImage: "plus")
                        .font(.system(size: 15))
                        .foregroundStyle(.secondary)
                }
                .disabled(!client.canEdit)
                .listRowBackground(Color.clear)
                .accessibilityIdentifier("newSpaceButton")
            }
        }
        .calmList()
        .contentMargins(.top, 4, for: .scrollContent)
        .contentMargins(.bottom, 84, for: .scrollContent)
        .navigationTitle("Home")
        .toolbar(.hidden, for: .navigationBar)
        .overlay(alignment: .bottom) { bottomBar }
        .sheet(isPresented: $creating) { NewItemSheet() }
        .namePrompt($prompt)
    }

    var places: [Place] {
        Place.allCases.filter { p in
            switch p {
            case .upcoming: return showUpcoming
            case .docs: return showDocs
            case .tools: return showTools
            default: return true
            }
        }
    }

    @ViewBuilder func row(_ p: Place) -> some View {
        let me = client.me ?? 0
        let v = client.view
        switch p {
        case .inbox:
            PlaceRow(place: p, count: client.inboxItems(me: me).filter { $0.tab == .primary }.count)
        case .today:
            let t = MyDay(client)
            PlaceRow(place: p, count: t.todo.count + t.evening.count, badge: t.overdue, badgeColor: .red, overdue: true)
        case .upcoming:
            PlaceRow(place: p, count: MyDay(client).later.count)
        case .chats:
            PlaceRow(place: p, badge: v.visibleChannels(me: me).reduce(0) { $0 + $1.unread(me: me) })
        case .docs, .tools:
            PlaceRow(place: p)
        }
    }

    /// A space: its name over a card of its lists, each with how far along it is.
    func spaceSection(_ s: SpaceView) -> some View {
        let v = client.view
        let lists = s.lists.map { ($0, nil as String?) } + s.folders.flatMap { f in f.lists.map { ($0, f.name as String?) } }
        return Section {
            ForEach(lists, id: \.0.id) { l, folder in
                Button { nav.path.append(.list(l.id)) } label: {
                    let tasks = v.tasks(inList: l.id).filter { $0.parent == nil }
                    let open = tasks.filter { !v.status(of: $0).isClosed }.count
                    HStack(spacing: 14) {
                        ProgressRing(done: tasks.isEmpty ? 0 : Double(tasks.count - open) / Double(tasks.count))
                            .frame(width: 26)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(l.name).font(.system(size: 17))
                            if let folder { Text(folder).font(.caption).foregroundStyle(.secondary) }
                        }
                        Spacer(minLength: 8)
                        if open > 0 { Text("\(open)").font(.system(size: 15)).foregroundStyle(.secondary).monospacedDigit() }
                    }
                    .frame(minHeight: 30)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("list-\(l.id)")
            }
            if lists.isEmpty {
                Text("No lists in this space").font(.subheadline).foregroundStyle(.secondary)
            }
        } header: {
            Button { nav.path.append(.space(s.id)) } label: {
                HStack(spacing: 8) {
                    Image(systemName: "shippingbox")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Color(rgb: s.color))
                    CalmHeader(text: s.privateOwner != nil ? Text("Personal") : Text(verbatim: s.name))
                        .foregroundStyle(Color(.secondaryLabel))
                    Image(systemName: "chevron.right").font(.system(size: 10, weight: .bold)).foregroundStyle(.tertiary)
                    Spacer()
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("space-\(s.id)")
            .contextMenu {
                if client.canEdit && s.privateOwner == nil {
                    Button { prompt = .renameSpace(s.id) } label: { Label("Rename", systemImage: "pencil") }
                    ColorMenu(title: "Color") { c in client.edit { d, _ in Edits.setSpaceColor(&d, space: s.id, to: c) } }
                    Button(role: .destructive) {
                        client.edit { d, _ in Edits.setSpaceArchived(&d, space: s.id, true) }
                    } label: { Label("Archive", systemImage: "archivebox") }
                }
            }
        }
    }

    var bottomBar: some View {
        HStack(alignment: .bottom) {
            SettingsButton()
            Spacer()
            FloatingAddButton(label: "Create new", id: "newItemButton") { creating = true }
        }
        .padding(.leading, 8)
        .padding(.trailing, 20)
        .padding(.bottom, 12)
        .padding(.top, 24)
        .background(alignment: .bottom) {
            LinearGradient(colors: [Calm.page.opacity(0), Calm.page], startPoint: .top, endPoint: .center)
                .ignoresSafeArea(edges: .bottom)
                .allowsHitTesting(false)
        }
    }
}

/// My open tasks by when they're due, for Home's counts, Today and Upcoming.
struct MyDay {
    /// overdue, then due today before the evening, by day and time
    let todo: [TaskView]
    /// due today from 6 PM
    let evening: [TaskView]
    /// how many of `todo` are overdue
    let overdue: Int
    /// due after today
    let later: [TaskView]
    let undated: [TaskView]

    @MainActor
    init(_ client: SyncClient) {
        let mine = client.view.myTasks(me: client.me ?? 0)
        let today = dayString()
        let now = timeString()
        // by day, then by time within it (a day without one last)
        let key = { (t: TaskView) in (t.due ?? "") + (t.dueTime ?? "99:99") }
        let late = mine.filter { $0.isOverdue(today: today, now: now) }.sorted { key($0) < key($1) }
        let due = mine.filter { $0.due == today && !$0.isOverdue(today: today, now: now) }.sorted { key($0) < key($1) }
        let isEvening = { (t: TaskView) in (t.dueTime ?? "") >= "18:00" }
        todo = late + due.filter { !isEvening($0) }
        evening = due.filter(isEvening)
        overdue = late.count
        later = mine.filter { ($0.due ?? "") > today }.sorted { key($0) < key($1) }
        undated = mine.filter { $0.due == nil }
    }
}

/// Today: today's meetings, what I have to do today (overdue first), and
/// what is due this evening.
struct TodayView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @State private var creating = false

    var body: some View {
        let day = MyDay(client)
        let meetings = (client.data.map { client.view.meetings($0) } ?? []).filter { $0.date == dayString() }
        List {
            CalmTitle(place: .today)
            if !meetings.isEmpty {
                Section {
                    ForEach(meetings) { m in MeetingLine(meeting: m) }
                } header: {
                    CalmHeader("Schedule today")
                }
            }
            if !day.todo.isEmpty {
                Section {
                    ForEach(day.todo) { t in CalmTaskRow(task: t, tags: .today) }
                } header: {
                    CalmHeader(text: Text("To do \(day.todo.count)"))
                }
            }
            if !day.evening.isEmpty {
                Section {
                    ForEach(day.evening) { t in CalmTaskRow(task: t, tags: .today) }
                } header: {
                    HStack(spacing: 6) {
                        Image(systemName: "moon.fill").font(.system(size: 11)).foregroundStyle(Calm.dynamic(light: 0x3A5BA8, dark: 0x7F9CE0))
                        CalmHeader("This evening")
                    }
                }
            }
            if day.todo.isEmpty && day.evening.isEmpty {
                EmptyNote(title: "Nothing due today", systemImage: "checkmark.circle", detail: "Tasks assigned to you that are due today show up here.")
                    .listRowBackground(Color.clear)
            }
        }
        .calmList()
        .calmBar(String(localized: "Today"))
        .floatingAdd("New task", id: "newTaskButton") { creating = true }
        .sheet(isPresented: $creating) { NewItemSheet(kind: .task, dueToday: true) }
    }
}

/// Upcoming: a card for each day with something due, then the tasks with no date.
struct UpcomingView: View {
    @Environment(SyncClient.self) private var client
    @State private var creating = false

    var body: some View {
        let day = MyDay(client)
        let today = dayString()
        let meetings = (client.data.map { client.view.meetings($0) } ?? []).filter { $0.date > today }
        let days = Array(Set(day.later.compactMap(\.due) + meetings.map(\.date))).sorted()
        List {
            CalmTitle(place: .upcoming)
            ForEach(days, id: \.self) { d in
                Section {
                    DayHeading(day: d)
                    ForEach(meetings.filter { $0.date == d }) { m in MeetingLine(meeting: m) }
                    ForEach(day.later.filter { $0.due == d }) { t in CalmTaskRow(task: t, tags: .timeOnly) }
                }
            }
            if !day.undated.isEmpty {
                Section {
                    ForEach(day.undated) { t in CalmTaskRow(task: t) }
                } header: {
                    CalmHeader("No date")
                }
            }
            if days.isEmpty && day.undated.isEmpty {
                EmptyNote(title: "Nothing coming up", systemImage: "calendar", detail: "Tasks assigned to you with a later due date show up here.")
                    .listRowBackground(Color.clear)
            }
        }
        .calmList()
        .calmBar(String(localized: "Upcoming"))
        .floatingAdd("New task", id: "newTaskButton") { creating = true }
        .sheet(isPresented: $creating) { NewItemSheet(kind: .task) }
    }
}

/// "22  Tomorrow" at the top of a day's card.
private struct DayHeading: View {
    let day: String

    var body: some View {
        let date = DayFormat.date(fromDay: day)
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(verbatim: date.map { "\(Calendar.current.component(.day, from: $0))" } ?? day)
                .font(.system(size: 24, weight: .bold))
            Text(verbatim: label(date)).font(.system(size: 14, weight: .semibold)).foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }

    func label(_ d: Date?) -> String {
        guard let d else { return "" }
        if day == dayString(Date().addingTimeInterval(86400)) { return String(localized: "Tomorrow") }
        let days = Calendar.current.dateComponents([.day], from: Calendar.current.startOfDay(for: Date()), to: d).day ?? 99
        // the day's number is already there: a weekday, and the month once it's further off
        let weekday = d.formatted(.dateTime.weekday(.wide))
        return days < 7 ? weekday : "\(d.formatted(.dateTime.month(.abbreviated))) · \(weekday)"
    }
}

/// A meeting on a day: its time in green, then its title.
struct MeetingLine: View {
    let meeting: MeetingView

    var body: some View {
        NavigationLink(value: Nav.Route.meeting(meeting.id)) {
            HStack(spacing: 12) {
                Text(verbatim: meeting.time.isEmpty ? "" : Fmt.clock(meeting.time))
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(Calm.dynamic(light: 0x1F7A4D, dark: 0x5CC98F))
                    .frame(minWidth: 44, alignment: .leading)
                Text(meeting.title).font(.system(size: 15)).lineLimit(1)
            }
        }
        .accessibilityIdentifier("meeting-\(meeting.id)")
    }
}

struct TaskRow: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    var showList = false

    var body: some View {
        let v = client.view
        let status = v.status(of: task)
        HStack(spacing: 12) {
            StatusDot(status: status, size: 14)
            VStack(alignment: .leading, spacing: 3) {
                Text(task.name)
                    .strikethrough(status.isClosed)
                    .foregroundStyle(status.isClosed ? .secondary : .primary)
                    .lineLimit(2)
                HStack(spacing: 6) {
                    if showList, let l = v.list(task.listId) {
                        Text(l.name)
                    }
                    if let due = Fmt.due(task) {
                        Label(due, systemImage: "calendar")
                            .foregroundStyle(task.isOverdue(today: dayString(), now: timeString()) && !status.isClosed ? .red : .secondary)
                    }
                    if task.priority == 1 {
                        Label("Urgent", systemImage: "flag.fill").foregroundStyle(.red)
                    } else if task.priority == 2 {
                        Label("High", systemImage: "flag.fill").foregroundStyle(.orange)
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                .labelStyle(.titleAndIcon)
            }
            Spacer(minLength: 0)
            HStack(spacing: -6) {
                ForEach(task.assignees.prefix(3), id: \.self) { id in
                    Avatar(member: v.member(id), size: 22)
                        .overlay(Circle().stroke(Color(.systemBackground), lineWidth: 1.5))
                }
            }
        }
        .padding(.vertical, 2)
    }
}
