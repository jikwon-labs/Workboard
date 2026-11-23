// Read-only views over the raw workspace tree, for the screens. They are made
// fresh from the JSON whenever it changes and are never written back; edits go
// through the functions in Edits.swift, which touch only the fields they mean
// to change. Field names follow src/model.rs.

import Foundation

public struct MemberView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    /// The name people see: the display name when there is one, else the
    /// full name (the Mac app's `Member::shown`).
    public let name: String
    public let fullName: String
    public let displayName: String
    /// What they do on the team ("Designer"); empty when not set.
    public let title: String
    public let color: [Int]
    public let email: String
    public let role: String
    public let invited: Bool
    /// Their profile photo: a square picture on the team server, named like
    /// a chat picture (Member::photo). Nil without one.
    public let photo: AttachmentView?
    /// What they're up to ("📅 In a meeting"), from Set status; empty when
    /// not set.
    public let statusText: String

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        fullName = j["name"]?.string ?? ""
        displayName = (j["display_name"]?.string ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        title = (j["title"]?.string ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        name = Mentions.shownName(j)
        color = (j["color"]?.array ?? []).compactMap { $0.int.map(Int.init) }
        email = j["email"]?.string ?? ""
        role = j["role"]?.string ?? ""
        invited = j["invited"]?.bool ?? false
        photo = j["photo"].flatMap { $0.object == nil ? nil : AttachmentView($0) }.flatMap { FileAPI.isFileName($0.file) ? $0 : nil }
        statusText = (j["status_text"]?.string ?? "").trimmed
    }

    public var initials: String { Self.initials(of: name) }

    /// The first letters of the first two words (Member::initials), "?"
    /// for none.
    public static func initials(of name: String) -> String {
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
    /// Archived folders leave the sidebar with their lists until restored.
    public let archived: Bool

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        archived = j["archived"]?.bool ?? false
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
        folders = (j["folders"]?.array ?? []).map(FolderView.init).filter { !$0.archived }
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
    /// Time of day on the due date as "HH:mm" (the Mac app's `due_time`),
    /// only when there is a due date.
    public let dueTime: String?
    public let tags: [String]
    public let checklist: [CheckItemView]
    public let activity: [ActivityView]
    public let order: Int64
    public let archived: Bool
    public let updated: Int64
    public let created: Int64
    public let start: String?
    /// Time of day on the start date ("HH:mm"), only with a start date.
    public let startTime: String?
    /// People following the task besides its assignees.
    public let watchers: [UInt64]
    /// Assignees and the maker who turned the task's updates off
    /// (Task::unwatched in src/model/task.rs).
    public let unwatched: [UInt64]
    /// Tasks this one waits on (Blocking is the other side, worked out in
    /// WorkspaceView.relations).
    public let waitingOn: [UInt64]
    public let linked: [UInt64]
    /// daily, weekly, monthly or yearly; nil when it doesn't repeat.
    public let recur: String?
    public let estimateMin: UInt32?
    public let timeEntries: [TimeEntryView]
    /// Every running timer: who started it (nil for an old timer that
    /// didn't record it) and when (Task::running_timers).
    public let timers: [RunningTimerView]
    /// Custom field values by field id.
    public let custom: [UInt64: String]
    public let milestone: Bool
    public let attachments: [AttachmentView]

    /// Task::watched_by in src/model/task.rs: whether `id` gets the task's
    /// comments and changes (assignees and the maker unless they turned
    /// that off, and the people who chose to watch).
    public func watchedBy(_ id: UInt64) -> Bool {
        if unwatched.contains(id) { return false }
        if watchers.contains(id) || assignees.contains(id) { return true }
        return activity.contains { a in
            a.author == id && !a.comment && (a.text == "created this task" || a.text.hasPrefix("duplicated this task from "))
        }
    }

    init(_ j: JSON) {
        created = j["created"]?.int ?? 0
        start = j["start"]?.string
        startTime = start == nil ? nil : j["start_time"]?.string.flatMap(TaskView.hhmm)
        watchers = (j["watchers"]?.array ?? []).compactMap(\.uint)
        unwatched = (j["unwatched"]?.array ?? []).compactMap(\.uint)
        waitingOn = (j["waiting_on"]?.array ?? []).compactMap(\.uint)
        linked = (j["linked"]?.array ?? []).compactMap(\.uint)
        recur = j["recur"]?.string
        estimateMin = j["estimate_min"]?.uint.map { UInt32(clamping: $0) }
        timeEntries = (j["time_entries"]?.array ?? []).map(TimeEntryView.init)
        var running: [RunningTimerView] = []
        if let st = j["timer_started"]?.int { running.append(RunningTimerView(member: j["timer_by"]?.uint, start: st)) }
        running += (j["timers"]?.array ?? []).compactMap { r in r["start"]?.int.map { RunningTimerView(member: r["member"]?.uint, start: $0) } }
        timers = running
        var cf: [UInt64: String] = [:]
        for (k, v) in j["custom"]?.object ?? [:] {
            if let id = UInt64(k), let s = v.string, !s.isEmpty { cf[id] = s }
        }
        custom = cf
        milestone = j["milestone"]?.bool ?? false
        attachments = (j["attachments"]?.array ?? []).map(AttachmentView.init).filter { !$0.file.isEmpty }
        id = j["id"]?.uint ?? 0
        listId = j["list_id"]?.uint ?? 0
        parent = j["parent"]?.uint
        name = j["name"]?.string ?? ""
        description = j["description"]?.string ?? ""
        status = j["status"]?.string ?? ""
        priority = Int(j["priority"]?.int ?? 0)
        assignees = (j["assignees"]?.array ?? []).compactMap(\.uint)
        due = j["due"]?.string
        dueTime = due == nil ? nil : j["due_time"]?.string.flatMap(TaskView.hhmm)
        tags = (j["tags"]?.array ?? []).compactMap(\.string)
        checklist = (j["checklist"]?.array ?? []).map(CheckItemView.init)
        activity = (j["activity"]?.array ?? []).map(ActivityView.init)
        order = j["order"]?.int ?? 0
        archived = j["archived"]?.bool ?? false
        updated = j["updated"]?.int ?? 0
    }
}

/// A stretch of tracked time (TimeEntry in src/model.rs); `member` 0 is an
/// entry from before entries had an owner.
public struct TimeEntryView: Hashable, Sendable {
    public let start: Int64
    public let end: Int64
    public let member: UInt64

    init(_ j: JSON) {
        start = j["start"]?.int ?? 0
        end = j["end"]?.int ?? 0
        member = j["member"]?.uint ?? 0
    }

    public var secs: Int64 { max(0, end - start) }
}

public struct RunningTimerView: Hashable, Sendable {
    public let member: UInt64?
    public let start: Int64
}

/// A custom field (CustomField in src/model.rs): text, number, dropdown,
/// checkbox or date.
public struct CustomFieldView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let name: String
    public let kind: String
    public let options: [String]

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        kind = j["kind"]?.string ?? "text"
        options = (j["options"]?.array ?? []).compactMap(\.string)
    }
}

