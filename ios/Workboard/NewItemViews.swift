import SwiftUI
import WorkboardKit

/// Home's "+": make a task, send a message or start a doc, picked at the top.
struct NewItemSheet: View {
    enum Kind: Hashable { case task, message, doc }

    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @Environment(Nav.self) private var nav
    @Environment(\.dismiss) private var dismiss
    @State var kind: Kind = .task
    var dueToday = false

    @State private var name = ""
    @State private var note = ""
    @State private var due: String?
    @State private var list: UInt64?
    @State private var assignToMe = true
    @State private var channel: UInt64?
    /// someone to start a direct message with, when there is none yet
    @State private var person: UInt64?
    @State private var docSpace: UInt64?
    @FocusState private var focused: Bool
    @AppStorage("newTask.list") private var lastList = 0

    init(kind: Kind = .task, dueToday: Bool = false) {
        _kind = State(initialValue: kind)
        self.dueToday = dueToday
    }

    var body: some View {
        VStack(spacing: 0) {
            Picker("Create new", selection: $kind) {
                Text("Task").tag(Kind.task)
                Text("Message").tag(Kind.message)
                Text("Doc").tag(Kind.doc)
            }
            .pickerStyle(.segmented)
            .padding(14)
            .accessibilityIdentifier("newKind")
            HStack(alignment: .top, spacing: 12) {
                RoundedRectangle(cornerRadius: kind == .message ? 10 : kind == .task ? 6 : 4, style: .continuous)
                    .strokeBorder(Color(.tertiaryLabel), lineWidth: 2)
                    .frame(width: 20, height: 20)
                    .padding(.top, 3)
                VStack(alignment: .leading, spacing: 8) {
                    TextField(kind == .task ? "New task" : kind == .message ? "Message" : "Untitled doc", text: $name, axis: .vertical)
                        .font(.system(size: 19, weight: .semibold))
                        .focused($focused)
                        .lineLimit(1...4)
                        .accessibilityIdentifier("newItemName")
                    if kind == .task {
                        TextField("Notes", text: $note, axis: .vertical)
                            .font(.system(size: 15))
                            .foregroundStyle(.secondary)
                            .lineLimit(1...4)
                            .accessibilityIdentifier("newItemNote")
                    }
                }
            }
            .padding(.horizontal, 18)
            .padding(.bottom, 16)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) { chips }
                    .padding(.horizontal, 14)
                    .padding(.vertical, 12)
            }
            .background(Color(.secondarySystemBackground))
            .overlay(alignment: .top) { Divider() }
            Divider()
            HStack(spacing: 8) {
                Spacer()
                Button("Cancel") { dismiss() }
                    .frame(height: 44)
                    .padding(.horizontal, 12)
                Button(action: save) {
                    Text(kind == .task ? "Add" : kind == .message ? "Send" : "Create")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 20)
                        .frame(height: 44)
                        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.wbAccent))
                }
                .buttonStyle(.plain)
                .disabled(!ready)
                .opacity(ready ? 1 : 0.5)
                .accessibilityIdentifier("newItemSave")
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            Spacer(minLength: 0)
        }
        .presentationDetents([.height(330), .large])
        .presentationDragIndicator(.visible)
        .onAppear {
            if dueToday { due = dayString() }
            list = defaultList
            docSpace = client.view.visibleSpaces(me: client.me ?? 0).first { $0.privateOwner == nil }?.id
            focused = true
        }
    }

    var ready: Bool {
        guard client.canEdit, !name.isBlank else { return false }
        switch kind {
        case .task: return list != nil
        case .message: return channel != nil || person != nil
        case .doc: return true
        }
    }

    var allLists: [(space: SpaceView, list: ListView)] {
        client.view.visibleSpaces(me: client.me ?? 0).flatMap { s in
            (s.lists + s.folders.flatMap(\.lists)).map { (s, $0) }
        }
    }

    var defaultList: UInt64? {
        let lists = allLists
        if lists.contains(where: { $0.list.id == UInt64(lastList) }) { return UInt64(lastList) }
        return lists.first?.list.id
    }

    @ViewBuilder var chips: some View {
        let v = client.view
        let me = client.me ?? 0
        switch kind {
        case .task:
            Menu {
                Button { due = dayString() } label: { CheckedLabel("Today", checked: due == dayString()) }
                Button { due = dayString(Date().addingTimeInterval(86400)) } label: {
                    CheckedLabel("Tomorrow", checked: due == dayString(Date().addingTimeInterval(86400)))
                }
                Button { due = nil } label: { CheckedLabel("No date", checked: due == nil) }
            } label: {
                chip(icon: "calendar", text: due.map { Text(verbatim: Fmt.due($0)) } ?? Text("Due"))
            }
            .accessibilityIdentifier("newItemDue")
            Menu {
                ForEach(allLists, id: \.list.id) { s, l in
                    Button { list = l.id } label: {
                        CheckedLabel("\(s.privateOwner != nil ? String(localized: "Personal") : s.name) · \(l.name)", checked: list == l.id)
                    }
                }
            } label: {
                chip(icon: "list.bullet", text: list.flatMap { v.list($0) }.map { Text(verbatim: $0.name) } ?? Text("List"))
            }
            .accessibilityIdentifier("newItemList")
            Button { assignToMe.toggle() } label: {
                chip(icon: assignToMe ? "person.fill" : "person", text: Text(assignToMe ? "Me" : "Nobody"))
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("newItemAssignee")
        case .message:
            let convs = v.visibleChannels(me: me)
            let withDM = Set(convs.filter { $0.kind == .direct }.flatMap(\.members))
            let people = v.members.filter { !$0.invited && $0.id != me && !withDM.contains($0.id) }
            Menu {
                Section("Channels") {
                    ForEach(convs.filter { $0.kind == .channel }) { c in
                        Button { channel = c.id; person = nil } label: { CheckedLabel("#\(c.name)", checked: channel == c.id) }
                    }
                }
                Section("Direct messages") {
                    ForEach(convs.filter { $0.kind == .direct }) { c in
                        Button { channel = c.id; person = nil } label: { CheckedLabel(v.title(of: c, me: me), checked: channel == c.id) }
                    }
                    ForEach(people) { m in
                        Button { person = m.id; channel = nil } label: { CheckedLabel(m.name, checked: person == m.id) }
                    }
                }
            } label: {
                let title = channel.flatMap { id in convs.first { $0.id == id } }.map { v.place(of: $0, me: me) }
                    ?? person.flatMap { v.member($0)?.name }
                chip(icon: "number", text: title.map { Text(verbatim: $0) } ?? Text("To"))
            }
            .accessibilityIdentifier("newItemTo")
        case .doc:
            let spaces = v.visibleSpaces(me: me)
            Menu {
                ForEach(spaces) { s in
                    Button { docSpace = s.id } label: {
                        CheckedLabel(s.privateOwner != nil ? String(localized: "Personal") : s.name, checked: docSpace == s.id)
                    }
                }
                Button { docSpace = nil } label: { CheckedLabel("No space", checked: docSpace == nil) }
            } label: {
                let s = spaces.first { $0.id == docSpace }
                chip(icon: "shippingbox", text: s.map { Text(verbatim: $0.privateOwner != nil ? String(localized: "Personal") : $0.name) } ?? Text("No space"))
            }
            .accessibilityIdentifier("newItemSpace")
        }
    }

    func chip(icon: String, text: Text) -> some View {
        HStack(spacing: 6) {
            Image(systemName: icon).font(.system(size: 13, weight: .medium))
            text.lineLimit(1)
        }
        .font(.system(size: 14, weight: .medium))
        .foregroundStyle(.primary)
        .padding(.horizontal, 11)
        .frame(height: 32)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color(.systemBackground)))
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Color(.separator).opacity(0.5)))
    }

    func save() {
        guard ready else { return }
        let text = name.trimmed
        switch kind {
        case .task:
            guard let list else { return }
            let id = client.newId()
            let (d, n, mine) = (due, note, assignToMe)
            let ok = client.edit { data, me in
                Edits.addTask(&data, list: list, name: text, assignees: mine ? [me] : [], me: me, id: id, ts: nowTs())
                if let d { Edits.setDue(&data, task: id, to: d, me: me, ts: nowTs()) }
                if !n.isBlank { Edits.setDescription(&data, task: id, to: n, me: me, ts: nowTs()) }
            }
            if ok {
                lastList = Int(list)
                dismiss()
            }
        case .message:
            var to = channel
            if to == nil, let person {
                let newId = client.newId()
                client.edit { d, me in to = Edits.openDM(&d, with: person, me: me, id: newId, ts: nowTs()) }
            }
            guard let to else { return }
            // as the channel's message box sends: "/" commands, workflows, link previews
            if sendChannelMessage(client, notifier: notifier, channel: to, text: text) {
                dismiss()
            }
        case .doc:
            let id = client.newId()
            let space = docSpace
            guard client.edit({ d, me in Edits.addDoc(&d, title: text, space: space, me: me, id: id, ts: nowTs()) }) else { return }
            dismiss()
            nav.newDoc = id
            nav.path.append(.doc(id))
        }
    }
}
