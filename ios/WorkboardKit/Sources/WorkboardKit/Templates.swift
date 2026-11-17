// Template Center (src/app/templates.rs): the built-in templates and the
// team's own (`extra.templates`, TaskTemplate in src/model.rs). A template
// makes a List with its statuses and starting tasks (in a new space or an
// existing one), a task, or a Doc; Lists and Docs can be saved as team
// templates. Built-ins are in English here, as the phone's screens are.

import Foundation

public struct TemplateTaskView: Hashable, Sendable {
    public let name: String
    public let status: String
    public let description: String
    public let checklist: [String]
    public let tags: [String]
    public let priority: Int

    public init(_ name: String, _ status: String, _ priority: Int, description: String = "", checklist: [String] = [], tags: [String] = []) {
        self.name = name
        self.status = status
        self.priority = priority
        self.description = description
        self.checklist = checklist
        self.tags = tags
    }

    init(_ j: JSON) {
        self.init(j["name"]?.string ?? "", j["status"]?.string ?? "", Int(j["priority"]?.int ?? 0),
                  description: j["description"]?.string ?? "",
                  checklist: (j["checklist"]?.array ?? []).compactMap(\.string), tags: (j["tags"]?.array ?? []).compactMap(\.string))
    }

    var json: JSON {
        var j: JSON = ["name": .string(name), "priority": .int(Int64(priority))]
        if !status.isEmpty { j["status"] = .string(status) }
        if !description.isEmpty { j["description"] = .string(description) }
        if !checklist.isEmpty { j["checklist"] = .array(checklist.map(JSON.string)) }
        if !tags.isEmpty { j["tags"] = .array(tags.map(JSON.string)) }
        return j
    }
}

public struct TemplateView: Identifiable, Hashable, Sendable {
    /// "builtin:<key>" or "team:<id>".
    public let id: String
    public let teamId: UInt64?
    public let name: String
    public let about: String
    /// The name of what it makes (task name, List name or Doc title).
    public let taskName: String
    public let description: String
    public let checklist: [String]
    public let tags: [String]
    public let priority: Int
    /// "" a task, "list" a List, "doc" a Doc.
    public let kind: String
    public let statuses: [StatusView]
    public let tasks: [TemplateTaskView]

    init(_ j: JSON) {
        teamId = j["id"]?.uint
        id = "team:\(teamId ?? 0)"
        name = j["name"]?.string ?? ""
        taskName = j["task_name"]?.string ?? ""
        description = j["description"]?.string ?? ""
        checklist = (j["checklist"]?.array ?? []).compactMap(\.string)
        tags = (j["tags"]?.array ?? []).compactMap(\.string)
        priority = Int(j["priority"]?.int ?? 0)
        kind = j["kind"]?.string ?? ""
        statuses = (j["statuses"]?.array ?? []).map(StatusView.init)
        tasks = (j["tasks"]?.array ?? []).map(TemplateTaskView.init)
        about = kind == "list" ? "\(tasks.count) tasks, \(statuses.count) statuses" : kind == "doc" ? "A doc" : "A task"
    }

    init(key: String, name: String, about: String, taskName: String? = nil, description: String = "", checklist: [String] = [], tags: [String] = [], priority: Int = 0, kind: String, statuses: [StatusView] = [], tasks: [TemplateTaskView] = []) {
        id = "builtin:\(key)"
        teamId = nil
        self.name = name
        self.about = about
        self.taskName = taskName ?? name
        self.description = description
        self.checklist = checklist
        self.tags = tags
        self.priority = priority
        self.kind = kind
        self.statuses = statuses
        self.tasks = tasks
    }

