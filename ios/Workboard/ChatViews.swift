import PhotosUI
import SwiftUI
import WorkboardKit

/// Chat: unread conversations first, then channels and direct messages,
/// each in a card. The "+" starts a direct message.
struct ChatListView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @State private var newChannel = false
    @State private var newDM = false
    @State private var showDrafts = false
    @State private var showScheduled = false

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        let all = v.visibleChannels(me: me)
        let quiet = { (c: ChannelView) in c.kind == .channel && notifier.prefs.channelMode(c.id) == "nothing" }
        let isUnread = { (c: ChannelView) in !quiet(c) && c.unread(me: me) > 0 }
        let byLast = { (a: ChannelView, b: ChannelView) in (a.messages.last?.ts ?? 0) > (b.messages.last?.ts ?? 0) }
        let unread = all.filter(isUnread).sorted(by: byLast)
        let channels = all.filter { $0.kind == .channel && !isUnread($0) }.sorted { $0.name < $1.name }
        let dms = all.filter { $0.kind == .direct && !isUnread($0) }.sorted(by: byLast)
        List {
            CalmTitle(place: .chats)
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
            section("Unread", unread)
            section("Channels", channels)
            section("Direct messages", dms)
        }
        .calmList()
        .calmBar(String(localized: "Chat"))
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button { newChannel = true } label: { Image(systemName: "number") }
                    .disabled(!client.canEdit)
                    .accessibilityLabel("New channel")
                    .accessibilityIdentifier("newChannelButton")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Button { showDrafts = true } label: { Label("Drafts", systemImage: "square.and.pencil") }
                    Button { showScheduled = true } label: { Label("Scheduled messages", systemImage: "clock") }
                } label: { Image(systemName: "tray") }
                .accessibilityLabel("Drafts and scheduled")
                .accessibilityIdentifier("draftsMenu")
            }
        }
        .floatingAdd("New message", id: "newDMButton") { newDM = true }
        .sheet(isPresented: $newChannel) { NewChannelSheet() }
        .sheet(isPresented: $newDM) { NewDMSheet() }
        .sheet(isPresented: $showDrafts) { DraftsSheet() }
        .sheet(isPresented: $showScheduled) { ScheduledSheet() }
    }

    @ViewBuilder
    func section(_ title: LocalizedStringKey, _ list: [ChannelView]) -> some View {
        if !list.isEmpty {
            Section {
                ForEach(list) { c in
                    NavigationLink(value: Nav.Route.channel(c.id)) { ChannelRow(channel: c) }
                        .accessibilityIdentifier("channel-\(c.id)")
                }
            } header: {
                CalmHeader(title)
            }
        }
    }
}

/// Picks a person to message, like the Mac app's direct message list
/// (ChatAction::Dm): opens the conversation with them, starting it if needed.
struct NewDMSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let me = client.me ?? 0
        let people = client.view.members.filter { !$0.invited && $0.id != me }.sorted { $0.name < $1.name }
        NavigationStack {
            List {
                if people.isEmpty {
                    EmptyNote(title: "No one else is here yet", systemImage: "person.2").listRowBackground(Color.clear)
                }
                ForEach(people) { m in
                    Button { open(m.id) } label: {
                        HStack(spacing: 12) {
                            Avatar(member: m, size: 32)
                            Text(m.name).foregroundStyle(.primary)
                        }
                    }
                    .accessibilityIdentifier("dmPerson-\(m.id)")
                }
            }
            .navigationTitle("New Message")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
            }
        }
    }

    func open(_ person: UInt64) {
        let newId = client.newId()
        var opened: UInt64?
        let ok = client.edit { d, me in opened = Edits.openDM(&d, with: person, me: me, id: newId, ts: nowTs()) }
        guard ok, let id = opened else { return }
        dismiss()
        nav.path.append(Nav.Route.channel(id))
    }
}

/// Like the Mac app's Create Channel window (create_channel in src/app/parity.rs).
struct NewChannelSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var isPrivate = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("e.g. marketing", text: $name)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onSubmit(create)
                        .accessibilityIdentifier("channelNameField")
                } header: {
                    Text("Name")
                }
                Section {
                    Toggle("Make Private", isOn: $isPrivate)
                        .accessibilityIdentifier("channelPrivateToggle")
                } footer: {
                    Text(isPrivate ? "Only invited people can find this channel." : "Everyone in the workspace is in it.")
                }
            }
            .navigationTitle("Create Channel")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create", action: create)
                        .disabled(name.isBlank)
                        .accessibilityIdentifier("createChannelButton")
                }
            }
        }
    }

    func create() {
        let id = client.newId()
        let ok = client.edit { d, me in Edits.createChannel(&d, name: name, isPrivate: isPrivate, me: me, id: id, ts: nowTs()) }
        guard ok else { return }
        dismiss()
        nav.path.append(Nav.Route.channel(id))
    }
}

struct ChannelRow: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    let channel: ChannelView

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        // a channel set to notify about nothing is quiet: dimmed, no count
        let quiet = channel.kind == .channel && notifier.prefs.channelMode(channel.id) == "nothing"
        let unread = quiet ? 0 : channel.unread(me: me)
        HStack(spacing: 12) {
            if channel.kind == .direct {
                Avatar(member: v.member(channel.members.first { $0 != me } ?? me), size: 34)
            } else {
                Circle()
                    .fill(Color(.tertiarySystemFill))
                    .frame(width: 34, height: 34)
                    .overlay(Image(systemName: "number").font(.system(size: 15, weight: .semibold)).foregroundStyle(.secondary))
            }
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(v.title(of: channel, me: me))
                        .fontWeight(unread > 0 ? .semibold : .regular)
                        .foregroundStyle(quiet ? .secondary : .primary)
                        .lineLimit(1)
                    if quiet {
                        Image(systemName: "bell.slash").font(.caption).foregroundStyle(.secondary)
                            .accessibilityLabel("Notifications off")
                    }
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
        var text = m.text
        if text.isEmpty, let a = m.attachments.first {
            if m.attachments.contains(where: \.isFile) {
                text = String(localized: "File")
            } else {
                text = a.isVideo ? String(localized: "Video") : (m.attachments.count == 1 ? String(localized: "Photo") : String(localized: "\(m.attachments.count) photos"))
            }
        }
        return who.isEmpty ? text : "\(who): \(text)"
    }
}

