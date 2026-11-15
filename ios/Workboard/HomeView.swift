import SwiftUI
import WorkboardKit

/// My Tasks: open tasks assigned to me, overdue and today first.
struct HomeView: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        let mine = v.myTasks(me: me)
        let today = dayString()
        let overdue = mine.filter { ($0.due ?? "9999") < today }.sorted { ($0.due ?? "") < ($1.due ?? "") }
        let due = mine.filter { $0.due == today }
        let later = mine.filter { ($0.due ?? "") > today }.sorted { ($0.due ?? "") < ($1.due ?? "") }
        let undated = mine.filter { $0.due == nil }
        List {
            if mine.isEmpty {
                EmptyNote(title: "Nothing assigned to you", systemImage: "checkmark.circle", detail: "Tasks assigned to you show up here.")
                    .listRowBackground(Color.clear)
            }
            section("Overdue", overdue, tint: .red)
            section("Today", due)
            section("Upcoming", later)
            section("No date", undated)
        }
        .navigationTitle("My Tasks")
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
                    Text(title).foregroundStyle(tint ?? .secondary)
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
                    if let due = task.due {
                        Label(Fmt.due(due), systemImage: "calendar")
                            .foregroundStyle(due < dayString() && !status.isClosed ? .red : .secondary)
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
