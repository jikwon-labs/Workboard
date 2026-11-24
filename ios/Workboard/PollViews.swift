// Polls in chat, as on the Mac (src/app/polls.rs): "/poll [question]" in a
// message box opens New poll (a question, 2 to 10 choices, one or several
// may be picked, votes shown or anonymous); in a message the poll shows as a
// card instead of its text: a row per choice with its votes and a bar, tapped
// to vote (again to take it back), who voted when it isn't anonymous, and
// for its author Close poll. The rules and edits are WorkboardKit's Polls.

import SwiftUI
import WorkboardKit

/// A New poll window asked for by "/poll" in a message box.
struct PollRequest: Identifiable {
    let id = UUID()
    let channel: UInt64
    /// The thread it goes in (nil: the conversation).
    let parent: UInt64?
    let question: String
}

/// The New poll window to show (presented over every screen, MainScreen).
@MainActor
@Observable
final class PollSheets {
    static let shared = PollSheets()
    var request: PollRequest?
}

struct NewPollSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let request: PollRequest
    @State private var question = ""
    @State private var options = Array(repeating: "", count: Polls.minOptions)
    @State private var multi = false
    @State private var anonymous = false
    @FocusState private var focus: Int?

    var body: some View {
        NavigationStack {
            Form {
                Section("Question") {
                    TextField("Ask something...", text: $question, axis: .vertical)
                        .focused($focus, equals: -1)
                        .accessibilityIdentifier("pollQuestion")
                }
                Section("Choices") {
                    ForEach(options.indices, id: \.self) { i in
                        HStack {
                            TextField("Choice \(i + 1)", text: $options[i])
                                .focused($focus, equals: i)
                                .submitLabel(.next)
                                .onSubmit { focus = i + 1 < options.count ? i + 1 : nil }
                                .accessibilityIdentifier("pollChoice-\(i)")
                            if options.count > Polls.minOptions {
                                Button { options.remove(at: i) } label: {
                                    Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                                }
                                .buttonStyle(.plain)
                                .accessibilityLabel("Remove choice")
                                .accessibilityIdentifier("removePollChoice-\(i)")
                            }
                        }
                    }
                    if options.count < Polls.maxOptions {
                        Button("+ Add a choice") {
                            options.append("")
                            focus = options.count - 1
                        }
                        .accessibilityIdentifier("addPollChoice")
                    }
                }
                Section {
                    Toggle("Allow more than one choice", isOn: $multi).accessibilityIdentifier("pollMulti")
                    Toggle("Anonymous votes", isOn: $anonymous).accessibilityIdentifier("pollAnonymous")
                }
            }
            .navigationTitle("New poll")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create poll") { create() }
                        .disabled(!Polls.ready(question: question, options: options) || !client.canEdit)
                        .accessibilityIdentifier("createPoll")
                }
            }
            .onAppear {
                question = request.question
                focus = question.isEmpty ? -1 : 0
            }
        }
    }

    func create() {
        let r = request, q = question, o = options, m = multi, a = anonymous
        let id = client.newId()
        let sent = client.edit { d, me in
            Edits.sendPoll(&d, channel: r.channel, parent: r.parent, question: q, options: o, multi: m, anonymous: a, online: client.online, me: me, id: id, ts: nowTs())
        }
        if sent { dismiss() }
    }
}

/// A poll in a message, drawn instead of its text.
struct PollCard: View {
    @Environment(SyncClient.self) private var client
    let message: MessageView
    let poll: PollView
    let channelId: UInt64
    @State private var showVotes = false

