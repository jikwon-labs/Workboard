import PhotosUI
import SwiftUI
import WorkboardKit

struct ChatListView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @State private var newChannel = false
    @State private var newDM = false

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
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button { newChannel = true } label: { Image(systemName: "square.and.pencil") }
                    .disabled(!client.canEdit)
                    .accessibilityLabel("New channel")
                    .accessibilityIdentifier("newChannelButton")
            }
            ToolbarItem(placement: .topBarLeading) {
                Button { newDM = true } label: { Image(systemName: "person.crop.circle.badge.plus") }
                    .disabled(!client.canEdit)
                    .accessibilityLabel("New message")
                    .accessibilityIdentifier("newDMButton")
            }
            ToolbarItem(placement: .topBarTrailing) { SettingsButton() }
        }
        .sheet(isPresented: $newChannel) { NewChannelSheet() }
        .sheet(isPresented: $newDM) { NewDMSheet() }
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
        nav.chat.append(Nav.Route.channel(id))
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
        nav.chat.append(Nav.Route.channel(id))
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
    @Environment(CallManager.self) private var calls
    @Environment(\.openURL) private var openURL
    let channelId: UInt64
    @State private var settings = false
    @State private var posting = false

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
                Composer(placeholder: "Message \(c.kind == .channel ? "#" + c.name : v.title(of: c, me: me))", attachments: true) { text, files, picked in
                    client.edit { d, me in
                        Edits.sendMessage(&d, channel: c.id, text: text, attachments: files, picked: picked, me: me, id: client.newId(), ts: nowTs())
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
            .safeAreaInset(edge: .bottom, spacing: 0) {
                Composer(placeholder: "Reply…", attachments: true) { text, files, picked in
                    client.edit { d, me in
                        Edits.sendReply(&d, channel: c.id, to: m.id, text: text, attachments: files, picked: picked, me: me, id: client.newId(), ts: nowTs())
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
    @Environment(CallManager.self) private var calls
    @Environment(\.openURL) private var openURL
    let message: MessageView
    let channelId: UInt64
    let compact: Bool
    var isReply = false
    @State private var editing = false
    @State private var draft = ""
    @State private var confirmDelete = false

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
                    Text(author?.name ?? String(localized: "Someone")).font(.subheadline.weight(.semibold))
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
            if let title = message.postTitle, !title.isEmpty {
                Text(title).font(.headline)
            }
            if !message.text.isEmpty || message.edited != nil {
                (Text(message.text) + (message.edited != nil ? Text(message.text.isEmpty ? "(edited)" : " (edited)").font(.caption).foregroundColor(.secondary) : Text("")))
                    .textSelection(.enabled)
                    .accessibilityIdentifier("messageText-\(message.id)")
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

    var body: some View {
        NavigationStack {
            Form {
                TextField("Message", text: $draft, axis: .vertical)
                    .lineLimit(1...10)
                    .accessibilityIdentifier("editMessageField")
            }
            .navigationTitle("Edit message")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                CancelButton { editing = false }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        client.edit { d, me in Edits.editMessage(&d, channel: channelId, message: message.id, text: draft, me: me, ts: nowTs()) }
                        editing = false
                    }
                    .disabled(draft.isBlank)
                    .accessibilityIdentifier("saveEditButton")
                }
            }
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

    /// The people the @ list shows now (none: no list).
    var mentionChoices: [MemberView] {
        guard let q = Mentions.query(in: text), let at = text.lastIndex(of: "@") else { return [] }
        if let c = completed, text.hasPrefix(c), let cAt = c.lastIndex(of: "@"),
           text.distance(from: text.startIndex, to: at) == c.distance(from: c.startIndex, to: cAt) { return [] }
        return Mentions.candidates(q, members: client.view.members, me: client.me)
    }

    var ready: [JSON] { pending.compactMap(\.attachment) }
    var uploading: Bool { pending.contains { $0.attachment == nil && $0.error == nil } }

    var body: some View {
        let can = client.canEdit
        VStack(spacing: 6) {
            let choices = mentionChoices
            if !choices.isEmpty {
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
                                .padding(.horizontal, 10)
                                .padding(.vertical, 5)
                                .background(Capsule().fill(Color(.secondarySystemBackground)))
                            }
                            .buttonStyle(.plain)
                            .accessibilityElement(children: .combine)
                            .accessibilityLabel("Mention \(m.name)" + (m.email.isEmpty ? "" : ", \(m.email)"))
                            .accessibilityIdentifier("mention-\(m.id)")
                        }
                    }
                    .padding(.horizontal, 4)
                }
                .accessibilityIdentifier("mentionList")
            }
            if !pending.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(pending) { f in PendingChip(file: f) { pending.removeAll { $0.id == f.id } } }
                    }
                    .padding(.horizontal, 4)
                }
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
                TextField(can ? placeholder : "Sign in to send messages", text: $text, axis: .vertical)
                    .lineLimit(1...5)
                    .focused($focused)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 9)
                    .background(RoundedRectangle(cornerRadius: 20, style: .continuous).fill(Color(.secondarySystemBackground)))
                    .disabled(!can)
                    .accessibilityIdentifier("composer")
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
                        client.edit { d, me in Edits.sendPost(&d, channel: channelId, title: title, text: text, me: me, id: id, ts: nowTs()) }
                        dismiss()
                    }
                    .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty && text.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("postSend")
                }
            }
        }
    }
}
