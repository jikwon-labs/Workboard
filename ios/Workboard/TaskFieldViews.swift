import PhotosUI
import SwiftUI
import WorkboardKit

// The task window's other fields, as on the Mac: task type, start date,
// due time, repeat, time estimate, watchers, tags, time tracking,
// relationships and custom fields. Workspace > ClickApps turns tags, time
// tracking, estimates and custom fields off for everyone.

/// Task type, start date, due time, repeat, estimate, watchers and tags.
struct TaskDetailsSection: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let can: Bool
    @State private var estimate = ""
    @State private var estimateBad = false
    @State private var picking: DateField?
    @State private var addingTag = false
    @FocusState private var estimateFocused: Bool

    enum DateField: Identifiable { case start, startTime, dueTime; var id: Self { self } }

    var body: some View {
        let v = client.view
        let apps = v.apps
        Section("Details") {
            Menu {
                Button { client.edit { d, _ in Edits.setMilestone(&d, task: task.id, false, ts: nowTs()) } } label: {
                    CheckedLabel("Task", checked: !task.milestone)
                }
                Button { client.edit { d, _ in Edits.setMilestone(&d, task: task.id, true, ts: nowTs()) } } label: {
                    CheckedLabel("Milestone", checked: task.milestone)
                }
            } label: {
                LabeledContent("Task type") {
                    Label(task.milestone ? "Milestone" : "Task", systemImage: task.milestone ? "diamond" : "checkmark.circle")
                }
            }
            .disabled(!can)
            .accessibilityIdentifier("taskTypeMenu")

            Button { picking = .start } label: {
                LabeledContent("Start") {
                    Text(task.start.map { [Fmt.due($0), task.startTime.map(Fmt.clock)].compactMap { $0 }.joined(separator: " ") } ?? String(localized: "None"))
                }
            }
            .foregroundStyle(.primary)
            .disabled(!can)
            .accessibilityIdentifier("startButton")
            .sheet(item: $picking) { f in
                TaskDateSheet(task: task, field: f).presentationDetents([.medium, .large])
            }
            if task.start != nil {
                Button { picking = .startTime } label: {
                    LabeledContent("Start time", value: task.startTime.map(Fmt.clock) ?? String(localized: "None"))
                }
                .foregroundStyle(.primary)
                .disabled(!can)
                .accessibilityIdentifier("startTimeButton")
            }
            if task.due != nil {
                Button { picking = .dueTime } label: {
                    LabeledContent("Due time", value: task.dueTime.map(Fmt.clock) ?? String(localized: "None"))
                }
                .foregroundStyle(.primary)
                .disabled(!can)
                .accessibilityIdentifier("dueTimeButton")
            }

            Menu {
                ForEach(Edits.repeats, id: \.label) { r in
                    Button {
                        client.edit { d, me in Edits.setRecur(&d, task: task.id, to: r.value, me: me, ts: nowTs()) }
                    } label: {
                        CheckedLabel(L(r.label), checked: task.recur == r.value)
                    }
                }
            } label: {
                LabeledContent("Repeat") {
                    Text(L(Edits.repeats.first { $0.value == task.recur }?.label ?? "Never"))
                }
            }
            .disabled(!can)
            .accessibilityIdentifier("repeatMenu")

            if apps.timeEstimates {
                LabeledContent("Estimate") {
                    TextField("e.g. 1h 30m", text: $estimate)
                        .multilineTextAlignment(.trailing)
                        .focused($estimateFocused)
                        .submitLabel(.done)
                        .onSubmit(saveEstimate)
                        .foregroundStyle(estimateBad ? .red : .primary)
                        .disabled(!can)
                        .accessibilityIdentifier("estimateField")
                }
                .onAppear { estimate = task.estimateMin.map(Edits.fmtEstimate) ?? "" }
                .onChange(of: task.estimateMin) { if !estimateFocused { estimate = task.estimateMin.map(Edits.fmtEstimate) ?? "" } }
                .onChange(of: estimateFocused) { _, now in if !now { saveEstimate() } }
            }

            Menu {
                ForEach(v.activeMembers) { m in
                    Button {
                        client.edit { d, _ in Edits.toggleWatcher(&d, task: task.id, member: m.id, ts: nowTs()) }
                    } label: {
                        CheckedLabel(m.name, checked: task.watchers.contains(m.id))
                    }
                }
            } label: {
                LabeledContent("Watchers") {
                    Text(task.watchers.isEmpty ? String(localized: "None") : task.watchers.compactMap { v.member($0)?.name }.joined(separator: ", ")).lineLimit(1)
                }
            }
            .disabled(!can)
            .accessibilityIdentifier("watchersMenu")

            if apps.tags {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("Tags")
                        Spacer()
                        Button { addingTag = true } label: { Image(systemName: "plus.circle") }
                            .disabled(!can)
                            .accessibilityLabel("Add tag")
                            .accessibilityIdentifier("addTagButton")
                            .sheet(isPresented: $addingTag) {
                                AddTagSheet(task: task).presentationDetents([.medium, .large])
                            }
                    }
                    if !task.tags.isEmpty {
                        TagFlow(tags: task.tags) { tag in
                            client.edit { d, me in Edits.removeTag(&d, task: task.id, tag: tag, me: me, ts: nowTs()) }
                        }
                        .disabled(!can)
                    }
                }
                .buttonStyle(.borderless)
            }
        }
    }

    func saveEstimate() {
        let text = estimate.trimmingCharacters(in: .whitespaces)
        if text.isEmpty {
            estimateBad = false
            client.edit { d, me in Edits.setEstimate(&d, task: task.id, minutes: nil, me: me, ts: nowTs()) }
            return
        }
        guard let min = Edits.parseEstimate(text) else { estimateBad = true; return }
        estimateBad = false
        estimate = Edits.fmtEstimate(min)
        client.edit { d, me in Edits.setEstimate(&d, task: task.id, minutes: min, me: me, ts: nowTs()) }
    }
}

