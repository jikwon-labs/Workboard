import SwiftUI
import WorkboardKit

/// Spaces -> folders -> lists -> tasks.
struct SpacesView: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        let spaces = client.view.visibleSpaces(me: client.me ?? 0)
        List {
            if spaces.isEmpty {
                EmptyNote(title: "No spaces yet", systemImage: "square.grid.2x2").listRowBackground(Color.clear)
            }
            ForEach(spaces) { s in
                NavigationLink(value: Nav.Route.space(s.id)) {
                    HStack(spacing: 12) {
                        RoundedRectangle(cornerRadius: 7, style: .continuous)
                            .fill(Color(rgb: s.color))
                            .frame(width: 28, height: 28)
                            .overlay(Text(String(s.name.prefix(1)).uppercased()).font(.footnote.weight(.bold)).foregroundStyle(.white))
                        Text(s.privateOwner != nil ? "Personal" : s.name)
                        Spacer()
                        Text("\(openCount(s))").foregroundStyle(.secondary).monospacedDigit()
                    }
                }
                .accessibilityIdentifier("space-\(s.id)")
            }
        }
        .navigationTitle("Tasks")
        .toolbar { ToolbarItem(placement: .topBarTrailing) { SettingsButton() } }
    }

    func openCount(_ s: SpaceView) -> Int {
        let ids = Set(s.allListIds)
        return client.view.tasks.filter { ids.contains($0.listId) && $0.parent == nil && !client.view.status(of: $0).isClosed }.count
    }
}

struct SpaceDetailView: View {
    @Environment(SyncClient.self) private var client
    let spaceId: UInt64

    var body: some View {
        let v = client.view
        if let s = v.spaces.first(where: { $0.id == spaceId }) {
            List {
                if !s.lists.isEmpty {
                    Section("Lists") {
                        ForEach(s.lists) { l in listLink(l) }
                    }
                }
                ForEach(s.folders) { f in
                    Section {
                        ForEach(f.lists) { l in listLink(l) }
                    } header: {
                        Label(f.name, systemImage: "folder")
                    }
                }
                if s.lists.isEmpty && s.folders.allSatisfy({ $0.lists.isEmpty }) {
                    EmptyNote(title: "No lists in this space", systemImage: "list.bullet").listRowBackground(Color.clear)
                }
            }
            .navigationTitle(s.name)
        } else {
            EmptyNote(title: "This space is gone", systemImage: "square.grid.2x2")
        }
    }

    func listLink(_ l: ListView) -> some View {
        NavigationLink(value: Nav.Route.list(l.id)) {
            HStack {
                Label(l.name, systemImage: "list.bullet")
                Spacer()
                Text("\(client.view.tasks(inList: l.id).filter { !client.view.status(of: $0).isClosed }.count)")
                    .foregroundStyle(.secondary).monospacedDigit()
            }
        }
        .accessibilityIdentifier("list-\(l.id)")
    }
}

struct ListTasksView: View {
    @Environment(SyncClient.self) private var client
    let listId: UInt64
    @State private var adding = false
    @State private var newName = ""
    @FocusState private var newFocused: Bool

    var body: some View {
        let v = client.view
        let tasks = v.tasks(inList: listId)
        let statuses = v.statuses(forList: listId)
        List {
            ForEach(statuses, id: \.name) { st in
                let group = tasks.filter { v.status(of: $0).name == st.name }
                if !group.isEmpty {
                    Section {
                        ForEach(group) { t in
                            NavigationLink(value: Nav.Route.task(t.id)) { TaskRow(task: t) }
                                .swipeActions(edge: .leading) {
                                    if client.canEdit {
                                        Button(st.isClosed ? "Reopen" : "Complete") {
                                            client.edit { d, me in Edits.setDone(&d, task: t.id, done: !st.isClosed, me: me, ts: nowTs()) }
                                        }
                                        .tint(st.isClosed ? .gray : .green)
                                    }
                                }
                                .accessibilityIdentifier("task-\(t.id)")
                        }
                    } header: {
                        HStack(spacing: 6) {
                            StatusDot(status: st, size: 10)
                            Text(st.name.uppercased())
                            Text("\(group.count)").foregroundStyle(.secondary)
                        }
                    }
                }
            }
            if tasks.isEmpty && !adding {
                EmptyNote(title: "No tasks yet", systemImage: "checklist").listRowBackground(Color.clear)
            }
            if adding {
                Section {
                    TextField("Task name", text: $newName)
                        .focused($newFocused)
                        .submitLabel(.done)
                        .onSubmit(add)
                        .accessibilityIdentifier("newTaskField")
                }
            }
        }
        .navigationTitle(v.list(listId)?.name ?? "List")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                if adding {
                    Button("Add", action: add)
                        .disabled(newName.trimmingCharacters(in: .whitespaces).isEmpty)
                } else {
                    Button {
                        adding = true
                        newFocused = true
                    } label: { Image(systemName: "plus") }
                    .disabled(!client.canEdit)
                    .accessibilityLabel("New task")
                    .accessibilityIdentifier("addTaskButton")
                }
            }
        }
    }

    func add() {
        let name = newName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { adding = false; return }
        let ok = client.edit { d, me in
            Edits.addTask(&d, list: listId, name: name, assignees: [me], me: me, id: client.newId(), ts: nowTs())
        }
        if ok {
            newName = ""
            adding = false
        }
    }
}