struct ConversationView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Environment(Notifier.self) private var notifier
    @Environment(CallManager.self) private var calls
    @Environment(\.openURL) private var openURL
    let channelId: UInt64
    @Environment(\.scenePhase) private var scenePhase
    @State private var settings = false
    @State private var posting = false
    /// The red "New" line sits above this message: the first unread one on
    /// opening, or the first that came in while scrolled up or away.
    @State private var readLine: UInt64?
    /// The message at the bottom edge of the list (nil: not scrolled yet,
    /// so still at the bottom where it opens).
    @State private var position: UInt64?
    /// The newest message the last time the list was at the bottom.
    @State private var bottomSeen: Int64 = 0
    /// Only some of the messages (the filter menu), or the files.
    @State private var filter: MessageFilter = .all
    /// A message to scroll to once the list is back ("Show in conversation").
    @State private var jump: UInt64?

    var body: some View {
        let me = client.me ?? 0
        let v = client.view
        if let c = v.channels.first(where: { $0.id == channelId }) {
            let shown = filter == .all ? c.messages : c.messages.filter(filter.keeps)
            let atBottom = position == nil || position == shown.last?.id
            Group {
                if filter == .files {
                    ConversationFilesView(channel: c) { f in
                        if let parent = f.parent {
                            nav.path.append(.thread(channel: c.id, message: parent))
                        } else {
                            filter = .all
                            jump = f.message.id
                        }
                    }
                } else {
                    messageList(c, shown: shown, atBottom: atBottom, me: me)
                }
            }
            .safeAreaInset(edge: .top, spacing: 0) {
                if filter != .all { MessageFilterBanner(filter: $filter) }
            }
            .onAppear {
                readLine = c.firstUnread(me: me)
                bottomSeen = c.messages.last?.ts ?? 0
                markRead(c)
            }
            .onChange(of: atBottom) { if atBottom { bottomSeen = c.messages.last?.ts ?? 0 } }
            .onChange(of: c.messages.last?.id) { old, new in
                // at the bottom when the one before it was (set at once
                // below, so a quick run of messages keeps following)
                let was = position == nil || position == old
                    // mine: just sent, down to it from anywhere
                    || c.messages.last?.author == me
                // came in while I'm scrolled up or away: the line goes
                // above the first of them, and the list stays put
                if readLine == nil && (!was || scenePhase != .active) {
                    readLine = c.firstUnread(me: me)
                }
                markRead(c)
                if was, let new, filter == .all || c.messages.last.map(filter.keeps) == true {
                    bottomSeen = c.messages.last?.ts ?? 0
                    withAnimation { position = new }
                }
            }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                VStack(spacing: 0) {
                    TypingLine(channelId: c.id)
                    Composer(placeholder: "Message \(v.place(of: c, me: me))", attachments: true) { text, files, picked in
                        sendChannelMessage(client, notifier: notifier, channel: c.id, text: text, files: files, picked: picked)
                    }
                    .onTyping { client.typingNow(channel: c.id) }
                    .draft("\(c.id)|0")
                    .onSchedule { text, picked, at in client.schedule(channel: c.id, text: text, picked: picked, at: at) }
                }
            }
            .navigationTitle(v.place(of: c, me: me))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    MessageFilterMenu(filter: $filter)
                }
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
                if c.kind == .channel {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button { posting = true } label: { Image(systemName: "megaphone") }
                            .disabled(!client.canEdit)
                            .accessibilityLabel("Write a post")
                            .accessibilityIdentifier("writePostButton")
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { settings = true } label: { Image(systemName: "person.2") }
                        .accessibilityLabel("Channel settings")
                        .accessibilityIdentifier("channelSettingsButton")
                }
            }
            .sheet(isPresented: $settings) { ChannelSettingsView(channelId: c.id) }
            .sheet(isPresented: $posting) { NewPostSheet(channelId: c.id) }
        } else {
            EmptyNote(title: "This conversation is gone", systemImage: "bubble.left")
        }
    }

    /// The messages the filter keeps (all of them when it is off).
    func messageList(_ c: ChannelView, shown: [MessageView], atBottom: Bool, me: UInt64) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                if !c.topic.isEmpty && filter == .all {
                    Text(c.topic).font(.footnote).foregroundStyle(.secondary).padding(.horizontal, 16).padding(.vertical, 8)
                }
                if shown.isEmpty && filter == .links {
                    EmptyNote(title: "No links in this conversation yet", systemImage: "link")
                        .padding(.top, 40)
                        .accessibilityIdentifier("noLinksNote")
                }
                ForEach(Array(shown.enumerated()), id: \.element.id) { i, m in
                    let prev = i > 0 ? shown[i - 1] : nil
                    let grouped = prev != nil && prev!.author == m.author && m.ts - prev!.ts < 300 && prev!.replies.isEmpty && m.id != readLine
                    VStack(alignment: .leading, spacing: 0) {
                        if m.id == readLine { ReadLine() }
                        MessageRow(message: m, channelId: c.id, compact: grouped)
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
                    .id(m.id)
                }
            }
            .scrollTargetLayout()
            .padding(.vertical, 8)
        }
        .defaultScrollAnchor(.bottom)
        .scrollPosition(id: $position, anchor: .bottom)
        .overlay(alignment: .bottom) {
            let below = atBottom || filter != .all ? 0 : c.newer(than: bottomSeen, me: me)
            if below > 0 {
                NewBelowNote(count: below) {
                    withAnimation { position = c.messages.last?.id }
                }
                .padding(.bottom, 10)
                .transition(.opacity)
            }
        }
        .task(id: jump) {
            // back from the files: once the list is laid out
            guard let id = jump else { return }
            try? await Task.sleep(for: .milliseconds(200))
            withAnimation { position = id }
            jump = nil
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

/// The red "New" line above the first message I hadn't read.
struct ReadLine: View {
    var body: some View {
        // the word in the middle, the line on both sides of it
        HStack(spacing: 8) {
            Rectangle().fill(Color.red).frame(height: 1)
            Text("New").font(.caption2.weight(.medium)).foregroundStyle(.red).fixedSize()
            Rectangle().fill(Color.red).frame(height: 1)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 4)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Read up to here")
        .accessibilityIdentifier("readLine")
    }
}

/// "N new messages" in grey over the foot of the list, for messages that
/// came in below where it's scrolled to; a tap goes down to them.
struct NewBelowNote: View {
    let count: Int
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Text(count == 1 ? "1 new message" : "\(count) new messages")
                Image(systemName: "arrow.down")
            }
            .font(.footnote)
            .foregroundStyle(.secondary)
            .padding(.horizontal, 14)
            .padding(.vertical, 7)
            .background(.regularMaterial, in: Capsule())
            .overlay(Capsule().strokeBorder(Color.secondary.opacity(0.2)))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("newMessagesButton")
    }
}