    /// builtins() in src/app/templates.rs.
    public static let builtins: [TemplateView] = {
        let grey = [0x87, 0x90, 0x9e], review = [0xb6, 0x60, 0xe0], prog = [8, 128, 234], done = [41, 151, 100]
        let s = { (n: String, c: [Int], k: StatusKind) in StatusView(name: n, color: c, kind: k) }
        let bugText = "Steps to reproduce:\n1. \n2. \n\nWhat happened:\n\nWhat should happen:\n"
        return [
            TemplateView(key: "project-tracker", name: "Project tracker", about: "Plan a project from kickoff to wrap-up, with a review step", kind: "list",
                         statuses: [s("to do", grey, .open), s("in progress", prog, .custom), s("in review", review, .custom), s("complete", done, .closed)],
                         tasks: [TemplateTaskView("Write the project brief", "to do", 2, description: "Goals, scope, and what done looks like.", checklist: ["Goals", "Scope", "Success measures"]),
                                 TemplateTaskView("Plan milestones and dates", "to do", 3),
                                 TemplateTaskView("Hold the kickoff meeting", "to do", 3, checklist: ["Book a time", "Share the brief", "Agree on roles"]),
                                 TemplateTaskView("Weekly status update", "to do", 0),
                                 TemplateTaskView("Wrap up and note lessons learned", "to do", 4)]),
            TemplateView(key: "bug-tracker", name: "Bug tracker", about: "Report, sort and fix bugs until they are closed", kind: "list",
                         statuses: [s("reported", grey, .open), s("triaged", [0xf8, 0xae, 0x00], .custom), s("in progress", prog, .custom), s("fixed", [0x3d, 0xb8, 0x8b], .done), s("closed", done, .closed)],
                         tasks: [TemplateTaskView("Example: The sign-in button does nothing", "reported", 2, description: bugText, checklist: ["Reproduce", "Find the cause", "Fix and test"], tags: ["bug"]),
                                 TemplateTaskView("Example: A typo on the settings page", "triaged", 4, tags: ["bug"])]),
            TemplateView(key: "content-calendar", name: "Content calendar", about: "Take posts from idea to published on a schedule", kind: "list",
                         statuses: [s("idea", grey, .open), s("writing", [0x44, 0x66, 0xff], .custom), s("editing", [0xe1, 0x6b, 0x16], .custom), s("scheduled", review, .custom), s("published", done, .closed)],
                         tasks: [TemplateTaskView("Blog post: product update", "writing", 3, checklist: ["Outline", "Draft", "Edit", "Pick an image"], tags: ["blog"]),
                                 TemplateTaskView("Newsletter for this month", "idea", 0, tags: ["newsletter"]),
                                 TemplateTaskView("Social posts for the launch", "idea", 0, tags: ["social"]),
                                 TemplateTaskView("Customer story interview", "idea", 0)]),
            TemplateView(key: "onboarding", name: "Onboarding checklist", about: "Help a new teammate settle in during the first month", kind: "list",
                         statuses: StatusView.defaults,
                         tasks: [TemplateTaskView("Set up accounts and tools", "to do", 2, checklist: ["Email", "Chat", "Workboard", "Work tools"]),
                                 TemplateTaskView("Meet the team", "to do", 3, checklist: ["One-on-one with your manager", "Team lunch", "Meet your buddy"]),
                                 TemplateTaskView("Read how we work", "to do", 0),
                                 TemplateTaskView("Finish a first small task", "to do", 0),
                                 TemplateTaskView("30-day check-in", "to do", 3)]),
            TemplateView(key: "bug-report", name: "Bug report", about: "One bug with steps to reproduce and a fix checklist", taskName: "New bug", description: bugText,
                         checklist: ["Reproduce", "Find the cause", "Fix", "Test the fix"], tags: ["bug"], priority: 2, kind: ""),
            TemplateView(key: "feature-request", name: "Feature request", about: "Describe a problem and the change that would solve it", taskName: "New feature request",
                         description: "Problem:\n\nProposed change:\n\nWho it helps:\n", checklist: ["Talk to the people it helps", "Agree on the scope", "Build it", "Announce it"], tags: ["feature"], priority: 3, kind: ""),
            TemplateView(key: "project-overview", name: "Project Overview", about: "Summarize goals, scope, and milestones", description: "Goals\n- \n\nScope\n- \n\nMilestones\n- \n\nTeam\n- ", kind: "doc"),
            TemplateView(key: "meeting-notes", name: "Meeting Notes", about: "Capture an agenda, notes, and action items", description: "Agenda\n- \n\nNotes\n- \n\nAction items\n- [ ] ", kind: "doc"),
            TemplateView(key: "wiki", name: "Wiki", about: "Organize information in one place", description: "Overview\n\nHow we work\n\nLinks\n", kind: "doc"),
        ]
    }()
}