/// Tags as small capsules; tapping one offers to remove it.
struct TagFlow: View {
    let tags: [String]
    let remove: (String) -> Void

    var body: some View {
        FlowLayout(spacing: 6) {
            ForEach(tags, id: \.self) { tag in
                Menu {
                    Button("Remove tag", role: .destructive) { remove(tag) }
                } label: {
                    Text(tag)
                        .font(.caption.weight(.medium))
                        .padding(.horizontal, 8).padding(.vertical, 4)
                        .background(Capsule().fill(Color.accentColor.opacity(0.15)))
                }
                .accessibilityIdentifier("tag-\(tag)")
            }
        }
    }
}

/// Lays children out left to right, wrapping to new rows.
struct FlowLayout: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, widest: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x > 0 && x + size.width > width { x = 0; y += row + spacing; row = 0 }
            x += size.width + spacing
            row = max(row, size.height)
            widest = max(widest, x - spacing)
        }
        return CGSize(width: min(widest, width), height: y + row)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, row: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x > bounds.minX && x + size.width > bounds.maxX { x = bounds.minX; y += row + spacing; row = 0 }
            s.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            row = max(row, size.height)
        }
    }
}

/// A tag typed in, or one already used on another task.
struct AddTagSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let task: TaskView
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            List {
                Section {
                    TextField("New tag", text: $text)
                        .focused($focused)
                        .onAppear { focused = true }
                        .textInputAutocapitalization(.never)
                        .submitLabel(.done)
                        .onSubmit { add(text) }
                        .accessibilityIdentifier("newTagField")
                }
                let used = client.view.allTags.filter { !task.tags.contains($0) && (text.isEmpty || $0.contains(text.lowercased())) }
                if !used.isEmpty {
                    Section("Used before") {
                        ForEach(used, id: \.self) { tag in
                            Button(tag) { add(tag) }
                        }
                    }
                }
            }
            .navigationTitle("Add tag")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") { add(text) }.disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
    }

    func add(_ tag: String) {
        guard !tag.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        client.edit { d, me in Edits.addTag(&d, task: task.id, tag: tag, me: me, ts: nowTs()) }
        dismiss()
    }
}

