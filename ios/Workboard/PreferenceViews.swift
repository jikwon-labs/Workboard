import SwiftUI
import WorkboardKit

// Settings > Notifications and Settings > Theme, as the Mac's dialogs have
// them. Both are this person's own and stay on this iPhone.

struct NotificationSettingsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @Environment(Nav.self) private var nav
    @Environment(\.openURL) private var openURL

    static let modes: [(String, String)] = [("all", "All messages"), ("mentions", "Only @mentions"), ("nothing", "Nothing")]

    var body: some View {
        @Bindable var n = notifier
        let now = nowTs()
        Form {
            Section {
                switch notifier.permission {
                case .allowed: Label("On", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                case .denied:
                    Button("Turned off. Open iOS Settings") {
                        if let u = URL(string: UIApplication.openSettingsURLString) { openURL(u) }
                    }
                case .unknown:
                    Button("Turn on notifications") { Task { await notifier.requestPermission() } }
                        .accessibilityIdentifier("notifyTurnOn")
                }
                if n.prefs.muted(now: now) {
                    LabeledContent("Muted until", value: Date(timeIntervalSince1970: TimeInterval(n.prefs.mutedUntil)).formatted(date: .abbreviated, time: .shortened))
                        .accessibilityIdentifier("mutedUntil")
                    Button("Unmute") { n.prefs.mutedUntil = 0 }.accessibilityIdentifier("unmuteButton")
                } else {
                    Menu {
                        Button("For 1 hour") { n.prefs.mutedUntil = now + 3600 }
                        Button("For 4 hours") { n.prefs.mutedUntil = now + 4 * 3600 }
                        Button("Until tomorrow") {
                            let t = Calendar.current.date(bySettingHour: 8, minute: 0, second: 0, of: Calendar.current.date(byAdding: .day, value: 1, to: Date())!)!
                            n.prefs.mutedUntil = Int64(t.timeIntervalSince1970)
                        }
                    } label: { Label("Mute notifications", systemImage: "bell.slash") }
                        .accessibilityIdentifier("muteMenu")
                }
                Toggle("Sound", isOn: $n.prefs.sound)
                Toggle("Number on the app icon", isOn: $n.prefs.badge).accessibilityIdentifier("badgeToggle")
            } header: {
                Text("Notifications")
            }

            Section("Chat") {
                Toggle("Direct messages", isOn: $n.prefs.direct).accessibilityIdentifier("notifyDirect")
                Toggle("@Mentions", isOn: $n.prefs.mentions)
                Toggle("Replies in my threads", isOn: $n.prefs.replies)
                Toggle("Huddles", isOn: $n.prefs.huddles)
                Toggle("Reactions to my messages", isOn: $n.prefs.reactions)
                Picker("Channel messages", selection: $n.prefs.channelDefault) {
                    ForEach(Self.modes, id: \.0) { Text(L($0.1)).tag($0.0) }
                }
                .accessibilityIdentifier("channelDefault")
            }

            let set = client.view.visibleChannels(me: client.me ?? 0).filter { $0.kind == .channel }
            if !set.isEmpty {
                Section {
                    ForEach(set) { c in
                        Picker("#\(c.name)", selection: Binding(get: { n.prefs.channelMode(c.id) }, set: { n.prefs.setChannelMode(c.id, $0) })) {
                            ForEach(Self.modes, id: \.0) { Text(L($0.1)).tag($0.0) }
                        }
                        .accessibilityIdentifier("channelMode-\(c.id)")
                    }
                } header: {
                    Text("Each channel")
                }
            }

            Section("Tasks") {
                Toggle("Tasks assigned to me", isOn: $n.prefs.assigned)
                Toggle("Comments on my tasks", isOn: $n.prefs.comments)
                Toggle("Changes to my tasks", isOn: $n.prefs.taskChanges)
                Toggle("Tasks due today", isOn: $n.prefs.due).accessibilityIdentifier("notifyDue")
                if n.prefs.due {
                    DatePicker("At", selection: Binding(get: { Self.time(n.prefs.dueTime) }, set: { n.prefs.dueTime = timeString($0) }), displayedComponents: .hourAndMinute)
                }
                Toggle("Answers to my forms", isOn: $n.prefs.forms)
            }

            Section("Calendar") {
                Toggle("Meetings", isOn: $n.prefs.meetings)
                if n.prefs.meetings {
                    Stepper(value: $n.prefs.meetingMinutes, in: 0...60, step: 5) {
                        Text("\(n.prefs.meetingMinutes) minutes before")
                    }
                }
                Toggle("Goals due today", isOn: $n.prefs.goals)
                if ["owner", "admin"].contains(client.settings.role ?? "") {
                    Toggle("Someone joins the workspace", isOn: $n.prefs.team)
                }
            }

            Section {
                EmptyView()
            } footer: {
                Text("These choices are kept on this iPhone. While the app is closed, the team server's push notifications for direct messages, @mentions and replies come as the server sends them.")
            }
        }
        .navigationTitle("Notifications")
        .navigationBarTitleDisplayMode(.inline)
        .onChange(of: notifier.prefs) { notifier.scan(client: client, nav: nav) }
    }

    static func time(_ hhmm: String) -> Date {
        let p = hhmm.split(separator: ":").compactMap { Int($0) }
        return Calendar.current.date(bySettingHour: p.first ?? 9, minute: p.count > 1 ? p[1] : 0, second: 0, of: Date()) ?? Date()
    }
}

