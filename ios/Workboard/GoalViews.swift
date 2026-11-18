// Goals: a goal's targets and progress (More tab).

import SwiftUI
import WorkboardKit

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
