// What the message box offers over itself as the person types: slash
// commands after "/", people and groups after "@".

import SwiftUI
import WorkboardKit

struct SuggestionBar: View {
    @Environment(SyncClient.self) private var client
    @Binding var text: String
    /// People chosen from the @ list for this message.
    @Binding var mentioned: [UInt64]
    /// The text right after a pick from the list: no list again for that
    /// "@" while the text still starts with it (Mentions.isCompleted).
    @Binding var completed: String?

    /// The people the @ list shows now (none: no list).
    var mentionChoices: [MemberView] {
        guard let q = Mentions.query(in: text), !Mentions.isCompleted(text, after: completed) else { return [] }
        return Mentions.candidates(q, members: client.view.members, me: client.me)
    }

    /// The groups the @ list shows after the people: "@channel", "@here", teams.
    var groupChoices: [String] {
        guard let q = Mentions.query(in: text), let d = client.data, !Mentions.isCompleted(text, after: completed) else { return [] }
        return Mentions.groupCandidates(q, data: d)
    }

    var body: some View {
        let slash = SlashCommands.choices(text, commands: client.commands)
        if !slash.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(slash, id: \.0) { name, about in
                        Button {
                            text = "/\(name) "
                        } label: {
                            VStack(alignment: .leading, spacing: 0) {
                                Text("/" + name).font(.subheadline.weight(.medium).monospaced())
                                if !about.isEmpty { Text(L(about)).font(.caption2).foregroundStyle(.secondary).lineLimit(1) }
                            }
                            .pill()
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("command-\(name)")
                    }
                }
                .padding(.horizontal, 4)
            }
        }
        let choices = mentionChoices
        let groups = groupChoices
        if !choices.isEmpty || !groups.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(choices) { m in
                        Button {
                            text = Mentions.complete(text, with: m.name)
                            completed = text
                            if !mentioned.contains(m.id) { mentioned.append(m.id) }
                        } label: {
                            HStack(spacing: 6) {
                                Avatar(member: m, size: 22)
                                VStack(alignment: .leading, spacing: 0) {
                                    Text(m.name).font(.subheadline.weight(.medium))
                                    // tells apart people with the same name
                                    if !m.email.isEmpty { Text(m.email).font(.caption2).foregroundStyle(.secondary) }
                                }
                            }
                            .pill()
                        }
                        .buttonStyle(.plain)
                        .accessibilityElement(children: .combine)
                        .accessibilityLabel("Mention \(m.name)" + (m.email.isEmpty ? "" : ", \(m.email)"))
                        .accessibilityIdentifier("mention-\(m.id)")
                    }
                    ForEach(groups, id: \.self) { g in
                        Button {
                            text = Mentions.complete(text, with: g)
                            completed = text
                        } label: {
                            HStack(spacing: 6) {
                                Image(systemName: "person.3").font(.caption)
                                VStack(alignment: .leading, spacing: 0) {
                                    Text("@" + g).font(.subheadline.weight(.medium))
                                    Text(g == "channel" ? L("Everyone in this conversation") : g == "here" ? L("Everyone here who is online") : L("Everyone in this team"))
                                        .font(.caption2).foregroundStyle(.secondary)
                                }
                            }
                            .pill()
                        }
                        .buttonStyle(.plain)
                        .accessibilityElement(children: .combine)
                        .accessibilityLabel("Mention @\(g)")
                        .accessibilityIdentifier("mention-group-\(g)")
                    }
                }
                .padding(.horizontal, 4)
            }
            .accessibilityIdentifier("mentionList")
        }
    }
}
