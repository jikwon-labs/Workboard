import SwiftUI
import UserNotifications
import WorkboardKit

// The More tab: the Mac rail's other apps (Goals, Meetings, and the rest as
// they are added), each a page of its own.

struct MoreView: View {
    static let apps: [(id: String, title: String, icon: String)] = [
        ("goals", "Goals", "target"),
        ("meetings", "Meetings", "person.3"),
        ("forms", "Forms", "list.clipboard"),
        ("whiteboards", "Whiteboards", "scribble"),
        ("dashboards", "Dashboards", "chart.bar"),
        ("timesheets", "Timesheets", "clock"),
        ("templates", "Templates", "square.on.square"),
        ("teams", "Teams", "person.3.sequence"),
        ("posts", "Posts", "megaphone"),
        ("saved", "Saved", "bookmark"),
        ("reminders", "Reminders", "alarm"),
        ("notepad", "Notepad", "note.text"),
    ]

    var body: some View {
        List {
            ForEach(Self.apps, id: \.id) { a in
                NavigationLink(value: Nav.Route.app(a.id)) { Label(L(a.title), systemImage: a.icon) }
                    .accessibilityIdentifier("app-\(a.id)")
            }
        }
        .navigationTitle("More")
        .toolbar { ToolbarItem(placement: .topBarTrailing) { SettingsButton() } }
    }
}

/// One app's page.
struct AppPage: View {
    let name: String

    var body: some View {
        switch name {
        case "goals": GoalsView()
        case "meetings": MeetingsView()
        case "forms": FormsView()
        case "whiteboards": WhiteboardsView()
        case "dashboards": DashboardsView()
        case "timesheets": TimesheetsView()
        case "templates": TemplatesView()
        case "teams": TeamsView()
        case "posts": MessageListView(title: "Posts", posts: true)
        case "saved": MessageListView(title: "Saved", posts: false)
        case "reminders": RemindersView()
        case "notepad": NotepadView()
        default: EmptyNote(title: "Not on the iPhone yet", systemImage: "square.grid.2x2")
        }
    }
}

// MARK: goals

struct GoalsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav

    var body: some View {
        let v = client.view
        let goals = client.data.map { v.goals($0) } ?? []
        List {
            if goals.isEmpty {
                EmptyNote(title: "Make your goals a reality", systemImage: "target", detail: "Set a goal and track it with numbers, yes/no targets or tasks.")
                    .listRowBackground(Color.clear)
            }
            ForEach(goals) { g in
                NavigationLink(value: Nav.Route.goal(g.id)) {
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Circle().fill(Color(rgb: g.color)).frame(width: 10, height: 10)
                            Text(g.name).fontWeight(.medium)
                            Spacer()
                            Text("\(Int((v.progress(of: g) * 100).rounded()))%").monospacedDigit().foregroundStyle(.secondary)
                        }
                        ProgressView(value: v.progress(of: g)).tint(Color(rgb: g.color))
                        if let due = g.due { Text("Due \(Fmt.due(due))").font(.caption).foregroundStyle(.secondary) }
                    }
                }
                .accessibilityIdentifier("goal-\(g.id)")
            }
        }
        .navigationTitle("Goals")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New goal", id: "newGoalButton") {
                    let id = client.newId()
                    if client.edit({ d, me in Edits.addGoal(&d, name: "New goal", me: me, id: id) }) { nav.more.append(.goal(id)) }
                }
            }
        }
    }
}

struct GoalDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let goalId: UInt64
    @State private var name = ""
    @State private var addingTarget = false
    @State private var pickingTasks: Int?
    @FocusState private var nameFocused: Bool

    var body: some View {
        let v = client.view
        if let data = client.data, let g = v.goals(data).first(where: { $0.id == goalId }) {
            let can = client.canEdit
            Form {
                Section {
                    TextField("Goal name", text: $name)
                        .font(.title3.weight(.semibold))
                        .focused($nameFocused)
                        .onSubmit { client.edit { d, _ in Edits.renameGoal(&d, goal: g.id, to: name) } }
                        .accessibilityIdentifier("goalName")
                    LabeledContent("Progress") {
                        Text("\(Int((v.progress(of: g) * 100).rounded()))%").monospacedDigit().accessibilityIdentifier("goalProgress")
                    }
                    if let due = g.due {
                        DatePicker("Due", selection: Binding(get: { DayFormat.date(fromDay: due) ?? Date() }, set: { new in client.edit { d, _ in Edits.setGoalDue(&d, goal: g.id, to: dayString(new)) } }), displayedComponents: .date)
                        Button("Remove due date") { client.edit { d, _ in Edits.setGoalDue(&d, goal: g.id, to: nil) } }
                    } else {
                        Button("Set a due date") { client.edit { d, _ in Edits.setGoalDue(&d, goal: g.id, to: dayString(Date().addingTimeInterval(86400 * 30))) } }
                            .disabled(!can)
                    }
                }
                Section("Targets") {
                    ForEach(Array(g.targets.enumerated()), id: \.offset) { i, t in
                        TargetRow(goal: g.id, index: i, target: t, pickTasks: { pickingTasks = i })
                            .disabled(!can)
                            .deleteSwipe(can) { client.edit { d, _ in Edits.removeTarget(&d, goal: g.id, index: i) } }
                    }
                    Button { addingTarget = true } label: { Label("Add target", systemImage: "plus") }
                        .disabled(!can)
                        .accessibilityIdentifier("addTargetButton")
                        .sheet(isPresented: $addingTarget) { NewTargetView(goal: g.id) }
                }
            }
            .navigationTitle("Goal")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button(role: .destructive) {
                            if client.edit({ d, _ in Edits.deleteGoal(&d, goal: g.id) }) { dismiss() }
                        } label: { Label("Delete goal", systemImage: "trash") }
                    } label: { Image(systemName: "ellipsis.circle") }
                        .disabled(!can)
                        .accessibilityLabel("More")
                        .accessibilityIdentifier("goalMoreMenu")
                }
            }
            .onAppear { name = g.name }
            .onChange(of: g.name) { if !nameFocused { name = g.name } }
            .onChange(of: nameFocused) { _, now in if !now { client.edit { d, _ in Edits.renameGoal(&d, goal: g.id, to: name) } } }
            .sheet(item: Binding(get: { pickingTasks.map { PickIndex(i: $0) } }, set: { pickingTasks = $0?.i })) { p in
                if case .tasks(let ids) = g.targets[safe: p.i]?.kind {
                    TasksTargetPicker(goal: g.id, index: p.i, name: g.targets[p.i].name, chosen: ids)
                }
            }
        } else {
            EmptyNote(title: "This goal is gone", systemImage: "target")
        }
    }

    struct PickIndex: Identifiable { let i: Int; var id: Int { i } }
}

extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}

private struct TargetRow: View {
    @Environment(SyncClient.self) private var client
    let goal: UInt64
    let index: Int
    let target: GoalTargetView
    let pickTasks: () -> Void
    @State private var current = ""

    var body: some View {
        switch target.kind {
        case .number(let s, let t, let c, let unit):
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(target.name)
                    Spacer()
                    TextField("0", text: $current)
                        .keyboardType(.decimalPad)
                        .multilineTextAlignment(.trailing)
                        .frame(width: 80)
                        .onSubmit { save(s, t, unit) }
                        .accessibilityIdentifier("targetCurrent-\(index)")
                    Text("/ \(Self.num(t)) \(unit)").foregroundStyle(.secondary)
                }
                ProgressView(value: abs(t - s) < .ulpOfOne ? 1 : min(1, max(0, (c - s) / (t - s))))
            }
            .onAppear { current = Self.num(c) }
            .onChange(of: c) { current = Self.num(c) }
            .toolbar {
                ToolbarItemGroup(placement: .keyboard) {
                    Spacer()
                    Button("Save") { save(s, t, unit) }
                }
            }
        case .trueFalse(let done):
            Toggle(target.name, isOn: Binding(get: { done }, set: { on in
                client.edit { d, _ in Edits.setTarget(&d, goal: goal, index: index, to: GoalTargetView(name: target.name, kind: .trueFalse(done: on))) }
            }))
            .accessibilityIdentifier("targetDone-\(index)")
        case .tasks(let ids):
            Button(action: pickTasks) {
                let v = client.view
                let live = ids.compactMap { id in v.tasks.first { $0.id == id } }
                LabeledContent(target.name, value: "\(live.filter { v.status(of: $0).isClosed }.count) of \(live.count) done")
            }
            .foregroundStyle(.primary)
        }
    }

    func save(_ s: Double, _ t: Double, _ unit: String) {
        guard let c = Double(current.replacingOccurrences(of: ",", with: ".")) else { return }
        client.edit { d, _ in Edits.setTarget(&d, goal: goal, index: index, to: GoalTargetView(name: target.name, kind: .number(start: s, target: t, current: c, unit: unit))) }
    }

    static func num(_ d: Double) -> String { d == d.rounded() ? String(Int(d)) : String(d) }
}

private struct NewTargetView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let goal: UInt64
    @State private var name = ""
    @State private var kind = 0
    @State private var start = "0"
    @State private var target = "100"
    @State private var unit = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Target name", text: $name).accessibilityIdentifier("targetName")
                Picker("Kind", selection: $kind) {
                    Text("Number").tag(0)
                    Text("Yes or no").tag(1)
                    Text("Tasks").tag(2)
                }
                .pickerStyle(.segmented)
                if kind == 0 {
                    LabeledContent("Start") { TextField("0", text: $start).keyboardType(.decimalPad).multilineTextAlignment(.trailing) }
                    LabeledContent("Target") { TextField("100", text: $target).keyboardType(.decimalPad).multilineTextAlignment(.trailing) }
                    LabeledContent("Unit") { TextField("e.g. users", text: $unit).multilineTextAlignment(.trailing) }
                }
            }
            .navigationTitle("New target")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        let k: GoalTargetView.Kind
                        switch kind {
                        case 0:
                            let s = Double(start) ?? 0
                            k = .number(start: s, target: Double(target) ?? 100, current: s, unit: unit)
                        case 1: k = .trueFalse(done: false)
                        default: k = .tasks(ids: [])
                        }
                        client.edit { d, _ in Edits.addTarget(&d, goal: goal, GoalTargetView(name: name, kind: k)) }
                        dismiss()
                    }
                    .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("targetAdd")
                }
            }
        }
    }
}

private struct TasksTargetPicker: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let goal: UInt64
    let index: Int
    let name: String
    @State var chosen: [UInt64]
    @State private var query = ""

    var body: some View {
        NavigationStack {
            List(client.view.tasks.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) }) { t in
                Button {
                    if let i = chosen.firstIndex(of: t.id) { chosen.remove(at: i) } else { chosen.append(t.id) }
                } label: {
                    HStack {
                        TaskRow(task: t)
                        Spacer()
                        if chosen.contains(t.id) { Image(systemName: "checkmark").foregroundStyle(Color.accentColor) }
                    }
                }
                .foregroundStyle(.primary)
            }
            .searchable(text: $query)
            .navigationTitle(name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        client.edit { d, _ in Edits.setTarget(&d, goal: goal, index: index, to: GoalTargetView(name: name, kind: .tasks(ids: chosen))) }
                        dismiss()
                    }
                }
            }
        }
    }
}

// MARK: meetings