/// A status automation ("When status changes to X, then ...") or a
/// channel's workflow (`trigger` "message"), as Automation in
/// src/model/extra.rs.
public struct AutomationView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    public let enabled: Bool
    /// The status that starts a status automation (empty for a workflow).
    public let triggerStatus: String
    /// assign, priority, comment or move; for a workflow post, reply or task.
    public let action: String
    /// A member id, priority, comment text or List id for a status
    /// automation; the message text, or the List id for "task", for a workflow.
    public let value: String
    /// The List or Space it runs in; nil: everywhere.
    public let list: UInt64?
    /// "" for a status automation, "message" for a workflow.
    public let trigger: String
    /// The conversation whose messages start a workflow.
    public let channel: UInt64?
    /// A workflow runs only for messages with this in them, when set.
    public let keyword: String
    /// Where a workflow's "post" goes.
    public let target: UInt64?

    public var isWorkflow: Bool { trigger == "message" }

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        enabled = j["enabled"]?.bool ?? false
        triggerStatus = j["trigger_status"]?.string ?? ""
        action = j["action"]?.string ?? ""
        value = j["value"]?.string ?? ""
        list = j["list"]?.uint
        trigger = j["trigger"]?.string ?? ""
        channel = j["channel"]?.uint
        keyword = j["keyword"]?.string ?? ""
        target = j["target"]?.uint
    }
}