/// Start date, or the time of day on the start or due date.
struct TaskDateSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let task: TaskView
    let field: TaskDetailsSection.DateField
    @State private var date = Date()

    var title: String {
        switch field {
        case .start: return "Start date"
        case .startTime: return "Start time"
        case .dueTime: return "Due time"
        }
    }

    var body: some View {
        NavigationStack {
            Group {
                if field == .start {
                    DatePicker(title, selection: $date, displayedComponents: .date).datePickerStyle(.graphical)
                } else {
                    DatePicker(title, selection: $date, displayedComponents: .hourAndMinute).datePickerStyle(.wheel).labelsHidden()
                }
            }
            .padding(.horizontal)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(field == .start ? "No date" : "No time") { set(nil) }
                        .disabled(current == nil)
                        .accessibilityIdentifier("dateSheetClear")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Set") { set(date) }.accessibilityIdentifier("dateSheetSet")
                }
            }
            .onAppear(perform: load)
        }
    }

    var current: String? {
        switch field {
        case .start: return task.start
        case .startTime: return task.startTime
        case .dueTime: return task.dueTime
        }
    }

    func load() {
        if let c = current, let d = field == .start ? DayFormat.date(fromDay: c) : DayFormat.date(fromTime: c) {
            if field == .start {
                date = d
            } else {
                let parts = Calendar.current.dateComponents([.hour, .minute], from: d)
                date = Calendar.current.date(bySettingHour: parts.hour ?? 9, minute: parts.minute ?? 0, second: 0, of: Date()) ?? Date()
            }
        } else if field != .start {
            date = Calendar.current.date(bySettingHour: 9, minute: 0, second: 0, of: Date()) ?? Date()
        }
    }

    func set(_ d: Date?) {
        let hhmm = d.map { timeString($0) }
        client.edit { data, me in
            switch field {
            case .start: Edits.setStart(&data, task: task.id, to: d.map { dayString($0) }, me: me, ts: nowTs())
            case .startTime: Edits.setStartTime(&data, task: task.id, to: hhmm, me: me, ts: nowTs())
            case .dueTime: Edits.setDueTime(&data, task: task.id, to: hhmm, me: me, ts: nowTs())
            }
        }
        dismiss()
    }
}

/// Time tracked: everyone's total, my timer, the entries, and Add time.
struct TimeSection: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let can: Bool
    @State private var addingTime = false

    var body: some View {
        let v = client.view
        let me = client.me ?? 0
        let mine = v.myTimerStart(task, me: me)
        Section {
            // ticks only while someone's timer runs
            TimelineView(TrackedSchedule(running: !task.timers.isEmpty)) { ctx in
                let now = Int64(ctx.date.timeIntervalSince1970)
                LabeledContent("Tracked") {
                    Text(Self.hms(v.trackedSecs(task, now: now))).monospacedDigit()
                        .accessibilityIdentifier("trackedTotal")
                }
            }
            Button {
                client.edit { d, me in
                    if mine != nil { Edits.stopTimer(&d, task: task.id, me: me, ts: nowTs()) } else { Edits.startTimer(&d, task: task.id, me: me, ts: nowTs()) }
                }
            } label: {
                Label(mine != nil ? "Stop timer" : "Start timer", systemImage: mine != nil ? "stop.circle.fill" : "play.circle")
                    .foregroundStyle(mine != nil ? .red : .accentColor)
            }
            .disabled(!can)
            .accessibilityIdentifier("timerButton")
            ForEach(task.timers.filter { $0.member != me && !($0.member == nil && v.solo) }, id: \.self) { r in
                Label("\(r.member.flatMap { v.member($0)?.name } ?? String(localized: "Someone")) is tracking", systemImage: "timer")
                    .font(.subheadline).foregroundStyle(.secondary)
            }
            ForEach(task.timeEntries.sorted { $0.start > $1.start }, id: \.self) { e in
                let owner = v.entryOwner(task, e)
                HStack {
                    Avatar(member: v.member(owner), size: 20)
                    Text(Fmt.time(e.end)).font(.subheadline)
                    Spacer()
                    Text(Self.hm(e.secs)).monospacedDigit().foregroundStyle(.secondary)
                }
                .deleteSwipe(can && (owner == me || (owner == 0 && v.solo))) {
                    client.edit { d, _ in Edits.removeTimeEntry(&d, task: task.id, member: e.member, start: e.start) }
                }
            }
            Button { addingTime = true } label: { Label("Add time", systemImage: "plus") }
                .disabled(!can)
                .accessibilityIdentifier("addTimeButton")
                .sheet(isPresented: $addingTime) {
                    AddTimeSheet(task: task).presentationDetents([.medium])
                }
        } header: {
            Text("Time tracked")
        }
    }

    /// "1:05:09" / "5:09".
    static func hms(_ secs: Int64) -> String {
        let h = secs / 3600, m = (secs % 3600) / 60, s = secs % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }

    /// fmt_hm: "1h 20m", "40m", "2h".
    static func hm(_ secs: Int64) -> String {
        let min = Int64((Double(secs) / 60).rounded())
        switch (min / 60, min % 60) {
        case (0, 0): return "-"
        case (0, let m): return "\(m)m"
        case (let h, 0): return "\(h)h"
        case (let h, let m): return "\(h)h \(m)m"
        }
    }
}

