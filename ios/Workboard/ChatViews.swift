import SwiftUI
import WorkboardKit

struct ChatListView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        let all = v.visibleChannels(me: me)
        let channels = all.filter { $0.kind == .channel }.sorted { $0.name < $1.name }
        let dms = all.filter { $0.kind == .direct }.sorted { ($0.messages.last?.ts ?? 0) > ($1.messages.last?.ts ?? 0) }
        List {
            if notifier.permission == .unknown && !all.isEmpty {
                Section {
                    HStack(spacing: 12) {
                        Image(systemName: "bell.badge").font(.title3).foregroundStyle(Color.wbAccent)
                        Text("Get notified when someone messages you.").font(.subheadline)
                        Spacer(minLength: 0)
                        Button("Turn on") { Task { await notifier.requestPermission() } }
                            .buttonStyle(.bordered)
                            .accessibilityIdentifier("notifyCardButton")
                    }
                }
            }
            if all.isEmpty {
                EmptyNote(title: "No channels yet", systemImage: "bubble.left.and.bubble.right")
                    .listRowBackground(Color.clear)
            }
            if !channels.isEmpty {
                Section("Channels") {
                    ForEach(channels) { c in
                        NavigationLink(value: Nav.Route.channel(c.id)) { ChannelRow(channel: c) }
                            .accessibilityIdentifier("channel-\(c.id)")
                    }
                }
            }
            if !dms.isEmpty {
                Section("Direct messages") {
                    ForEach(dms) { c in
                        NavigationLink(value: Nav.Route.channel(c.id)) { ChannelRow(channel: c) }
                            .accessibilityIdentifier("channel-\(c.id)")
                    }
                }
            }
        }
        .navigationTitle("Chat")
        .toolbar { ToolbarItem(placement: .topBarTrailing) { SettingsButton() } }
    }
}

struct ChannelRow: View {
    @Environment(SyncClient.self) private var client
    let channel: ChannelView

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        let unread = channel.unread(me: me)
        HStack(spacing: 12) {
            if channel.kind == .direct {
                Avatar(member: v.member(channel.members.first { $0 != me } ?? me), size: 36)
            } else {
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .fill(Color(.secondarySystemFill))
                    .frame(width: 36, height: 36)
                    .overlay(Image(systemName: "number").font(.system(size: 15, weight: .semibold)).foregroundStyle(.secondary))
            }
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(v.title(of: channel, me: me))
                        .fontWeight(unread > 0 ? .semibold : .regular)
                        .lineLimit(1)
                    Spacer()
                    if let last = channel.messages.last {
                        Text(Fmt.time(last.ts)).font(.caption).foregroundStyle(.secondary)
                    }
                }
                HStack {
                    Text(preview)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                    Spacer()
                    if unread > 0 {
                        Text("\(unread)")
                            .font(.caption2.weight(.bold))
                            .monospacedDigit()
                            .foregroundStyle(.white)
                            .padding(.horizontal, 6)
                            .frame(minWidth: 20, minHeight: 20)
                            .background(Capsule().fill(Color.wbAccent))
                            .accessibilityLabel("\(unread) unread")
                    }
                }
            }
        }
    }

    var preview: String {
        guard let m = channel.messages.last else { return channel.topic.isEmpty ? "No messages yet" : channel.topic }
        let who = m.author == client.me ? "You" : (client.view.member(m.author)?.name.split(separator: " ").first.map(String.init) ?? "")
        return who.isEmpty ? m.text : "\(who): \(m.text)"
    }
}

