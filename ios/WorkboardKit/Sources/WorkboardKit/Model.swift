// Read-only views over the raw workspace tree, for the screens. They are made
// fresh from the JSON whenever it changes and are never written back; edits go
// through the functions in Edits.swift, which touch only the fields they mean
// to change. Field names follow src/model.rs.

import Foundation

public struct MemberView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let color: [Int]
    public let email: String
    public let role: String
    public let invited: Bool

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        color = (j["color"]?.array ?? []).compactMap { $0.int.map(Int.init) }
        email = j["email"]?.string ?? ""
        role = j["role"]?.string ?? ""
        invited = j["invited"]?.bool ?? false
    }

    public var initials: String {
        let s = name.split(separator: " ").compactMap(\.first).prefix(2).map(String.init).joined().uppercased()
        return s.isEmpty ? "?" : s
    }
}

public enum StatusKind: String, Sendable { case open = "Open", custom = "Custom", done = "Done", closed = "Closed" }

public struct StatusView: Hashable, Sendable {
    public let name: String
    public let color: [Int]
    public let kind: StatusKind

    init(_ j: JSON) {
        name = j["name"]?.string ?? ""
        color = (j["color"]?.array ?? []).compactMap { $0.int.map(Int.init) }
        kind = StatusKind(rawValue: j["kind"]?.string ?? "") ?? .custom
    }

    init(name: String, color: [Int], kind: StatusKind) {
        self.name = name
        self.color = color
        self.kind = kind
    }

    public var isClosed: Bool { kind == .done || kind == .closed }

    /// default_statuses() in src/model.rs.
    public static let defaults: [StatusView] = [
        StatusView(name: "to do", color: [0x87, 0x90, 0x9e], kind: .open),
        StatusView(name: "in progress", color: [8, 128, 234], kind: .custom),
        StatusView(name: "complete", color: [41, 151, 100], kind: .closed),
    ]
}

public struct ListView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let archived: Bool

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        archived = j["archived"]?.bool ?? false
    }
}

public struct FolderView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let lists: [ListView]

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        lists = (j["lists"]?.array ?? []).map(ListView.init).filter { !$0.archived }
    }
}

public struct SpaceView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let color: [Int]
    public let statuses: [StatusView]
    public let folders: [FolderView]
    public let lists: [ListView]
    public let privateOwner: UInt64?
    public let archived: Bool

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        color = (j["color"]?.array ?? []).compactMap { $0.int.map(Int.init) }
        let st = (j["statuses"]?.array ?? []).map(StatusView.init)
        statuses = st.isEmpty ? StatusView.defaults : st
        folders = (j["folders"]?.array ?? []).map(FolderView.init)
        lists = (j["lists"]?.array ?? []).map(ListView.init).filter { !$0.archived }
        privateOwner = j["private_owner"]?.uint
        archived = j["archived"]?.bool ?? false
    }

    public var allListIds: [UInt64] { lists.map(\.id) + folders.flatMap { $0.lists.map(\.id) } }
}

public struct ActivityView: Hashable, Sendable {
    public let ts: Int64
    public let author: UInt64
    public let text: String
    public let comment: Bool

    init(_ j: JSON) {
        ts = j["ts"]?.int ?? 0
        author = j["author"]?.uint ?? 0
        text = j["text"]?.string ?? ""
        comment = j["comment"]?.bool ?? false
    }
}

public struct TaskView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let listId: UInt64
    public let parent: UInt64?
    public let name: String
    public let description: String
    public let status: String
    public let priority: Int
    public let assignees: [UInt64]
    public let due: String?
    public let tags: [String]
    public let activity: [ActivityView]
    public let order: Int64
    public let archived: Bool
    public let updated: Int64

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        listId = j["list_id"]?.uint ?? 0
        parent = j["parent"]?.uint
        name = j["name"]?.string ?? ""
        description = j["description"]?.string ?? ""
        status = j["status"]?.string ?? ""
        priority = Int(j["priority"]?.int ?? 0)
        assignees = (j["assignees"]?.array ?? []).compactMap(\.uint)
        due = j["due"]?.string
        tags = (j["tags"]?.array ?? []).compactMap(\.string)
        activity = (j["activity"]?.array ?? []).map(ActivityView.init)
        order = j["order"]?.int ?? 0
        archived = j["archived"]?.bool ?? false
        updated = j["updated"]?.int ?? 0
    }
}

public struct DocView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let space: UInt64?
    public let owner: UInt64
    public let isPrivate: Bool
    public let archived: Bool
    public let parent: UInt64?
    public let title: String
    public let body: String
    public let updated: Int64

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        space = j["space"]?.uint
        owner = j["owner"]?.uint ?? 0
        isPrivate = j["private"]?.bool ?? false
        archived = j["archived"]?.bool ?? false
        parent = j["parent"]?.uint
        title = j["title"]?.string ?? ""
        body = j["body"]?.string ?? ""
        updated = j["updated"]?.int ?? 0
    }
}

public struct ReactionView: Hashable, Sendable {
    public let emoji: String
    public let users: [UInt64]
}

