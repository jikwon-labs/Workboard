// A conversation narrowed to some of its messages (the Mac app's channel
// panel: Replies, Pinned messages, Saved, Files, Links, Turned into tasks),
// and its files gathered in one place. The rules are MessageFilter in
// WorkboardKit (ChatFilters.swift).

import SwiftUI
import WorkboardKit

extension MessageFilter {
    /// In the filter menu.
    var menuTitle: LocalizedStringKey {
        switch self {
        case .all: return "All messages"
        case .replies: return "Replies"
        case .pinned: return "Pinned messages"
        case .announcements: return "Announcements"
        case .saved: return "Saved"
        case .files: return "Files"
        case .links: return "Links"
        case .tasks: return "Turned into tasks"
        }
    }

    /// Over the list while the filter is on.
    var bannerTitle: LocalizedStringKey {
        switch self {
        case .all: return "All messages"
        case .replies: return "Messages with replies"
        case .pinned: return "Pinned messages"
        case .announcements: return "Announcements"
        case .saved: return "Saved messages"
        case .files: return "Messages with files"
        case .links: return "Messages with links"
        case .tasks: return "Messages turned into tasks"
        }
    }

    var icon: String {
        switch self {
        case .all: return "bubble.left.and.bubble.right"
        case .replies: return "arrowshape.turn.up.left"
        case .pinned: return "pin"
        case .announcements: return "megaphone"
        case .saved: return "bookmark"
        case .files: return "paperclip"
        case .links: return "link"
        case .tasks: return "checkmark.circle"
        }
    }
}

/// The toolbar button that picks what the conversation shows.
struct MessageFilterMenu: View {
    @Binding var filter: MessageFilter

    var body: some View {
        Menu {
            Picker("Filter messages", selection: $filter) {
                ForEach(MessageFilter.allCases, id: \.self) { f in
                    Label(f.menuTitle, systemImage: f.icon).tag(f)
                }
            }
        } label: {
            Image(systemName: filter == .all ? "line.3.horizontal.decrease.circle" : "line.3.horizontal.decrease.circle.fill")
        }
        .accessibilityLabel("Filter messages")
        .accessibilityIdentifier("filterMessagesButton")
    }
}

/// Names the filter that is on, with an x back to all messages.
struct MessageFilterBanner: View {
    @Binding var filter: MessageFilter

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: filter.icon).foregroundStyle(.secondary).accessibilityHidden(true)
            Text(filter.bannerTitle)
                .font(.subheadline.weight(.medium))
                .accessibilityIdentifier("filterBanner")
            Spacer(minLength: 0)
            Button { withAnimation { filter = .all } } label: {
                Image(systemName: "xmark.circle.fill").font(.title3).foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("All messages")
            .accessibilityIdentifier("clearFilterButton")
        }
        .pill(horizontal: 12, vertical: 8)
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(.bar)
    }
}

/// The conversation's newest announcement over its messages: tapping it
/// goes to the post, "Show all" lists only the announcements, the x hides
/// it until a newer one is posted.
struct AnnouncementBanner: View {
    let message: MessageView
    let author: String
    let show: () -> Void
    let showAll: () -> Void
    let hide: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Button(action: show) {
                HStack(spacing: 8) {
                    Image(systemName: "megaphone").foregroundStyle(Color.accentColor).accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(verbatim: line).font(.subheadline.weight(.medium)).lineLimit(1)
                        Text(verbatim: "\(author) · \(Fmt.time(message.ts))").font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("announcementBanner")
            Button("Show all", action: showAll)
                .font(.subheadline)
                .buttonStyle(.borderless)
                .accessibilityIdentifier("allAnnouncementsButton")
            Button(action: hide) {
                Image(systemName: "xmark").font(.footnote.weight(.semibold)).foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Hide announcement")
            .accessibilityIdentifier("hideAnnouncementButton")
        }
        .pill(horizontal: 12, vertical: 8)
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(.bar)
    }

    /// Its title, else the first line of its text.
    var line: String {
        if let t = message.postTitle?.trimmed, !t.isEmpty, !message.hasDefaultPostTitle { return t }
        return message.text.split(separator: "\n").map { $0.trimmingCharacters(in: .whitespaces) }.first { !$0.isEmpty }
            ?? String(localized: "Update")
    }
}

/// The conversation's files, newest first, with those sent in thread
/// replies: pictures and videos as a grid, other files as a list. Each can
/// go to the message it was sent in.
struct ConversationFilesView: View {
    @Environment(SyncClient.self) private var client
    let channel: ChannelView
    /// Go to the message a file was sent in.
    let show: (ConversationFile) -> Void
    @State private var part: Part = .media

    enum Part: Hashable { case media, other }

    var body: some View {
        let all = channel.files
        // sounds go with the other files (they have nothing to show in a grid)
        let media = all.enumerated().filter { !$0.element.attachment.isFile && !$0.element.attachment.isAudio }
        let other = all.enumerated().filter { $0.element.attachment.isFile || $0.element.attachment.isAudio }
        if all.isEmpty {
            EmptyNote(title: "No files in this conversation yet", systemImage: "paperclip")
                .accessibilityIdentifier("noFilesNote")
        } else {
            GeometryReader { geo in
                let gap: CGFloat = 2
                let side = max(60, ((geo.size.width - 24 - gap * 2) / 3).rounded(.down))
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        Picker("Files", selection: $part) {
                            Text("Pictures and videos").tag(Part.media)
                            Text("Other files").tag(Part.other)
                        }
                        .pickerStyle(.segmented)
                        .accessibilityIdentifier("filesPart")
                        if part == .media {
                            LazyVGrid(columns: Array(repeating: GridItem(.fixed(side), spacing: gap), count: 3), alignment: .leading, spacing: gap) {
                                // by the message and the file's sha, not the place in the
                                // list: a new file at the top doesn't make every tile new
                                ForEach(media, id: \.element.tileId) { _, f in
                                    Group {
                                        if f.attachment.isVideo {
                                            VideoTile(attachment: f.attachment, box: CGSize(width: side, height: side))
                                        } else {
                                            PictureView(attachment: f.attachment, box: CGSize(width: side, height: side))
                                        }
                                    }
                                    .contextMenu { showButton(f) }
                                }
                            }
                        } else {
                            LazyVStack(alignment: .leading, spacing: 8) {
                                ForEach(other, id: \.element.tileId) { _, f in
                                    Group {
                                        if f.attachment.isAudio {
                                            VoiceCard(attachment: f.attachment, detail: detail(f))
                                        } else {
                                            FileTile(attachment: f.attachment, detail: detail(f))
                                        }
                                    }
                                    .contextMenu { showButton(f) }
                                }
                            }
                        }
                    }
                    .padding(12)
                }
            }
        }
    }

    func showButton(_ f: ConversationFile) -> some View {
        Button { show(f) } label: { Label("Show in conversation", systemImage: "text.bubble") }
    }

    /// Who sent it and when.
    func detail(_ f: ConversationFile) -> String {
        let who = f.message.bot ?? client.view.member(f.message.author)?.name ?? String(localized: "Someone")
        return "\(who) · \(Fmt.time(f.message.ts))"
    }
}

extension ConversationFile {
    /// One tile per file of a message (the same file twice in one message
    /// is one tile).
    var tileId: String { "\(message.id)/\(attachment.file)/\(attachment.name)" }
}
