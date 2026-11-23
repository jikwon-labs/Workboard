// Teams and the people in them (More tab).

import SwiftUI
import WorkboardKit

struct TeamsView: View {
    @Environment(SyncClient.self) private var client
    @State private var renaming: TeamView?
    @State private var name = ""

    var body: some View {
        let v = client.view
        let teams = client.data.map { v.teams($0) } ?? []
        List {
            Section("Teams") {
                if teams.isEmpty { Text("No teams yet").foregroundStyle(.secondary) }
                ForEach(teams) { t in
                    DisclosureGroup {
                        ForEach(v.activeMembers) { m in
                            MemberToggleRow(member: m, on: t.members.contains(m.id)) {
                                client.edit { d, _ in Edits.toggleTeamMember(&d, team: t.id, member: m.id) }
                            }
                            .disabled(!client.canEdit)
                        }
                    } label: {
                        HStack {
                            Circle().fill(Color(rgb: t.color)).frame(width: 10, height: 10)
                            Text(t.name)
                            Spacer()
                            Text("\(t.members.count)").foregroundStyle(.secondary).monospacedDigit()
                        }
                    }
                    .contextMenu {
                        if client.canEdit {
                            Button { name = t.name; renaming = t } label: { Label("Rename", systemImage: "pencil") }
                            Button(role: .destructive) { client.edit { d, _ in Edits.deleteTeam(&d, team: t.id) } } label: { Label("Delete", systemImage: "trash") }
                        }
                    }
                    .accessibilityIdentifier("team-\(t.id)")
                }
            }
            Section("All People") {
                ForEach(v.members) { m in
                    HStack(spacing: 10) {
                        Avatar(member: m, size: 26)
                        VStack(alignment: .leading) {
                            Text(m.name)
                            // what they do and what they're up to, as on the Mac's People page
                            let about = [m.title, m.statusText].filter { !$0.isEmpty }.joined(separator: " · ")
                            if !about.isEmpty {
                                Text(verbatim: about).font(.caption)
                                    .accessibilityIdentifier("memberAbout-\(m.id)")
                            }
                            Text([m.role, m.invited ? "Invited" : nil, teams.filter { $0.members.contains(m.id) }.map(\.name).joined(separator: ", ")].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · "))
                                .font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .navigationTitle("Teams")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                AddButton("Create Team", id: "newTeamButton") {
                    let id = client.newId()
                    client.edit { d, me in Edits.addTeam(&d, me: me, id: id) }
                }
            }
        }
        .alert("Rename team", isPresented: Binding(isPresent: $renaming)) {
            TextField("Name", text: $name)
            Button("Cancel", role: .cancel) { renaming = nil }
            Button("Save") {
                if let t = renaming { client.edit { d, _ in Edits.renameTeam(&d, team: t.id, to: name) } }
                renaming = nil
            }
        }
    }
}
