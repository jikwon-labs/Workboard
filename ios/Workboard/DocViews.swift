import SwiftUI
import WorkboardKit

/// Docs: the newest ones, then each space's docs a tap away. The "+"
/// starts a new doc.
struct DocsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav

    var body: some View {
        let v = client.view
        let docs = topDocs(client)
        let recent = docs.sorted { $0.updated > $1.updated }.prefix(5)
        let spaces = v.visibleSpaces(me: client.me ?? 0)
        let noSpace = docs.filter { d in !spaces.contains { $0.id == d.space } }
        List {
            CalmTitle(place: .docs)
            if docs.isEmpty {
                EmptyNote(title: "No docs yet", systemImage: "doc.text").listRowBackground(Color.clear)
            }
            if !recent.isEmpty {
                Section {
                    ForEach(Array(recent)) { d in DocRow(doc: d) }
                } header: {
                    CalmHeader("Recent")
                }
            }
            if !docs.isEmpty {
                Section {
                    ForEach(spaces) { s in
                        let n = docs.filter { $0.space == s.id }.count
                        if n > 0 {
                            NavigationLink(value: Nav.Route.docsIn(space: s.id)) {
                                spaceRow(Text(s.privateOwner != nil ? String(localized: "Personal") : s.name), color: Color(rgb: s.color), count: n)
                            }
                            .accessibilityIdentifier("docsIn-\(s.id)")
                        }
                    }
                    if !noSpace.isEmpty {
                        NavigationLink(value: Nav.Route.docsIn(space: nil)) {
                            spaceRow(Text("No space"), color: .secondary, count: noSpace.count)
                        }
                        .accessibilityIdentifier("docsIn-none")
                    }
                } header: {
                    CalmHeader("By space")
                }
            }
        }
        .calmList()
        .calmBar(String(localized: "Docs"))
        .floatingAdd("New doc", id: "newDocButton", action: newDoc)
    }

    func spaceRow(_ name: Text, color: Color, count: Int) -> some View {
        HStack(spacing: 14) {
            Image(systemName: "shippingbox").font(.system(size: 17)).foregroundStyle(color).frame(width: 26)
            name.font(.system(size: 16))
            Spacer()
            Text("\(count)").font(.system(size: 15)).foregroundStyle(.secondary).monospacedDigit()
        }
        .frame(minHeight: 32)
    }

    func newDoc() {
        let id = client.newId()
        let space = client.view.visibleSpaces(me: client.me ?? 0).first { $0.privateOwner == nil }?.id
        guard client.edit({ d, me in Edits.addDoc(&d, title: "Untitled", space: space, me: me, id: id, ts: nowTs()) }) else { return }
        nav.newDoc = id
        nav.path.append(.doc(id))
    }
}

/// Docs with no page above them (sub pages are listed in their page; one
/// whose page is gone shows here too).
@MainActor
func topDocs(_ client: SyncClient) -> [DocView] {
    let all = client.view.visibleDocs(me: client.me ?? 0)
    let ids = Set(all.map(\.id))
    return all.filter { $0.parent.map { !ids.contains($0) } ?? true }
}

/// One space's docs (or the docs in no space).
struct DocsInSpaceView: View {
    @Environment(SyncClient.self) private var client
    let spaceId: UInt64?

    var body: some View {
        let v = client.view
        let spaces = v.visibleSpaces(me: client.me ?? 0)
        let space = spaces.first { $0.id == spaceId }
        let docs = topDocs(client).filter { d in spaceId == nil ? !spaces.contains { $0.id == d.space } : d.space == spaceId }
            .sorted { $0.updated > $1.updated }
        let name = space.map { $0.privateOwner != nil ? String(localized: "Personal") : $0.name } ?? String(localized: "No space")
        List {
            CalmTitle(verbatim: name, icon: Image(systemName: "shippingbox").font(.system(size: 24, weight: .semibold)).foregroundStyle(space.map { Color(rgb: $0.color) } ?? .secondary))
            Section {
                ForEach(docs) { d in DocRow(doc: d) }
            }
            if docs.isEmpty {
                EmptyNote(title: "No docs yet", systemImage: "doc.text").listRowBackground(Color.clear)
            }
        }
        .calmList()
        .calmBar(name)
    }
}

/// A doc: its icon, title, and who changed it when.
struct DocRow: View {
    @Environment(SyncClient.self) private var client
    let doc: DocView

    var body: some View {
        let all = client.view.visibleDocs(me: client.me ?? 0)
        let pages = all.filter { $0.parent == doc.id }.count
        NavigationLink(value: Nav.Route.doc(doc.id)) {
            HStack(spacing: 14) {
                Image(systemName: "doc.text").font(.system(size: 19)).foregroundStyle(.tertiary).frame(width: 26)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(doc.title.isEmpty ? String(localized: "Untitled") : doc.title).font(.system(size: 16)).lineLimit(1)
                        if doc.isPrivate { Image(systemName: "lock.fill").font(.caption2).foregroundStyle(.secondary) }
                    }
                    HStack(spacing: 4) {
                        if let who = client.view.member(doc.owner) { Text(verbatim: who.name + " ·") }
                        Text("Edited \(Fmt.relative(doc.updated))")
                        if pages > 0 { Text(pages == 1 ? "· 1 page" : "· \(pages) pages") }
                    }
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                }
            }
            .padding(.vertical, 3)
        }
        .accessibilityIdentifier("doc-\(doc.id)")
    }
}