    var body: some View {
        let me = client.me
        let counts = message.pollCounts
        let total = counts.reduce(0, +)
        let most = counts.max() ?? 0
        let mine = message.picks(of: me)
        let closed = poll.isClosed
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Image(systemName: "chart.bar.fill").font(.footnote).foregroundStyle(Color.accentColor)
                    .accessibilityHidden(true)
                Text(verbatim: poll.question).font(.subheadline.weight(.semibold))
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("pollQuestion-\(message.id)")
            }
            Text(poll.multi ? "Pick one or more" : "Pick one").font(.caption).foregroundStyle(.secondary)
            ForEach(Array(poll.options.enumerated()), id: \.offset) { i, o in
                let picked = mine.contains(i)
                let n = counts.indices.contains(i) ? counts[i] : 0
                Button { vote(i) } label: {
                    HStack(spacing: 10) {
                        Image(systemName: mark(picked: picked))
                            .foregroundStyle(picked ? Color.accentColor : Color.secondary)
                        Text(verbatim: o).font(.subheadline).foregroundStyle(.primary)
                            .lineLimit(2)
                            .multilineTextAlignment(.leading)
                        Spacer(minLength: 8)
                        Text(verbatim: "\(n)").font(.subheadline).monospacedDigit().foregroundStyle(.secondary)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(alignment: .leading) {
                        // the bar: this choice's share of all votes
                        GeometryReader { geo in
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(Color.accentColor.opacity(n == most && closed ? 0.3 : 0.15))
                                .frame(width: total > 0 ? geo.size.width * CGFloat(n) / CGFloat(total) : 0)
                        }
                    }
                    .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color(.systemBackground)))
                    .overlay(
                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                            .strokeBorder(picked ? Color.accentColor : Color(.separator), lineWidth: picked ? 1.5 : 1)
                    )
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(closed || !client.canEdit)
                .accessibilityLabel(Text("Vote \(o)"))
                .accessibilityValue(Text(verbatim: "\(n)"))
                .accessibilityAddTraits(picked ? .isSelected : [])
                .accessibilityIdentifier("pollOption-\(message.id)-\(i)")
            }
            HStack(spacing: 12) {
                Text(verbatim: status(closed: closed))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("pollStatus-\(message.id)")
                Spacer(minLength: 0)
                if !poll.anonymous && message.voters > 0 {
                    Button("Show votes") { showVotes = true }
                        .font(.caption.weight(.medium))
                        .accessibilityIdentifier("showVotes-\(message.id)")
                }
                if message.author == me && client.canEdit {
                    Button(closed ? "Reopen poll" : "Close poll") {
                        client.edit { d, me in Edits.closePoll(&d, channel: channelId, message: message.id, close: !closed, me: me, ts: nowTs()) }
                    }
                    .font(.caption.weight(.medium))
                    .accessibilityIdentifier(closed ? "reopenPoll-\(message.id)" : "closePoll-\(message.id)")
                }
            }
        }
        .padding(12)
        .frame(maxWidth: 420, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color(.secondarySystemBackground)))
        .padding(.top, 2)
        .sheet(isPresented: $showVotes) { PollVotesSheet(message: message, poll: poll) }
    }

    /// A round mark for one choice, a square for several.
    func mark(picked: Bool) -> String {
        poll.multi ? (picked ? "checkmark.square.fill" : "square") : (picked ? "largecircle.fill.circle" : "circle")
    }

    /// "2 people voted · Anonymous · Poll closed"
    func status(closed: Bool) -> String {
        let n = message.voters
        var s = n == 1 ? String(localized: "1 person voted") : String(localized: "\(n) people voted")
        if poll.anonymous { s += " · " + String(localized: "Anonymous") }
        if closed { s += " · " + String(localized: "Poll closed") }
        return s
    }

    func vote(_ i: Int) {
        client.edit { d, me in Edits.vote(&d, channel: channelId, message: message.id, option: i, me: me) }
    }
}

/// Who picked each choice of a poll whose votes are shown (on the Mac, a
/// choice's tooltip).
struct PollVotesSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let message: MessageView
    let poll: PollView

    var body: some View {
        NavigationStack {
            List {
                ForEach(Array(poll.options.enumerated()), id: \.offset) { i, o in
                    let who = message.voters(for: i)
                    Section {
                        if who.isEmpty {
                            Text("No votes").foregroundStyle(.secondary)
                        }
                        ForEach(who, id: \.self) { m in
                            let member = client.view.member(m)
                            HStack(spacing: 10) {
                                Avatar(member: member, size: 28)
                                Text(member?.name ?? String(localized: "Someone"))
                            }
                        }
                    } header: {
                        Text(verbatim: "\(o) · \(who.count)")
                    }
                }
            }
            .navigationTitle(Text(verbatim: poll.question))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
            .accessibilityIdentifier("pollVotes")
        }
        .presentationDetents([.medium, .large])
    }
}
