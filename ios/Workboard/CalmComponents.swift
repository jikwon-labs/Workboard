// The parts every screen is built from: a light grey page with white
// section cards, a big title with a coloured icon under the back button,
// four kinds of rows (a place, a task, a person or conversation, a
// setting), the floating "+" at the bottom right, and the small tags on
// tasks (overdue red, today yellow, a time purple).
//
// Most of it is SwiftUI's inset grouped List; only what it doesn't give is
// drawn here.

import SwiftUI
import WorkboardKit

enum Calm {
    /// The page behind the cards (#F2F2F5; the system's grouped background in dark mode).
    static let page = Color(UIColor { t in
        t.userInterfaceStyle == .dark ? .systemGroupedBackground : UIColor(red: 0xF2 / 255, green: 0xF2 / 255, blue: 0xF5 / 255, alpha: 1)
    })
    /// A card (white; the system's raised grouped colour in dark mode).
    static let card = Color(UIColor { t in t.userInterfaceStyle == .dark ? .secondarySystemGroupedBackground : .white })

    static func dynamic(light: UInt32, dark: UInt32) -> Color {
        Color(UIColor { t in
            let c = t.userInterfaceStyle == .dark ? dark : light
            return UIColor(red: CGFloat((c >> 16) & 0xFF) / 255, green: CGFloat((c >> 8) & 0xFF) / 255, blue: CGFloat(c & 0xFF) / 255, alpha: 1)
        })
    }
}

// MARK: places

/// The places the first screen leads to.
enum Place: String, CaseIterable, Hashable {
    case inbox, today, upcoming, chats, docs, tools

    var title: String {
        switch self {
        case .inbox: return "Inbox"
        case .today: return "Today"
        case .upcoming: return "Upcoming"
        case .chats: return "Chat"
        case .docs: return "Docs"
        case .tools: return "Tools"
        }
    }

    var icon: String {
        switch self {
        case .inbox: return "tray"
        case .today: return "star.fill"
        case .upcoming: return "calendar"
        case .chats: return "bubble.left"
        case .docs: return "doc.text"
        case .tools: return "square.grid.2x2"
        }
    }

    var color: Color {
        switch self {
        case .inbox: return Calm.dynamic(light: 0x2F6FDB, dark: 0x5B92F0)
        case .today: return Calm.dynamic(light: 0xF5C211, dark: 0xF5C211)
        case .upcoming: return Calm.dynamic(light: 0xD93036, dark: 0xF0585D)
        case .chats: return .wbAccent
        case .docs: return .secondary
        case .tools: return Calm.dynamic(light: 0x2C7A7B, dark: 0x4FB3B4)
        }
    }

    /// Settings > Show on Home can hide these; the rest are always there.
    var canHide: Bool { self == .upcoming || self == .docs || self == .tools }

    /// The UserDefaults key for whether it is shown on Home.
    var shownKey: String { "home.show.\(rawValue)" }
}

// MARK: screen parts

extension View {
    /// A list screen: grey page, white section cards.
    func calmList() -> some View {
        listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(Calm.page)
            .listSectionSpacing(18)
    }

    /// The bar of a pushed screen: only "<" (and the screen's own buttons).
    /// The title stays set, for VoiceOver and the screens' tests, but is
    /// drawn big in the page by CalmTitle instead of in the bar.
    func calmBar(_ title: String) -> some View {
        navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbarRole(.editor)
            .toolbar { ToolbarItem(placement: .principal) { Color.clear.frame(width: 1, height: 1).accessibilityHidden(true) } }
    }

    /// A grey pill behind the content: the message box's suggestion chips
    /// and the filter banner over a conversation.
    func pill(horizontal: CGFloat = 10, vertical: CGFloat = 5) -> some View {
        padding(.horizontal, horizontal)
            .padding(.vertical, vertical)
            .background(Capsule().fill(Color(.secondarySystemBackground)))
    }
}

/// The big title under "<", with the place's coloured icon.
struct CalmTitle<Icon: View>: View {
    let title: Text
    let icon: Icon
    var detail: Text?

    var body: some View {
        Section {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 10) {
                    icon
                    title
                        .font(.system(size: 30, weight: .bold))
                        .lineLimit(2)
                        .accessibilityAddTraits(.isHeader)
                }
                if let detail {
                    detail.font(.subheadline).foregroundStyle(.secondary)
                }
            }
            .listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: 0, leading: 8, bottom: 0, trailing: 8))
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("screenTitle")
        }
    }
}

extension CalmTitle where Icon == AnyView {
    init(_ key: LocalizedStringKey, icon: String? = nil, color: Color = .primary) {
        title = Text(key)
        self.icon = AnyView(icon.map { Image(systemName: $0).font(.system(size: 25, weight: .semibold)).foregroundStyle(color) })
    }