struct ConversationView: View {
    @Environment(SyncClient.self) private var client
    @Environment(CallManager.self) private var calls
    @Environment(\.openURL) private var openURL
    let channelId: UInt64

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        if let c = v.channels.first(where: { $0.id == channelId }) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        if !c.topic.isEmpty {
                            Text(c.topic).font(.footnote).foregroundStyle(.secondary).padding(.horizontal, 16).padding(.vertical, 8)
                        }
                        ForEach(Array(c.messages.enumerated()), id: \.element.id) { i, m in
                            let prev = i > 0 ? c.messages[i - 1] : nil
                            let grouped = prev != nil && prev!.author == m.author && m.ts - prev!.ts < 300 && prev!.replies.isEmpty
                            MessageRow(message: m, channelId: c.id, compact: grouped)
                                .id(m.id)
                            if !m.replies.isEmpty {
                                NavigationLink(value: Nav.Route.thread(channel: c.id, message: m.id)) {
                                    RepliesChip(message: m)
                                }
                                .buttonStyle(.plain)
                                .padding(.leading, 64)
                                .padding(.bottom, 6)
                                .accessibilityIdentifier("replies-\(m.id)")
                            }
                        }
                    }
                    .padding(.vertical, 8)
                }
                .defaultScrollAnchor(.bottom)
                .onAppear { markRead(c) }
                .onChange(of: c.messages.last?.id) {
                    markRead(c)
                    if let id = c.messages.last?.id { withAnimation { proxy.scrollTo(id, anchor: .bottom) } }
                }
            }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                Composer(placeholder: "Message \(c.kind == .channel ? "#" + c.name : v.title(of: c, me: me))") { text in
                    client.edit { d, me in
                        Edits.sendMessage(&d, channel: c.id, text: text, me: me, id: client.newId(), ts: nowTs())
                    }
                }
            }
            .navigationTitle(c.kind == .channel ? "#\(c.name)" : v.title(of: c, me: me))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button { huddle(c.id, video: false) } label: { Label("Start huddle", systemImage: "headphones") }
                        Button { huddle(c.id, video: true) } label: { Label("Start with video", systemImage: "video") }
                    } label: {
                        Image(systemName: calls.channel == c.id && calls.phase == .inCall ? "waveform" : "headphones")
                    } primaryAction: {
                        huddle(c.id, video: false)
                    }
                    .disabled(!client.state.isConnected)
                    .accessibilityLabel("Huddle")
                    .accessibilityIdentifier("huddleButton")
                }
            }
        } else {
            EmptyNote(title: "This conversation is gone", systemImage: "bubble.left")
        }
    }

    func huddle(_ ch: UInt64, video: Bool) {
        Task {
            if let link = await calls.start(channel: ch, client: client, video: video) { openURL(link) }
        }
    }

    func markRead(_ c: ChannelView) {
        guard let last = c.messages.last?.ts, last > c.lastRead else { return }
        client.editLocal { d in Edits.markRead(&d, channel: c.id, ts: max(last, nowTs())) }
    }
}

struct ThreadView: View {
    @Environment(SyncClient.self) private var client
    let channelId: UInt64
    let messageId: UInt64

    var body: some View {
        let c = client.view.channels.first { $0.id == channelId }
        if let c, let m = c.messages.first(where: { $0.id == messageId }) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    MessageRow(message: m, channelId: c.id, compact: false)
                    HStack {
                        Text(m.replies.isEmpty ? "No replies yet" : "\(m.replies.count) \(m.replies.count == 1 ? "reply" : "replies")")
                            .font(.footnote.weight(.medium))
                            .foregroundStyle(.secondary)
                        VStack { Divider() }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    ForEach(m.replies) { r in MessageRow(message: r, channelId: c.id, compact: false) }
                }
                .padding(.vertical, 8)
            }
            .defaultScrollAnchor(.bottom)
            .safeAreaInset(edge: .bottom, spacing: 0) {
                Composer(placeholder: "Reply…") { text in
                    client.edit { d, me in
                        Edits.sendReply(&d, channel: c.id, to: m.id, text: text, me: me, id: client.newId(), ts: nowTs())
                    }
                }
            }
            .navigationTitle("Thread")
            .navigationBarTitleDisplayMode(.inline)
        } else {
            EmptyNote(title: "This message is gone", systemImage: "bubble.left")
        }
    }
}

struct MessageRow: View {
    @Environment(SyncClient.self) private var client
    let message: MessageView
    let channelId: UInt64
    let compact: Bool

    /// Quick reactions offered on a long press.
    static let quick = ["👍", "❤️", "😂", "🎉", "👀", "✅"]

