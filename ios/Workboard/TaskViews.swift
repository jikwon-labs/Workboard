import SwiftUI
import WorkboardKit

/// Spaces -> folders -> lists -> tasks.
struct SpacesView: View {
    @Environment(SyncClient.self) private var client
    @State private var prompt: NamePrompt?

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
        .navigationTitle("Tasks")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { SettingsButton() }
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New space", id: "newSpaceButton") { prompt = .newSpace }
            }
        }
        .namePrompt($prompt)
    }

    func openCount(_ s: SpaceView) -> Int {
        let ids = Set(s.allListIds)
        return client.view.tasks.filter { ids.contains($0.listId) && $0.parent == nil && !client.view.status(of: $0).isClosed }.count
    }
}

struct SpaceDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let spaceId: UInt64
    @State private var prompt: NamePrompt?

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
                        HStack {
                            Label(f.name, systemImage: "folder")
                            Spacer()
                            if client.canEdit {
                                Menu {
                                    Button { prompt = .newList(space: s.id, folder: f.id) } label: { Label("New list", systemImage: "plus") }
                                    Button { prompt = .renameFolder(f.id) } label: { Label("Rename", systemImage: "pencil") }
                                    Button(role: .destructive) {
                                        client.edit { d, _ in Edits.setFolderArchived(&d, folder: f.id, true) }
                                    } label: { Label("Archive", systemImage: "archivebox") }
                                } label: {
                                    Image(systemName: "ellipsis")
                                }
                                .accessibilityLabel("Folder \(f.name)")
                                .accessibilityIdentifier("folderMenu-\(f.id)")
                            }
                        }
                    }
                }
                if s.lists.isEmpty && s.folders.allSatisfy({ $0.lists.isEmpty }) {
                    EmptyNote(title: "No lists in this space", systemImage: "list.bullet").listRowBackground(Color.clear)
                }
            }
            .navigationTitle(s.name)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button { prompt = .newList(space: s.id, folder: nil) } label: { Label("New list", systemImage: "list.bullet") }
                            .accessibilityIdentifier("newListButton")
                        Button { prompt = .newFolder(s.id) } label: { Label("New folder", systemImage: "folder.badge.plus") }
                            .accessibilityIdentifier("newFolderButton")
                        NavigationLink(value: Nav.Route.statuses(s.id)) { Label("Edit statuses", systemImage: "circle.dashed") }
                            .accessibilityIdentifier("editStatusesButton")
                        if s.privateOwner == nil {
                            Button { prompt = .renameSpace(s.id) } label: { Label("Rename space", systemImage: "pencil") }
                            ColorMenu(title: "Color") { c in client.edit { d, _ in Edits.setSpaceColor(&d, space: s.id, to: c) } }
                            Button(role: .destructive) {
                                if client.edit({ d, _ in Edits.setSpaceArchived(&d, space: s.id, true) }) { dismiss() }
                            } label: { Label("Archive space", systemImage: "archivebox") }
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                    .disabled(!client.canEdit)
                    .accessibilityLabel("Space menu")
                    .accessibilityIdentifier("spaceMenu")
                }
            }
            .namePrompt($prompt)
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
        .contextMenu {
            if client.canEdit {
                Button { prompt = .renameList(l.id) } label: { Label("Rename", systemImage: "pencil") }
                Button {
                    let id = client.newId()
                    client.edit { d, _ in Edits.saveListAsTemplate(&d, list: l.id, id: id) }
                } label: { Label("Save as template", systemImage: "square.on.square") }
                Button(role: .destructive) {
                    client.edit { d, _ in Edits.setListArchived(&d, list: l.id, true) }
                } label: { Label("Archive", systemImage: "archivebox") }
            }
        }
    }
}

struct ListTasksView: View {
    @Environment(SyncClient.self) private var client
    let listId: UInt64
    @State private var adding = false
    @State private var newName = ""
    @FocusState private var newFocused: Bool
    /// List, Board or Calendar, remembered per list on this phone.
    @State private var mode: ListMode = .list