/// Every second while a timer runs, otherwise once.
struct TrackedSchedule: TimelineSchedule {
    let running: Bool

    func entries(from start: Date, mode: TimelineScheduleMode) -> AnyIterator<Date> {
        var next: Date? = start
        return AnyIterator {
            guard let d = next else { return nil }
            next = running ? d.addingTimeInterval(1) : nil
            return d
        }
    }
}

struct AddTimeSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let task: TaskView
    @State private var text = ""
    @State private var yesterday = false
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            Form {
                TextField("e.g. 1h 30m", text: $text)
                    .focused($focused)
                    .onAppear { focused = true }
                    .submitLabel(.done)
                    .onSubmit(add)
                    .accessibilityIdentifier("addTimeField")
                Picker("Day", selection: $yesterday) {
                    Text("Today").tag(false)
                    Text("Yesterday").tag(true)
                }
                .pickerStyle(.segmented)
                if !text.isEmpty && minutes == nil {
                    Text("Write it like 1h 30m, 45m or 2:30").font(.footnote).foregroundStyle(.red)
                }
            }
            .navigationTitle("Add time")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add", action: add).disabled(minutes == nil).accessibilityIdentifier("addTimeConfirm")
                }
            }
        }
    }

    var minutes: UInt32? { Edits.parseEstimate(text).flatMap { $0 > 0 ? $0 : nil } }

    func add() {
        guard let m = minutes else { return }
        client.edit { d, me in Edits.addTimeEntry(&d, task: task.id, minutes: m, yesterday: yesterday, me: me, ts: nowTs()) }
        dismiss()
    }
}