struct MeetingsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav

    var body: some View {
        let ms = client.data.map { client.view.meetings($0) } ?? []
        let today = dayString()
        List {
            if ms.isEmpty {
                EmptyNote(title: "No meetings yet", systemImage: "person.3").listRowBackground(Color.clear)
            }
            let upcoming = ms.filter { $0.date >= today }
            let past = ms.filter { $0.date < today }.reversed()
            if !upcoming.isEmpty { Section("Upcoming") { ForEach(upcoming) { row($0) } } }
            if !past.isEmpty { Section("Past") { ForEach(Array(past)) { row($0) } } }
        }
        .navigationTitle("Meetings")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New meeting", id: "newMeetingButton") {
                    let id = client.newId()
                    if client.edit({ d, me in Edits.addMeeting(&d, title: "New meeting", date: dayString(), me: me, id: id) }) { nav.more.append(.meeting(id)) }
                }
            }
        }
    }

    func row(_ m: MeetingView) -> some View {
        NavigationLink(value: Nav.Route.meeting(m.id)) {
            VStack(alignment: .leading, spacing: 2) {
                Text(m.title).fontWeight(.medium)
                (Text("\(Fmt.due(m.date)) \(Fmt.clock(m.time)) · ") + Text(m.attendees.count == 1 ? "1 person" : "\(m.attendees.count) people"))
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .accessibilityIdentifier("meeting-\(m.id)")
    }
}

struct MeetingDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let meetingId: UInt64
    @State private var title = ""
    @State private var agenda = ""
    @State private var notes = ""
    @State private var choosingList = false
    @State private var made: String?
    @FocusState private var focus: Field?

    enum Field { case title, agenda, notes }

    var body: some View {
        let v = client.view
        if let data = client.data, let m = v.meetings(data).first(where: { $0.id == meetingId }) {
            let can = client.canEdit
            Form {
                Section {
                    TextField("Title", text: $title).font(.title3.weight(.semibold)).focused($focus, equals: .title).accessibilityIdentifier("meetingTitle")
                    DatePicker("When", selection: Binding(get: { Self.when(m) }, set: { d in
                        client.edit { data, _ in Edits.updateMeeting(&data, meeting: m.id, date: dayString(d), time: timeString(d)) }
                    }))
                }
                .disabled(!can)
                Section("Attendees") {
                    ForEach(v.activeMembers) { p in
                        MemberToggleRow(member: p, on: m.attendees.contains(p.id)) {
                            client.edit { d, _ in Edits.toggleAttendee(&d, meeting: m.id, member: p.id) }
                        }
                        .disabled(!can)
                    }
                }
                Section("Agenda") {
                    TextField("What will you discuss?", text: $agenda, axis: .vertical).lineLimit(3...).focused($focus, equals: .agenda).disabled(!can)
                }
                Section {
                    TextField("Write notes", text: $notes, axis: .vertical).lineLimit(6...).focused($focus, equals: .notes).disabled(!can)
                        .accessibilityIdentifier("meetingNotes")
                    let items = MeetingView.items(in: notes)
                    if !items.isEmpty {
                        Button(items.count == 1 ? "Create 1 task from action items" : "Create \(items.count) tasks from action items") {
                            save(m)
                            focus = nil
                            choosingList = true
                        }
                        .disabled(!can)
                        .accessibilityIdentifier("meetingMakeTasks")
                    }
                    if let made { Text(made).font(.footnote).foregroundStyle(.secondary) }
                } header: {
                    Text("Notes")
                } footer: {
                    Text("Lines starting with [ ] become action items.")
                }
            }
            .navigationTitle("Meeting")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if focus != nil {
                    ToolbarItem(placement: .topBarTrailing) { Button("Done") { focus = nil } }
                } else {
                    ToolbarItem(placement: .topBarTrailing) {
                        Menu {
                            Button(role: .destructive) {
                                if client.edit({ d, _ in Edits.deleteMeeting(&d, meeting: m.id) }) { dismiss() }
                            } label: { Label("Delete meeting", systemImage: "trash") }
                        } label: { Image(systemName: "ellipsis.circle") }
                            .disabled(!can)
                    }
                }
            }
            .sheet(isPresented: $choosingList) {
                ListPickerSheet(current: 0) { list in
                    var n = 0
                    client.edit { d, me in n = Edits.meetingActionsToTasks(&d, meeting: m.id, list: list, me: me, ts: nowTs()) { client.newId() } }
                    // what is on screen is what was saved, now ticked off
                    if n > 0 { notes = notes.replacingOccurrences(of: "[ ]", with: "[x]") }
                    let where_ = client.view.list(list)?.name ?? String(localized: "the list")
                    made = n == 1 ? String(localized: "Made 1 task in \(where_).") : String(localized: "Made \(n) tasks in \(where_).")
                }
            }
            .onAppear { title = m.title; agenda = m.agenda; notes = m.notes }
            .onChange(of: m.notes) { if focus != .notes { notes = m.notes } }
            .onChange(of: m.agenda) { if focus != .agenda { agenda = m.agenda } }
            .onChange(of: m.title) { if focus != .title { title = m.title } }
            .onChange(of: focus) { _, _ in save(m) }
            .onDisappear { save(m) }
        } else {
            EmptyNote(title: "This meeting is gone", systemImage: "person.3")
        }
    }

    func save(_ m: MeetingView) {
        guard title != m.title || agenda != m.agenda || notes != m.notes else { return }
        client.edit { d, _ in
            Edits.updateMeeting(&d, meeting: m.id, title: title != m.title ? title : nil, agenda: agenda != m.agenda ? agenda : nil, notes: notes != m.notes ? notes : nil)
        }
    }

    static func when(_ m: MeetingView) -> Date {
        DayFormat.date(day: m.date, time: m.time) ?? Date()
    }
}

extension MeetingView {
    /// Action items in notes being typed (before they are saved).
    static func items(in notes: String) -> [String] {
        notes.components(separatedBy: "\n").compactMap { l in
            let t = l.trimmingCharacters(in: .whitespaces)
            guard t.hasPrefix("[ ]") else { return nil }
            let s = t.dropFirst(3).trimmingCharacters(in: .whitespaces)
            return s.isEmpty ? nil : s
        }
    }
}

// MARK: reminders and notepad (kept on this iPhone, like the Mac's)

