import SwiftUI
import WorkboardKit

// The Home tab's Inbox and search, as on the Mac: the Inbox's Primary,
// Other, Later and Cleared rows (Later and Cleared kept on this phone), and
// one search over tasks, docs, conversations, messages and people.

struct InboxView: View {
    @Environment(SyncClient.self) private var client
    @State private var tab: InboxItem.Tab = .primary

    var body: some View {
        let me = client.me ?? 0
        let all = client.inboxItems(me: me)
        let rows = all.filter { $0.tab == tab }
        Section {
            Picker("Inbox", selection: $tab) {
                ForEach(InboxItem.Tab.allCases, id: \.self) { t in
                    let n = all.filter { $0.tab == t }.count
                    Text(t == .primary && n > 0 ? "\(String(localized: String.LocalizationValue(t.title))) \(n)" : String(localized: String.LocalizationValue(t.title))).tag(t)
                }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("inboxTabs")
            if rows.isEmpty {
                Text(tab == .primary ? "You're all caught up." : "Nothing here.").font(.subheadline).foregroundStyle(.secondary)
            }
            ForEach(rows.prefix(100)) { item in
                NavigationLink(value: route(item.source)) { InboxRow(item: item) }
                    .swipeActions(edge: .trailing) {
                        if tab == .cleared || tab == .later {
                            Button("Back to Inbox") { move([item.key], nil) }.tint(.blue)
                        } else {
                            Button("Clear") { move([item.key], .cleared) }.tint(.green)
                        }
                    }
                    .swipeActions(edge: .leading) {
                        if tab != .later { Button("Later") { move([item.key], .later) }.tint(.orange) }
                    }
                    .accessibilityIdentifier("inbox-\(item.key)")
            }
            if !rows.isEmpty && (tab == .primary || tab == .other) {
                Button("Clear all") { move(rows.map(\.key), .cleared) }
                    .accessibilityIdentifier("inboxClearAll")
            }
        } header: {
            Text("Inbox")
        }
    }

    func route(_ s: InboxItem.Source) -> Nav.Route {
        switch s {
        case .task(let id): return .task(id)
        case .chat(let ch, _, let thread?): return .thread(channel: ch, message: thread)
        case .chat(let ch, _, nil): return .channel(ch)
        }
    }

    func move(_ keys: [String], _ to: InboxItem.Tab?) {
        client.editLocal { d in Inbox.move(&d, keys: keys, to: to) }
    }
}

private struct InboxRow: View {
    @Environment(SyncClient.self) private var client
    let item: InboxItem

    /// inbox_line, in the phone's language.
    func inboxLine(_ who: String) -> String {
        switch item.source {
        case .chat(_, _, let thread) where thread != nil && !item.mention: return String(localized: "\(who) replied: \(item.text)")
        case .chat: return String(localized: "\(who) mentioned you: \(item.text)")
        case .task where item.comment: return String(localized: "\(who) commented: \(item.text)")
        case .task: return History.line(who: who, text: item.text)
        }
    }

    var body: some View {
        let who = client.view.member(item.author)
        HStack(alignment: .top, spacing: 10) {
            Avatar(member: who, size: 28)
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(item.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                    Spacer()
                    Text(Fmt.relative(item.ts)).font(.caption2).foregroundStyle(.tertiary)
                }
                Text(verbatim: inboxLine(who?.name ?? String(localized: "Someone"))).font(.subheadline).foregroundStyle(.secondary).lineLimit(2)
            }
        }
    }
}

/// What a search finds.
struct SearchResults: View {
    @Environment(SyncClient.self) private var client
    let query: String

    var body: some View {
        let v = client.view
        let me = client.me ?? 0
        let q = query.trimmingCharacters(in: .whitespaces)
        let has = { (s: String) in s.localizedCaseInsensitiveContains(q) }
        let tasks = v.tasks.filter { has($0.name) || has($0.description) || $0.tags.contains(where: has) }.prefix(30)
        let docs = v.visibleDocs(me: me).filter { has($0.title) || has($0.body) }.prefix(20)
        let channels = v.visibleChannels(me: me)
        let convs = channels.filter { has(v.title(of: $0, me: me)) }
        let messages = channels.flatMap { c in (c.messages + c.messages.flatMap(\.replies)).filter { has($0.text) }.map { (c, $0) } }.suffix(30).reversed()
        let people = v.members.filter { has($0.name) || has($0.email) }
        if tasks.isEmpty && docs.isEmpty && convs.isEmpty && messages.isEmpty && people.isEmpty {
            Text("No results for \"\(q)\"").foregroundStyle(.secondary).accessibilityIdentifier("noResults")
        }
        if !tasks.isEmpty {
            Section("Tasks") {
                ForEach(Array(tasks)) { t in
                    NavigationLink(value: Nav.Route.task(t.id)) { TaskRow(task: t, showList: true) }
                        .accessibilityIdentifier("result-task-\(t.id)")
                }
            }
        }
        if !docs.isEmpty {
            Section("Docs") {
                ForEach(Array(docs)) { d in
                    NavigationLink(value: Nav.Route.doc(d.id)) { Label(d.title.isEmpty ? String(localized: "Untitled") : d.title, systemImage: "doc.text") }
                        .accessibilityIdentifier("result-doc-\(d.id)")
                }
            }
        }
        if !convs.isEmpty {
            Section("Conversations") {
                ForEach(convs) { c in
                    NavigationLink(value: Nav.Route.channel(c.id)) {
                        Label(c.kind == .channel ? "#\(c.name)" : v.title(of: c, me: me), systemImage: c.kind == .channel ? "number" : "person.2")
                    }
                }
            }
        }
        if !messages.isEmpty {
            Section("Messages") {
                ForEach(Array(messages), id: \.1.id) { c, m in
                    NavigationLink(value: Nav.Route.channel(c.id)) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(v.member(m.author)?.name ?? String(localized: "Someone")) in \(c.kind == .channel ? "#" + c.name : v.title(of: c, me: me))")
                                .font(.caption).foregroundStyle(.secondary)
                            Text(m.text).lineLimit(2)
                        }
                    }
                    .accessibilityIdentifier("result-message-\(m.id)")
                }
            }
        }
        if !people.isEmpty {
            Section("People") {
                ForEach(people) { p in
                    HStack(spacing: 10) {
                        Avatar(member: p, size: 24)
                        Text(p.name)
                        Spacer()
                        Text(p.role).font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
    }
}