/// Workspace > ClickApps (ClickApps in src/model.rs): every one is on
/// unless turned off.
public struct AppsView: Hashable, Sendable {
    public let priorities: Bool
    public let tags: Bool
    public let timeTracking: Bool
    public let timeEstimates: Bool
    public let customFields: Bool
    public let multipleAssignees: Bool

    init(_ j: JSON?) {
        priorities = j?["priorities"]?.bool ?? true
        tags = j?["tags"]?.bool ?? true
        timeTracking = j?["time_tracking"]?.bool ?? true
        timeEstimates = j?["time_estimates"]?.bool ?? true
        customFields = j?["custom_fields"]?.bool ?? true
        multipleAssignees = j?["multiple_assignees"]?.bool ?? true
    }
}

/// One checklist line (CheckItem in src/model.rs).
public struct CheckItemView: Hashable, Sendable {
    public let text: String
    public let done: Bool
    /// The one person this item is for (set on the Mac).
    public let assignee: UInt64?

    init(_ j: JSON) {
        text = j["text"]?.string ?? ""
        done = j["done"]?.bool ?? false
        assignee = j["assignee"]?.uint
    }
}

extension TaskView {
    /// "14:30" or "14:30:00" -> "14:30"; nil when it isn't a time.
    static func hhmm(_ s: String) -> String? {
        let p = s.split(separator: ":")
        guard p.count >= 2, let h = Int(p[0]), let m = Int(p[1]), (0..<24).contains(h), (0..<60).contains(m) else { return nil }
        return String(format: "%02d:%02d", h, m)
    }

    /// Past due: a day gone by, or today from its due time on (like the Mac
    /// app's `is_overdue_at`). `today` is "YYYY-MM-DD", `now` "HH:mm".
    public func isOverdue(today: String, now: String) -> Bool {
        guard let due else { return false }
        if due < today { return true }
        if due == today, let t = dueTime { return t <= now }
        return false
    }

    /// The due time as the Mac app shows it ("2:30 PM").
    public var dueTimeLabel: String? { dueTime.map(Edits.macTime) }
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

/// A picture, video or other file in a message (Attachment in
/// src/model.rs). The file lives on the team server, named by the sha256 of
/// its contents.
public struct AttachmentView: Hashable, Sendable {
    public let file: String
    public let name: String
    public let isVideo: Bool
    /// Any other file (a PDF, a Markdown note...): shown by its name and
    /// size, opened on a tap. A kind this app doesn't know is one too, as
    /// on the Mac; no kind at all is a picture (older messages).
    public let isFile: Bool
    public let bytes: UInt64
    public let w: Int
    public let h: Int
    public let secs: Double
    /// A video's still (a small JPEG on the server, by sha256 like `file`),
    /// shown instead of fetching the whole video; nil for pictures and for
    /// videos sent before stills were made.
    public let thumb: String?

    init(_ j: JSON) {
        file = j["file"]?.string ?? ""
        name = j["name"]?.string ?? ""
        let kind = j["kind"]?.string
        isVideo = kind == "Video"
        isFile = kind != nil && kind != "Video" && kind != "Image"
        bytes = j["bytes"]?.uint ?? 0
        w = Int(j["w"]?.int ?? 0)
        h = Int(j["h"]?.int ?? 0)
        secs = j["secs"]?.double ?? 0
        let t = j["thumb"]?.string ?? ""
        thumb = isVideo && FileAPI.isFileName(t) ? t : nil
    }
}

public struct MessageView: Identifiable, Hashable, Sendable {
    /// The title the desktop app gives a post sent without one. It is saved
    /// in English like typed text, so screens show it in the phone's language.
    public static let defaultPostTitle = "Update"
    public var hasDefaultPostTitle: Bool { postTitle == Self.defaultPostTitle }