enum ReminderAlerts {
    /// A local notification at the reminder's time (replaced when it changes).
    static func schedule(id: UInt64, text: String, at ts: Int64) {
        let c = UNMutableNotificationContent()
        c.title = "Reminder"
        c.body = text
        c.sound = .default
        let when = Date(timeIntervalSince1970: TimeInterval(ts))
        guard when > Date() else { return }
        let parts = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: when)
        let req = UNNotificationRequest(identifier: "reminder-\(id)", content: c, trigger: UNCalendarNotificationTrigger(dateMatching: parts, repeats: false))
        UNUserNotificationCenter.current().add(req)
    }

    static func cancel(_ id: UInt64) {
        UNUserNotificationCenter.current().removePendingNotificationRequests(withIdentifiers: ["reminder-\(id)"])
    }
}

struct RemindersView: View {
    @Environment(SyncClient.self) private var client
    @State private var adding = false

    var body: some View {
        let me = client.me ?? 0
        let rs = client.data.map { client.view.reminders($0, me: me) } ?? []
        let now = nowTs()
        List {
            if rs.isEmpty {
                EmptyNote(title: "No reminders", systemImage: "alarm", detail: "Reminders are kept on this iPhone.").listRowBackground(Color.clear)
            }
            let open = rs.filter { !$0.done }
            let done = rs.filter(\.done)
            if !open.isEmpty { Section("To do") { ForEach(open) { row($0, overdue: $0.dueTs <= now) } } }
            if !done.isEmpty { Section("Done") { ForEach(done) { row($0, overdue: false) } } }
        }
        .navigationTitle("Reminders")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New reminder", id: "newReminderButton", needsEdit: false) { adding = true }
            }
        }
        .sheet(isPresented: $adding) { NewReminderView(task: nil) }
    }

    func row(_ r: ReminderView, overdue: Bool) -> some View {
        HStack(spacing: 10) {
            Button {
                client.editLocal { d in Edits.setReminderDone(&d, reminder: r.id, !r.done) }
                if !r.done { ReminderAlerts.cancel(r.id) } else { ReminderAlerts.schedule(id: r.id, text: r.text, at: r.dueTs) }
            } label: {
                Image(systemName: r.done ? "checkmark.circle.fill" : "circle").foregroundStyle(r.done ? .green : .secondary)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(r.done ? "Mark not done" : "Mark done")
            .accessibilityIdentifier("reminderDone-\(r.id)")
            VStack(alignment: .leading, spacing: 2) {
                Text(r.text).strikethrough(r.done)
                Text(Fmt.time(r.dueTs)).font(.caption).foregroundStyle(overdue ? .red : .secondary)
                if let t = r.task, let task = client.view.tasks.first(where: { $0.id == t }) {
                    NavigationLink(value: Nav.Route.task(t)) { Label(task.name, systemImage: "checklist").font(.caption) }
                }
            }
        }
        .swipeActions {
            Button("Delete", role: .destructive) {
                ReminderAlerts.cancel(r.id)
                client.editLocal { d in Edits.deleteReminder(&d, reminder: r.id) }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("reminder-\(r.id)")
    }
}

/// A new reminder, about a task when made from one ("Remind me").
struct NewReminderView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @Environment(\.dismiss) private var dismiss
    let task: TaskView?
    @State private var text = ""
    @State private var when = Date().addingTimeInterval(3600)

    var body: some View {
        NavigationStack {
            Form {
                TextField("Remind me to...", text: $text).accessibilityIdentifier("reminderText")
                DatePicker("When", selection: $when)
            }
            .navigationTitle("New reminder")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        let id = client.newId(), ts = Int64(when.timeIntervalSince1970), me = client.me ?? 0, t = text
                        client.editLocal { d in Edits.addReminder(&d, text: t, dueTs: ts, task: task?.id, me: me, id: id) }
                        Task {
                            await notifier.requestPermission()
                            ReminderAlerts.schedule(id: id, text: t, at: ts)
                        }
                        dismiss()
                    }
                    .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("reminderAdd")
                }
            }
            .onAppear { if let task, text.isEmpty { text = task.name } }
        }
    }
}

struct NotepadView: View {
    @Environment(SyncClient.self) private var client
    @State private var text = ""
    @FocusState private var focused: Bool
    @Environment(\.scenePhase) private var scenePhase
    /// The save a second after the last keystroke.
    @State private var pending: Task<Void, Never>?

    var body: some View {
        TextEditor(text: $text)
            .focused($focused)
            .padding(.horizontal, 12)
            .navigationTitle("Notepad")
            .navigationBarTitleDisplayMode(.inline)
            .accessibilityIdentifier("notepad")
            .onAppear { text = client.data?.at("extra.notepad")?.string ?? "" }
            // saved a moment after typing stops, not on every key (each save
            // copies the whole workspace); leaving the field, the screen or
            // the app saves right away
            .onChange(of: text) { _, _ in saveSoon() }
            .onChange(of: focused) { _, f in if !f { save() } }
            // the app's own flush may run before this one
            .onChange(of: scenePhase) { _, p in if p != .active { save(); client.flushSnapshot() } }
            .onDisappear { save() }
            .toolbar { if focused { ToolbarItem(placement: .topBarTrailing) { Button("Done") { focused = false } } } }
    }

    func saveSoon() {
        pending?.cancel()
        pending = Task {
            try? await Task.sleep(nanoseconds: 1_000_000_000)
            guard !Task.isCancelled else { return }
            save()
        }
    }

    func save() {
        pending?.cancel()
        pending = nil
        let t = text
        client.editLocal { d in Edits.setNotepad(&d, to: t) }
    }
}

// MARK: timesheets

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

// MARK: dashboards

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

// MARK: forms

