import SwiftUI
import WorkboardKit

struct DocsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav

    var body: some View {
        let docs = client.view.visibleDocs(me: client.me ?? 0)
        List {
            if docs.isEmpty {
                EmptyNote(title: "No docs yet", systemImage: "doc.text").listRowBackground(Color.clear)
            }
            ForEach(docs) { d in
                NavigationLink(value: Nav.Route.doc(d.id)) {
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 6) {
                            Text(d.title.isEmpty ? "Untitled" : d.title).fontWeight(.medium).lineLimit(1)
                            if d.isPrivate { Image(systemName: "lock.fill").font(.caption2).foregroundStyle(.secondary) }
                        }
                        Text(firstLine(d.body))
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                        Text("Edited \(Fmt.relative(d.updated))")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                    }
                    .padding(.vertical, 2)
                }
                .accessibilityIdentifier("doc-\(d.id)")
            }
        }
        .navigationTitle("Docs")
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button(action: newDoc) { Image(systemName: "square.and.pencil") }
                    .disabled(!client.canEdit)
                    .accessibilityLabel("New doc")
                SettingsButton()
            }
        }
    }

    func newDoc() {
        let id = client.newId()
        let space = client.view.visibleSpaces(me: client.me ?? 0).first { $0.privateOwner == nil }?.id
        guard client.edit({ d, me in Edits.addDoc(&d, title: "Untitled", space: space, me: me, id: id, ts: nowTs()) }) else { return }
        nav.newDoc = id
        nav.docs.append(.doc(id))
    }

    func firstLine(_ body: String) -> String {
        body.split(separator: "\n").map { $0.trimmingCharacters(in: .whitespaces) }.first { !$0.isEmpty } ?? "Empty"
    }
}

/// Reads a doc; Edit turns the body into plain text editing.
struct DocDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    let docId: UInt64
    @State private var editing = false
    @State private var title = ""
    @State private var draft = ""
    @FocusState private var bodyFocused: Bool

    var body: some View {
        if let d = client.view.docs.first(where: { $0.id == docId }) {
            Group {
                if editing {
                    VStack(spacing: 0) {
                        TextField("Title", text: $title)
                            .font(.title2.bold())
                            .padding(.horizontal, 20)
                            .padding(.top, 12)
                            .accessibilityIdentifier("docTitleField")
                        TextEditor(text: $draft)
                            .focused($bodyFocused)
                            .scrollContentBackground(.hidden)
                            .padding(.horizontal, 15)
                            .accessibilityIdentifier("docEditor")
                    }
                } else {
                    ScrollView {
                        VStack(alignment: .leading, spacing: 12) {
                            Text(d.title.isEmpty ? "Untitled" : d.title).font(.title2.bold())
                            Text("Edited \(Fmt.relative(d.updated))").font(.caption).foregroundStyle(.secondary)
                            Text(d.body.isEmpty ? "This doc is empty." : d.body)
                                .foregroundStyle(d.body.isEmpty ? .secondary : .primary)
                                .textSelection(.enabled)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .accessibilityIdentifier("docBody")
                        }
                        .padding(20)
                    }
                }
            }
            .onAppear {
                // a doc just made with the New button opens for writing
                if nav.newDoc == d.id {
                    nav.newDoc = nil
                    title = d.title
                    draft = d.body
                    editing = true
                    bodyFocused = true
                }
            }
            .navigationTitle(editing ? "Editing" : "")
            .navigationBarTitleDisplayMode(.inline)
            .navigationBarBackButtonHidden(editing)
            .toolbar {
                if editing {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { editing = false }
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") { save(d) }
                            .disabled(!client.canEdit)
                            .accessibilityIdentifier("docDone")
                    }
                } else {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Edit") {
                            title = d.title
                            draft = d.body
                            editing = true
                            bodyFocused = true
                        }
                        .disabled(!client.canEdit)
                        .accessibilityIdentifier("docEdit")
                    }
                }
            }
        } else {
            EmptyNote(title: "This doc is gone", systemImage: "doc.text")
        }
    }

    func save(_ d: DocView) {
        let ok = client.edit { data, _ in
            if title != d.title { Edits.setDocTitle(&data, doc: d.id, to: title, ts: nowTs()) }
            if draft != d.body { Edits.setDocBody(&data, doc: d.id, to: draft, ts: nowTs()) }
        }
        if ok { editing = false }
    }
}
