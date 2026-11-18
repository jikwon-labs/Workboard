import SwiftUI
import WorkboardKit

// The More tab: the Mac rail's other apps (Goals, Meetings, and the rest as
// they are added), each a page of its own.

struct MoreView: View {
    static let apps: [(id: String, title: String, icon: String)] = [
        ("goals", "Goals", "target"),
        ("meetings", "Meetings", "person.3"),
        ("forms", "Forms", "list.clipboard"),
        ("whiteboards", "Whiteboards", "scribble"),
        ("dashboards", "Dashboards", "chart.bar"),
        ("timesheets", "Timesheets", "clock"),
        ("templates", "Templates", "square.on.square"),
        ("teams", "Teams", "person.3.sequence"),
        ("posts", "Posts", "megaphone"),
        ("saved", "Saved", "bookmark"),
        ("reminders", "Reminders", "alarm"),
        ("notepad", "Notepad", "note.text"),
    ]

    var body: some View {
        List {
            ForEach(Self.apps, id: \.id) { a in
                NavigationLink(value: Nav.Route.app(a.id)) { Label(L(a.title), systemImage: a.icon) }
                    .accessibilityIdentifier("app-\(a.id)")
            }
        }
        .navigationTitle("More")
        .toolbar { ToolbarItem(placement: .topBarTrailing) { SettingsButton() } }
    }
}

/// One app's page.
struct AppPage: View {
    let name: String

    var body: some View {
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
