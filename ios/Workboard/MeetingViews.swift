// Meetings: attendees, agenda and notes (More tab).

import SwiftUI
import WorkboardKit

struct MeetingsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav

    var body: some View {
        let ms = client.data.map { client.view.meetings($0) } ?? []
        let today = dayString()
        List {
            if ms.isEmpty {
                EmptyNote(title: "No meetings yet", systemImage: "person.3").listRowBackground(Color.clear)
            }
            let upcoming = ms.filter { $0.date >= today }
            let past = ms.filter { $0.date < today }.reversed()
            if !upcoming.isEmpty { Section("Upcoming") { ForEach(upcoming) { row($0) } } }
            if !past.isEmpty { Section("Past") { ForEach(Array(past)) { row($0) } } }
        }
        .navigationTitle("Meetings")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("New meeting", id: "newMeetingButton") {
                    let id = client.newId()
                    if client.edit({ d, me in Edits.addMeeting(&d, title: "New meeting", date: dayString(), me: me, id: id) }) { nav.path.append(.meeting(id)) }
                }
            }
        }
    }

    func row(_ m: MeetingView) -> some View {
        NavigationLink(value: Nav.Route.meeting(m.id)) {
            VStack(alignment: .leading, spacing: 2) {
                Text(m.title).fontWeight(.medium)
                (Text("\(Fmt.due(m.date)) \(Fmt.clock(m.time)) · ") + Text(m.attendees.count == 1 ? "1 person" : "\(m.attendees.count) people"))
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .accessibilityIdentifier("meeting-\(m.id)")
    }
}

struct MeetingDetailView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let meetingId: UInt64
    @State private var title = ""
    @State private var agenda = ""
    @State private var notes = ""
    @State private var choosingList = false
    @State private var made: String?
    @FocusState private var focus: Field?

    enum Field { case title, agenda, notes }

    var body: some View {
        let v = client.view
        if let data = client.data, let m = v.meetings(data).first(where: { $0.id == meetingId }) {
            let can = client.canEdit
            Form {
                Section {
                    TextField("Title", text: $title).font(.title3.weight(.semibold)).focused($focus, equals: .title).accessibilityIdentifier("meetingTitle")
                    DatePicker("When", selection: Binding(get: { Self.when(m) }, set: { d in
                        client.edit { data, _ in Edits.updateMeeting(&data, meeting: m.id, date: dayString(d), time: timeString(d)) }
                    }))
                }
                .disabled(!can)
                Section("Attendees") {
                    ForEach(v.activeMembers) { p in
                        MemberToggleRow(member: p, on: m.attendees.contains(p.id)) {
                            client.edit { d, _ in Edits.toggleAttendee(&d, meeting: m.id, member: p.id) }
                        }
                        .disabled(!can)
                    }
                }
                Section("Agenda") {
                    TextField("What will you discuss?", text: $agenda, axis: .vertical).lineLimit(3...).focused($focus, equals: .agenda).disabled(!can)
                }
                Section {
                    TextField("Write notes", text: $notes, axis: .vertical).lineLimit(6...).focused($focus, equals: .notes).disabled(!can)
                        .accessibilityIdentifier("meetingNotes")
                    let items = MeetingView.items(in: notes)
                    if !items.isEmpty {
                        Button(items.count == 1 ? "Create 1 task from action items" : "Create \(items.count) tasks from action items") {
                            save(m)
                            focus = nil
                            choosingList = true
                        }
                        .disabled(!can)
                        .accessibilityIdentifier("meetingMakeTasks")
                    }
                    if let made { Text(made).font(.footnote).foregroundStyle(.secondary) }
                } header: {
                    Text("Notes")
                } footer: {
                    Text("Lines starting with [ ] become action items.")
                }
            }
            .navigationTitle("Meeting")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if focus != nil {
                    ToolbarItem(placement: .topBarTrailing) { Button("Done") { focus = nil } }
                } else {
                    ToolbarItem(placement: .topBarTrailing) {
                        Menu {
                            Button(role: .destructive) {
                                if client.edit({ d, _ in Edits.deleteMeeting(&d, meeting: m.id) }) { dismiss() }
                            } label: { Label("Delete meeting", systemImage: "trash") }
                        } label: { Image(systemName: "ellipsis.circle") }
                            .disabled(!can)
                    }
                }
            }
            .sheet(isPresented: $choosingList) {
                ListPickerSheet(current: 0) { list in
                    var n = 0
                    client.edit { d, me in n = Edits.meetingActionsToTasks(&d, meeting: m.id, list: list, me: me, ts: nowTs()) { client.newId() } }
                    // what is on screen is what was saved, now ticked off
                    if n > 0 { notes = notes.replacingOccurrences(of: "[ ]", with: "[x]") }
                    let where_ = client.view.list(list)?.name ?? String(localized: "the list")
                    made = n == 1 ? String(localized: "Made 1 task in \(where_).") : String(localized: "Made \(n) tasks in \(where_).")
                }
            }
            .onAppear { title = m.title; agenda = m.agenda; notes = m.notes }
            .onChange(of: m.notes) { if focus != .notes { notes = m.notes } }
            .onChange(of: m.agenda) { if focus != .agenda { agenda = m.agenda } }
            .onChange(of: m.title) { if focus != .title { title = m.title } }
            .onChange(of: focus) { _, _ in save(m) }
            .onDisappear { save(m) }
        } else {
            EmptyNote(title: "This meeting is gone", systemImage: "person.3")
        }
    }

    func save(_ m: MeetingView) {
        guard title != m.title || agenda != m.agenda || notes != m.notes else { return }
        client.edit { d, _ in
            Edits.updateMeeting(&d, meeting: m.id, title: title != m.title ? title : nil, agenda: agenda != m.agenda ? agenda : nil, notes: notes != m.notes ? notes : nil)
        }
    }

    static func when(_ m: MeetingView) -> Date {
        DayFormat.date(day: m.date, time: m.time) ?? Date()
    }
}

extension MeetingView {
    /// Action items in notes being typed (before they are saved).
    static func items(in notes: String) -> [String] {
        notes.components(separatedBy: "\n").compactMap { l in
            let t = l.trimmingCharacters(in: .whitespaces)
            guard t.hasPrefix("[ ]") else { return nil }
            let s = t.dropFirst(3).trimmingCharacters(in: .whitespaces)
            return s.isEmpty ? nil : s
        }
    }
}