    public let id: UInt64
    public let author: UInt64
    public let text: String
    public let ts: Int64
    public let replies: [MessageView]
    public let postTitle: String?
    public let pinned: Bool
    public let call: String?
    public let reactions: [ReactionView]
    public let attachments: [AttachmentView]
    /// When its author last changed the text.
    public let edited: Int64?
    /// Brought in from a Slack export by a Mac (`imported` holds where it
    /// came from): old history, so it doesn't notify or count as unread.
    public let imported: Bool
    /// The task made from this message ("Create task").
    public let task: UInt64?
    /// Saved for later by this person (kept on this device).
    public let saved: Bool
    /// The page its first web address points to (Message::preview).
    public let preview: LinkPreviewView?
    /// The message it forwards from another conversation.
    public let forwarded: ForwardedView?
    /// The channel's copy of a reply in this thread ("Also send to the channel").
    public let threadOf: UInt64?
    /// Posted by a webhook, slash command or workflow: its name, shown in
    /// place of the person who set it up.
    public let bot: String?

    /// Still says something with its text taken away (Edits.standsWithoutText).
    public var standsWithoutText: Bool {
        !attachments.isEmpty || forwarded != nil || !(postTitle?.trimmed ?? "").isEmpty
    }

    /// Message::imported: set once by the import and never cleared; the
    /// field itself stays in the JSON as it came.
    static func isImported(_ j: JSON) -> Bool { j["imported"]?.string != nil }

    init(_ j: JSON) {
        imported = Self.isImported(j)
        task = j["task"]?.uint
        saved = j["saved"]?.bool ?? false
        preview = j["preview"].flatMap(LinkPreviewView.init)
        forwarded = j["forwarded"].flatMap(ForwardedView.init)
        threadOf = j["thread_of"]?.uint
        bot = j["bot"]?.string
        attachments = (j["attachments"]?.array ?? []).map(AttachmentView.init).filter { !$0.file.isEmpty }
        edited = j["edited"]?.int
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

    /// Only my own messages, and not huddle or call cards (the Mac app
    /// offers no Edit for a message with `call`).
    public func isEditable(by me: UInt64?) -> Bool { me == author && call == nil }
}

public struct LinkPreviewView: Hashable, Sendable {
    public let url: String
    public let title: String
    public let description: String
    public let site: String

