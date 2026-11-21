import SwiftUI
import WorkboardKit

// Tools: the Mac rail's other apps (Goals, Meetings, and the rest), each a
// page of its own. The four used most are in their own card on top.

struct ToolsView: View {
    @Environment(SyncClient.self) private var client

    static let apps: [(id: String, title: String, icon: String, color: UInt32)] = [
        ("goals", "Goals", "target", 0xC42B2B),
        ("meetings", "Meetings", "person.3", 0x2F6FDB),
        ("reminders", "Reminders", "alarm", 0xB35A00),
        ("saved", "Saved", "bookmark", 0x5B49D6),
        ("posts", "Posts", "megaphone", 0xC2477A),
        ("notepad", "Notepad", "note.text", 0x8A5A00),
        ("forms", "Forms", "list.clipboard", 0x2C7A7B),
        ("whiteboards", "Whiteboards", "scribble", 0x2C7A7B),
        ("dashboards", "Dashboards", "chart.bar", 0x2C7A7B),
        ("timesheets", "Timesheets", "clock", 0x6E6E76),
        ("templates", "Templates", "square.on.square", 0x6E6E76),
        ("teams", "Teams", "person.3.sequence", 0x6E6E76),
    ]
    static let often = ["goals", "meetings", "reminders", "saved"]

    var body: some View {
        List {
            CalmTitle(place: .tools)
            Section {
                ForEach(Self.apps.filter { Self.often.contains($0.id) }, id: \.id) { row($0) }
            } header: {
                CalmHeader("Used most")
            }
            Section {
                ForEach(Self.apps.filter { !Self.often.contains($0.id) }, id: \.id) { row($0) }
            } header: {
                CalmHeader("All tools")
            }
        }
        .calmList()
        .calmBar(String(localized: "Tools"))
    }

    func row(_ a: (id: String, title: String, icon: String, color: UInt32)) -> some View {
        NavigationLink(value: Nav.Route.app(a.id)) {
            HStack(spacing: 14) {
                Image(systemName: a.icon)
                    .font(.system(size: 17))
                    .foregroundStyle(a.color == 0x6E6E76 ? Color.secondary : Calm.dynamic(light: a.color, dark: a.color))
                    .frame(width: 26)
                Text(L(a.title)).font(.system(size: 16))
                Spacer()
                if let n = note(a.id) { Text(n).font(.system(size: 13)).foregroundStyle(.secondary) }
            }
            .frame(minHeight: 30)
            .accessibilityElement(children: .combine)
        }
        .accessibilityIdentifier("app-\(a.id)")
    }

    /// How many there are, for the tools used most.
    func note(_ id: String) -> String? {
        guard let data = client.data else { return nil }
        let v = client.view
        let me = client.me ?? 0
        let n: Int
        switch id {
        case "goals": n = v.goals(data).count
        case "meetings": n = v.meetings(data).filter { $0.date == dayString() }.count
            return n > 0 ? String(localized: "Today \(n)") : nil
        case "reminders": n = v.reminders(data, me: me).filter { !$0.done }.count
        case "saved": n = v.savedMessages(data, me: me).count
        default: return nil
        }
        return n > 0 ? "\(n)" : nil
    }
}

/// One app's page.
struct AppPage: View {
    let name: String

    var body: some View {
        // the title small in the bar, as the other pushed screens: a large
        // one under Home's hidden bar stayed drawn over the first row and
        // took its taps (Dashboards' "Workspace overview" didn't open)
        page.navigationBarTitleDisplayMode(.inline)
    }

    @ViewBuilder var page: some View {
        switch name {
        case "goals": GoalsView()
        case "meetings": MeetingsView()
        case "forms": FormsView()
        case "whiteboards": WhiteboardsView()
        case "dashboards": DashboardsView()
        case "timesheets": TimesheetsView()
        case "templates": TemplatesView()
        case "teams": TeamsView()
        case "posts": MessageListView(title: "Posts", posts: true)
        case "saved": MessageListView(title: "Saved", posts: false)
        case "reminders": RemindersView()
        case "notepad": NotepadView()
        default: EmptyNote(title: "Not on the iPhone yet", systemImage: "square.grid.2x2")
        }
    }
}
