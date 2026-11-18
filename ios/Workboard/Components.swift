// Small pieces many screens use: a menu row with a checkmark, the sheet's
// Cancel button, the toolbar "+", the delete swipe, and a person row to
// tick on or off.

import SwiftUI
import WorkboardKit

/// A menu row with a checkmark when it is the chosen one. Words written in
/// the code are shown in the phone's language; names people typed are not.
struct CheckedLabel: View {
    private enum Title {
        case key(LocalizedStringKey)
        case verbatim(String)
    }

    private let title: Title
    private let checked: Bool

    init(_ title: LocalizedStringKey, checked: Bool) {
        self.title = .key(title)
        self.checked = checked
    }

    @_disfavoredOverload
    init<S: StringProtocol>(_ title: S, checked: Bool) {
        self.title = .verbatim(String(title))
        self.checked = checked
    }

    var body: some View {
        switch title {
        case .key(let k):
            if checked { Label(k, systemImage: "checkmark") } else { Text(k) }
        case .verbatim(let s):
            if checked { Label(s, systemImage: "checkmark") } else { Text(s) }
        }
    }
}

extension Binding where Value == Bool {
    /// True while `value` holds something; setting it false clears `value`
    /// (an alert or dialog shown for the item it holds).
    init<T>(isPresent value: Binding<T?>) {
        self.init(get: { value.wrappedValue != nil }, set: { if !$0 { value.wrappedValue = nil } })
    }
}

/// "Cancel" at the top left of a sheet or an editor.
struct CancelButton: ToolbarContent {
    let action: () -> Void

    var body: some ToolbarContent {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { action() } }
    }
}

/// The toolbar "+" that adds something; off while the workspace can't be
/// edited (unless `needsEdit` is false: things kept only on this phone).
struct AddButton: View {
    @Environment(SyncClient.self) private var client
    let label: LocalizedStringKey
    let id: String
    var needsEdit = true
    let action: () -> Void

    init(_ label: LocalizedStringKey, id: String, needsEdit: Bool = true, action: @escaping () -> Void) {
        self.label = label
        self.id = id
        self.needsEdit = needsEdit
        self.action = action
    }

    var body: some View {
        Button { action() } label: { Image(systemName: "plus") }
            .disabled(needsEdit && !client.canEdit)
            .accessibilityLabel(label)
            .accessibilityIdentifier(id)
    }
}

extension View {
    /// Swipe left to delete, offered only when `enabled`.
    func deleteSwipe(_ enabled: Bool, _ title: LocalizedStringKey = "Delete", action: @escaping () -> Void) -> some View {
        swipeActions {
            if enabled { Button(title, role: .destructive) { action() } }
        }
    }
}

/// A person to tick on or off (meeting attendees, team members).
struct MemberToggleRow: View {
    let member: MemberView
    let on: Bool
    let toggle: () -> Void

    var body: some View {
        Button { toggle() } label: {
            HStack {
                Avatar(member: member, size: 22)
                Text(member.name).foregroundStyle(.primary)
                Spacer()
                if on { Image(systemName: "checkmark").foregroundStyle(Color.accentColor) }
            }
        }
    }
}