struct TaskDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let taskId: UInt64
    @State private var confirmDelete = false
    @State private var name = ""
    @State private var desc = ""
    @State private var loaded = false
    @FocusState private var focus: Field?

    enum Field { case name, desc }

    var body: some View {
        let v = client.view
        if let t = v.tasks.first(where: { $0.id == taskId }) {
            let status = v.status(of: t)
            let can = client.canEdit
            Form {
                Section {
                    TextField("Task name", text: $name, axis: .vertical)
                        .font(.title3.weight(.semibold))
                        .focused($focus, equals: .name)
                        .disabled(!can)
                        .submitLabel(.done)
                        .onChange(of: name) {
                            // a return key ends the edit, like the Mac app's one-line name
                            if name.contains("\n") {
                                name = name.replacingOccurrences(of: "\n", with: "")
                                focus = nil
                            }
                        }
                        .accessibilityIdentifier("taskName")
                    if let l = v.list(t.listId) {
                        LabeledContent("List", value: l.name)
                    }
                }
                Section {
                    Menu {
                        ForEach(v.statuses(forList: t.listId), id: \.name) { st in
                            Button {
                                client.edit { d, me in Edits.setStatus(&d, task: t.id, to: st.name, me: me, ts: nowTs()) }
                            } label: {
                                if st.name == t.status { Label(st.name.uppercased(), systemImage: "checkmark") } else { Text(st.name.uppercased()) }
                            }
                        }
                    } label: {
                        LabeledContent("Status") {
                            HStack(spacing: 6) {
                                StatusDot(status: status, size: 10)
                                Text(status.name.uppercased()).font(.subheadline.weight(.semibold)).foregroundStyle(Color(rgb: status.color))
                            }
                        }
                    }
                    .disabled(!can)
                    .accessibilityIdentifier("statusMenu")
                    Button {
                        client.edit { d, me in Edits.setDone(&d, task: t.id, done: !status.isClosed, me: me, ts: nowTs()) }
                    } label: {
                        Label(status.isClosed ? "Reopen" : "Mark complete", systemImage: status.isClosed ? "arrow.uturn.backward.circle" : "checkmark.circle")
                    }
                    .disabled(!can)
                    .accessibilityIdentifier("completeButton")
                    Menu {
                        ForEach(v.members.filter { !$0.invited }) { m in
                            Button {
                                client.edit { d, me in Edits.toggleAssignee(&d, task: t.id, member: m.id, me: me, ts: nowTs()) }
                            } label: {
                                if t.assignees.contains(m.id) { Label(m.name, systemImage: "checkmark") } else { Text(m.name) }
                            }
                        }
                    } label: {
                        LabeledContent("Assignees") {
                            Text(t.assignees.isEmpty ? "None" : t.assignees.compactMap { v.member($0)?.name }.joined(separator: ", "))
                                .lineLimit(1)
                        }
                    }
                    .disabled(!can)
                    .accessibilityIdentifier("assigneeMenu")
                    DueRow(task: t, enabled: can)
                    Menu {
                        ForEach(0..<5) { p in
                            Button {
                                client.edit { d, me in Edits.setPriority(&d, task: t.id, to: p, me: me, ts: nowTs()) }
                            } label: {
                                if t.priority == p { Label(Edits.priorityLabels[p], systemImage: "checkmark") } else { Text(Edits.priorityLabels[p]) }
                            }
                        }
                    } label: {
                        LabeledContent("Priority") {
                            Label(Edits.priorityLabels[min(max(t.priority, 0), 4)], systemImage: t.priority == 0 ? "flag" : "flag.fill")
                                .foregroundStyle(priorityColor(t.priority))
                        }
                    }
                    .disabled(!can)
                    .accessibilityIdentifier("priorityMenu")
                }
                Section("Description") {
                    TextField("Add a description", text: $desc, axis: .vertical)
                        .lineLimit(3...)
                        .focused($focus, equals: .desc)
                        .disabled(!can)
                        .accessibilityIdentifier("taskDescription")
                }
                let subs = v.subtasks(of: t.id)
                if !subs.isEmpty {
                    Section("Subtasks") {
                        ForEach(subs) { s in NavigationLink(value: Nav.Route.task(s.id)) { TaskRow(task: s) } }
                    }
                }
                Section {
                    ForEach(Array(t.activity.suffix(30).reversed().enumerated()), id: \.offset) { _, a in
                        HStack(alignment: .top, spacing: 10) {
                            Avatar(member: v.member(a.author), size: 22)
                            VStack(alignment: .leading, spacing: 2) {
                                if a.comment {
                                    Text(v.member(a.author)?.name ?? "Someone").font(.caption.weight(.semibold))
                                }
                                Text(a.comment ? a.text : "\(v.member(a.author)?.name ?? "Someone") \(a.text)")
                                    .font(.subheadline)
                                    .foregroundStyle(a.comment ? .primary : .secondary)
                                Text(Fmt.relative(a.ts)).font(.caption2).foregroundStyle(.tertiary)
                            }
                        }
                    }
                } header: {
                    Text("Activity")
                }
            }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                Composer(placeholder: "Comment") { text, picked in
                    client.edit { d, me in Edits.addComment(&d, task: t.id, text: text, picked: picked, me: me, ts: nowTs()) }
                }
            }
            .navigationTitle("Task")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if focus != nil {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Done") { focus = nil }.accessibilityIdentifier("taskDone")
                    }
                } else {
                    ToolbarItem(placement: .topBarTrailing) {
                        Menu {
                            Button(role: .destructive) { confirmDelete = true } label: { Label("Delete task", systemImage: "trash") }
                                .accessibilityIdentifier("deleteTaskButton")
                        } label: {
                            Image(systemName: "ellipsis.circle")
                        }
                        .disabled(!client.canEdit)
                        .accessibilityLabel("More")
                        .accessibilityIdentifier("taskMoreMenu")
                    }
                }
            }
            .confirmationDialog("Delete this task?", isPresented: $confirmDelete, titleVisibility: .visible) {
                Button("Delete", role: .destructive) {
                    loaded = false // nothing left to save
                    if client.trashTask(t.id) { dismiss() }
                }
                .accessibilityIdentifier("confirmDeleteTask")
            } message: {
                Text(client.view.subtasks(of: t.id).isEmpty ? "It's removed for everyone. You can restore it from Trash in Settings." : "It's removed for everyone, with its subtasks. You can restore it from Trash in Settings.")
            }
            .onAppear {
                if !loaded { name = t.name; desc = t.description; loaded = true }
            }
            .onChange(of: focus) { old, _ in
                // save when leaving a field
                if old == .name { save(t) }
                if old == .desc { save(t) }
            }
            .onChange(of: t.name) { if focus != .name { name = t.name } }
            .onChange(of: t.description) { if focus != .desc { desc = t.description } }
            .onDisappear { save(t) }
        } else {
            EmptyNote(title: "This task is gone", systemImage: "checklist")
        }
    }

    func priorityColor(_ p: Int) -> Color {
        switch p {
        case 1: return .red
        case 2: return .orange
        case 3: return .blue
        default: return .secondary
        }
    }

    func save(_ t: TaskView) {
        guard loaded else { return }
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let nameChanged = !n.isEmpty && n != t.name
        let descChanged = desc != t.description
        guard nameChanged || descChanged else { return }
        let ok = client.edit { d, _ in
            if nameChanged { Edits.rename(&d, task: t.id, to: n, ts: nowTs()) }
            if descChanged { Edits.setDescription(&d, task: t.id, to: desc, ts: nowTs()) }
        }
        if !ok {
            name = t.name
            desc = t.description
        }
    }
}