    init(place: Place) {
        self.init(L(place.title), icon: place.icon, color: place.color)
    }

    init(verbatim name: String, detail: String? = nil, icon: some View) {
        title = Text(verbatim: name)
        self.icon = AnyView(icon)
        self.detail = detail.map { Text(verbatim: $0) }
    }
}

/// A section's title: 13pt grey, as written (no capitals).
struct CalmHeader: View {
    let text: Text

    init(_ key: LocalizedStringKey) { text = Text(key) }
    init(verbatim s: String) { text = Text(verbatim: s) }
    init(text: Text) { self.text = text }

    var body: some View {
        text
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(.secondary)
            .textCase(nil)
    }
}

// MARK: rows

/// A place: coloured icon, name, and a count or a badge.
struct PlaceRow: View {
    let place: Place
    var count: Int = 0
    var badge: Int = 0
    var badgeColor: Color = .wbAccent
    /// the badge counts overdue tasks, not unread messages
    var overdue = false

    var body: some View {
        HStack(spacing: 14) {
            Image(systemName: place.icon)
                .font(.system(size: 18, weight: .medium))
                .foregroundStyle(place.color)
                .frame(width: 26)
            Text(L(place.title)).font(.system(size: 17))
            Spacer(minLength: 8)
            if badge > 0 { CountBadge(count: badge, color: badgeColor, overdue: overdue) }
            if count > 0 {
                Text("\(count)").font(.system(size: 15)).foregroundStyle(.secondary).monospacedDigit()
            }
        }
        .frame(minHeight: 30)
    }
}

/// White number on a coloured pill.
struct CountBadge: View {
    let count: Int
    var color: Color = .wbAccent
    var overdue = false

    var body: some View {
        Text("\(count)")
            .font(.caption2.weight(.bold))
            .monospacedDigit()
            .foregroundStyle(.white)
            .padding(.horizontal, 6)
            .frame(minWidth: 22, minHeight: 22)
            .background(Capsule().fill(color))
            .accessibilityLabel(overdue ? "\(count) overdue" : "\(count) unread")
    }
}

/// The rounded square a task is ticked in.
struct TaskCheck: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView

    var body: some View {
        let closed = client.view.status(of: task).isClosed
        Button {
            client.edit { d, me in Edits.setDone(&d, task: task.id, done: !closed, me: me, ts: nowTs()) }
        } label: {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .strokeBorder(closed ? Color.wbAccent : Color(.tertiaryLabel), lineWidth: 2)
                .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(closed ? Color.wbAccent : .clear))
                .overlay {
                    if closed { Image(systemName: "checkmark").font(.system(size: 11, weight: .heavy)).foregroundStyle(.white) }
                }
                .frame(width: 20, height: 20)
                .frame(width: 30, height: 40)
                .contentShape(Rectangle())
        }
        .buttonStyle(.borderless)
        .disabled(!client.canEdit)
        .accessibilityLabel(closed ? "Reopen" : "Mark complete")
        .accessibilityIdentifier("check-\(task.id)")
    }
}

/// A task: tick box, name, where it lives, and its due tag. The tick box
/// is its own button; the rest opens the task.
struct CalmTaskRow: View {
    @Environment(SyncClient.self) private var client
    let task: TaskView
    var showList = true
    var tags: DueTag.Style = .full
    var showPeople = false

    var body: some View {
        let v = client.view
        let closed = v.status(of: task).isClosed
        HStack(spacing: 10) {
            TaskCheck(task: task)
            NavigationLink(value: Nav.Route.task(task.id)) {
                HStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(task.name)
                            .font(.system(size: 17))
                            .strikethrough(closed)
                            .foregroundStyle(closed ? .secondary : .primary)
                            .lineLimit(2)
                        if showList, let place = where_(v) {
                            Text(place).font(.system(size: 13)).foregroundStyle(.secondary).lineLimit(1)
                        }
                    }
                    Spacer(minLength: 0)
                    if task.priority == 1 || task.priority == 2 {
                        Image(systemName: "flag.fill")
                            .font(.caption)
                            .foregroundStyle(task.priority == 1 ? .red : .orange)
                            .accessibilityLabel(task.priority == 1 ? "Urgent" : "High")
                    }
                    if !closed, let tag = DueTag(task, style: tags) { TagChip(tag: tag) }
                    if showPeople {
                        HStack(spacing: -6) {
                            ForEach(task.assignees.prefix(3), id: \.self) { id in
                                Avatar(member: v.member(id), size: 22)
                                    .overlay(Circle().stroke(Calm.card, lineWidth: 1.5))
                            }
                        }
                    }
                }
                .padding(.vertical, 4)
            }
            .accessibilityIdentifier("task-\(task.id)")
        }
        .alignmentGuide(.listRowSeparatorLeading) { d in d[.leading] + 40 }
    }

    /// "Space · List".
    func where_(_ v: WorkspaceView) -> String? {
        guard let l = v.list(task.listId) else { return nil }
        guard let s = v.space(ofList: l.id) else { return l.name }
        return "\(s.privateOwner != nil ? String(localized: "Personal") : s.name) · \(l.name)"
    }
}