struct ThreadView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    let channelId: UInt64
    let messageId: UInt64
    /// "Also send to #channel" is on for the next reply.
    @State private var alsoToChannel = false

    var body: some View {
        let c = client.view.channels.first { $0.id == channelId }
        if let c, let m = c.messages.first(where: { $0.id == messageId }) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    MessageRow(message: m, channelId: c.id, compact: false)
                    HStack {
                        Text(m.replies.isEmpty ? "No replies yet" : m.replies.count == 1 ? "1 reply" : "\(m.replies.count) replies")
                            .font(.footnote.weight(.medium))
                            .foregroundStyle(.secondary)
                        VStack { Divider() }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    ForEach(m.replies) { r in MessageRow(message: r, channelId: c.id, compact: false, isReply: true) }
                }
                .padding(.vertical, 8)
            }
            .defaultScrollAnchor(.bottom)
            // the replies here count as read (on my Mac too), new ones as they come
            .task(id: m.replies.count) { client.markThreadRead(channel: c.id, parent: m.id) }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                VStack(spacing: 0) {
                    Toggle(isOn: $alsoToChannel) {
                        Text("Also send to \(client.view.place(of: c, me: client.me ?? 0))").font(.footnote)
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 4)
                    .background(.bar)
                    .accessibilityIdentifier("alsoToChannel")
                    // who else is answering in this thread (not shown in the channel)
                    TypingLine(channelId: c.id, thread: m.id)
                    Composer(placeholder: "Reply…", attachments: true) { text, files, picked in
                        var text = text
                        switch runSlash(client, notifier: notifier, text: text, channel: c.id, parent: m.id, hasFiles: !files.isEmpty) {
                        case .notCommand: break
                        case .done: return true
                        case .keep: return false
                        case .post(let t): text = t
                        }
                        let id = client.newId()
                        let copy = alsoToChannel ? client.newId() : nil
                        let sent = client.edit { d, me in
                            Edits.sendReply(&d, channel: c.id, to: m.id, text: text, attachments: files, picked: picked, online: client.online, me: me, id: id, ts: nowTs())
                            if let copy { Edits.copyReplyToChannel(&d, channel: c.id, thread: m.id, reply: id, me: me, id: copy) }
                        }
                        if sent {
                            alsoToChannel = false
                            fetchPreview(client, channel: c.id, message: id, text: text)
                        }
                        return sent
                    }
                    .onTyping { client.typingNow(channel: c.id, thread: m.id) }
                    .draft("\(c.id)|\(m.id)")
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
    @Environment(CallManager.self) private var calls
    @Environment(\.openURL) private var openURL
    let message: MessageView
    let channelId: UInt64
    let compact: Bool
    var isReply = false
    @State private var editing = false
    @State private var draft = ""
    @State private var confirmDelete = false
    @State private var forwarding = false

    func remind(at ts: Int64) {
        let id = client.newId(), me = client.me ?? 0, text = String(message.text.prefix(80))
        client.editLocal { d in Edits.remindAboutMessage(&d, channel: channelId, message: message.id, at: ts, me: me, id: id) }
        ReminderAlerts.schedule(id: id, text: text.isEmpty ? "Message" : text, at: ts)
    }

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
            content
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 16)
        .padding(.top, compact ? 2 : 10)
        .contentShape(Rectangle())
        .contextMenu { menu }
        .sheet(isPresented: $forwarding) { ForwardSheet(channelId: channelId, message: message) }
        .confirmationDialog("Delete this message?", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                client.edit { d, me in Edits.deleteMessage(&d, channel: channelId, message: message.id, me: me) }
            }
            .accessibilityIdentifier("confirmDeleteMessage")
        } message: {
            Text(isReply || message.replies.isEmpty ? "It's removed for everyone." : "It's removed for everyone, with its replies.")
        }
        .sheet(isPresented: $editing) {
            EditMessageSheet(message: message, channelId: channelId, draft: $draft, editing: $editing)
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("message-\(message.id)")
    }

    /// The name line, the text, pictures, a linked task, a call card and reactions.
    @ViewBuilder private var content: some View {
        let author = client.view.member(message.author)
        VStack(alignment: .leading, spacing: 2) {
            if !compact {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(message.bot ?? author?.name ?? String(localized: "Someone")).font(.subheadline.weight(.semibold))
                    if message.bot != nil {
                        Text("APP").font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
                            .padding(.horizontal, 4).background(RoundedRectangle(cornerRadius: 3).fill(Color(.secondarySystemFill)))
                    }
                    Text(Fmt.time(message.ts)).font(.caption).foregroundStyle(.secondary)
                    if message.pinned { Image(systemName: "pin.fill").font(.caption2).foregroundStyle(.secondary).accessibilityLabel("Pinned") }
                    if message.saved { Image(systemName: "bookmark.fill").font(.caption2).foregroundStyle(.secondary).accessibilityLabel("Saved") }
                }
            }
            if compact && (message.pinned || message.saved) {
                // a message under the one before it has no name line for these
                HStack(spacing: 4) {
                    if message.pinned { Image(systemName: "pin.fill").accessibilityLabel("Pinned") }
                    if message.saved { Image(systemName: "bookmark.fill").accessibilityLabel("Saved") }
                }
                .font(.caption2).foregroundStyle(.secondary)
            }
            if let title = shownPostTitle(message) {
                Text(verbatim: title).font(.headline)
            }
            if let tid = message.threadOf {
                NavigationLink(value: Nav.Route.thread(channel: channelId, message: tid)) {
                    Text("Replied in a thread · View thread").font(.caption).foregroundStyle(Color.accentColor)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("viewThread-\(message.id)")
            }
            if !message.text.isEmpty || message.edited != nil {
                ChatText(text: message.text, edited: message.edited != nil)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("messageText-\(message.id)")
            }
            if let f = message.forwarded {
                ForwardedCard(forwarded: f)
            }
            if let p = message.preview {
                LinkPreviewCard(preview: p, mine: message.author == client.me) {
                    _ = client.edit { d, me in Edits.removePreview(&d, channel: channelId, message: message.id, me: me) }
                }
            }
            if !message.attachments.isEmpty {
                AttachmentsView(attachments: message.attachments)
                    .padding(.top, 4)
            }
            if let tid = message.task, let t = client.view.tasks.first(where: { $0.id == tid }) {
                NavigationLink(value: Nav.Route.task(tid)) {
                    Label(t.name, systemImage: "checklist").font(.caption).lineLimit(1)
                        .padding(.horizontal, 8).padding(.vertical, 4)
                        .background(Capsule().fill(Color.accentColor.opacity(0.12)))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("messageTask-\(message.id)")
            }
            if let call = message.call {
                if let ch = CallAPI.huddleChannel(call) {
                    Button {
                        Task { _ = await calls.start(channel: ch, client: client, announce: false) }
                    } label: { Label("Join huddle", systemImage: "headphones") }
                        .font(.subheadline.weight(.medium))
                        .padding(.top, 2)
                        .disabled(!client.state.isConnected)
                        .accessibilityIdentifier("joinHuddle-\(message.id)")
                } else if let url = URL(string: call) {
                    Link(destination: url) { Label("Join call", systemImage: "phone.fill") }
                        .font(.subheadline.weight(.medium))
                        .padding(.top, 2)
                }
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
    }

    /// What a long press offers.
    @ViewBuilder private var menu: some View {
        let mine = message.author == client.me
        if client.canEdit {
            Section("React") {
                ForEach(Self.quick, id: \.self) { e in
                    Button("React \(e)") { react(e) }
                }
            }
            Section {
                // huddle and call cards aren't edited, as on the Mac
                if message.isEditable(by: client.me) {
                    Button { draft = message.text; editing = true } label: { Label("Edit message", systemImage: "pencil") }
                }
                if !isReply {
                    Button { client.edit { d, _ in Edits.togglePin(&d, channel: channelId, message: message.id) } } label: {
                        Label(message.pinned ? "Unpin" : "Pin to channel", systemImage: message.pinned ? "pin.slash" : "pin")
                    }
                }
            }
        }
        if !message.text.isEmpty {
            Button { UIPasteboard.general.string = message.text } label: { Label("Copy text", systemImage: "doc.on.doc") }
        }
        if client.canEdit && message.call == nil {
            Button { forwarding = true } label: { Label("Forward message", systemImage: "arrowshape.turn.up.right") }
        }
        Button {
            client.editLocal { d in Edits.toggleSaved(&d, channel: channelId, message: message.id) }
        } label: { Label(message.saved ? "Remove from saved" : "Save for later", systemImage: message.saved ? "bookmark.slash" : "bookmark") }
        if client.canEdit && !isReply && message.task == nil && !message.text.isEmpty {
            Button {
                let id = client.newId()
                let list = client.view.visibleSpaces(me: client.me ?? 0).flatMap(\.allListIds).first ?? 0
                client.edit { d, me in Edits.messageToTask(&d, channel: channelId, message: message.id, list: list, me: me, id: id, ts: nowTs()) }
            } label: { Label("Create task", systemImage: "checklist") }
        }
        if !isReply {
            Menu {
                Button("In 1 hour") { remind(at: nowTs() + 3600) }
                Button("In 3 hours") { remind(at: nowTs() + 3 * 3600) }
                Button("Tomorrow at 9 AM") {
                    let cal = Calendar.current
                    let t = cal.date(bySettingHour: 9, minute: 0, second: 0, of: cal.date(byAdding: .day, value: 1, to: Date())!)!
                    remind(at: Int64(t.timeIntervalSince1970))
                }
            } label: { Label("Remind me", systemImage: "alarm") }
        }
        if client.canEdit && mine {
            Button(role: .destructive) { confirmDelete = true } label: { Label("Delete message", systemImage: "trash") }
        }
    }

    func react(_ emoji: String) {
        client.edit { d, me in Edits.toggleReaction(&d, channel: channelId, message: message.id, emoji: emoji, me: me) }
    }
}

/// Changing the text of one of my messages.
struct EditMessageSheet: View {
    @Environment(SyncClient.self) private var client
    let message: MessageView
    let channelId: UInt64
    @Binding var draft: String
    @Binding var editing: Bool
    @State private var confirmDelete = false

    var body: some View {
        NavigationStack {
            Form {
                TextField("Message", text: $draft, axis: .vertical)
                    .lineLimit(1...10)
                    .accessibilityIdentifier("editMessageField")
                if draft.isBlank && !message.standsWithoutText {
                    Text("Empty: saving deletes the message").font(.footnote).foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Edit message")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { editing = false }
                ToolbarItem(placement: .confirmationAction) {
                    // emptied: saving deletes it (a message with pictures just loses its text)
                    if draft.isBlank && !message.standsWithoutText {
                        Button("Delete", role: .destructive) { confirmDelete = true }
                            .accessibilityIdentifier("saveEditButton")
                    } else {
                        Button("Save") {
                            let had = message.preview?.url
                            client.edit { d, me in Edits.editMessage(&d, channel: channelId, message: message.id, text: draft, me: me, ts: nowTs()) }
                            // another address: its preview comes
                            if ChatFormat.firstURL(draft) != had { fetchPreview(client, channel: channelId, message: message.id, text: draft) }
                            editing = false
                        }
                        .accessibilityIdentifier("saveEditButton")
                    }
                }
            }
        }
        .confirmationDialog("Delete this message?", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                client.edit { d, me in Edits.deleteMessage(&d, channel: channelId, message: message.id, me: me) }
                editing = false
            }
            .accessibilityIdentifier("confirmDeleteMessage")
        } message: {
            Text(message.replies.isEmpty ? "It's removed for everyone." : "It's removed for everyone, with its replies.")
        }
        .presentationDetents([.medium])
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
            Text(message.replies.count == 1 ? "1 reply" : "\(message.replies.count) replies")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Color.wbAccent)
            if let last = message.replies.last {
                Text(Fmt.relative(last.ts)).font(.footnote).foregroundStyle(.secondary)
            }
        }
    }
}