/// Waiting on, Blocking and Linked tasks.
struct RelationshipsSection: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let can: Bool
    @State private var adding: Edits.Relation?
    @State private var choosing = false

    var body: some View {
        let r = client.view.relations(of: task.id)
        let groups: [(Edits.Relation, [TaskView])] = [(.waitingOn, r.waitingOn), (.blocking, r.blocking), (.linked, r.linked)]
        Section {
            ForEach(groups, id: \.0) { rel, tasks in
                ForEach(tasks) { t in
                    NavigationLink(value: Nav.Route.task(t.id)) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(L(rel.rawValue)).textCase(.uppercase).font(.caption2.weight(.semibold)).foregroundStyle(rel == .waitingOn ? .orange : rel == .blocking ? .red : .secondary)
                            TaskRow(task: t)
                        }
                    }
                    .deleteSwipe(can, "Remove") {
                        client.edit { d, _ in Edits.removeRelation(&d, task: task.id, rel, other: t.id, ts: nowTs()) }
                    }
                    .accessibilityIdentifier("relation-\(rel.rawValue)-\(t.id)")
                }
            }
            if can {
                Button { choosing = true } label: {
                    Label("Add relationship", systemImage: "link")
                }
                .accessibilityIdentifier("addRelationMenu")
                .confirmationDialog("Add relationship", isPresented: $choosing, titleVisibility: .visible) {
                    ForEach(Edits.Relation.allCases, id: \.self) { rel in
                        Button(L(rel.rawValue)) { adding = rel }
                    }
                }
                .sheet(item: $adding) { rel in
                    let have = Set(groups.first { $0.0 == rel }?.1.map(\.id) ?? [])
                    TaskPickerSheet(title: rel.rawValue, tasks: client.view.tasks.filter { $0.id != task.id && !have.contains($0.id) }) { other in
                        client.edit { d, me in Edits.addRelation(&d, task: task.id, rel, other: other, me: me, ts: nowTs()) }
                    }
                }
            }
        } header: {
            Text("Relationships")
        }
    }
}

extension Edits.Relation: Identifiable { public var id: String { rawValue } }

/// Picks one task, searching by name.
struct TaskPickerSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let title: String
    let tasks: [TaskView]
    let pick: (UInt64) -> Void
    @State private var query = ""

    var body: some View {
        NavigationStack {
            let shown = tasks.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) }
            List(shown) { t in
                Button {
                    pick(t.id)
                    dismiss()
                } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(t.name).foregroundStyle(.primary)
                        if let l = client.view.list(t.listId) { Text(l.name).font(.caption).foregroundStyle(.secondary) }
                    }
                }
                .accessibilityIdentifier("pick-\(t.id)")
            }
            .overlay { if shown.isEmpty { EmptyNote(title: "No tasks", systemImage: "checklist") } }
            .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always))
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { CancelButton { dismiss() } }
        }
    }
}

/// Picks a list to move a task to.
struct ListPickerSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let current: UInt64
    let pick: (UInt64) -> Void

    var body: some View {
        NavigationStack {
            List {
                ForEach(client.view.visibleSpaces(me: client.me ?? 0)) { s in
                    Section(s.privateOwner != nil ? "Personal" : s.name) {
                        ForEach(s.lists + s.folders.flatMap(\.lists)) { l in
                            Button {
                                pick(l.id)
                                dismiss()
                            } label: {
                                HStack {
                                    Label(l.name, systemImage: "list.bullet").foregroundStyle(.primary)
                                    Spacer()
                                    if l.id == current { Image(systemName: "checkmark") }
                                }
                            }
                            .disabled(l.id == current)
                            .accessibilityIdentifier("pickList-\(l.id)")
                        }
                    }
                }
            }
            .navigationTitle("Move to")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { CancelButton { dismiss() } }
        }
    }
}

/// The workspace's custom fields, each with the control its kind needs.
struct CustomFieldsSection: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let can: Bool

    var body: some View {
        let fields = client.view.fields
        if client.view.apps.customFields && !fields.isEmpty {
            Section("Custom fields") {
                ForEach(fields) { f in
                    CustomFieldRow(task: task, field: f)
                        .disabled(!can)
                        .accessibilityIdentifier("field-\(f.id)")
                }
            }
        }
    }
}