struct FormsView: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        let forms = client.data.map { client.view.forms($0) } ?? []
        List {
            if forms.isEmpty {
                EmptyNote(title: "No forms yet", systemImage: "list.clipboard", detail: "A form's answers become a task in its list.").listRowBackground(Color.clear)
            }
            ForEach(forms) { f in
                NavigationLink { FormDetailView(formId: f.id) } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(f.name).fontWeight(.medium)
                        Text("Answers go to \(client.view.list(f.listId)?.name ?? "no list")").font(.caption).foregroundStyle(.secondary)
                    }
                }
                .accessibilityIdentifier("form-\(f.id)")
            }
        }
        .navigationTitle("Forms")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New form", id: "newFormButton") {
                    let id = client.newId()
                    let list = client.view.visibleSpaces(me: client.me ?? 0).flatMap(\.allListIds).first ?? 0
                    client.edit { d, me in Edits.addForm(&d, list: list, me: me, id: id) }
                }
            }
        }
    }
}

struct FormDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let formId: UInt64
    @State private var editing = false
    @State private var answers: [Int: String] = [:]
    @State private var message: String?
    @State private var failed = false

    var body: some View {
        if let data = client.data, let f = client.view.forms(data).first(where: { $0.id == formId }) {
            Group {
                if editing { FormEditor(form: f) } else { fill(f) }
            }
            .navigationTitle(f.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(editing ? "Done" : "Edit") { editing.toggle() }
                        .disabled(!client.canEdit)
                        .accessibilityIdentifier("formEditToggle")
                }
            }
        } else {
            EmptyNote(title: "This form is gone", systemImage: "list.clipboard")
        }
    }

    func fill(_ f: FormView) -> some View {
        Form {
            if !f.description.isEmpty { Section { Text(f.description).foregroundStyle(.secondary) } }
            ForEach(Array(f.fields.enumerated()), id: \.offset) { i, fld in
                Section {
                    answer(i, fld)
                } header: {
                    Text(fld.label + (fld.required ? " *" : ""))
                }
            }
            Section {
                Button("Submit") {
                    let id = client.newId()
                    var result: Result<UInt64, Edits.FormError> = .failure(.noList)
                    client.edit { d, me in result = Edits.submitForm(&d, form: f.id, answers: answers, me: me, id: id, ts: nowTs()) }
                    switch result {
                    case .success:
                        answers = [:]
                        failed = false
                        message = "Submitted. A task was created in \(client.view.list(f.listId)?.name ?? "its list")."
                    case .failure(.answer(let why)):
                        failed = true
                        message = why
                    case .failure(.noList):
                        failed = true
                        message = "This form has no List for its answers. Choose one under Edit."
                    }
                }
                .disabled(!client.canEdit)
                .accessibilityIdentifier("formSubmit")
                if let message {
                    Text(message).font(.footnote).foregroundStyle(failed ? .red : .green).accessibilityIdentifier("formMessage")
                }
            }
        }
    }

    @ViewBuilder func answer(_ i: Int, _ fld: FormFieldView) -> some View {
        let text = Binding(get: { answers[i] ?? "" }, set: { answers[i] = $0 })
        switch fld.map {
        case "priority":
            Picker("Priority", selection: text) {
                Text("Choose").tag("")
                ForEach(Edits.priorityLabels, id: \.self) { Text(L($0)).tag($0) }
            }
        case "assignee":
            Picker("Person", selection: text) {
                Text("Choose").tag("")
                ForEach(client.view.activeMembers) { Text($0.name).tag(String($0.id)) }
            }
        case "dropdown":
            Picker("Choice", selection: text) {
                Text("Choose").tag("")
                ForEach(fld.options, id: \.self) { Text($0).tag($0) }
            }
        case "checkbox":
            Toggle(fld.label, isOn: Binding(get: { answers[i] == "Yes" }, set: { answers[i] = $0 ? "Yes" : "" }))
        case "due":
            DatePicker("Date", selection: Binding(get: { DayFormat.date(fromDay: answers[i] ?? "") ?? Date() }, set: { answers[i] = dayString($0) }), displayedComponents: .date)
        case "description":
            TextField("Answer", text: text, axis: .vertical).lineLimit(3...).accessibilityIdentifier("formAnswer-\(i)")
        default:
            TextField("Answer", text: text)
                .keyboardType(fld.map == "number" ? .decimalPad : fld.map == "email" ? .emailAddress : .default)
                .textInputAutocapitalization(fld.map == "email" ? .never : .sentences)
                .accessibilityIdentifier("formAnswer-\(i)")
        }
    }
}