/// A list's progress: a ring filled as far as its tasks are done.
struct ProgressRing: View {
    let done: Double
    var color: Color = Color(.systemGray)
    var size: CGFloat = 18

    var body: some View {
        ZStack {
            Circle().strokeBorder(color, lineWidth: 2)
            PieSlice(fraction: done)
                .fill(color)
                .padding(4)
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

private struct PieSlice: Shape {
    var fraction: Double

    func path(in r: CGRect) -> Path {
        var p = Path()
        guard fraction > 0 else { return p }
        let c = CGPoint(x: r.midX, y: r.midY)
        p.move(to: c)
        p.addArc(center: c, radius: min(r.width, r.height) / 2, startAngle: .degrees(-90), endAngle: .degrees(-90 + 360 * min(fraction, 1)), clockwise: false)
        p.closeSubpath()
        return p
    }
}

// MARK: tags

/// The small tag after a task: overdue (red), due today (yellow), its
/// time today (purple), a later day (grey).
struct DueTag: Equatable {
    enum Kind { case overdue, today, time, later }
    enum Style {
        /// every due date gets a tag
        case full
        /// in Today: only overdue days and times (the day is today)
        case today
        /// in Upcoming: the day is the card's, so only times
        case timeOnly
    }

    let kind: Kind
    let text: String

    init?(_ t: TaskView, style: Style) {
        guard let due = t.due else { return nil }
        let today = dayString()
        if t.isOverdue(today: today, now: timeString()) {
            kind = .overdue
            text = due == today ? (t.dueTime.map(Fmt.clock) ?? Fmt.due(due)) : Fmt.due(due)
        } else if let time = t.dueTime, due == today || style == .timeOnly {
            kind = .time
            text = Fmt.clock(time)
        } else if style != .full {
            return nil
        } else if due == today {
            kind = .today
            text = String(localized: "Today")
        } else {
            kind = .later
            text = Fmt.due(due)
        }
    }

    var colors: (fg: Color, bg: Color) {
        switch kind {
        case .overdue: return (Calm.dynamic(light: 0xB42323, dark: 0xFF8A8A), Calm.dynamic(light: 0xFDE8E8, dark: 0x4A1F22))
        case .today: return (Calm.dynamic(light: 0x7A5A00, dark: 0xF5D46B), Calm.dynamic(light: 0xFFF4CC, dark: 0x3D3214))
        case .time: return (Calm.dynamic(light: 0x4535B8, dark: 0xB9AFFF), Calm.dynamic(light: 0xEEEBFF, dark: 0x2C2752))
        case .later: return (Calm.dynamic(light: 0x4A4A52, dark: 0xC7C7CC), Calm.dynamic(light: 0xF1F1F4, dark: 0x2C2C2E))
        }
    }
}

struct TagChip: View {
    let tag: DueTag

    var body: some View {
        let c = tag.colors
        Text(verbatim: tag.text)
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(c.fg)
            .padding(.horizontal, 7)
            .frame(height: 20)
            .background(RoundedRectangle(cornerRadius: 5, style: .continuous).fill(c.bg))
            .lineLimit(1)
            .fixedSize()
    }
}

// MARK: the floating +

/// The round "+" at the bottom right, in the same place on every screen.
struct FloatingAddButton: View {
    @Environment(SyncClient.self) private var client
    let label: LocalizedStringKey
    let id: String
    var needsEdit = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: "plus")
                .font(.system(size: 22, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 56, height: 56)
                .background(Circle().fill(Color.wbAccent))
                .shadow(color: Color.wbAccent.opacity(0.3), radius: 9, y: 6)
        }
        .buttonStyle(.plain)
        .disabled(needsEdit && !client.canEdit)
        .opacity(needsEdit && !client.canEdit ? 0.5 : 1)
        .accessibilityLabel(label)
        .accessibilityIdentifier(id)
    }
}

extension View {
    /// The floating "+" over the bottom right, with room left under the
    /// list so it never covers the last row.
    func floatingAdd(_ label: LocalizedStringKey, id: String, needsEdit: Bool = true, action: @escaping () -> Void) -> some View {
        contentMargins(.bottom, 84, for: .scrollContent)
            .overlay(alignment: .bottomTrailing) {
                FloatingAddButton(label: label, id: id, needsEdit: needsEdit, action: action)
                    .padding(.trailing, 20)
                    .padding(.bottom, 12)
            }
    }
}