    init?(_ j: JSON) {
        guard let u = j["url"]?.string, !u.isEmpty else { return nil }
        url = u
        title = j["title"]?.string ?? ""
        description = j["description"]?.string ?? ""
        site = j["site"]?.string ?? ""
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
    /// Made with "Make Private": only its members see it.
    public let isPrivate: Bool

    /// A message after the read position (not imported): who wrote it.
    struct Later: Hashable, Sendable {
        let author: UInt64
        let id: UInt64
    }

    /// Messages after `lastRead`, in order: few, so unread counts (asked
    /// for by every row of the chat list, on every redraw) needn't go over
    /// a long history.
    let afterRead: [Later]

    init(_ j: JSON) { self.init(j, reusing: nil) }

    /// `prev`: this conversation as it was (its JSON and view); messages
    /// that didn't change keep their views.
    init(_ j: JSON, reusing prev: (json: JSON, view: ChannelView)?) {
        isPrivate = j["private"]?.bool ?? false
        id = j["id"]?.uint ?? 0
        name = j["name"]?.string ?? ""
        kind = ChannelKind(rawValue: j["kind"]?.string ?? "") ?? .channel
        topic = j["topic"]?.string ?? ""
        members = (j["members"]?.array ?? []).compactMap(\.uint)
        let msgs = j["messages"]?.array ?? []
        if let prev, let old = prev.json["messages"]?.array, old.count == prev.view.messages.count {
            var out: [MessageView] = []
            out.reserveCapacity(msgs.count)
            for (i, m) in msgs.enumerated() {
                out.append(i < old.count && old[i] == m ? prev.view.messages[i] : MessageView(m))
            }
            messages = out
        } else {
            messages = msgs.map(MessageView.init)
        }
        let read = j["last_read"]?.int ?? 0
        lastRead = read
        afterRead = messages.filter { !$0.imported && $0.ts > read }.map { Later(author: $0.author, id: $0.id) }
    }

    /// Messages from others newer than this person's read position
    /// (imported history isn't new to anyone).
    public func unread(me: UInt64) -> Int {
        afterRead.reduce(0) { $0 + ($1.author != me ? 1 : 0) }
    }

    /// The first message from others I haven't read: the red "New" line
    /// goes above it.
    public func firstUnread(me: UInt64) -> UInt64? {
        afterRead.first { $0.author != me }?.id
    }

    /// Messages from others after `ts`: those that came in below where the
    /// list is scrolled to, for the "N new messages" note.
    public func newer(than ts: Int64, me: UInt64) -> Int {
        messages.filter { $0.author != me && !$0.imported && $0.ts > ts }.count
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
    public let fields: [CustomFieldView]
    public let apps: AppsView
    /// Status automations and workflows (extra.automations).
    public let automations: [AutomationView]
    /// Every task by id, archived ones too, and whether it is closed in its
    /// list's statuses (archived spaces and lists too): what a goal's tasks
    /// target counts, as goal_progress does on the Mac.
    let goalTaskClosed: [UInt64: Bool]
    /// Every task, archived ones too, one per entry of `tasks` in the JSON
    /// (kept to reuse on the next change).
    let allTasks: [TaskView]

    // looked up by id instead of going through the lists
    let memberAt: [UInt64: Int]
    /// list id -> the space it is in (its index in `spaces`)
    let spaceAt: [UInt64: Int]
    /// list id -> its top-level tasks, in order
    let topTasks: [UInt64: [TaskView]]
    /// task id -> its subtasks, in order
    let subtasksOf: [UInt64: [TaskView]]

    public init(_ data: JSON) { self.init(data, reusing: nil) }

    /// `prev`: the view made from an earlier copy, and that copy. Tasks and
    /// conversations (and their messages) that didn't change keep their
    /// views: most changes touch one of thousands.
    public init(_ data: JSON, reusing prev: (data: JSON, view: WorkspaceView)?) {
        fields = (data.at("extra.fields")?.array ?? []).map(CustomFieldView.init)
        apps = AppsView(data.at("extra.apps"))
        automations = (data.at("extra.automations")?.array ?? []).map(AutomationView.init)
        name = data["workspace"]?.string ?? ""
        members = (data["members"]?.array ?? []).map(MemberView.init)
        spaces = (data["spaces"]?.array ?? []).map(SpaceView.init).filter { !$0.archived }
        let taskJSON = data["tasks"]?.array ?? []
        let allTasks: [TaskView]
        if let prev, let old = prev.data["tasks"]?.array, old.count == prev.view.allTasks.count {
            if old == taskJSON {
                allTasks = prev.view.allTasks
            } else {
                var byId: [UInt64: Int] = [:]
                for (i, t) in old.enumerated() { if let id = t["id"]?.uint, byId[id] == nil { byId[id] = i } }
                allTasks = taskJSON.enumerated().map { i, t in
                    if i < old.count, old[i] == t { return prev.view.allTasks[i] }
                    if let id = t["id"]?.uint, let j = byId[id], old[j] == t { return prev.view.allTasks[j] }
                    return TaskView(t)
                }
            }
        } else {
            allTasks = taskJSON.map(TaskView.init)
        }
        self.allTasks = allTasks
        tasks = allTasks.filter { !$0.archived }
        goalTaskClosed = WorkspaceView.closedTasks(allTasks, spaces: data["spaces"]?.array ?? [])
        docs = (data["docs"]?.array ?? []).map(DocView.init).filter { !$0.archived }
        let chJSON = data["channels"]?.array ?? []
        if let prev, let old = prev.data["channels"]?.array, old.count == prev.view.channels.count {
            var byId: [UInt64: Int] = [:]
            for (i, c) in old.enumerated() { if let id = c["id"]?.uint, byId[id] == nil { byId[id] = i } }
            channels = chJSON.enumerated().map { i, c in
                let j = i < old.count && old[i]["id"] == c["id"] ? i : c["id"]?.uint.flatMap { byId[$0] }
                guard let j else { return ChannelView(c) }
                if old[j] == c { return prev.view.channels[j] }
                return ChannelView(c, reusing: (old[j], prev.view.channels[j]))
            }
        } else {
            channels = chJSON.map { ChannelView($0) }
        }
        var memberAt: [UInt64: Int] = [:]
        for (i, m) in members.enumerated() where memberAt[m.id] == nil { memberAt[m.id] = i }
        self.memberAt = memberAt
        var spaceAt: [UInt64: Int] = [:]
        for (i, s) in spaces.enumerated() {
            for l in s.allListIds where spaceAt[l] == nil { spaceAt[l] = i }
        }
        self.spaceAt = spaceAt
        // in the order of `tasks`, then sorted as tasks(inList:) sorts them
        var top: [UInt64: [TaskView]] = [:]
        var subs: [UInt64: [TaskView]] = [:]
        for t in tasks {
            if let p = t.parent { subs[p, default: []].append(t) } else { top[t.listId, default: []].append(t) }
        }
        topTasks = top.mapValues { $0.sorted { $0.order < $1.order } }
        subtasksOf = subs.mapValues { $0.sorted { $0.order < $1.order } }
    }

    public static let empty = WorkspaceView(.object([:]))

    /// status_ref and is_closed in src/model.rs over every space and list,
    /// archived ones too: a list's space's statuses (the defaults when no
    /// space has the list), the status by name, else the first one.
    static func closedTasks(_ tasks: [TaskView], spaces: [JSON]) -> [UInt64: Bool] {
        var byList: [UInt64: [StatusView]] = [:]
        for s in spaces {
            let statuses = (s["statuses"]?.array ?? []).map(StatusView.init)
            let folders = s["folders"]?.array ?? []
            let lists = (s["lists"]?.array ?? []) + folders.flatMap { $0["lists"]?.array ?? [] }
            for l in lists {
                if let id = l["id"]?.uint, byList[id] == nil { byList[id] = statuses }
            }
        }
        var out: [UInt64: Bool] = [:]
        for t in tasks {
            let sts = byList[t.listId] ?? StatusView.defaults
            let st = sts.first { $0.name == t.status } ?? sts.first ?? StatusView.defaults[0]
            out[t.id] = st.isClosed
        }
        return out
    }

    public func member(_ id: UInt64) -> MemberView? { memberAt[id].map { members[$0] } }

    /// The workflows that start from messages in `channel`.
    public func workflows(in channel: UInt64) -> [AutomationView] {
        automations.filter { $0.isWorkflow && $0.channel == channel }
    }

    /// The people who have joined (not those still invited).
    public var activeMembers: [MemberView] { members.filter { !$0.invited } }

    public func space(ofList list: UInt64) -> SpaceView? { spaceAt[list].map { spaces[$0] } }

    public func statuses(forList list: UInt64) -> [StatusView] { space(ofList: list)?.statuses ?? StatusView.defaults }

    /// status_of in src/model.rs: unknown names read as the first status.
    public func status(of t: TaskView) -> StatusView {
        let sts = statuses(forList: t.listId)
        return sts.first { $0.name == t.status } ?? sts[0]
    }

    public func list(_ id: UInt64) -> ListView? {
        if let i = spaceAt[id] {
            let s = spaces[i]
            if let l = s.lists.first(where: { $0.id == id }) { return l }
            for f in s.folders { if let l = f.lists.first(where: { $0.id == id }) { return l } }
        }
        for s in spaces {
            if let l = s.lists.first(where: { $0.id == id }) { return l }
            for f in s.folders { if let l = f.lists.first(where: { $0.id == id }) { return l } }
        }
        return nil
    }

    public func tasks(inList list: UInt64) -> [TaskView] { topTasks[list] ?? [] }

    public func subtasks(of id: UInt64) -> [TaskView] { subtasksOf[id] ?? [] }

    /// Spaces this person can see (another person's private space is hidden).
    public func visibleSpaces(me: UInt64) -> [SpaceView] {
        spaces.filter { $0.privateOwner == nil || $0.privateOwner == me }
    }

    public func visibleDocs(me: UInt64) -> [DocView] {
        docs.filter { !$0.isPrivate || $0.owner == me }.sorted { $0.updated > $1.updated }
    }

    /// Channels this person is in, plus every public channel.
    public func visibleChannels(me: UInt64) -> [ChannelView] {
        channels.filter { ($0.kind == .channel && !$0.isPrivate) || $0.members.contains(me) }
    }

    /// A DM's title: the other people in it.
    public func title(of c: ChannelView, me: UInt64) -> String {
        if c.kind == .channel { return c.name }
        let others = c.members.filter { $0 != me }.compactMap { member($0)?.name }
        if others.isEmpty { return c.name.isEmpty ? (member(me)?.name ?? "Me") : c.name }
        return others.joined(separator: ", ")
    }

    /// A conversation as a line of text names it: "#general" for a channel,
    /// the other people for a direct message.
    public func place(of c: ChannelView, me: UInt64) -> String {
        c.kind == .channel ? "#" + c.name : title(of: c, me: me)
    }

    /// Tasks this one waits on, tasks waiting on it, and linked tasks (both
    /// directions); missing tasks are left out (relations in
    /// src/app/taskfields.rs).
    public func relations(of id: UInt64) -> (waitingOn: [TaskView], blocking: [TaskView], linked: [TaskView]) {
        guard let t = tasks.first(where: { $0.id == id }) else { return ([], [], []) }
        let byId = Dictionary(tasks.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        let waiting = t.waitingOn.compactMap { byId[$0] }
        let blocking = tasks.filter { $0.waitingOn.contains(id) }
        var linked = t.linked.compactMap { byId[$0] }
        for x in tasks where x.linked.contains(id) && !linked.contains(where: { $0.id == x.id }) { linked.append(x) }
        return (waiting, blocking, linked)
    }

    /// Every tag used on a task, sorted (what the tag field offers).
    public var allTags: [String] { Array(Set(tasks.flatMap(\.tags))).sorted() }

    /// I'm the only member, so an old timer or entry without an owner is mine.
    public var solo: Bool { members.count <= 1 }

    /// When my timer on the task started, if one runs (my_timer_start).
    public func myTimerStart(_ t: TaskView, me: UInt64) -> Int64? {
        t.timers.first { $0.member == me || ($0.member == nil && solo) }?.start
    }

    /// All tracked time on the task, running timers counted up to `now`.
    public func trackedSecs(_ t: TaskView, now: Int64) -> Int64 {
        t.timeEntries.reduce(0) { $0 + $1.secs } + t.timers.reduce(0) { $0 + max(0, now - $1.start) }
    }

    /// Who a time entry belongs to: its owner, or for an old entry the
    /// task's first assignee (Task::entry_owner).
    public func entryOwner(_ t: TaskView, _ e: TimeEntryView) -> UInt64 {
        e.member != 0 ? e.member : (t.assignees.first ?? 0)
    }

    /// Tasks a picker offers for making `id` a subtask of (not itself or
    /// anything under it, not its current parent).
    public func parentCandidates(for id: UInt64) -> [TaskView] {
        var below: Set<UInt64> = [id]
        var grew = true
        while grew {
            grew = false
            for t in tasks where t.parent.map(below.contains) == true && !below.contains(t.id) {
                below.insert(t.id)
                grew = true
            }
        }
        let current = tasks.first { $0.id == id }?.parent
        return tasks.filter { !below.contains($0.id) && $0.id != current }
    }

    /// My Tasks: open tasks assigned to me.
    public func myTasks(me: UInt64) -> [TaskView] {
        tasks.filter { $0.assignees.contains(me) && !status(of: $0).isClosed }
    }
}