extension WorkspaceView {
    /// Built-ins first, then the team's own.
    public func templates(_ data: JSON) -> [TemplateView] {
        TemplateView.builtins + (data.at("extra.templates")?.array ?? []).map(TemplateView.init)
    }
}

extension Edits {
    /// merge_statuses: the statuses `add` has that `to` lacks, open and
    /// custom ones before the first done or closed one, done and closed ones
    /// before the closed ones; the space keeps its own first status.
    static func mergeStatuses(_ to: [StatusView], _ add: [StatusView]) -> [StatusView] {
        var to = to
        for s in add where !to.contains(where: { $0.name == s.name }) {
            var kind = s.kind
            let at: Int
            switch s.kind {
            case .open, .custom:
                kind = .custom
                at = to.firstIndex { $0.kind == .done || $0.kind == .closed } ?? to.count
            case .done, .closed:
                if to.contains(where: { $0.kind == .closed }) { kind = .done }
                at = to.firstIndex { $0.kind == .closed } ?? to.count
            }
            to.insert(StatusView(name: s.name, color: s.color, kind: kind), at: min(max(at, 1), to.count))
        }
        return to
    }

    /// A template's task in `list`: its description, tags and checklist.
    static func addTemplateTask(_ data: inout JSON, list: UInt64, name: String, status: String?, priority: Int, description: String, checklist: [String], tags: [String], me: UInt64, id: UInt64, ts: Int64) {
        addTask(&data, list: list, name: name, assignees: [], status: status, me: me, id: id, ts: ts)
        data.updateItem(at: "tasks", id: id) { t in
            t["priority"] = .int(Int64(max(0, min(4, priority))))
            t["description"] = .string(description)
            t["tags"] = .array(tags.map(JSON.string))
            t["checklist"] = .array(checklist.map { ["text": .string($0), "done": false] })
        }
    }

    /// make_list_from_template: in a new space named like the list, or in
    /// `space` (optionally adding the template's statuses to it). Returns
    /// the new list's id.
    @discardableResult
    public static func useListTemplate(_ data: inout JSON, _ tp: TemplateView, name: String, space: UInt64?, folder: UInt64? = nil, addStatuses: Bool = true, me: UInt64, ts: Int64, newId: () -> UInt64) -> UInt64? {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let listName = clean.isEmpty ? tp.taskName : clean
        let statuses = tp.statuses.isEmpty ? StatusView.defaults : tp.statuses
        let lid: UInt64
        if let space {
            if addStatuses, let (have, _) = statusContext(data, space: space) {
                let merged = mergeStatuses(have, statuses)
                data.updateItem(at: "spaces", id: space) { $0["statuses"] = .array(merged.map(statusJSON)) }
            }
            lid = newId()
            guard createList(&data, space: space, folder: folder, name: listName, id: lid) else { return nil }
        } else {
            let sid = newId()
            lid = newId()
            let count = (data["spaces"]?.array ?? []).count
            createSpace(&data, name: listName, color: statusPalette[1 + (count * 3) % 13], id: sid, listId: lid)
            data.updateItem(at: "spaces", id: sid) { s in
                s["statuses"] = .array(statuses.map(statusJSON))
                s["lists"] = [listJSON(id: lid, name: listName)]
            }
        }
        let have = WorkspaceView(data).statuses(forList: lid).map(\.name)
        for x in tp.tasks {
            addTemplateTask(&data, list: lid, name: x.name, status: have.contains(x.status) ? x.status : nil, priority: x.priority,
                            description: x.description, checklist: x.checklist, tags: x.tags, me: me, id: newId(), ts: ts)
        }
        return lid
    }