/// The message box. Works offline too: messages wait and go up on reconnect.
/// With `attachments`, a photo button adds pictures and videos and a clip
/// button files from Files: they go up to the server first (PUT /files) and
/// the message names them.
/// Typing "@" at the start of a word offers the people to mention; the ones
/// picked go along with the text, so a name several people share mentions
/// the person picked (the message's `mentions`).
struct Composer: View {
    @Environment(SyncClient.self) private var client
    let placeholder: String
    let allowsAttachments: Bool
    let send: (String, [JSON], [UInt64]) -> Bool
    /// Called as the person types (the others see "... is typing").
    var typed: (() -> Void)?
    /// Where the unsent text is kept ("channel|thread", see ChatDrafts).
    var draftKey: String?
    /// Keep the message to post at a time (the clock); nil: no clock.
    var schedule: ((String, [UInt64], Int64) -> Bool)?
    @State private var customTime = false
    @State private var customAt = Date().addingTimeInterval(3600)
    @State private var text = ""
    @State private var picked: [PhotosPickerItem] = []
    @State private var pending: [PendingFile] = []
    @State private var importing = false
    /// People chosen from the @ list for this message.
    @State private var mentioned: [UInt64] = []
    /// The text right after a pick from the list: no list again for that
    /// "@" while the text still starts with it.
    @State private var completed: String?
    @FocusState private var focused: Bool
    /// The B, I, S and code buttons are showing.
    @State private var formatting = false
    /// A mark a button asked to put around the selection.
    @State private var wrapMark: String?

    init(placeholder: String, send: @escaping (String, [UInt64]) -> Bool) {
        self.placeholder = placeholder
        self.allowsAttachments = false
        self.send = { text, _, picked in send(text, picked) }
    }

    init(placeholder: String, attachments: Bool, send: @escaping (String, [JSON], [UInt64]) -> Bool) {
        self.placeholder = placeholder
        self.allowsAttachments = attachments
        self.send = send
    }

    var ready: [JSON] { pending.compactMap(\.attachment) }
    var uploading: Bool { pending.contains { $0.attachment == nil && $0.error == nil } }

