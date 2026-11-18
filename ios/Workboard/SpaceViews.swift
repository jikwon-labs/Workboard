import SwiftUI
import WorkboardKit

// Making and changing spaces, folders and lists, and a space's statuses, as
// the Mac sidebar's menus do.

/// What a name prompt is for.
enum NamePrompt: Identifiable, Equatable {
    case newSpace
    case renameSpace(UInt64)
    case newList(space: UInt64, folder: UInt64?)
    case newFolder(UInt64)
    case renameFolder(UInt64)
    case renameList(UInt64)

    var id: String {
        switch self {
        case .newSpace: return "newSpace"
        case .renameSpace(let s): return "renameSpace-\(s)"
        case .newList(let s, let f): return "newList-\(s)-\(f ?? 0)"
        case .newFolder(let s): return "newFolder-\(s)"
        case .renameFolder(let f): return "renameFolder-\(f)"
        case .renameList(let l): return "renameList-\(l)"
        }
    }

    // keys, not plain strings, so the alert shows them in the phone's language
    var title: LocalizedStringKey {
        switch self {
        case .newSpace: return "New space"
        case .renameSpace: return "Rename space"
        case .newList: return "New list"
        case .newFolder: return "New folder"
        case .renameFolder: return "Rename folder"
        case .renameList: return "Rename list"
        }
    }

    var action: LocalizedStringKey {
        switch self {
        case .newSpace, .newList, .newFolder: return "Create"
        default: return "Save"
        }
    }
}

/// Asks for a name and does what the prompt is for.
struct NamePromptModifier: ViewModifier {
    @Environment(SyncClient.self) private var client
    @Binding var prompt: NamePrompt?
    @State private var text = ""

    func body(content: Content) -> some View {
        content.alert(prompt?.title ?? "", isPresented: Binding(isPresent: $prompt)) {
            TextField("Name", text: $text)
            Button("Cancel", role: .cancel) { prompt = nil }
            Button(prompt?.action ?? "Save") { apply() }
        }
        .onChange(of: prompt) { _, p in text = p.map(current) ?? "" }
    }

    func current(_ p: NamePrompt) -> String {
        let v = client.view
        switch p {
        case .renameSpace(let s): return v.spaces.first { $0.id == s }?.name ?? ""
        case .renameFolder(let f): return v.spaces.flatMap(\.folders).first { $0.id == f }?.name ?? ""
        case .renameList(let l): return v.list(l)?.name ?? ""
        default: return ""
        }
    }

    func apply() {
        guard let p = prompt else { return }
        let name = text
        prompt = nil
        switch p {
        case .newSpace:
            let id = client.newId(), list = client.newId()
            let color = Edits.statusPalette[1 + Int.random(in: 0..<(Edits.statusPalette.count - 1))]
            client.edit { d, _ in Edits.createSpace(&d, name: name, color: color, id: id, listId: list) }
        case .renameSpace(let s): client.edit { d, _ in Edits.renameSpace(&d, space: s, to: name) }
        case .newList(let s, let f):
            let id = client.newId()
            client.edit { d, _ in Edits.createList(&d, space: s, folder: f, name: name, id: id) }
        case .newFolder(let s):
            let id = client.newId()
            client.edit { d, _ in Edits.createFolder(&d, space: s, name: name, id: id) }
        case .renameFolder(let f): client.edit { d, _ in Edits.renameFolder(&d, folder: f, to: name) }
        case .renameList(let l): client.edit { d, _ in Edits.renameList(&d, list: l, to: name) }
        }
    }
}

extension View {
    func namePrompt(_ prompt: Binding<NamePrompt?>) -> some View { modifier(NamePromptModifier(prompt: prompt)) }
}

/// The colours a space can have.
struct ColorMenu: View {
    let title: String
    let pick: ([Int]) -> Void

