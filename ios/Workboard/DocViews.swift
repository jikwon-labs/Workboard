import SwiftUI
import WorkboardKit

struct DocsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav

    var body: some View {
        let all = client.view.visibleDocs(me: client.me ?? 0)
        let ids = Set(all.map(\.id))
        // sub pages are listed in their page; one whose page is gone shows here
        let docs = all.filter { $0.parent.map { !ids.contains($0) } ?? true }
        List {
            if docs.isEmpty {
                EmptyNote(title: "No docs yet", systemImage: "doc.text").listRowBackground(Color.clear)
            }
            ForEach(docs) { d in
                NavigationLink(value: Nav.Route.doc(d.id)) {
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(spacing: 6) {
                            Text(d.title.isEmpty ? String(localized: "Untitled") : d.title).fontWeight(.medium).lineLimit(1)
                            if d.isPrivate { Image(systemName: "lock.fill").font(.caption2).foregroundStyle(.secondary) }
                        }
                        Text(firstLine(d.body))
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                        HStack(spacing: 6) {
                            Text("Edited \(Fmt.relative(d.updated))")
                            let pages = all.filter { $0.parent == d.id }.count
                            if pages > 0 { Text(pages == 1 ? "· 1 page" : "· \(pages) pages") }
                        }
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
    @Environment(\.dismiss) private var dismiss
    let docId: UInt64
    @State private var confirmDelete = false
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
                            Text(d.title.isEmpty ? String(localized: "Untitled") : d.title).font(.title2.bold())
                            Text("Edited \(Fmt.relative(d.updated))").font(.caption).foregroundStyle(.secondary)
                            if d.body.isEmpty {
                                Text("This doc is empty.").foregroundStyle(.secondary).accessibilityIdentifier("docBody")
                            } else {
                                MarkdownBody(text: d.body)
                                    .textSelection(.enabled)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    // read as one text, like the plain body was
                                    .accessibilityElement(children: .ignore)
                                    .accessibilityLabel(d.body)
                                    .accessibilityAddTraits(.isStaticText)
                                    .accessibilityIdentifier("docBody")
                            }
                            let pages = client.view.visibleDocs(me: client.me ?? 0).filter { $0.parent == d.id }.sorted { $0.id < $1.id }
                            if !pages.isEmpty {
                                Divider()
                                Text("Pages").font(.headline)
                                ForEach(pages) { p in
                                    NavigationLink(value: Nav.Route.doc(p.id)) {
                                        Label(p.title.isEmpty ? String(localized: "Untitled") : p.title, systemImage: "doc.text")
                                    }
                                    .accessibilityIdentifier("page-\(p.id)")
                                }
                            }
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
                    CancelButton { editing = false }
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
                    ToolbarItem(placement: .topBarTrailing) {
                        Menu {
                            Button {
                                let id = client.newId()
                                if client.edit({ data, me in Edits.addPage(&data, parent: d.id, me: me, id: id, ts: nowTs()) }) {
                                    nav.newDoc = id
                                    path(append: id)
                                }
                            } label: { Label("Add sub page", systemImage: "doc.badge.plus") }
                                .accessibilityIdentifier("addPageButton")
                            if d.owner == client.me {
                                Button {
                                    client.edit { data, _ in Edits.setDocPrivate(&data, doc: d.id, !d.isPrivate, ts: nowTs()) }
                                } label: { Label(d.isPrivate ? "Share with everyone" : "Make private", systemImage: d.isPrivate ? "lock.open" : "lock") }
                                    .accessibilityIdentifier("docPrivateButton")
                            }
                            Button {
                                let id = client.newId()
                                client.edit { data, _ in Edits.saveDocAsTemplate(&data, doc: d.id, id: id) }
                            } label: { Label("Save as template", systemImage: "square.on.square") }
                            Button {
                                if client.edit({ data, _ in Edits.setDocArchived(&data, doc: d.id, true, ts: nowTs()) }) { dismiss() }
                            } label: { Label("Archive", systemImage: "archivebox") }
                                .accessibilityIdentifier("archiveDocButton")
                            Button(role: .destructive) { confirmDelete = true } label: { Label("Delete doc", systemImage: "trash") }
                                .accessibilityIdentifier("deleteDocButton")
                        } label: {
                            Image(systemName: "ellipsis.circle")
                        }
                        .disabled(!client.canEdit)
                        .accessibilityLabel("More")
                        .accessibilityIdentifier("docMoreMenu")
                    }
                }
            }
            .confirmationDialog("Delete this doc?", isPresented: $confirmDelete, titleVisibility: .visible) {
                Button("Delete", role: .destructive) {
                    if client.trashDoc(d.id) { dismiss() }
                }
                .accessibilityIdentifier("confirmDeleteDoc")
            } message: {
                Text("It's removed for everyone, with its sub pages. You can restore it from Trash in Settings.")
            }
        } else {
            EmptyNote(title: "This doc is gone", systemImage: "doc.text")
        }
    }

    /// Opens a page in whichever tab this doc is in.
    func path(append id: UInt64) {
        switch nav.tab {
        case .home: nav.home.append(.doc(id))
        case .chat: nav.chat.append(.doc(id))
        case .tasks: nav.tasks.append(.doc(id))
        case .more: nav.more.append(.doc(id))
        case .docs: nav.docs.append(.doc(id))
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

/// A doc's Markdown as the Mac shows it: headings, bullets, numbered lines,
/// checkboxes, quotes, code blocks and inline bold, italics, code and links.
struct MarkdownBody: View {
    let text: String

    enum Line: Hashable {
        case heading(Int, String), bullet(String), number(String, String), check(Bool, String), quote(String), code(String), rule, text(String), blank
    }

    var lines: [Line] {
        var out: [Line] = []
        var code: [String]? = nil
        for raw in text.components(separatedBy: "\n") {
            let t = raw.trimmingCharacters(in: .whitespaces)
            if t.hasPrefix("```") {
                if let c = code { out.append(.code(c.joined(separator: "\n"))); code = nil } else { code = [] }
                continue
            }
            if code != nil { code!.append(raw); continue }
            if t.isEmpty { out.append(.blank) }
            else if t.hasPrefix("### ") { out.append(.heading(3, String(t.dropFirst(4)))) }
            else if t.hasPrefix("## ") { out.append(.heading(2, String(t.dropFirst(3)))) }
            else if t.hasPrefix("# ") { out.append(.heading(1, String(t.dropFirst(2)))) }
            else if t.hasPrefix("- [ ] ") { out.append(.check(false, String(t.dropFirst(6)))) }
            else if t.lowercased().hasPrefix("- [x] ") { out.append(.check(true, String(t.dropFirst(6)))) }
            else if t.hasPrefix("- ") || t.hasPrefix("* ") { out.append(.bullet(String(t.dropFirst(2)))) }
            else if t.hasPrefix("> ") { out.append(.quote(String(t.dropFirst(2)))) }
            else if t == "---" { out.append(.rule) }
            else if let dot = t.firstIndex(of: "."), t[..<dot].allSatisfy(\.isNumber), !t[..<dot].isEmpty, t[t.index(after: dot)...].hasPrefix(" ") {
                out.append(.number(String(t[..<dot]), String(t[t.index(dot, offsetBy: 2)...])))
            } else { out.append(.text(raw)) }
        }
        if let c = code { out.append(.code(c.joined(separator: "\n"))) }
        return out
    }

    func inline(_ s: String) -> Text {
        let a = (try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
        return Text(a)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(Array(lines.enumerated()), id: \.offset) { _, l in
                switch l {
                case .heading(let n, let s): inline(s).font(n == 1 ? .title2.bold() : n == 2 ? .title3.bold() : .headline).padding(.top, 4)
                case .bullet(let s): HStack(alignment: .firstTextBaseline, spacing: 8) { Text("\u{2022}"); inline(s) }
                case .number(let n, let s): HStack(alignment: .firstTextBaseline, spacing: 6) { Text("\(n).").monospacedDigit(); inline(s) }
                case .check(let on, let s):
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Image(systemName: on ? "checkmark.square.fill" : "square").foregroundStyle(on ? Color.green : Color.secondary)
                        inline(s).strikethrough(on).foregroundStyle(on ? .secondary : .primary)
                    }
                case .quote(let s):
                    inline(s).foregroundStyle(.secondary).padding(.leading, 10)
                        .overlay(alignment: .leading) { Rectangle().fill(Color.secondary.opacity(0.4)).frame(width: 3) }
                case .code(let s):
                    Text(s).font(.system(.footnote, design: .monospaced)).padding(8).frame(maxWidth: .infinity, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 8).fill(Color(.secondarySystemBackground)))
                case .rule: Divider()
                case .text(let s): inline(s)
                case .blank: Color.clear.frame(height: 4)
                }
            }
        }
    }
}
