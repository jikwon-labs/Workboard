// Templates: making lists, tasks and docs from them (More tab).

import SwiftUI
import WorkboardKit

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