    var body: some View {
        let can = client.canEdit
        VStack(spacing: 6) {
            SuggestionBar(text: $text, mentioned: $mentioned, completed: $completed)
            if !pending.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(pending) { f in PendingChip(file: f) { pending.removeAll { $0.id == f.id } } }
                    }
                    .padding(.horizontal, 4)
                }
            }
            if formatting {
                HStack(spacing: 18) {
                    ForEach([("**", "bold", "Bold"), ("*", "italic", "Italic"), ("~~", "strikethrough", "Strikethrough"), ("`", "chevron.left.forwardslash.chevron.right", "Code")], id: \.0) { mark, icon, name in
                        Button { wrapMark = mark } label: { Image(systemName: icon) }
                            .accessibilityLabel(L(name))
                            .accessibilityIdentifier("format-\(name)")
                    }
                    Spacer()
                }
                .padding(.horizontal, 8)
                .font(.system(size: 17))
            }
            HStack(alignment: .bottom, spacing: 8) {
                if allowsAttachments {
                    PhotosPicker(selection: $picked, maxSelectionCount: 10, matching: .any(of: [.images, .videos])) {
                        Image(systemName: "photo.on.rectangle").font(.system(size: 22)).padding(.bottom, 5)
                    }
                    .disabled(!can || !client.state.isConnected)
                    .accessibilityLabel("Add photos or videos")
                    .accessibilityIdentifier("attachButton")
                    Button { importing = true } label: {
                        Image(systemName: "paperclip").font(.system(size: 22)).padding(.bottom, 5)
                    }
                    .disabled(!can || !client.state.isConnected || pending.count >= maxFiles)
                    .accessibilityLabel("Add files")
                    .accessibilityIdentifier("attachFileButton")
                }
                Button { formatting.toggle() } label: {
                    Image(systemName: "textformat").font(.system(size: 20)).padding(.bottom, 6)
                }
                .disabled(!can)
                .accessibilityLabel("Formatting")
                .accessibilityIdentifier("formatButton")
                ComposerField(placeholder: can ? placeholder : "Sign in to send messages", text: $text, wrapMark: $wrapMark)
                    .lineLimit(1...5)
                    .focused($focused)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 9)
                    .background(RoundedRectangle(cornerRadius: 20, style: .continuous).fill(Color(.secondarySystemBackground)))
                    .disabled(!can)
                    .accessibilityIdentifier("composer")
                if let schedule, !text.isBlank, pending.isEmpty {
                    Menu {
                        ForEach(ScheduleTimes.choices(), id: \.0) { label, at in
                            Button(L(label)) {
                                if schedule(text, mentioned, at) { text = ""; mentioned = []; completed = nil }
                            }
                        }
                        Button("Custom time…") { customAt = Date().addingTimeInterval(3600); customTime = true }
                    } label: {
                        Image(systemName: "clock").font(.system(size: 20)).padding(.bottom, 6)
                    }
                    .accessibilityLabel("Schedule message")
                    .accessibilityIdentifier("scheduleButton")
                }
                Button {
                    if send(text, ready, mentioned) {
                        text = ""
                        pending = []
                        mentioned = []
                        completed = nil
                    }
                } label: {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.system(size: 32))
                        .symbolRenderingMode(.hierarchical)
                }
                .disabled(!can || uploading || (text.isBlank && ready.isEmpty))
                .accessibilityLabel("Send")
                .accessibilityIdentifier("sendButton")
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.bar)
        .overlay(alignment: .top) { Divider() }
        .onChange(of: text) {
            if !text.isBlank { typed?() }
            if let draftKey { ChatDrafts.save(text, key: draftKey, server: client.settings.server) }
        }
        .onAppear {
            if let draftKey, text.isEmpty { text = ChatDrafts.load(key: draftKey, server: client.settings.server) }
        }
        .sheet(isPresented: $customTime) {
            NavigationStack {
                Form {
                    DatePicker("Send at", selection: $customAt, in: Date()..., displayedComponents: [.date, .hourAndMinute])
                        .accessibilityIdentifier("scheduleDate")
                }
                .navigationTitle("Schedule message")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    CancelButton { customTime = false }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Schedule") {
                            if let schedule, schedule(text, mentioned, Int64(customAt.timeIntervalSince1970)) {
                                text = ""; mentioned = []; completed = nil
                            }
                            customTime = false
                        }
                        .accessibilityIdentifier("scheduleConfirm")
                    }
                }
            }
            .presentationDetents([.medium])
        }
        .onChange(of: picked) {
            let items = picked
            picked = []
            for item in items { upload(item) }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            guard case .success(let urls) = result else { return }
            for url in urls.prefix(max(0, maxFiles - pending.count)) { upload(file: url) }
        }
    }

    /// Attachments in one message, as on the Mac (MAX_PER_MESSAGE).
    let maxFiles = 10

    /// This composer, telling `f` as the person types.
    func onTyping(_ f: @escaping () -> Void) -> Composer {
        var c = self
        c.typed = f
        return c
    }

    /// Keep the unsent text under `key` (ChatDrafts).
    func draft(_ key: String) -> Composer {
        var c = self
        c.draftKey = key
        return c
    }

    /// Offer the clock: keep the message to post at a time.
    func onSchedule(_ f: @escaping (String, [UInt64], Int64) -> Bool) -> Composer {
        var c = self
        c.schedule = f
        return c
    }

    func upload(_ item: PhotosPickerItem) {
        let f = PendingFile()
        pending.append(f)
        let (server, auth) = (client.settings.server, client.fileAuth)
        Task { await uploadMedia(item, server: server, auth: auth, into: f) }
    }

    func upload(file url: URL) {
        let f = PendingFile()
        f.isFile = true
        f.name = url.lastPathComponent
        pending.append(f)
        let (server, auth) = (client.settings.server, client.fileAuth)
        Task { await uploadFile(url, server: server, auth: auth, into: f) }
    }
}

/// "Bo is typing..." above the message box while the server says others
/// are writing in this conversation (see Typing.swift); nothing otherwise.
struct TypingLine: View {
    @Environment(SyncClient.self) private var client
    let channelId: UInt64
    /// The message whose thread this line is under; nil for the conversation.
    var thread: UInt64? = nil

    var body: some View {
        // looked at again every second, so it goes away when they stop
        TimelineView(.periodic(from: .now, by: 1)) { tl in
            let who = client.typing(in: channelId, thread: thread, at: tl.date)
            if !who.isEmpty {
                let name = { (m: UInt64) in client.view.members.first { $0.id == m }?.name ?? String(localized: "Someone") }
                Group {
                    switch who.count {
                    case 1: Text("\(name(who[0])) is typing...")
                    case 2: Text("\(name(who[0])) and \(name(who[1])) are typing...")
                    default: Text("Several people are typing...")
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 20)
                .padding(.vertical, 4)
                .background(.bar)
                .accessibilityIdentifier("typingLine")
            }
        }
    }
}

/// Channel Settings, as on the Mac: name and topic, and who is in it. A
/// private channel with chosen people is how a group conversation is made.
/// Whoever made the channel, an owner or an admin may change who else is in
/// it; anyone may join or leave.
struct ChannelSettingsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @Environment(\.dismiss) private var dismiss
    let channelId: UInt64
    @State private var name = ""
    @State private var topic = ""