/// Due date: a date picker plus "No date".
struct DueRow: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let enabled: Bool
    @State private var picking = false

    var body: some View {
        Button { picking = true } label: {
            LabeledContent("Due") {
                Text(Fmt.due(task) ?? "None")
                    .foregroundStyle(task.isOverdue(today: dayString(), now: timeString()) ? .red : .secondary)
            }
        }
        .foregroundStyle(.primary)
        .disabled(!enabled)
        .accessibilityIdentifier("dueButton")
        .sheet(isPresented: $picking) {
            DuePicker(task: task).presentationDetents([.medium, .large])
        }
    }
}

private struct DuePicker: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let task: TaskView
    @State private var date = Date()

    var body: some View {
        NavigationStack {
            DatePicker("Due", selection: $date, displayedComponents: .date)
                .datePickerStyle(.graphical)
                .padding(.horizontal)
                .navigationTitle("Due date")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("No date") { set(nil) }.disabled(task.due == nil)
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Set") { set(dayString(date)) }.accessibilityIdentifier("dueSet")
                    }
                }
                .onAppear {
                    let f = DateFormatter()
                    f.dateFormat = "yyyy-MM-dd"
                    f.locale = Locale(identifier: "en_US_POSIX")
                    if let d = task.due.flatMap(f.date(from:)) { date = d }
                }
        }
    }

    func set(_ day: String?) {
        client.edit { d, me in Edits.setDue(&d, task: task.id, to: day, me: me, ts: nowTs()) }
        dismiss()
    }
}