struct CustomFieldRow: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let field: CustomFieldView
    @State private var text = ""
    @State private var date = Date()
    @FocusState private var focused: Bool

    var value: String { task.custom[field.id] ?? "" }

    var body: some View {
        Group {
            switch field.kind {
            case "checkbox":
                Toggle(field.name, isOn: Binding(get: { value == "true" }, set: { set($0 ? "true" : "") }))
            case "dropdown":
                Picker(field.name, selection: Binding(get: { value }, set: { set($0) })) {
                    Text("Empty").tag("")
                    ForEach(field.options, id: \.self) { Text($0).tag($0) }
                }
            case "date":
                HStack {
                    Text(field.name)
                    Spacer()
                    if value.isEmpty {
                        Button("Set") { set(dayString()) }.buttonStyle(.borderless)
                    } else {
                        DatePicker("", selection: Binding(get: { DayFormat.date(fromDay: value) ?? Date() }, set: { set(dayString($0)) }), displayedComponents: .date)
                            .labelsHidden()
                        Button { set("") } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary) }
                            .buttonStyle(.borderless)
                            .accessibilityLabel("Clear \(field.name)")
                    }
                }
            default:
                LabeledContent(field.name) {
                    TextField(field.kind == "number" ? "0" : "Empty", text: $text)
                        .multilineTextAlignment(.trailing)
                        .keyboardType(field.kind == "number" ? .decimalPad : .default)
                        .focused($focused)
                        .submitLabel(.done)
                        .onSubmit(saveText)
                        .foregroundStyle(badNumber ? .red : .primary)
                }
            }
        }
        .onAppear { text = value }
        .onChange(of: value) { if !focused { text = value } }
        .onChange(of: focused) { _, now in if !now { saveText() } }
    }

    var badNumber: Bool {
        field.kind == "number" && !text.trimmingCharacters(in: .whitespaces).isEmpty && Double(text.trimmingCharacters(in: .whitespaces)) == nil
    }

    func saveText() {
        guard !badNumber else { return }
        set(text)
    }

    func set(_ v: String) {
        client.edit { d, _ in Edits.setCustom(&d, task: task.id, field: field.id, to: v, ts: nowTs()) }
    }
}


/// The task's pictures, videos and files, and Attach (they go to the team
/// server first, then join the task, as on the Mac).
struct TaskAttachmentsSection: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let can: Bool
    @State private var picked: [PhotosPickerItem] = []
    @State private var pending: [PendingFile] = []
    @State private var importing = false

    var body: some View {
        Section {
            if !task.attachments.isEmpty {
                AttachmentsView(attachments: task.attachments)
                    .contextMenu {
                        if can {
                            ForEach(task.attachments, id: \.file) { a in
                                Button(role: .destructive) {
                                    client.edit { d, _ in Edits.removeTaskAttachment(&d, task: task.id, file: a.file, ts: nowTs()) }
                                } label: { Label("Remove \(a.name)", systemImage: "trash") }
                            }
                        }
                    }
            }
            ForEach(pending) { f in
                HStack {
                    if let e = f.error { Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red).font(.footnote) } else { ProgressView(); Text("Uploading").font(.footnote) }
                }
            }
            if can && client.state.isConnected {
                PhotosPicker(selection: $picked, maxSelectionCount: 10, matching: .any(of: [.images, .videos])) {
                    Label("Attach", systemImage: "paperclip")
                }
                .accessibilityIdentifier("attachButton")
                Button { importing = true } label: {
                    Label("Attach file", systemImage: "doc")
                }
                .accessibilityIdentifier("attachFileButton")
            }
        } header: {
            Text("Attachments")
        }
        .onChange(of: picked) {
            let items = picked
            picked = []
            let (server, auth, tid) = (client.settings.server, client.fileAuth, task.id)
            for item in items {
                let f = PendingFile()
                pending.append(f)
                Task {
                    await uploadMedia(item, server: server, auth: auth, into: f)
                    attach(f, task: tid)
                }
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            guard case .success(let urls) = result else { return }
            let (server, auth, tid) = (client.settings.server, client.fileAuth, task.id)
            for url in urls {
                let f = PendingFile()
                pending.append(f)
                Task {
                    await uploadFile(url, server: server, auth: auth, into: f)
                    attach(f, task: tid)
                }
            }
        }
    }

    /// A file that got to the server joins the task.
    func attach(_ f: PendingFile, task tid: UInt64) {
        if let a = f.attachment {
            client.edit { d, me in Edits.addTaskAttachment(&d, task: tid, attachment: a, me: me, ts: nowTs()) }
            pending.removeAll { $0.id == f.id }
        }
    }
}
