import SwiftUI
import WorkboardKit

/// Home: the Inbox, then my open tasks (overdue and today first), and a
/// search over everything.
struct HomeView: View {
    @Environment(SyncClient.self) private var client
    @State private var query = ""

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        let mine = v.myTasks(me: me)
        let today = dayString()
        let now = timeString()
        // by day, then by time within it (a day without one last)
        let key = { (t: TaskView) in (t.due ?? "") + (t.dueTime ?? "99:99") }
        let overdue = mine.filter { $0.isOverdue(today: today, now: now) }.sorted { key($0) < key($1) }
        let due = mine.filter { $0.due == today && !$0.isOverdue(today: today, now: now) }.sorted { key($0) < key($1) }
        let later = mine.filter { ($0.due ?? "") > today }.sorted { key($0) < key($1) }
        let undated = mine.filter { $0.due == nil }
        List {
            if !query.trimmingCharacters(in: .whitespaces).isEmpty {
                SearchResults(query: query)
            } else {
                InboxView()
                if mine.isEmpty {
                    EmptyNote(title: "Nothing assigned to you", systemImage: "checkmark.circle", detail: "Tasks assigned to you show up here.")
                        .listRowBackground(Color.clear)
                }
                section("Overdue", overdue, tint: .red)
                section("Today", due)
                section("Upcoming", later)
                section("No date", undated)
            }
        }
        .searchable(text: $query, prompt: "Search tasks, docs, messages, people")
        .navigationTitle("Home")
        .toolbar { ToolbarItem(placement: .topBarTrailing) { SettingsButton() } }
    }

    @ViewBuilder
    func section(_ title: String, _ tasks: [TaskView], tint: Color? = nil) -> some View {
        if !tasks.isEmpty {
            Section {
                ForEach(tasks) { t in
                    NavigationLink(value: Nav.Route.task(t.id)) { TaskRow(task: t, showList: true) }
                }
            } header: {
                HStack {
                    Text(L(title)).foregroundStyle(tint ?? .secondary)
                    Spacer()
                    Text("\(tasks.count)").monospacedDigit()
                }
            }
        }
    }
}

struct TaskRow: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    var showList = false

    var body: some View {
        let v = client.view
        let status = v.status(of: task)
        HStack(spacing: 12) {
            StatusDot(status: status, size: 14)
            VStack(alignment: .leading, spacing: 3) {
                Text(task.name)
                    .strikethrough(status.isClosed)
                    .foregroundStyle(status.isClosed ? .secondary : .primary)
                    .lineLimit(2)
                HStack(spacing: 6) {
                    if showList, let l = v.list(task.listId) {
                        Text(l.name)
                    }
                    if let due = Fmt.due(task) {
                        Label(due, systemImage: "calendar")
                            .foregroundStyle(task.isOverdue(today: dayString(), now: timeString()) && !status.isClosed ? .red : .secondary)
                    }
                    if task.priority == 1 {
                        Label("Urgent", systemImage: "flag.fill").foregroundStyle(.red)
                    } else if task.priority == 2 {
                        Label("High", systemImage: "flag.fill").foregroundStyle(.orange)
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                .labelStyle(.titleAndIcon)
            }
            Spacer(minLength: 0)
            HStack(spacing: -6) {
                ForEach(task.assignees.prefix(3), id: \.self) { id in
                    Avatar(member: v.member(id), size: 22)
                        .overlay(Circle().stroke(Color(.systemBackground), lineWidth: 1.5))
                }
            }
        }
        .padding(.vertical, 2)
    }
}