public struct MessageView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let author: UInt64
    public let text: String
    public let ts: Int64
    public let replies: [MessageView]
    public let postTitle: String?
    public let pinned: Bool
    public let call: String?
    public let reactions: [ReactionView]

    init(_ j: JSON) {
        reactions = (j["reactions"]?.array ?? []).map { ReactionView(emoji: $0["emoji"]?.string ?? "", users: ($0["users"]?.array ?? []).compactMap(\.uint)) }
        id = j["id"]?.uint ?? 0
        author = j["author"]?.uint ?? 0
        text = j["text"]?.string ?? ""
        ts = j["ts"]?.int ?? 0
        replies = (j["replies"]?.array ?? []).map(MessageView.init)
        postTitle = j["post_title"]?.string
        pinned = j["pinned"]?.bool ?? false
        call = j["call"]?.string
    }
}

public enum ChannelKind: String, Sendable { case channel = "Channel", direct = "Direct" }

public struct ChannelView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let kind: ChannelKind
    public let topic: String
    public let members: [UInt64]
    public let messages: [MessageView]
    public let lastRead: Int64

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        kind = ChannelKind(rawValue: j["kind"]?.string ?? "") ?? .channel
        topic = j["topic"]?.string ?? ""
        members = (j["members"]?.array ?? []).compactMap(\.uint)
        messages = (j["messages"]?.array ?? []).map(MessageView.init)
        lastRead = j["last_read"]?.int ?? 0
    }

    /// Messages from others newer than this person's read position.
    public func unread(me: UInt64) -> Int {
        messages.filter { $0.author != me && $0.ts > lastRead }.count
    }
}

/// Everything the screens read, made from one snapshot.
public struct WorkspaceView: Sendable {
    public let name: String
    public let members: [MemberView]
    public let spaces: [SpaceView]
    public let tasks: [TaskView]
    public let docs: [DocView]
    public let channels: [ChannelView]

    public init(_ data: JSON) {
        name = data["workspace"]?.string ?? ""
        members = (data["members"]?.array ?? []).map(MemberView.init)
        spaces = (data["spaces"]?.array ?? []).map(SpaceView.init).filter { !$0.archived }
        tasks = (data["tasks"]?.array ?? []).map(TaskView.init).filter { !$0.archived }
        docs = (data["docs"]?.array ?? []).map(DocView.init).filter { !$0.archived }
        channels = (data["channels"]?.array ?? []).map(ChannelView.init)
    }

    public static let empty = WorkspaceView(.object([:]))

    public func member(_ id: UInt64) -> MemberView? { members.first { $0.id == id } }

    public func space(ofList list: UInt64) -> SpaceView? { spaces.first { $0.allListIds.contains(list) } }

    public func statuses(forList list: UInt64) -> [StatusView] { space(ofList: list)?.statuses ?? StatusView.defaults }

    /// status_of in src/model.rs: unknown names read as the first status.
    public func status(of t: TaskView) -> StatusView {
        let sts = statuses(forList: t.listId)
        return sts.first { $0.name == t.status } ?? sts[0]
    }

    public func list(_ id: UInt64) -> ListView? {
        for s in spaces {
            if let l = s.lists.first(where: { $0.id == id }) { return l }
            for f in s.folders { if let l = f.lists.first(where: { $0.id == id }) { return l } }
        }
        return nil
    }

    public func tasks(inList list: UInt64) -> [TaskView] {
        tasks.filter { $0.listId == list && $0.parent == nil }.sorted { $0.order < $1.order }
    }

    public func subtasks(of id: UInt64) -> [TaskView] {
        tasks.filter { $0.parent == id }.sorted { $0.order < $1.order }
    }

    /// Spaces this person can see (another person's private space is hidden).
    public func visibleSpaces(me: UInt64) -> [SpaceView] {
        spaces.filter { $0.privateOwner == nil || $0.privateOwner == me }
    }

    public func visibleDocs(me: UInt64) -> [DocView] {
        docs.filter { !$0.isPrivate || $0.owner == me }.sorted { $0.updated > $1.updated }
    }

    /// Channels this person is in, plus every public channel.
    public func visibleChannels(me: UInt64) -> [ChannelView] {
        channels.filter { $0.kind == .channel || $0.members.contains(me) }
    }

    /// A DM's title: the other people in it.
    public func title(of c: ChannelView, me: UInt64) -> String {
        if c.kind == .channel { return c.name }
        let others = c.members.filter { $0 != me }.compactMap { member($0)?.name }
        if others.isEmpty { return c.name.isEmpty ? (member(me)?.name ?? "Me") : c.name }
        return others.joined(separator: ", ")
    }

    /// My Tasks: open tasks assigned to me.
    public func myTasks(me: UInt64) -> [TaskView] {
        tasks.filter { $0.assignees.contains(me) && !status(of: $0).isClosed }
    }
}

/// "YYYY-MM-DD" in the device's time zone, as the Mac app's dates are local.
public func dayString(_ date: Date = Date()) -> String {
    let f = DateFormatter()
    f.calendar = Calendar(identifier: .gregorian)
    f.locale = Locale(identifier: "en_US_POSIX")
    f.dateFormat = "yyyy-MM-dd"
    return f.string(from: date)
}