private struct FormEditor: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let form: FormView
    @State private var name = ""
    @State private var desc = ""
    @State private var picking = false
    @State private var newLabel = ""
    @State private var newKind = "text"

    var body: some View {
        Form {
            Section("Form") {
                TextField("Name", text: $name).onSubmit { client.edit { d, _ in Edits.updateForm(&d, form: form.id, name: name) } }
                    .accessibilityIdentifier("formName")
                TextField("Description", text: $desc, axis: .vertical).onSubmit { client.edit { d, _ in Edits.updateForm(&d, form: form.id, description: desc) } }
                Button { picking = true } label: {
                    LabeledContent("Answers go to", value: client.view.list(form.listId)?.name ?? "Choose a list")
                }
                .foregroundStyle(.primary)
            }
            Section("Questions") {
                ForEach(Array(form.fields.enumerated()), id: \.offset) { i, f in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(f.label)
                        HStack {
                            Text(L(FormFieldView.kinds.first { $0.map == f.map }?.label ?? f.map)).font(.caption).foregroundStyle(.secondary)
                            Spacer()
                            Toggle("Required", isOn: Binding(get: { f.required }, set: { on in
                                client.edit { d, _ in Edits.setFormField(&d, form: form.id, index: i, to: FormFieldView(label: f.label, map: f.map, required: on, options: f.options)) }
                            }))
                            .labelsHidden()
                            Text("Required").font(.caption).foregroundStyle(.secondary)
                        }
                        if f.map == "dropdown" {
                            Text(f.options.isEmpty ? "No choices yet" : f.options.joined(separator: ", ")).font(.caption).foregroundStyle(.tertiary)
                        }
                    }
                    .swipeActions {
                        Button("Delete", role: .destructive) { client.edit { d, _ in Edits.removeFormField(&d, form: form.id, index: i) } }
                    }
                }
                TextField("New question", text: $newLabel).accessibilityIdentifier("newQuestion")
                Picker("Kind", selection: $newKind) {
                    ForEach(FormFieldView.kinds, id: \.map) { Text(L($0.label)).tag($0.map) }
                }
                Button("Add question") {
                    // a dropdown's choices are written after a colon: "Size: S, M, L"
                    var label = newLabel, options: [String] = []
                    if newKind == "dropdown", let c = newLabel.firstIndex(of: ":") {
                        label = String(newLabel[..<c])
                        options = newLabel[newLabel.index(after: c)...].split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
                    }
                    client.edit { d, _ in Edits.setFormField(&d, form: form.id, index: nil, to: FormFieldView(label: label, map: newKind, options: options)) }
                    newLabel = ""
                }
                .disabled(newLabel.trimmingCharacters(in: .whitespaces).isEmpty)
                .accessibilityIdentifier("addQuestion")
            }
            Section {
                Button("Delete form", role: .destructive) {
                    if client.edit({ d, _ in Edits.deleteForm(&d, form: form.id) }) { dismiss() }
                }
            }
        }
        .onAppear { name = form.name; desc = form.description }
        .onDisappear {
            if name != form.name || desc != form.description {
                client.edit { d, _ in Edits.updateForm(&d, form: form.id, name: name, description: desc) }
            }
        }
        .sheet(isPresented: $picking) {
            ListPickerSheet(current: form.listId) { l in client.edit { d, _ in Edits.updateForm(&d, form: form.id, list: l) } }
        }
    }
}

// MARK: whiteboards

struct WhiteboardsView: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        let boards = client.data.map { client.view.whiteboards($0) } ?? []
        List {
            if boards.isEmpty {
                EmptyNote(title: "No whiteboards yet", systemImage: "scribble").listRowBackground(Color.clear)
            }
            ForEach(boards) { b in
                NavigationLink { WhiteboardCanvas(boardId: b.id) } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(b.name).fontWeight(.medium)
                        Text("\(b.strokes.count) strokes · \(b.notes.count) notes").font(.caption).foregroundStyle(.secondary)
                    }
                }
                .deleteSwipe(client.canEdit) { client.edit { d, _ in Edits.deleteWhiteboard(&d, board: b.id) } }
                .accessibilityIdentifier("board-\(b.id)")
            }
        }
        .navigationTitle("Whiteboards")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New whiteboard", id: "newBoardButton") {
                    let id = client.newId()
                    client.edit { d, me in Edits.addWhiteboard(&d, me: me, id: id) }
                }
            }
        }
    }
}

struct WhiteboardCanvas: View {
    @Environment(SyncClient.self) private var client
    let boardId: UInt64
    enum Tool: String, CaseIterable { case pen = "Pen", eraser = "Eraser", note = "Sticky note", move = "Move" }
    @State private var tool: Tool = .pen
    @State private var color = WhiteboardView.colors[0]
    @State private var drawing: [[Double]] = []
    /// Where the board's top left corner is on screen.
    @State private var offset = CGSize.zero
    @State private var panStart: CGSize?
    @State private var dragging: (id: UInt64, start: [Double])?
    @State private var editing: StickyView?
    @State private var noteText = ""
    @State private var confirmClear = false

    var body: some View {
        let board = client.data.flatMap { client.view.whiteboards($0).first { $0.id == boardId } }
        VStack(spacing: 0) {
            toolbar
            GeometryReader { _ in
                ZStack(alignment: .topLeading) {
                    Color(.systemBackground)
                    Canvas { ctx, _ in
                        for s in board?.strokes ?? [] { draw(ctx, s.points, s.color, s.width) }
                        draw(ctx, drawing, color, 2.5)
                    }
                    ForEach(board?.notes ?? []) { n in
                        Text(n.text.isEmpty ? " " : n.text)
                            .font(.callout)
                            .padding(10)
                            .frame(width: WhiteboardView.noteSize.w, height: WhiteboardView.noteSize.h, alignment: .topLeading)
                            .background(RoundedRectangle(cornerRadius: 4).fill(Color(rgb: n.color).opacity(0.85)))
                            .offset(x: n.pos[0] + offset.width + (dragging?.id == n.id ? 0 : 0), y: n.pos[1] + offset.height)
                            .onTapGesture { if tool != .move { noteText = n.text; editing = n } }
                            .accessibilityIdentifier("note-\(n.id)")
                    }
                }
                .contentShape(Rectangle())
                .gesture(drag(board))
                .clipped()
            }
        }
        .navigationTitle(board?.name ?? "Whiteboard")
        .navigationBarTitleDisplayMode(.inline)
        .alert("Note", isPresented: Binding(isPresent: $editing)) {
            TextField("Text", text: $noteText)
            Button("Delete", role: .destructive) {
                if let n = editing { client.edit { d, _ in Edits.deleteNote(&d, board: boardId, note: n.id) } }
                editing = nil
            }
            Button("Save") {
                if let n = editing { client.edit { d, _ in Edits.updateNote(&d, board: boardId, note: n.id, text: noteText) } }
                editing = nil
            }
        }
        .confirmationDialog("Clear the whole board?", isPresented: $confirmClear, titleVisibility: .visible) {
            Button("Clear", role: .destructive) { client.edit { d, _ in Edits.clearWhiteboard(&d, board: boardId) } }
        }
    }