    var body: some View {
        let v = client.view
        let me = client.me ?? 0
        NavigationStack {
            if let c = v.channels.first(where: { $0.id == channelId }) {
                let manager = client.data.map { Edits.mayManageChannel($0, channel: c.id, me: me, role: client.settings.role) } ?? false
                Form {
                    if c.kind == .channel {
                        Section("Name") {
                            TextField("Channel name", text: $name)
                                .textInputAutocapitalization(.never)
                                .accessibilityIdentifier("channelSettingsName")
                        }
                        Section("Topic") {
                            TextField("What it's about", text: $topic, axis: .vertical)
                                .accessibilityIdentifier("channelTopicField")
                        }
                    }
                    if c.kind == .channel {
                        Section("Automate") {
                            NavigationLink { WorkflowsView(channelId: c.id) } label: { Label("Workflows", systemImage: "arrow.triangle.branch") }
                                .accessibilityIdentifier("workflowsLink")
                            if ["owner", "admin"].contains(client.settings.role ?? "") {
                                NavigationLink { IntegrationsView(channelId: c.id) } label: { Label("Integrations", systemImage: "link") }
                                    .accessibilityIdentifier("integrationsLink")
                            }
                        }
                        Section("Notify me about") {
                            Picker("Messages", selection: Binding(get: { notifier.prefs.channelMode(c.id) }, set: { notifier.prefs.setChannelMode(c.id, $0) })) {
                                ForEach(NotificationSettingsView.modes, id: \.0) { Text(L($0.1)).tag($0.0) }
                            }
                            .accessibilityIdentifier("channelNotifyMode")
                        }
                    }
                    Section {
                        ForEach(v.members.filter { !$0.invited || c.members.contains($0.id) }) { m in
                            let on = c.members.contains(m.id)
                            Toggle(isOn: Binding(get: { on }, set: { new in
                                client.edit { d, _ in Edits.setChannelMember(&d, channel: c.id, member: m.id, new) }
                            })) {
                                HStack(spacing: 10) {
                                    Avatar(member: m, size: 24)
                                    Text(m.id == me ? "\(m.name) (you)" : m.name)
                                }
                            }
                            .disabled(!client.canEdit || !(manager || m.id == me))
                            .accessibilityIdentifier("channelMember-\(m.id)")
                        }
                    } header: {
                        Text("People")
                    } footer: {
                        Text(c.isPrivate || c.kind == .direct ? "Only the people here see this conversation." : "Everyone can see a public channel; people here get its messages in their list.")
                    }
                }
                .navigationTitle("Settings")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Done") {
                            save(c)
                            dismiss()
                        }
                        .accessibilityIdentifier("channelSettingsDone")
                    }
                }
                .onAppear { name = c.name; topic = c.topic }
            }
        }
    }

    func save(_ c: ChannelView) {
        guard client.canEdit, c.kind == .channel else { return }
        client.edit { d, _ in
            if name.trimmingCharacters(in: .whitespaces) != c.name { Edits.renameChannel(&d, channel: c.id, to: name) }
            if topic.trimmed != c.topic { Edits.setChannelTopic(&d, channel: c.id, to: topic) }
        }
    }
}

/// A post: an announcement with a title (the Mac composer's Post tab).
struct NewPostSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let channelId: UInt64
    @State private var title = ""
    @State private var text = ""

    var body: some View {
        NavigationStack {
            Form {
                TextField("Title", text: $title).accessibilityIdentifier("postTitle")
                TextField("Share an announcement...", text: $text, axis: .vertical).lineLimit(5...).accessibilityIdentifier("postText")
            }
            .navigationTitle("New post")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Post") {
                        let id = client.newId()
                        client.edit { d, me in
                            Edits.sendPost(&d, channel: channelId, title: title, text: text, me: me, id: id, ts: nowTs())
                            // a post starts the channel's workflows too (ChatCommands.swift)
                            Edits.runWorkflows(&d, channel: channelId, message: id, me: me, ts: nowTs()) { client.newId() }
                        }
                        dismiss()
                    }
                    .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty && text.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("postSend")
                }
            }
        }
    }
}

/// A message's text with its formatting (ChatFormat, as on the Mac): inline
/// marks hidden, ``` blocks in a code font, "> " quotes, "- " lists.
struct ChatText: View {
    let text: String
    var edited = false

    var body: some View {
        let blocks = ChatFormat.blocks(text)
        let tail = edited ? Text(text.isEmpty ? "(edited)" : " (edited)").font(.caption).foregroundColor(.secondary) : Text("")
        if blocks.count <= 1, case .text(let t)? = blocks.first ?? .text("") {
            Text(ChatFormat.inline(t)) + tail
        } else {
            VStack(alignment: .leading, spacing: 4) {
                ForEach(Array(blocks.enumerated()), id: \.offset) { i, b in
                    let last = i == blocks.count - 1
                    switch b {
                    case .text(let t):
                        Text(ChatFormat.inline(t)) + (last ? tail : Text(""))
                    case .code(let c):
                        (Text(c).font(.system(.callout, design: .monospaced)) + (last ? tail : Text("")))
                            .padding(.horizontal, 8).padding(.vertical, 6)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(RoundedRectangle(cornerRadius: 6).fill(Color(.secondarySystemFill)))
                    case .quote(let q):
                        HStack(alignment: .top, spacing: 8) {
                            RoundedRectangle(cornerRadius: 1.5).fill(Color(.separator)).frame(width: 3)
                            Text(ChatFormat.inline(q)).foregroundStyle(.secondary) + (last ? tail : Text(""))
                        }
                        .fixedSize(horizontal: false, vertical: true)
                    case .bullet(let t):
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Text("\u{2022}")
                            Text(ChatFormat.inline(t)) + (last ? tail : Text(""))
                        }
                    }
                }
            }
        }
    }
}

/// The page a message's first web address points to: site, title (opens
/// it) and description; its writer can take it away.
struct LinkPreviewCard: View {
    let preview: LinkPreviewView
    let mine: Bool
    let remove: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            RoundedRectangle(cornerRadius: 1.5).fill(Color(.separator)).frame(width: 3)
            VStack(alignment: .leading, spacing: 2) {
                if !preview.site.isEmpty {
                    Text(preview.site).font(.caption).foregroundStyle(.secondary)
                }
                if !preview.title.isEmpty, let url = URL(string: preview.url) {
                    Link(preview.title, destination: url)
                        .font(.subheadline.weight(.semibold))
                        .multilineTextAlignment(.leading)
                        .accessibilityIdentifier("previewTitle")
                }
                if !preview.description.isEmpty {
                    Text(preview.description).font(.footnote).foregroundStyle(.secondary).lineLimit(3)
                }
            }
            Spacer(minLength: 0)
            if mine {
                Button(action: remove) {
                    Image(systemName: "xmark").font(.caption).foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Remove preview")
                .accessibilityIdentifier("removePreview")
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .padding(.top, 4)
    }
}

/// The message box's text field. A mark set in `wrapMark` (a formatting
/// button) goes around the selection (ChatFormat.wrap); before iOS 18,
/// which has no selection to read, two marks go at the end.
struct ComposerField: View {
    let placeholder: String
    @Binding var text: String
    @Binding var wrapMark: String?

    var body: some View {
        if #available(iOS 18.0, *) {
            SelectingField(placeholder: placeholder, text: $text, wrapMark: $wrapMark)
        } else {
            TextField(placeholder, text: $text, axis: .vertical)
                .onChange(of: wrapMark) {
                    guard let mark = wrapMark else { return }
                    let n = text.count
                    text = ChatFormat.wrap(text, n, n, mark: mark).0
                    wrapMark = nil
                }
        }
    }
}

@available(iOS 18.0, *)
private struct SelectingField: View {
    let placeholder: String
    @Binding var text: String
    @Binding var wrapMark: String?
    @State private var selection: TextSelection?
    /// The text was just changed by a formatting button (keep its selection).
    @State private var wrapped = false

