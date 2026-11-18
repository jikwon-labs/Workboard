import SwiftUI
import WorkboardKit

// Workspace > Automations, as on the Mac: "When status changes to X, then
// assign / set priority / comment / move", everywhere or in one space or
// list. They run on every device when a task's status changes.

struct AutomationRow: Identifiable, Hashable {
    let id: UInt64
    let enabled: Bool
    let trigger: String
    let action: String
    let value: String
    let list: UInt64?

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        enabled = j["enabled"]?.bool ?? false
        trigger = j["trigger_status"]?.string ?? ""
        action = j["action"]?.string ?? ""
        value = j["value"]?.string ?? ""
        list = j["list"]?.uint
    }
}

struct AutomationsView: View {
    @Environment(SyncClient.self) private var client
    @State private var adding = false

    var rules: [AutomationRow] { (client.data?.at("extra.automations")?.array ?? []).map(AutomationRow.init) }

    var body: some View {
        List {
            if rules.isEmpty {
                EmptyNote(title: "No automations yet", systemImage: "bolt").listRowBackground(Color.clear)
            }
            ForEach(rules) { r in
                Toggle(isOn: Binding(get: { r.enabled }, set: { on in
                    client.edit { d, _ in Edits.setAutomationEnabled(&d, automation: r.id, on) }
                })) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("When status is \(r.trigger.uppercased())").font(.subheadline.weight(.semibold))
                        Text(describe(r)).font(.subheadline)
                        Text(scope(r.list)).font(.caption).foregroundStyle(.secondary)
                    }
                }
                .disabled(!client.canEdit)
                .deleteSwipe(client.canEdit) {
                    client.edit { d, _ in Edits.deleteAutomation(&d, automation: r.id) }
                }
                .accessibilityIdentifier("automation-\(r.id)")
            }
        }
        .navigationTitle("Automations")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("Add automation", id: "addAutomationButton") { adding = true }
            }
        }
        .sheet(isPresented: $adding) { NewAutomationView() }
    }

    /// describe_action in src/app.rs.
    func describe(_ r: AutomationRow) -> String {
        let v = client.view
        switch r.action {
        case "assign": return "Assign to \(UInt64(r.value).flatMap { v.member($0)?.name } ?? "")"
        case "priority": return "Set priority to \(Int(r.value).flatMap { $0 >= 0 && $0 < 5 ? Edits.priorityLabels[$0] : nil } ?? "")"
        case "move": return "Move to \(UInt64(r.value).flatMap { v.list($0)?.name } ?? "List")"
        default: return "Comment \"\(r.value)\""
        }
    }

    func scope(_ id: UInt64?) -> String {
        guard let id else { return "Everywhere" }
        let v = client.view
        if let s = v.spaces.first(where: { $0.id == id }) { return "In \(s.name)" }
        if let l = v.list(id) { return "In \(l.name)" }
        return "In a List that is gone"
    }
}

struct NewAutomationView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    @State private var trigger = ""
    @State private var action = "assign"
    @State private var value = ""
    @State private var scope: UInt64 = 0

    var body: some View {
        let v = client.view
        var statusNames: [String] = []
        let _ = v.spaces.flatMap { $0.statuses.map(\.name) }.forEach { if !statusNames.contains($0) { statusNames.append($0) } }
        let chosen = trigger.isEmpty ? (statusNames.first ?? "") : trigger
        NavigationStack {
            Form {
                Section("When status changes to") {
                    Picker("Status", selection: Binding(get: { chosen }, set: { trigger = $0 })) {
                        ForEach(statusNames, id: \.self) { Text($0.uppercased()).tag($0) }
                    }
                    .accessibilityIdentifier("automationTrigger")
                }
                Section("Then") {
                    Picker("Action", selection: $action) {
                        ForEach(Edits.automationActions, id: \.value) { Text(L($0.label)).tag($0.value) }
                    }
                    .onChange(of: action) { value = "" }
                    .accessibilityIdentifier("automationAction")
                    switch action {
                    case "assign":
                        Picker("Person", selection: $value) {
                            Text("Choose").tag("")
                            ForEach(v.activeMembers) { Text($0.name).tag(String($0.id)) }
                        }
                    case "priority":
                        Picker("Priority", selection: $value) {
                            Text("Choose").tag("")
                            ForEach(0..<5) { Text(L(Edits.priorityLabels[$0])).tag(String($0)) }
                        }
                    case "move":
                        Picker("List", selection: $value) {
                            Text("Choose").tag("")
                            ForEach(v.spaces.flatMap { $0.lists + $0.folders.flatMap(\.lists) }) { Text($0.name).tag(String($0.id)) }
                        }
                    default:
                        TextField("Comment", text: $value, axis: .vertical).accessibilityIdentifier("automationComment")
                    }
                }
                Section("Where") {
                    Picker("Runs in", selection: $scope) {
                        Text("Everywhere").tag(UInt64(0))
                        ForEach(v.visibleSpaces(me: client.me ?? 0).filter { $0.privateOwner == nil }) { s in
                            Text(s.name).tag(s.id)
                            ForEach(s.lists + s.folders.flatMap(\.lists)) { l in Text("  \(l.name)").tag(l.id) }
                        }
                    }
                }
            }
            .navigationTitle("New automation")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        let id = client.newId()
                        let s = scope == 0 ? nil : scope
                        client.edit { d, _ in Edits.addAutomation(&d, trigger: chosen, action: action, value: value, list: s, id: id) }
                        dismiss()
                    }
                    .disabled(chosen.isEmpty || value.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("automationAdd")
                }
            }
        }
    }
}