    /// make_task_from_template: one task in `list`.
    @discardableResult
    public static func useTaskTemplate(_ data: inout JSON, _ tp: TemplateView, name: String, list: UInt64, me: UInt64, id: UInt64, ts: Int64) -> Bool {
        guard rawLists(data)[list] != nil else { return false }
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        addTemplateTask(&data, list: list, name: clean.isEmpty ? tp.taskName : clean, status: nil, priority: tp.priority,
                        description: tp.description, checklist: tp.checklist, tags: tp.tags, me: me, id: id, ts: ts)
        return true
    }

    /// make_doc_from_template: a doc with the template's text.
    @discardableResult
    public static func useDocTemplate(_ data: inout JSON, _ tp: TemplateView, title: String, space: UInt64?, me: UInt64, id: UInt64, ts: Int64) -> Bool {
        let clean = title.trimmingCharacters(in: .whitespacesAndNewlines)
        addDoc(&data, title: clean.isEmpty ? tp.taskName : clean, space: space, me: me, id: id, ts: ts)
        data.updateItem(at: "docs", id: id) { d in
            d["body"] = .string(tp.description)
            if !tp.tags.isEmpty { d["tags"] = .array(tp.tags.map(JSON.string)) }
            if tp.id == "builtin:meeting-notes" { d["meeting_note"] = true }
        }
        return true
    }

    /// save_list_as_template: the list's statuses and its live top-level tasks.
    @discardableResult
    public static func saveListAsTemplate(_ data: inout JSON, list: UInt64, id: UInt64) -> Bool {
        let v = WorkspaceView(data)
        guard let l = v.list(list) else { return false }
        var all: [JSON] = []
        for s in data["spaces"]?.array ?? [] {
            all += s["lists"]?.array ?? []
            for f in s["folders"]?.array ?? [] { all += f["lists"]?.array ?? [] }
        }
        let desc = all.first { $0["id"]?.uint == list }?["description"]?.string ?? ""
        let tasks = v.tasks(inList: list).map { t in
            TemplateTaskView(t.name, t.status, t.priority, description: t.description, checklist: t.checklist.map(\.text), tags: t.tags).json
        }
        var ts = data.at("extra.templates")?.array ?? []
        ts.append(["id": .id(id), "name": .string(l.name), "task_name": .string(l.name), "description": .string(desc), "checklist": [], "tags": [], "priority": 0,
                   "kind": "list", "statuses": .array(v.statuses(forList: list).map(statusJSON)), "tasks": .array(tasks)])
        data.set("extra.templates", .array(ts))
        return true
    }

    /// save_doc_as_template: its title and text.
    @discardableResult
    public static func saveDocAsTemplate(_ data: inout JSON, doc: UInt64, id: UInt64) -> Bool {
        guard let d = (data["docs"]?.array ?? []).first(where: { $0["id"]?.uint == doc }) else { return false }
        var ts = data.at("extra.templates")?.array ?? []
        ts.append(["id": .id(id), "name": d["title"] ?? "", "task_name": d["title"] ?? "", "description": d["body"] ?? "", "checklist": [], "tags": d["tags"] ?? [], "priority": 0, "kind": "doc"])
        data.set("extra.templates", .array(ts))
        return true
    }

    @discardableResult
    public static func deleteTemplate(_ data: inout JSON, template: UInt64) -> Bool {
        var ts = data.at("extra.templates")?.array ?? []
        let n = ts.count
        ts.removeAll { $0["id"]?.uint == template }
        guard ts.count != n else { return false }
        data.set("extra.templates", .array(ts))
        return true
    }
}