/// The look: light, dark or as the system is, and the accent colour (the
/// Mac's choices, plus Workboard's own purple).
enum Appearance {
    static let accents: [(String, String, [Int])] = [
        ("workboard", "Workboard", [0x8B, 0x7B, 0xFF]),
        ("black", "Black", [32, 32, 32]), ("purple", "Purple", [96, 73, 231]), ("blue", "Blue", [16, 144, 224]),
        ("pink", "Pink", [233, 61, 130]), ("violet", "Violet", [171, 74, 186]), ("indigo", "Indigo", [74, 84, 196]),
        ("orange", "Orange", [247, 104, 8]), ("teal", "Teal", [18, 165, 148]), ("bronze", "Bronze", [160, 117, 82]),
        ("mint", "Mint", [46, 178, 128]),
    ]

    /// Black reads as the text colour, so it shows on a dark screen too.
    static func color(_ name: String) -> Color {
        if name == "black" { return .primary }
        return Color(rgb: accents.first { $0.0 == name }?.2 ?? accents[0].2)
    }

    static func scheme(_ mode: String) -> ColorScheme? {
        switch mode {
        case "light": return .light
        case "dark": return .dark
        default: return nil
        }
    }
}

struct ThemeSettingsView: View {
    @AppStorage("appearance") private var appearance = "system"
    @AppStorage("accent") private var accent = "workboard"

    var body: some View {
        Form {
            Section("Appearance") {
                Picker("Appearance", selection: $appearance) {
                    Text("As the system").tag("system")
                    Text("Light").tag("light")
                    Text("Dark").tag("dark")
                }
                .pickerStyle(.inline)
                .labelsHidden()
                .accessibilityIdentifier("appearancePicker")
            }
            Section("Accent color") {
                LazyVGrid(columns: Array(repeating: GridItem(.flexible()), count: 6), spacing: 14) {
                    ForEach(Appearance.accents, id: \.0) { key, name, _ in
                        Button { accent = key } label: {
                            Circle().fill(Appearance.color(key)).frame(width: 34, height: 34)
                                .overlay(Circle().stroke(Color.primary, lineWidth: accent == key ? 3 : 0).padding(-4))
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(L(name))
                        .accessibilityAddTraits(accent == key ? .isSelected : [])
                        .accessibilityIdentifier("accent-\(key)")
                    }
                }
                .padding(.vertical, 6)
            }
        }
        .navigationTitle("Theme")
        .navigationBarTitleDisplayMode(.inline)
    }
}
