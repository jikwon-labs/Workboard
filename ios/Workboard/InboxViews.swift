import SwiftUI
import WorkboardKit

// The Inbox, as on the Mac: new notices (Primary, with Other below them)
// and Later; Later and Cleared are kept on this phone. And quick find: one
// search over tasks, docs, conversations, messages and people.

struct InboxScreen: View {
    @Environment(SyncClient.self) private var client
    @State private var tab: Shown = .new
    @State private var showCleared = false

    enum Shown: Hashable { case new, later }

    var body: some View {
        let me = client.me ?? 0
        let all = client.inboxItems(me: me)
        let primary = all.filter { $0.tab == .primary }
        let other = all.filter { $0.tab == .other }
        let later = all.filter { $0.tab == .later }
        let cleared = all.filter { $0.tab == .cleared }
        List {
            CalmTitle(place: .inbox)
            Section {
                Picker("Inbox", selection: $tab) {
                    Text(primary.isEmpty ? String(localized: "New notices") : String(localized: "New notices \(primary.count)")).tag(Shown.new)
                    Text(later.isEmpty ? String(localized: "Later") : String(localized: "Later \(later.count)")).tag(Shown.later)
                }
                .pickerStyle(.segmented)
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets())
                .accessibilityIdentifier("inboxTabs")
            }
            if tab == .new {
                if primary.isEmpty && other.isEmpty {
                    Section { Text("You're all caught up.").font(.subheadline).foregroundStyle(.secondary) }
                }
                ForEach(byDay(primary), id: \.0) { title, items in
                    Section {
                        rows(items, from: .primary)
                    } header: {
                        CalmHeader(verbatim: title)
                    }
                }
                if !other.isEmpty {
                    Section {
                        rows(other, from: .other)
                    } header: {
                        CalmHeader("Other")
                    }
                }
                Section {
                    Button(showCleared ? "Hide cleared" : (cleared.isEmpty ? String(localized: "Cleared") : String(localized: "Cleared \(cleared.count)"))) {
                        withAnimation { showCleared.toggle() }
                    }
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("inboxShowCleared")
                    if showCleared {
                        if cleared.isEmpty { Text("Nothing here.").font(.subheadline).foregroundStyle(.secondary) }
                        rows(cleared, from: .cleared)
                    }
                } footer: {
                    if !primary.isEmpty || !other.isEmpty {
                        Text("Swipe left to clear, right for later.").frame(maxWidth: .infinity)
                    }
                }
            } else {
                Section {
                    if later.isEmpty { Text("Nothing here.").font(.subheadline).foregroundStyle(.secondary) }
                    rows(later, from: .later)
                } header: {
                    if !later.isEmpty { CalmHeader("To look at later") }
                }
            }
        }
        .calmList()
        .calmBar(String(localized: "Inbox"))
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                let open = tab == .new ? primary + other : []
                if !open.isEmpty {
                    Button("Clear all") { move(open.map(\.key), .cleared) }
                        .accessibilityIdentifier("inboxClearAll")
                }
            }
        }
    }

    /// "Today", "Yesterday", then "Earlier".
    func byDay(_ items: [InboxItem]) -> [(String, [InboxItem])] {
        let cal = Calendar.current
        let day = { (i: InboxItem) in Date(timeIntervalSince1970: TimeInterval(i.ts)) }
        let groups: [(String, [InboxItem])] = [
            (String(localized: "Today"), items.filter { cal.isDateInToday(day($0)) }),
            (String(localized: "Yesterday"), items.filter { cal.isDateInYesterday(day($0)) }),
            (String(localized: "Earlier"), items.filter { !cal.isDateInToday(day($0)) && !cal.isDateInYesterday(day($0)) }),
        ]
        return groups.filter { !$0.1.isEmpty }
    }

    func rows(_ items: [InboxItem], from tab: InboxItem.Tab) -> some View {
        ForEach(items.prefix(100)) { item in
            NavigationLink(value: route(item.source)) { InboxRow(item: item, fresh: tab == .primary) }
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
    var fresh = false

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
        HStack(alignment: .top, spacing: 12) {
            Avatar(member: who, size: 34)
                .overlay(alignment: .leading) {
                    if fresh { Circle().fill(Place.inbox.color).frame(width: 6, height: 6).offset(x: -10).accessibilityHidden(true) }
                }
            VStack(alignment: .leading, spacing: 3) {
                HStack {
                    Text(item.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                    Spacer()
                    Text(Fmt.relative(item.ts)).font(.caption2).foregroundStyle(.tertiary)
                }
                Text(verbatim: inboxLine(who?.name ?? String(localized: "Someone"))).font(.system(size: 15)).foregroundStyle(.secondary).lineLimit(2)
            }
        }
    }
}

/// Quick find: the search field is open at once; the chips under it pick
/// what to look in.
struct SearchView: View {
    @State private var query = ""
    @State private var active = true
    @State private var scope: SearchScope = .all

    var body: some View {
        List {
            if query.trimmingCharacters(in: .whitespaces).isEmpty {
                Section {
                    Text("Search tasks, docs, messages, people").font(.subheadline).foregroundStyle(.secondary)
                        .listRowBackground(Color.clear)
                    SearchHint()
                }
            } else {
                SearchResults(query: query, scope: scope)
            }
        }
        .calmList()
        .navigationTitle("Quick find")
        .navigationBarTitleDisplayMode(.inline)
        .toolbarRole(.editor)
        .searchable(text: $query, isPresented: $active, placement: .navigationBarDrawer(displayMode: .always), prompt: "Search tasks, docs, messages, people")
        .searchScopes($scope, activation: .onSearchPresentation) {
            ForEach(SearchScope.allCases, id: \.self) { s in Text(L(s.title)).tag(s) }
        }
    }
}

/// How to narrow the messages found (SearchQuery).
struct SearchHint: View {
    var body: some View {
        Text("Narrow messages with has:file, has:link or from:name")
            .font(.footnote)
            .foregroundStyle(.secondary)
            .listRowBackground(Color.clear)
            .accessibilityIdentifier("searchHint")
    }
}

enum SearchScope: Hashable, CaseIterable {
    case all, tasks, messages, docs, people

    var title: String {
        switch self {
        case .all: return "All"
        case .tasks: return "Tasks"
        case .messages: return "Messages"
        case .docs: return "Docs"
        case .people: return "People"
        }
    }
}

/// What a search finds.
struct SearchResults: View {
    @Environment(SyncClient.self) private var client
    let query: String
    var scope: SearchScope = .all

    var body: some View {
        let v = client.view
        let me = client.me ?? 0
        let raw = query.trimmingCharacters(in: .whitespaces)
        // has:file, has:link and from:name narrow the messages; with any
        // of them only messages are searched (SearchQuery in WorkboardKit)
        let parsed = SearchQuery(raw)
        let q = parsed.text
        let only = parsed.hasOperators
        let has = { (s: String) in s.localizedCaseInsensitiveContains(q) }
        let on = { (s: SearchScope) in (scope == .all || scope == s) && (!only || s == .messages) }
        let tasks = on(.tasks) ? Array(v.tasks.filter { has($0.name) || has($0.description) || $0.tags.contains(where: has) }.prefix(30)) : []
        let docs = on(.docs) ? Array(v.visibleDocs(me: me).filter { has($0.title) || has($0.body) }.prefix(20)) : []
        let channels = v.visibleChannels(me: me)
        let convs = on(.messages) && !only ? channels.filter { has(v.title(of: $0, me: me)) } : []
        let messages = on(.messages) ? Array(channels.flatMap { c in (c.messages + c.messages.flatMap(\.replies)).filter { parsed.matches($0, members: v.members, me: me) }.map { (c, $0) } }.suffix(30).reversed()) : []
        let people = on(.people) ? v.members.filter { has($0.name) || has($0.email) } : []
        if tasks.isEmpty && docs.isEmpty && convs.isEmpty && messages.isEmpty && people.isEmpty {
            Section {
                Text("No results for \"\(raw)\"").foregroundStyle(.secondary).accessibilityIdentifier("noResults")
                SearchHint()
            }
        }
        if !tasks.isEmpty {
            Section {
                ForEach(Array(tasks)) { t in
                    NavigationLink(value: Nav.Route.task(t.id)) { TaskRow(task: t, showList: true) }
                        .accessibilityIdentifier("result-task-\(t.id)")
                }
            } header: {
                CalmHeader("Tasks")
            }
        }
        if !docs.isEmpty {
            Section {
                ForEach(Array(docs)) { d in
                    NavigationLink(value: Nav.Route.doc(d.id)) { Label(d.title.isEmpty ? String(localized: "Untitled") : d.title, systemImage: "doc.text") }
                        .accessibilityIdentifier("result-doc-\(d.id)")
                }
            } header: {
                CalmHeader("Docs")
            }
        }
        if !convs.isEmpty {
            Section {
                ForEach(convs) { c in
                    NavigationLink(value: Nav.Route.channel(c.id)) {
                        Label(v.place(of: c, me: me), systemImage: c.kind == .channel ? "number" : "person.2")
                    }
                }
            } header: {
                CalmHeader("Conversations")
            }
        }
        if !messages.isEmpty {
            Section {
                ForEach(Array(messages), id: \.1.id) { c, m in
                    NavigationLink(value: Nav.Route.channel(c.id)) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(v.member(m.author)?.name ?? String(localized: "Someone")) in \(v.place(of: c, me: me))")
                                .font(.caption).foregroundStyle(.secondary)
                            Text(m.text).lineLimit(2)
                        }
                    }
                    .accessibilityIdentifier("result-message-\(m.id)")
                }
            } header: {
                CalmHeader("Messages")
            }
        }
        if !people.isEmpty {
            Section {
                ForEach(people) { p in
                    HStack(spacing: 10) {
                        Avatar(member: p, size: 24)
                        Text(p.name)
                        Spacer()
                        Text(p.role).font(.caption).foregroundStyle(.secondary)
                    }
                }
            } header: {
                CalmHeader("People")
            }
        }
    }
}