    var body: some View {
        let author = client.view.member(message.author)
        HStack(alignment: .top, spacing: 12) {
            if compact {
                Color.clear.frame(width: 36, height: 1)
            } else {
                Avatar(member: author, size: 36)
            }
            VStack(alignment: .leading, spacing: 2) {
                if !compact {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(author?.name ?? "Someone").font(.subheadline.weight(.semibold))
                        Text(Fmt.time(message.ts)).font(.caption).foregroundStyle(.secondary)
                        if message.pinned { Image(systemName: "pin.fill").font(.caption2).foregroundStyle(.secondary) }
                    }
                }
                if let title = message.postTitle, !title.isEmpty {
                    Text(title).font(.headline)
                }
                Text(message.text)
                    .textSelection(.enabled)
                if let call = message.call, let url = URL(string: call) {
                    Link(destination: url) { Label("Join call", systemImage: "phone.fill") }
                        .font(.subheadline.weight(.medium))
                        .padding(.top, 2)
                }
                if !message.reactions.isEmpty {
                    HStack(spacing: 6) {
                        ForEach(message.reactions, id: \.emoji) { r in
                            let mine = r.users.contains(client.me ?? 0)
                            Button { react(r.emoji) } label: {
                                Text("\(r.emoji) \(r.users.count)")
                                    .font(.caption.weight(.medium))
                                    .monospacedDigit()
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 3)
                                    .background(Capsule().fill(mine ? Color.wbAccent.opacity(0.18) : Color(.secondarySystemFill)))
                                    .overlay(Capsule().strokeBorder(mine ? Color.wbAccent : .clear, lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                            .disabled(!client.canEdit)
                            .accessibilityLabel("\(r.emoji) \(r.users.count)\(mine ? ", yours" : "")")
                        }
                    }
                    .padding(.top, 2)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 16)
        .padding(.top, compact ? 2 : 10)
        .contentShape(Rectangle())
        .contextMenu {
            if client.canEdit {
                Section("React") {
                    ForEach(Self.quick, id: \.self) { e in
                        Button("React \(e)") { react(e) }
                    }
                }
            }
            Button { UIPasteboard.general.string = message.text } label: { Label("Copy text", systemImage: "doc.on.doc") }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("message-\(message.id)")
    }

    func react(_ emoji: String) {
        client.edit { d, me in Edits.toggleReaction(&d, channel: channelId, message: message.id, emoji: emoji, me: me) }
    }
}

struct RepliesChip: View {
    @Environment(SyncClient.self) private var client
    let message: MessageView

    var body: some View {
        HStack(spacing: 6) {
            HStack(spacing: -4) {
                ForEach(Array(Set(message.replies.map(\.author))).sorted().prefix(3), id: \.self) { id in
                    Avatar(member: client.view.member(id), size: 18)
                }
            }
            Text("\(message.replies.count) \(message.replies.count == 1 ? "reply" : "replies")")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Color.wbAccent)
            if let last = message.replies.last {
                Text(Fmt.relative(last.ts)).font(.footnote).foregroundStyle(.secondary)
            }
        }
    }
}

/// The message box. Works offline too: messages wait and go up on reconnect.
struct Composer: View {
    @Environment(SyncClient.self) private var client
    let placeholder: String
    let send: (String) -> Bool
    @State private var text = ""
    @FocusState private var focused: Bool

    init(placeholder: String, send: @escaping (String) -> Bool) {
        self.placeholder = placeholder
        self.send = send
    }

    var body: some View {
        let can = client.canEdit
        HStack(alignment: .bottom, spacing: 8) {
            TextField(can ? placeholder : "Sign in to send messages", text: $text, axis: .vertical)
                .lineLimit(1...5)
                .focused($focused)
                .padding(.horizontal, 14)
                .padding(.vertical, 9)
                .background(RoundedRectangle(cornerRadius: 20, style: .continuous).fill(Color(.secondarySystemBackground)))
                .disabled(!can)
                .accessibilityIdentifier("composer")
            Button {
                if send(text) { text = "" }
            } label: {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 32))
                    .symbolRenderingMode(.hierarchical)
            }
            .disabled(!can || text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .accessibilityLabel("Send")
            .accessibilityIdentifier("sendButton")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.bar)
        .overlay(alignment: .top) { Divider() }
    }
}