    /// The selection, but only while it still points inside `text`. Sending
    /// empties the box before `onChange` can clear the selection, and iOS 18
    /// crashes when the field is handed a range from the old, longer text.
    private var fittingSelection: Binding<TextSelection?> {
        Binding(
            get: { Self.fits(selection, in: text) ? selection : nil },
            set: { selection = $0 }
        )
    }

    static func fits(_ selection: TextSelection?, in text: String) -> Bool {
        guard let selection else { return true }
        func ok(_ i: String.Index) -> Bool {
            i >= text.startIndex && i <= text.endIndex && i.samePosition(in: text) != nil
        }
        switch selection.indices {
        case .selection(let r): return ok(r.lowerBound) && ok(r.upperBound)
        default: return false
        }
    }

    var body: some View {
        TextField(placeholder, text: $text, selection: fittingSelection, axis: .vertical)
            .onChange(of: text) { old, new in
                if wrapped {
                    wrapped = false
                } else if new.isEmpty {
                    selection = nil
                } else if new.count - old.count > 1 {
                    // filled in from outside (a name from the @ list): go on after it
                    selection = TextSelection(insertionPoint: new.endIndex)
                }
            }
            .onChange(of: wrapMark) {
                guard let mark = wrapMark else { return }
                wrapMark = nil
                wrapped = true
                let n = text.count
                var (a, b) = (n, n)
                if case .selection(let r)? = selection?.indices,
                   r.lowerBound >= text.startIndex, r.upperBound <= text.endIndex {
                    a = text.distance(from: text.startIndex, to: r.lowerBound)
                    b = text.distance(from: text.startIndex, to: r.upperBound)
                }
                let (t, a2, b2) = ChatFormat.wrap(text, a, b, mark: mark)
                text = t
                let lo = t.index(t.startIndex, offsetBy: a2), hi = t.index(t.startIndex, offsetBy: b2)
                selection = TextSelection(range: lo..<hi)
            }
    }
}

/// Drafts: every message started here and not sent; a tap opens it.
struct DraftsSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let v = client.view, me = client.me ?? 0
        let drafts = ChatDrafts.all(server: client.settings.server).filter { d in v.visibleChannels(me: me).contains { $0.id == d.0 } }
        NavigationStack {
            List {
                if drafts.isEmpty {
                    Text("Messages you start and don't send show up here.").foregroundStyle(.secondary)
                }
                ForEach(drafts, id: \.2) { ch, thread, text in
                    Button {
                        dismiss()
                        nav.path = [.place(.chats), .channel(ch)] + (thread.map { [.thread(channel: ch, message: $0)] } ?? [])
                    } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            let c = v.channels.first { $0.id == ch }
                            let place = c.map { v.place(of: $0, me: me) } ?? ""
                            Text(thread == nil ? place : String(localized: "Thread in \(place)")).font(.caption).foregroundStyle(.secondary)
                            Text(text).lineLimit(2).foregroundStyle(.primary)
                        }
                    }
                    .accessibilityIdentifier("draft-\(ch)-\(thread ?? 0)")
                }
            }
            .navigationTitle("Drafts")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }
}

/// Scheduled messages: what the server keeps to post later, with Send now,
/// Edit (back into its conversation's box) and Cancel.
struct ScheduledSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let v = client.view, me = client.me ?? 0
        let items = client.waitingToSend
        NavigationStack {
            List {
                if items.isEmpty {
                    Text("Messages you schedule show up here until they are sent.").foregroundStyle(.secondary)
                }
                ForEach(items) { item in
                    VStack(alignment: .leading, spacing: 6) {
                        let c = v.channels.first { $0.id == item.channel }
                        let place = c.map { v.place(of: $0, me: me) } ?? ""
                        Text("\(Fmt.time(item.at)) · \(place)").font(.caption).foregroundStyle(.secondary)
                        Text(item.text).lineLimit(3)
                        HStack(spacing: 16) {
                            Button("Send now") {
                                client.unschedule(item.id)
                                _ = client.edit { d, me in Edits.postScheduled(&d, item, me: me, ts: nowTs()) }
                            }
                            .accessibilityIdentifier("sendNow-\(item.id)")
                            Button("Edit") {
                                client.unschedule(item.id)
                                ChatDrafts.save(item.text, key: "\(item.channel)|\(item.parent ?? 0)", server: client.settings.server)
                                dismiss()
                                nav.path = [.place(.chats), .channel(item.channel)]
                            }
                            .accessibilityIdentifier("editScheduled-\(item.id)")
                            Button("Cancel", role: .destructive) { client.unschedule(item.id) }
                                .accessibilityIdentifier("cancelScheduled-\(item.id)")
                        }
                        .buttonStyle(.borderless)
                        .font(.subheadline)
                    }
                }
            }
            .navigationTitle("Scheduled messages")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .onAppear { client.askScheduled() }
        }
    }
}

/// Forward a message: pick a conversation, add a note.
struct ForwardSheet: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let channelId: UInt64
    let message: MessageView
    @State private var to: UInt64?
    @State private var note = ""

    var body: some View {
        let v = client.view, me = client.me ?? 0
        NavigationStack {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(v.member(message.author)?.name ?? "").font(.subheadline.weight(.semibold))
                        ChatText(text: message.text).lineLimit(4)
                    }
                }
                Section("Forward to") {
                    ForEach(v.visibleChannels(me: me)) { c in
                        Button {
                            to = c.id
                        } label: {
                            HStack {
                                Text(v.place(of: c, me: me)).foregroundStyle(.primary)
                                Spacer()
                                if to == c.id { Image(systemName: "checkmark").foregroundStyle(Color.accentColor) }
                            }
                        }
                        .accessibilityIdentifier("forwardTo-\(c.id)")
                    }
                }
                Section {
                    TextField("Add a message (optional)", text: $note, axis: .vertical).accessibilityIdentifier("forwardNote")
                }
            }
            .navigationTitle("Forward message")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { dismiss() }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Forward") {
                        guard let to else { return }
                        let id = client.newId()
                        _ = client.edit { d, me in
                            Edits.forward(&d, from: channelId, message: message.id, to: to, note: note, online: client.online, me: me, id: id, ts: nowTs())
                        }
                        dismiss()
                    }
                    .disabled(to == nil)
                    .accessibilityIdentifier("forwardButton")
                }
            }
        }
    }
}

/// A forwarded message under a message's text: who wrote it, when, what it
/// said, and where it came from (opens it, when I can see it).
struct ForwardedCard: View {
    @Environment(SyncClient.self) private var client
    let forwarded: ForwardedView