    var body: some View {
        Menu {
            ForEach(Edits.statusPalette.indices, id: \.self) { i in
                Button {
                    pick(Edits.statusPalette[i])
                } label: {
                    Label(Self.names[i], systemImage: "circle.fill")
                }
                .tint(Color(rgb: Edits.statusPalette[i]))
            }
        } label: {
            Label(title, systemImage: "paintpalette")
        }
    }

    static let names = ["Grey", "Purple", "Blue", "Sky", "Teal", "Mint", "Green", "Yellow", "Orange", "Red", "Pink", "Violet", "Brown", "Slate"]
}

/// A space's statuses: name, kind and colour, added and removed. Renames
/// reach the space's tasks and automations; a removed status's tasks move to
/// one of the same kind.
struct StatusEditorView: View {
    @Environment(SyncClient.self) private var client
    let spaceId: UInt64

    var body: some View {
        let statuses = client.view.spaces.first { $0.id == spaceId }?.statuses ?? []
        List {
            Section {
                ForEach(Array(statuses.enumerated()), id: \.offset) { i, st in
                    StatusRow(spaceId: spaceId, index: i, status: st, canDelete: statuses.count > 1)
                }
                Button {
                    client.edit { d, _ in Edits.addStatus(&d, space: spaceId) }
                } label: {
                    Label("Add status", systemImage: "plus")
                }
                .disabled(!client.canEdit)
                .accessibilityIdentifier("addStatusButton")
            } footer: {
                Text("Renamed statuses are renamed on their tasks too; tasks of a deleted status move to a status of the same kind.")
            }
        }
        .navigationTitle("Statuses")
        .navigationBarTitleDisplayMode(.inline)
    }
}

private struct StatusRow: View {
    @Environment(SyncClient.self) private var client
    let spaceId: UInt64
    let index: Int
    let status: StatusView
    let canDelete: Bool
    @State private var name = ""
    @State private var taken = false
    @FocusState private var focused: Bool

    static let kinds: [(StatusKind, String)] = [(.open, "Not started"), (.custom, "Active"), (.done, "Done"), (.closed, "Closed")]

    var body: some View {
        HStack(spacing: 10) {
            Menu {
                ForEach(Edits.statusPalette.indices, id: \.self) { i in
                    Button(ColorMenu.names[i]) {
                        client.edit { d, _ in Edits.setStatusColor(&d, space: spaceId, index: index, to: Edits.statusPalette[i]) }
                    }
                }
            } label: {
                StatusDot(status: status, size: 14)
            }
            .accessibilityLabel("Colour of \(status.name)")
            TextField("Status name", text: $name)
                .textInputAutocapitalization(.never)
                .focused($focused)
                .submitLabel(.done)
                .onSubmit(save)
                .foregroundStyle(taken ? .red : .primary)
                .accessibilityIdentifier("statusName-\(index)")
            Menu {
                ForEach(Self.kinds, id: \.0) { k, label in
                    Button {
                        client.edit { d, _ in Edits.setStatusKind(&d, space: spaceId, index: index, to: k) }
                    } label: {
                        CheckedLabel(L(label), checked: k == status.kind)
                    }
                }
            } label: {
                Text(L(Self.kinds.first { $0.0 == status.kind }?.1 ?? "")).font(.caption).foregroundStyle(.secondary)
            }
            .accessibilityIdentifier("statusKind-\(index)")
        }
        .disabled(!client.canEdit)
        .deleteSwipe(canDelete && client.canEdit) {
            client.edit { d, me in Edits.deleteStatus(&d, space: spaceId, index: index, me: me, ts: nowTs()) }
        }
        .onAppear { name = status.name }
        .onChange(of: status.name) { if !focused { name = status.name } }
        .onChange(of: focused) { _, now in if !now { save() } }
    }

    /// An empty or taken name goes back to the status's name.
    func save() {
        let clean = name.trimmed
        guard clean != status.name else { taken = false; return }
        let ok = client.edit { d, _ in
            if !Edits.renameStatus(&d, space: spaceId, index: index, to: clean, ts: nowTs()) { taken = true }
        }
        if taken || !ok {
            name = status.name
            taken = false
        }
    }
}