/// Reads a doc; Edit opens the formatting editor.
struct DocDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Environment(\.dismiss) private var dismiss
    let docId: UInt64
    @State private var confirmDelete = false
    @State private var editing = false
    @State private var title = ""
    @State private var draft = ""
    @State private var bodyFocused = false
    @State private var showVersions = false

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
                        // formatting shows while typing; a bar above the keyboard adds it
                        DocEditor(text: $draft, focus: $bodyFocused)
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
            .onDisappear {
                // taken off screen while writing (a notification opened a
                // chat or a task in its place): what was written is kept,
                // as the task and meeting screens keep theirs
                if editing && client.canEdit { save(d) }
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
                            Button { showVersions = true } label: { Label("Version history", systemImage: "clock.arrow.circlepath") }
                                .accessibilityIdentifier("docVersionsButton")
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
            .sheet(isPresented: $showVersions) {
                DocVersionsView(docId: d.id)
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

    /// Opens a page on top of this doc.
    func path(append id: UInt64) {
        nav.path.append(.doc(id))
    }

    func save(_ d: DocView) {
        // the text before and after are kept in the doc's Version history
        let ok = client.canEdit && (client.saveDoc(d.id, title: title, body: draft) || (title == d.title && draft == d.body))
        if ok { editing = false }
    }
}

/// A doc's Version history: earlier texts, newest first, with who wrote
/// them; one opens read-only with Restore this version (as on the Mac).
struct DocVersionsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let docId: UInt64

    var body: some View {
        NavigationStack {
            Group {
                if let d = client.view.docs.first(where: { $0.id == docId }), !d.versions.isEmpty {
                    List {
                        Section {
                            ForEach(d.versions) { v in
                                NavigationLink {
                                    DocVersionDetail(docId: docId, versionId: v.id)
                                } label: {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(Fmt.time(v.at))
                                        Text(versionAuthor(v, client)).font(.caption).foregroundStyle(.secondary)
                                    }
                                }
                                .accessibilityIdentifier("docVersion-\(v.id)")
                            }
                        } footer: {
                            Text("Earlier texts of this doc, kept when someone stops editing it.")
                        }
                    }
                } else {
                    EmptyNote(title: "No versions yet", systemImage: "clock.arrow.circlepath",
                              detail: "One is kept each time the doc is changed.")
                }
            }
            .navigationTitle("Version history")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
        }
    }
}

/// Who wrote a version, as the list shows it (the Mac's `version_author`).
@MainActor
func versionAuthor(_ v: DocVersionView, _ client: SyncClient) -> String {
    if v.by == 0 { return String(localized: "Earlier edits") }
    let name = client.view.member(v.by)?.name ?? String(localized: "Someone")
    if let r = v.restored { return String(localized: "\(name) restored \(Fmt.time(r))") }
    return name
}

/// One version, read-only, with Restore this version.
struct DocVersionDetail: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let docId: UInt64
    let versionId: UInt64

    var body: some View {
        if let d = client.view.docs.first(where: { $0.id == docId }),
           let v = d.versions.first(where: { $0.id == versionId }) {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    Text(v.title.isEmpty ? String(localized: "Untitled") : v.title).font(.title2.bold())
                    Text(versionAuthor(v, client)).font(.caption).foregroundStyle(.secondary)
                    if v.title == d.title && v.body == d.body {
                        Text("This is the doc's text now.").font(.callout).foregroundStyle(.secondary)
                            .accessibilityIdentifier("versionIsCurrent")
                    }
                    MarkdownBody(text: v.body)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .accessibilityElement(children: .ignore)
                        .accessibilityLabel(v.body)
                        .accessibilityAddTraits(.isStaticText)
                        .accessibilityIdentifier("versionBody")
                }
                .padding(20)
            }
            .navigationTitle(Fmt.time(v.at))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if v.title != d.title || v.body != d.body {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Restore this version") {
                            if client.restoreDocVersion(docId, version: v.id) { dismiss() }
                        }
                        .disabled(!client.canEdit)
                        .accessibilityIdentifier("restoreVersion")
                    }
                }
            }
        } else {
            EmptyNote(title: "This version is gone", systemImage: "clock.arrow.circlepath")
        }
    }
}

/// A doc's Markdown as the Mac shows it: headings, bullets, numbered lines,
/// checkboxes, quotes, code blocks and inline bold, italics, code and links.
struct MarkdownBody: View {
    let text: String
    @Environment(\.dynamicTypeSize) private var typeSize

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

    /// One line's marks in the font of `style`, italics leaning in Korean too.
    func inline(_ s: String, _ style: UIFont.TextStyle = .body, bold: Bool = false) -> Text {
        let a = (try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
        return Text(Slant.italics(a, style: style, bold: bold, size: typeSize))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(Array(lines.enumerated()), id: \.offset) { _, l in
                switch l {
                case .heading(let n, let s):
                    inline(s, n == 1 ? .title2 : n == 2 ? .title3 : .headline, bold: n < 3)
                        .font(n == 1 ? .title2.bold() : n == 2 ? .title3.bold() : .headline).padding(.top, 4)
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