    var body: some View {
        let v = client.view, me = client.me ?? 0
        let from = v.visibleChannels(me: me).first { $0.id == forwarded.channel }
        HStack(alignment: .top, spacing: 8) {
            RoundedRectangle(cornerRadius: 1.5).fill(Color(.separator)).frame(width: 3)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(v.member(forwarded.author)?.name ?? String(localized: "Someone")).font(.subheadline.weight(.semibold))
                    Text(Fmt.time(forwarded.ts)).font(.caption).foregroundStyle(.secondary)
                }
                ChatText(text: forwarded.text)
                if let c = from {
                    NavigationLink(value: Nav.Route.channel(c.id)) {
                        Text("Forwarded from \(v.place(of: c, me: me))").font(.caption).foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("forwardedFrom")
                }
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .padding(.top, 4)
    }
}

/// Workflows that start from this channel's messages, and a new one.
struct WorkflowsView: View {
    @Environment(SyncClient.self) private var client
    let channelId: UInt64
    @State private var keyword = ""
    @State private var action = "post"
    @State private var text = ""
    @State private var target: UInt64?
    @State private var list: UInt64?

    var body: some View {
        let v = client.view, me = client.me ?? 0
        let rules = v.workflows(in: channelId)
        let channels = v.visibleChannels(me: me).filter { $0.kind == .channel }
        Form {
            Section {
                if rules.isEmpty { Text("No workflows yet.").foregroundStyle(.secondary) }
                ForEach(rules.indices, id: \.self) { i in
                    let r = rules[i]
                    let id = r.id
                    VStack(alignment: .leading, spacing: 2) {
                        let k = r.keyword
                        Text(k.isEmpty ? String(localized: "Any message") : String(localized: "Messages with \"\(k)\"")).font(.caption).foregroundStyle(.secondary)
                        switch r.action {
                        case "reply": Text("Reply in its thread: \(r.value)")
                        case "task": Text("Make a task in \(v.list(UInt64(r.value) ?? 0)?.name ?? "")")
                        default:
                            let to = channels.first { $0.id == r.target }?.name ?? ""
                            Text("Post in #\(to): \(r.value)")
                        }
                    }
                    .swipeActions {
                        Button("Delete", role: .destructive) {
                            client.edit { d, _ in Edits.deleteAutomation(&d, automation: id) }
                        }
                    }
                }
            } header: { Text("Workflows") }
            Section {
                TextField("With this word in it (optional)", text: $keyword).accessibilityIdentifier("workflowKeyword")
                Picker("Then", selection: $action) {
                    Text("Post a message in").tag("post")
                    Text("Reply in its thread").tag("reply")
                    Text("Make a task in").tag("task")
                }
                .accessibilityIdentifier("workflowAction")
                if action == "post" {
                    Picker("Channel", selection: $target) {
                        Text("Choose").tag(UInt64?.none)
                        ForEach(channels) { c in Text("#" + c.name).tag(UInt64?.some(c.id)) }
                    }
                    .accessibilityIdentifier("workflowTarget")
                }
                if action == "task" {
                    Picker("List", selection: $list) {
                        Text("Choose").tag(UInt64?.none)
                        ForEach(v.visibleSpaces(me: me).flatMap(\.allListIds), id: \.self) { l in Text(v.list(l)?.name ?? "").tag(UInt64?.some(l)) }
                    }
                }
                if action != "task" {
                    TextField("Message, like: {user} said {text}", text: $text, axis: .vertical).accessibilityIdentifier("workflowText")
                }
                Button("Add workflow") {
                    let id = client.newId()
                    client.edit { d, _ in
                        Edits.addWorkflow(&d, channel: channelId, keyword: keyword, action: action, text: text, target: target, list: list, id: id)
                    }
                    keyword = ""; text = ""
                }
                .disabled(action == "post" ? (target == nil || text.isBlank) : action == "reply" ? text.isBlank : list == nil)
                .accessibilityIdentifier("addWorkflow")
            } header: { Text("When a message is posted here") }
        }
        .navigationTitle("Workflows")
        .navigationBarTitleDisplayMode(.inline)
    }
}

/// A channel's webhooks and the workspace's slash commands (owners and admins).
struct IntegrationsView: View {
    @Environment(SyncClient.self) private var client
    let channelId: UInt64
    @State private var inName = ""
    @State private var outName = ""
    @State private var outURL = ""
    @State private var outTrigger = ""
    @State private var cmdName = ""
    @State private var cmdURL = ""
    @State private var cmdAbout = ""

    var body: some View {
        Form {
            Section {
                ForEach(client.hooks.filter { $0.channel == channelId && $0.kind == "in" }) { h in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(h.name).font(.subheadline.weight(.semibold))
                        Text(client.hookAddress(h.token)).font(.caption.monospaced()).foregroundStyle(.secondary).textSelection(.enabled)
                    }
                    .swipeActions { Button("Remove", role: .destructive) { client.removeHook(h.token) } }
                    .contextMenu { Button("Copy") { UIPasteboard.general.string = client.hookAddress(h.token) } }
                }
                HStack {
                    TextField("Name, like CI", text: $inName).accessibilityIdentifier("incomingName")
                    Button("Add") { client.addHook(kind: "in", channel: channelId, name: inName); inName = "" }
                        .accessibilityIdentifier("addIncoming")
                }
            } header: { Text("Incoming webhooks") } footer: { Text("Other services post to this channel by sending {\"text\": \"...\"} to the address.") }
            Section {
                ForEach(client.hooks.filter { $0.channel == channelId && $0.kind == "out" }) { h in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(h.name).font(.subheadline.weight(.semibold))
                        Text(h.url + (h.trigger.isEmpty ? "" : " · " + h.trigger)).font(.caption).foregroundStyle(.secondary)
                    }
                    .swipeActions { Button("Remove", role: .destructive) { client.removeHook(h.token) } }
                }
                TextField("Name", text: $outName)
                TextField("https://...", text: $outURL).textInputAutocapitalization(.never).keyboardType(.URL)
                TextField("Trigger word", text: $outTrigger).textInputAutocapitalization(.never)
                Button("Add") { client.addHook(kind: "out", channel: channelId, name: outName, url: outURL, trigger: outTrigger); outName = ""; outURL = ""; outTrigger = "" }
                    .disabled(outURL.isBlank)
            } header: { Text("Outgoing webhooks") } footer: { Text("Each new message here (starting with the trigger word, if there is one) is sent to the address; an answer {\"text\": \"...\"} is posted back.") }
            Section {
                ForEach(client.commands) { c in
                    VStack(alignment: .leading, spacing: 2) {
                        Text("/" + c.name).font(.subheadline.weight(.semibold).monospaced())
                        if !c.description.isEmpty { Text(c.description).font(.caption).foregroundStyle(.secondary) }
                    }
                    .swipeActions { Button("Remove", role: .destructive) { client.removeCommand(c.name) } }
                }
                TextField("/deploy", text: $cmdName).textInputAutocapitalization(.never)
                TextField("https://...", text: $cmdURL).textInputAutocapitalization(.never).keyboardType(.URL)
                TextField("What it does", text: $cmdAbout)
                Button("Add") { client.addCommand(name: cmdName, url: cmdURL, description: cmdAbout); cmdName = ""; cmdURL = ""; cmdAbout = "" }
                    .disabled(cmdName.isBlank || cmdURL.isBlank)
            } header: { Text("Slash commands (whole workspace)") }
        }
        .navigationTitle("Integrations")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear { client.askIntegrations() }
    }
}