    var toolbar: some View {
        HStack(spacing: 12) {
            Picker("Tool", selection: $tool) {
                Image(systemName: "pencil.tip").tag(Tool.pen).accessibilityLabel("Pen")
                Image(systemName: "eraser").tag(Tool.eraser).accessibilityLabel("Eraser")
                Image(systemName: "note.text").tag(Tool.note).accessibilityLabel("Sticky note")
                Image(systemName: "hand.draw").tag(Tool.move).accessibilityLabel("Move")
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 220)
            .accessibilityIdentifier("boardTool")
            ForEach(WhiteboardView.colors, id: \.self) { c in
                Circle().fill(Color(rgb: c)).frame(width: 20, height: 20)
                    .overlay(Circle().stroke(Color.primary, lineWidth: c == color ? 2 : 0).padding(-3))
                    .onTapGesture { color = c }
                    .accessibilityAddTraits(.isButton)
            }
            Spacer(minLength: 0)
            Button { confirmClear = true } label: { Image(systemName: "trash") }
                .disabled(!client.canEdit)
                .accessibilityLabel("Clear board")
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
        .background(.bar)
    }

    func draw(_ ctx: GraphicsContext, _ pts: [[Double]], _ c: [Int], _ w: Double) {
        guard let first = pts.first else { return }
        var p = Path()
        p.move(to: CGPoint(x: first[0] + offset.width, y: first[1] + offset.height))
        if pts.count == 1 { p.addLine(to: CGPoint(x: first[0] + offset.width + 0.1, y: first[1] + offset.height)) }
        for q in pts.dropFirst() { p.addLine(to: CGPoint(x: q[0] + offset.width, y: q[1] + offset.height)) }
        ctx.stroke(p, with: .color(Color(rgb: c)), style: StrokeStyle(lineWidth: w, lineCap: .round, lineJoin: .round))
    }

    /// Board coordinates of a point on screen.
    func board(_ p: CGPoint) -> [Double] { [Double(p.x - offset.width), Double(p.y - offset.height)] }

    func drag(_ b: WhiteboardView?) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { g in
                guard client.canEdit || tool == .move else { return }
                switch tool {
                case .pen: drawing.append(board(g.location))
                case .move:
                    // a note under the finger moves; otherwise the board does
                    let start = board(g.startLocation)
                    if dragging == nil, panStart == nil {
                        if let n = b?.notes.last(where: { start[0] >= $0.pos[0] && start[0] <= $0.pos[0] + WhiteboardView.noteSize.w && start[1] >= $0.pos[1] && start[1] <= $0.pos[1] + WhiteboardView.noteSize.h }) {
                            dragging = (n.id, n.pos)
                        } else {
                            panStart = offset
                        }
                    }
                    if let p = panStart {
                        offset = CGSize(width: p.width + g.translation.width, height: p.height + g.translation.height)
                    }
                default: break
                }
            }
            .onEnded { g in
                switch tool {
                case .pen:
                    let pts = drawing
                    drawing = []
                    client.edit { d, _ in Edits.addStroke(&d, board: boardId, points: pts, color: color, width: 2.5) }
                case .eraser:
                    let p = board(g.location)
                    // the stroke passing nearest the finger
                    if let s = b?.strokes.min(by: { dist($0, p) < dist($1, p) }), dist(s, p) < 24 {
                        client.edit { d, _ in Edits.eraseStroke(&d, board: boardId, stroke: s.id) }
                    }
                case .note:
                    let p = board(g.location)
                    client.edit { d, _ in Edits.addNote(&d, board: boardId, at: p, color: color) }
                case .move:
                    if let (id, start) = dragging {
                        let pos = [start[0] + g.translation.width, start[1] + g.translation.height]
                        client.edit { d, _ in Edits.updateNote(&d, board: boardId, note: id, pos: pos) }
                    }
                    dragging = nil
                    panStart = nil
                }
            }
    }

    func dist(_ s: StrokeView, _ p: [Double]) -> Double {
        s.points.map { hypot($0[0] - p[0], $0[1] - p[1]) }.min() ?? .infinity
    }
}

// MARK: templates

struct TemplatesView: View {
    @Environment(SyncClient.self) private var client
    @State private var using: TemplateView?

    var body: some View {
        let all = client.data.map { client.view.templates($0) } ?? TemplateView.builtins
        List {
            ForEach([("list", "Lists & Spaces"), ("", "Tasks"), ("doc", "Docs")], id: \.0) { kind, title in
                let items = all.filter { $0.kind == kind }
                if !items.isEmpty {
                    Section(L(title)) {
                        ForEach(items) { tp in
                            Button { using = tp } label: {
                                VStack(alignment: .leading, spacing: 2) {
                                    HStack {
                                        Text(tp.name).foregroundStyle(.primary)
                                        if tp.teamId != nil { Text("Team").font(.caption2).padding(.horizontal, 5).background(Capsule().fill(Color.accentColor.opacity(0.15))) }
                                    }
                                    Text(tp.about).font(.caption).foregroundStyle(.secondary)
                                }
                            }
                            .disabled(!client.canEdit)
                            .deleteSwipe(tp.teamId != nil && client.canEdit) { if let id = tp.teamId { client.edit { d, _ in Edits.deleteTemplate(&d, template: id) } } }
                            .accessibilityIdentifier("template-\(tp.id)")
                        }
                    }
                }
            }
        }
        .navigationTitle("Templates")
        .sheet(item: $using) { UseTemplateSheet(template: $0) }
    }
}