    var body: some View {
        content
            .navigationTitle(client.view.list(listId)?.name ?? "List")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Picker("View", selection: $mode) {
                            ForEach(ListMode.allCases) { m in Label(L(m.rawValue), systemImage: m.icon).tag(m) }
                        }
                    } label: {
                        Image(systemName: mode.icon)
                    }
                    .accessibilityLabel("View")
                    .accessibilityIdentifier("viewModeMenu")
                }
            }
            .onAppear { mode = ListMode(rawValue: UserDefaults.standard.string(forKey: "listMode-\(listId)") ?? "") ?? .list }
            .onChange(of: mode) { UserDefaults.standard.set(mode.rawValue, forKey: "listMode-\(listId)") }
    }

    @ViewBuilder var content: some View {
        switch mode {
        case .list: listBody
        case .board: BoardView(listId: listId)
        case .calendar: CalendarListView(listId: listId)
        case .gantt: BarsView(listId: listId, lanes: false)
        case .timeline: BarsView(listId: listId, lanes: true)
        case .table: TableListView(listId: listId)
        }
    }

    var listBody: some View {
        let v = client.view
        let tasks = v.tasks(inList: listId)
        let statuses = v.statuses(forList: listId)
        return List {
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
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                if adding {
                    Button("Add", action: add)
                        .disabled(newName.isBlank)
                } else {
                    AddButton("New task", id: "addTaskButton") {
                        adding = true
                        newFocused = true
                    }
                }
            }
        }
    }

    func add() {
        let name = newName.trimmed
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
    @State private var newSubtask = ""
    @State private var newCheck = ""
    /// Counts comments sent here, to bring the newest into view (the
    /// subtask and checklist sections can push Activity below the fold).
    @State private var commented = 0
    @FocusState private var focus: Field?
    @State private var sheet: Sheet?
    @State private var renaming: Int?
    @State private var renameText = ""

    enum Sheet: Identifiable { case move, parent, remind; var id: Self { self } }

    enum Field { case name, desc, subtask, check }

    var body: some View {
        let v = client.view
        if let t = v.tasks.first(where: { $0.id == taskId }) {
            let can = client.canEdit
            ScrollViewReader { proxy in
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
                    TaskPropertiesSection(task: t, can: can)
                    TaskDetailsSection(task: t, can: can)
                    Section("Description") {
                        TextField("Add a description", text: $desc, axis: .vertical)
                            .lineLimit(3...)
                            .focused($focus, equals: .desc)
                            .disabled(!can)
                            .accessibilityIdentifier("taskDescription")
                    }
                    CustomFieldsSection(task: t, can: can)
                    let subs = v.subtasks(of: t.id)
                    if !subs.isEmpty || can {
                        Section("Subtasks") {
                            ForEach(subs) { s in
                                NavigationLink(value: Nav.Route.task(s.id)) { TaskRow(task: s) }
                                    .accessibilityIdentifier("subtask-\(s.id)")
                            }
                            if can {
                                TextField("Add subtask", text: $newSubtask)
                                    .focused($focus, equals: .subtask)
                                    .submitLabel(.done)
                                    .onSubmit { addSubtask(t) }
                                    .accessibilityIdentifier("newSubtaskField")
                            }
                        }
                    }
                    RelationshipsSection(task: t, can: can)
                    TaskAttachmentsSection(task: t, can: can)
                    if !t.checklist.isEmpty || can {
                        Section {
                            ChecklistRows(task: t, can: can) { i, text in renameText = text; renaming = i }
                            if can {
                                TextField(t.checklist.isEmpty ? "Create checklist" : "Add item", text: $newCheck)
                                    .focused($focus, equals: .check)
                                    .submitLabel(.done)
                                    .onSubmit { addCheck(t) }
                                    .accessibilityIdentifier("newCheckItemField")
                            }
                        } header: {
                            HStack {
                                Text("Checklist")
                                if !t.checklist.isEmpty {
                                    Text("\(t.checklist.filter(\.done).count)/\(t.checklist.count)")
                                        .monospacedDigit()
                                        .accessibilityIdentifier("checklistCount")
                                }
                            }
                        }
                    }
                    if v.apps.timeTracking {
                        TimeSection(task: t, can: can)
                    }
                    TaskActivitySection(task: t)
                }
                .onChange(of: commented) {
                    withAnimation { proxy.scrollTo("latestActivity", anchor: .top) }
                }
            }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                Composer(placeholder: "Comment") { text, picked in
                    let ok = client.edit { d, me in Edits.addComment(&d, task: t.id, text: text, picked: picked, me: me, ts: nowTs()) }
                    if ok { commented += 1 }
                    return ok
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
                            Button {
                                client.edit { d, me in Edits.duplicateTask(&d, task: t.id, me: me, ts: nowTs()) { client.newId() } }
                            } label: { Label("Duplicate", systemImage: "plus.square.on.square") }
                                .accessibilityIdentifier("duplicateTaskButton")
                            Button { sheet = .remind } label: { Label("Remind me", systemImage: "alarm") }
                                .accessibilityIdentifier("remindTaskButton")
                            Button { sheet = .move } label: { Label("Move to...", systemImage: "arrow.right.square") }
                                .accessibilityIdentifier("moveTaskButton")
                            Button { sheet = .parent } label: { Label("Make subtask of...", systemImage: "arrow.turn.down.right") }
                                .accessibilityIdentifier("makeSubtaskButton")
                            if t.parent != nil {
                                Button {
                                    client.edit { d, me in Edits.setParent(&d, task: t.id, to: nil, me: me, ts: nowTs()) }
                                } label: { Label("Make top-level task", systemImage: "arrow.turn.left.up") }
                                    .accessibilityIdentifier("makeTopLevelButton")
                            }
                            Button {
                                loaded = false
                                if client.edit({ d, me in Edits.setArchived(&d, task: t.id, true, me: me, ts: nowTs()) }) { dismiss() }
                            } label: { Label("Archive", systemImage: "archivebox") }
                                .accessibilityIdentifier("archiveTaskButton")
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
            .sheet(item: $sheet) { which in
                switch which {
                case .move:
                    ListPickerSheet(current: t.listId) { list in
                        client.edit { d, me in Edits.moveTasks(&d, ids: [t.id], to: list, me: me, ts: nowTs()) }
                    }
                case .parent:
                    TaskPickerSheet(title: "Make subtask of", tasks: v.parentCandidates(for: t.id)) { p in
                        client.edit { d, me in Edits.setParent(&d, task: t.id, to: p, me: me, ts: nowTs()) }
                    }
                case .remind:
                    NewReminderView(task: t)
                }
            }
            .alert("Rename item", isPresented: Binding(isPresent: $renaming)) {
                TextField("Item", text: $renameText).accessibilityIdentifier("renameCheckField")
                Button("Cancel", role: .cancel) { renaming = nil }
                Button("Save") {
                    if let i = renaming { client.edit { d, _ in Edits.renameCheckItem(&d, task: t.id, index: i, to: renameText, ts: nowTs()) } }
                    renaming = nil
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

    /// Like the Mac app's "Add subtask": in the parent's list, under it.
    func addSubtask(_ t: TaskView) {
        let n = newSubtask.trimmed
        guard !n.isEmpty else { focus = nil; return }
        let id = client.newId()
        if client.edit({ d, me in Edits.addTask(&d, list: t.listId, name: n, assignees: [], parent: t.id, me: me, id: id, ts: nowTs()) }) {
            newSubtask = ""
            focus = .subtask // keep typing the next one
        }
    }

    func addCheck(_ t: TaskView) {
        guard !newCheck.isBlank else { focus = nil; return }
        if client.edit({ d, _ in Edits.addCheckItem(&d, task: t.id, text: newCheck, ts: nowTs()) }) {
            newCheck = ""
            focus = .check
        }
    }

    func save(_ t: TaskView) {
        guard loaded else { return }
        let n = name.trimmed
        let nameChanged = !n.isEmpty && n != t.name
        let descChanged = desc != t.description
        guard nameChanged || descChanged else { return }
        let ok = client.edit { d, me in
            if nameChanged { Edits.rename(&d, task: t.id, to: n, me: me, ts: nowTs()) }
            if descChanged { Edits.setDescription(&d, task: t.id, to: desc, me: me, ts: nowTs()) }
        }
        if !ok {
            name = t.name
            desc = t.description
        }
    }
}

/// A task's status, Mark complete, assignees, due date and priority.
struct TaskPropertiesSection: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let can: Bool

    var body: some View {
        let v = client.view
        let t = task
        let status = v.status(of: t)
        Section {
            Menu {
                ForEach(v.statuses(forList: t.listId), id: \.name) { st in
                    Button {
                        client.edit { d, me in Edits.setStatus(&d, task: t.id, to: st.name, me: me, ts: nowTs()) }
                    } label: {
                        CheckedLabel(st.name.uppercased(), checked: st.name == t.status)
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
                ForEach(v.activeMembers) { m in
                    Button {
                        client.edit { d, me in Edits.toggleAssignee(&d, task: t.id, member: m.id, me: me, ts: nowTs()) }
                    } label: {
                        CheckedLabel(m.name, checked: t.assignees.contains(m.id))
                    }
                }
            } label: {
                LabeledContent("Assignees") {
                    Text(t.assignees.isEmpty ? String(localized: "None") : t.assignees.compactMap { v.member($0)?.name }.joined(separator: ", "))
                        .lineLimit(1)
                }
            }
            .disabled(!can)
            .accessibilityIdentifier("assigneeMenu")
            DueRow(task: t, enabled: can)
            if v.apps.priorities {
            Menu {
                ForEach(0..<5) { p in
                    Button {
                        client.edit { d, me in Edits.setPriority(&d, task: t.id, to: p, me: me, ts: nowTs()) }
                    } label: {
                        CheckedLabel(L(Edits.priorityLabels[p]), checked: t.priority == p)
                    }
                }
            } label: {
                LabeledContent("Priority") {
                    Label(L(Edits.priorityLabels[min(max(t.priority, 0), 4)]), systemImage: t.priority == 0 ? "flag" : "flag.fill")
                        .foregroundStyle(priorityColor(t.priority))
                }
            }
            .disabled(!can)
            .accessibilityIdentifier("priorityMenu")
            }
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
}

/// A task's checklist items: tap to tick, long-press to rename, assign or
/// turn into a subtask, swipe to delete.
struct ChecklistRows: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    let can: Bool
    /// Rename item `i` (its text now).
    let rename: (Int, String) -> Void

    var body: some View {
        let v = client.view
        let t = task
        ForEach(Array(t.checklist.enumerated()), id: \.offset) { i, item in
            Button {
                client.edit { d, _ in Edits.setCheckItem(&d, task: t.id, index: i, done: !item.done, ts: nowTs()) }
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: item.done ? "checkmark.circle.fill" : "circle")
                        .foregroundStyle(item.done ? Color.green : Color.secondary)
                    Text(item.text)
                        .strikethrough(item.done)
                        .foregroundStyle(item.done ? .secondary : .primary)
                    Spacer(minLength: 0)
                    if let who = item.assignee, let m = v.member(who) {
                        Avatar(member: m, size: 20)
                    }
                }
            }
            .disabled(!can)
            .accessibilityLabel(item.text)
            .accessibilityValue(item.done ? "Done" : "Not done")
            .accessibilityIdentifier("checkItem-\(i)")
            .contextMenu {
                if can {
                    Button { rename(i, item.text) } label: { Label("Rename", systemImage: "pencil") }
                    Menu {
                        Button("Nobody") { client.edit { d, _ in Edits.setCheckAssignee(&d, task: t.id, index: i, to: nil, ts: nowTs()) } }
                        ForEach(v.activeMembers) { m in
                            Button {
                                client.edit { d, _ in Edits.setCheckAssignee(&d, task: t.id, index: i, to: m.id, ts: nowTs()) }
                            } label: {
                                CheckedLabel(m.name, checked: item.assignee == m.id)
                            }
                        }
                    } label: { Label("Assign", systemImage: "person") }
                    Button {
                        let id = client.newId()
                        client.edit { d, me in Edits.checkItemToSubtask(&d, task: t.id, index: i, me: me, id: id, ts: nowTs()) }
                    } label: { Label("Convert to subtask", systemImage: "arrow.turn.down.right") }
                }
            }
            .deleteSwipe(can) {
                client.edit { d, _ in Edits.removeCheckItem(&d, task: t.id, index: i, ts: nowTs()) }
            }
        }
    }
}

/// The newest 30 lines of a task's activity, newest first.
struct TaskActivitySection: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView

    var body: some View {
        let v = client.view
        let t = task
        Section {
            ForEach(Array(t.activity.suffix(30).reversed().enumerated()), id: \.offset) { i, a in
                HStack(alignment: .top, spacing: 10) {
                    Avatar(member: v.member(a.author), size: 22)
                    VStack(alignment: .leading, spacing: 2) {
                        if a.comment {
                            Text(v.member(a.author)?.name ?? String(localized: "Someone")).font(.caption.weight(.semibold))
                        }
                        Text(verbatim: a.comment ? a.text : History.line(who: v.member(a.author)?.name ?? String(localized: "Someone"), text: a.text))
                            .font(.subheadline)
                            .foregroundStyle(a.comment ? .primary : .secondary)
                        Text(Fmt.relative(a.ts)).font(.caption2).foregroundStyle(.tertiary)
                    }
                }
                .id(i == 0 ? "latestActivity" : "activity-\(i)")
            }
        } header: {
            Text("Activity")
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
                Text(Fmt.due(task) ?? String(localized: "None"))
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
                    if let d = task.due.flatMap(DayFormat.date(fromDay:)) { date = d }
                }
        }
    }

    func set(_ day: String?) {
        client.edit { d, me in Edits.setDue(&d, task: task.id, to: day, me: me, ts: nowTs()) }
        dismiss()
    }
}
