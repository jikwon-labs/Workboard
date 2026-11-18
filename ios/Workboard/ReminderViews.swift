// Reminders and the notepad, kept on this iPhone like the Mac's (More tab).

import SwiftUI
import UserNotifications
import WorkboardKit

enum ReminderAlerts {
    /// A local notification at the reminder's time (replaced when it changes).
    static func schedule(id: UInt64, text: String, at ts: Int64) {
        let c = UNMutableNotificationContent()
        c.title = "Reminder"
        c.body = text
        c.sound = .default
        let when = Date(timeIntervalSince1970: TimeInterval(ts))
        guard when > Date() else { return }
        let parts = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: when)
        let req = UNNotificationRequest(identifier: "reminder-\(id)", content: c, trigger: UNCalendarNotificationTrigger(dateMatching: parts, repeats: false))
        UNUserNotificationCenter.current().add(req)
    }

    static func cancel(_ id: UInt64) {
        UNUserNotificationCenter.current().removePendingNotificationRequests(withIdentifiers: ["reminder-\(id)"])
    }
}

struct RemindersView: View {
    @Environment(SyncClient.self) private var client
    @State private var adding = false

    var body: some View {
        let me = client.me ?? 0
        let rs = client.data.map { client.view.reminders($0, me: me) } ?? []
        let now = nowTs()
        List {
            if rs.isEmpty {
                EmptyNote(title: "No reminders", systemImage: "alarm", detail: "Reminders are kept on this iPhone.").listRowBackground(Color.clear)
            }
            let open = rs.filter { !$0.done }
            let done = rs.filter(\.done)
            if !open.isEmpty { Section("To do") { ForEach(open) { row($0, overdue: $0.dueTs <= now) } } }
            if !done.isEmpty { Section("Done") { ForEach(done) { row($0, overdue: false) } } }
        }
        .navigationTitle("Reminders")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New reminder", id: "newReminderButton", needsEdit: false) { adding = true }
            }
        }
        .sheet(isPresented: $adding) { NewReminderView(task: nil) }
    }

    func row(_ r: ReminderView, overdue: Bool) -> some View {
        HStack(spacing: 10) {
            Button {
                client.editLocal { d in Edits.setReminderDone(&d, reminder: r.id, !r.done) }
                if !r.done { ReminderAlerts.cancel(r.id) } else { ReminderAlerts.schedule(id: r.id, text: r.text, at: r.dueTs) }
            } label: {
                Image(systemName: r.done ? "checkmark.circle.fill" : "circle").foregroundStyle(r.done ? .green : .secondary)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(r.done ? "Mark not done" : "Mark done")
            .accessibilityIdentifier("reminderDone-\(r.id)")
            VStack(alignment: .leading, spacing: 2) {
                Text(r.text).strikethrough(r.done)
                Text(Fmt.time(r.dueTs)).font(.caption).foregroundStyle(overdue ? .red : .secondary)
                if let t = r.task, let task = client.view.tasks.first(where: { $0.id == t }) {
                    NavigationLink(value: Nav.Route.task(t)) { Label(task.name, systemImage: "checklist").font(.caption) }
                }
            }
        }
        .swipeActions {
            Button("Delete", role: .destructive) {
                ReminderAlerts.cancel(r.id)
                client.editLocal { d in Edits.deleteReminder(&d, reminder: r.id) }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("reminder-\(r.id)")
    }
}

/// A new reminder, about a task when made from one ("Remind me").
struct NewReminderView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @Environment(\.dismiss) private var dismiss
    let task: TaskView?
    @State private var text = ""
    @State private var when = Date().addingTimeInterval(3600)

    var body: some View {
        NavigationStack {
            Form {
                TextField("Remind me to...", text: $text).accessibilityIdentifier("reminderText")
                DatePicker("When", selection: $when)
            }
            .navigationTitle("New reminder")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        let id = client.newId(), ts = Int64(when.timeIntervalSince1970), me = client.me ?? 0, t = text
                        client.editLocal { d in Edits.addReminder(&d, text: t, dueTs: ts, task: task?.id, me: me, id: id) }
                        Task {
                            await notifier.requestPermission()
                            ReminderAlerts.schedule(id: id, text: t, at: ts)
                        }
                        dismiss()
                    }
                    .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("reminderAdd")
                }
            }
            .onAppear { if let task, text.isEmpty { text = task.name } }
        }
    }
}

struct NotepadView: View {
    @Environment(SyncClient.self) private var client
    @State private var text = ""
    @FocusState private var focused: Bool
    @Environment(\.scenePhase) private var scenePhase
    /// The save a second after the last keystroke.
    @State private var pending: Task<Void, Never>?

    var body: some View {
        TextEditor(text: $text)
            .focused($focused)
            .padding(.horizontal, 12)
            .navigationTitle("Notepad")
            .navigationBarTitleDisplayMode(.inline)
            .accessibilityIdentifier("notepad")
            .onAppear { text = client.data?.at("extra.notepad")?.string ?? "" }
            // saved a moment after typing stops, not on every key (each save
            // copies the whole workspace); leaving the field, the screen or
            // the app saves right away
            .onChange(of: text) { _, _ in saveSoon() }
            .onChange(of: focused) { _, f in if !f { save() } }
            // the app's own flush may run before this one
            .onChange(of: scenePhase) { _, p in if p != .active { save(); client.flushSnapshot() } }
            .onDisappear { save() }
            .toolbar { if focused { ToolbarItem(placement: .topBarTrailing) { Button("Done") { focused = false } } } }
    }

    func saveSoon() {
        pending?.cancel()
        pending = Task {
            try? await Task.sleep(nanoseconds: 1_000_000_000)
            guard !Task.isCancelled else { return }
            save()
        }
    }

    func save() {
        pending?.cancel()
        pending = nil
        let t = text
        client.editLocal { d in Edits.setNotepad(&d, to: t) }
    }
}