private struct UseTemplateSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Environment(\.dismiss) private var dismiss
    let template: TemplateView
    @State private var name = ""
    /// 0: a new space (List templates), otherwise a space or list id.
    @State private var dest: UInt64 = 0

    var body: some View {
        let v = client.view
        let spaces = v.visibleSpaces(me: client.me ?? 0)
        NavigationStack {
            Form {
                Section {
                    TextField(template.taskName, text: $name).accessibilityIdentifier("templateName")
                } header: {
                    Text(template.kind == "list" ? "List name" : template.kind == "doc" ? "Doc title" : "Task name")
                }
                if template.kind == "list" {
                    Picker("Put it in", selection: $dest) {
                        Text("A new space").tag(UInt64(0))
                        ForEach(spaces) { Text($0.privateOwner != nil ? "Personal" : $0.name).tag($0.id) }
                    }
                } else if template.kind == "" {
                    Picker("List", selection: $dest) {
                        ForEach(spaces) { s in ForEach(s.lists + s.folders.flatMap(\.lists)) { Text("\(s.name) / \($0.name)").tag($0.id) } }
                    }
                }
                if template.kind == "list" {
                    Section("Statuses") { Text(template.statuses.map { $0.name.uppercased() }.joined(separator: ", ")).font(.footnote) }
                    Section("Tasks") { ForEach(template.tasks, id: \.name) { Text($0.name).font(.footnote) } }
                } else if !template.description.isEmpty {
                    Section("Preview") { Text(template.description).font(.footnote).foregroundStyle(.secondary) }
                }
            }
            .navigationTitle(template.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Use") { use() }
                        .disabled(template.kind == "" && dest == 0)
                        .accessibilityIdentifier("templateUse")
                }
            }
            .onAppear {
                if template.kind == "" { dest = spaces.flatMap(\.allListIds).first ?? 0 }
            }
        }
    }

    func use() {
        let n = name
        switch template.kind {
        case "list":
            var lid: UInt64?
            client.edit { d, me in lid = Edits.useListTemplate(&d, template, name: n, space: dest == 0 ? nil : dest, me: me, ts: nowTs()) { client.newId() } }
            dismiss()
            if let lid, let s = client.view.space(ofList: lid) { nav.tab = .tasks; nav.tasks = [.space(s.id), .list(lid)] }
        case "doc":
            let id = client.newId()
            let space = client.view.visibleSpaces(me: client.me ?? 0).first { $0.privateOwner == nil }?.id
            client.edit { d, me in Edits.useDocTemplate(&d, template, title: n, space: space, me: me, id: id, ts: nowTs()) }
            dismiss()
            nav.tab = .docs
            nav.docs = [.doc(id)]
        default:
            let id = client.newId()
            client.edit { d, me in Edits.useTaskTemplate(&d, template, name: n, list: dest, me: me, id: id, ts: nowTs()) }
            dismiss()
            nav.tab = .tasks
            nav.tasks = [.task(id)]
        }
    }
}

// MARK: posts and saved messages

struct MessageListView: View {
    @Environment(SyncClient.self) private var client
    let title: String
    let posts: Bool

    var body: some View {
        let v = client.view
        let me = client.me ?? 0
        let rows = posts ? v.posts(me: me) : (client.data.map { v.savedMessages($0, me: me) } ?? [])
        List {
            if rows.isEmpty {
                EmptyNote(title: posts ? "There are no Posts yet!" : "Nothing saved", systemImage: posts ? "megaphone" : "bookmark",
                          detail: posts ? "Write one from a channel's megaphone button." : "Long-press a message and choose Save for later.")
                    .listRowBackground(Color.clear)
            }
            ForEach(rows, id: \.message.id) { c, m in
                NavigationLink(value: Nav.Route.channel(c.id)) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("\(v.member(m.author)?.name ?? String(localized: "Someone")) in \(c.kind == .channel ? "#" + c.name : v.title(of: c, me: me)) · \(Fmt.time(m.ts))")
                            .font(.caption).foregroundStyle(.secondary)
                        if let t = m.postTitle, !t.isEmpty { Text(t).font(.headline) }
                        if !m.text.isEmpty { Text(m.text).lineLimit(4) }
                    }
                }
                .accessibilityIdentifier("listed-\(m.id)")
            }
        }
        .navigationTitle(title)
    }
}

// MARK: teams

struct TeamsView: View {
    @Environment(SyncClient.self) private var client
    @State private var renaming: TeamView?
    @State private var name = ""

    var body: some View {
        let v = client.view
        let teams = client.data.map { v.teams($0) } ?? []
        List {
            Section("Teams") {
                if teams.isEmpty { Text("No teams yet").foregroundStyle(.secondary) }
                ForEach(teams) { t in
                    DisclosureGroup {
                        ForEach(v.activeMembers) { m in
                            MemberToggleRow(member: m, on: t.members.contains(m.id)) {
                                client.edit { d, _ in Edits.toggleTeamMember(&d, team: t.id, member: m.id) }
                            }
                            .disabled(!client.canEdit)
                        }
                    } label: {
                        HStack {
                            Circle().fill(Color(rgb: t.color)).frame(width: 10, height: 10)
                            Text(t.name)
                            Spacer()
                            Text("\(t.members.count)").foregroundStyle(.secondary).monospacedDigit()
                        }
                    }
                    .contextMenu {
                        if client.canEdit {
                            Button { name = t.name; renaming = t } label: { Label("Rename", systemImage: "pencil") }
                            Button(role: .destructive) { client.edit { d, _ in Edits.deleteTeam(&d, team: t.id) } } label: { Label("Delete", systemImage: "trash") }
                        }
                    }
                    .accessibilityIdentifier("team-\(t.id)")
                }
            }
            Section("All People") {
                ForEach(v.members) { m in
                    HStack(spacing: 10) {
                        Avatar(member: m, size: 26)
                        VStack(alignment: .leading) {
                            Text(m.name)
                            Text([m.role, m.invited ? "Invited" : nil, teams.filter { $0.members.contains(m.id) }.map(\.name).joined(separator: ", ")].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · "))
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .navigationTitle("Teams")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("Create Team", id: "newTeamButton") {
                    let id = client.newId()
                    client.edit { d, me in Edits.addTeam(&d, me: me, id: id) }
                }
            }
        }
        .alert("Rename team", isPresented: Binding(isPresent: $renaming)) {
            TextField("Name", text: $name)
            Button("Cancel", role: .cancel) { renaming = nil }
            Button("Save") {
                if let t = renaming { client.edit { d, _ in Edits.renameTeam(&d, team: t.id, to: name) } }
                renaming = nil
            }
        }
    }
}
