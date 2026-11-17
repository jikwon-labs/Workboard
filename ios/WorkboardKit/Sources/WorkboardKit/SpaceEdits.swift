// Spaces, folders and lists, and a space's statuses, changed as the Mac app
// changes them (create_space / create_list / set_*_archived in
// src/app/parity.rs, the status editor in src/app.rs). New items carry every
// field src/model.rs requires.

import Foundation

extension Edits {
    /// STATUS_PALETTE in src/model.rs.
    public static let statusPalette: [[Int]] = [
        [0x87, 0x90, 0x9e], [0x5f, 0x55, 0xee], [0x44, 0x66, 0xff], [0x10, 0x90, 0xe0], [0x0f, 0x9d, 0x9f],
        [0x3d, 0xb8, 0x8b], [0x00, 0x88, 0x44], [0xf8, 0xae, 0x00], [0xe1, 0x6b, 0x16], [0xd3, 0x3d, 0x44],
        [0xee, 0x5e, 0x99], [0xb6, 0x60, 0xe0], [0xaa, 0x8d, 0x80], [0x65, 0x6f, 0x7d],
    ]

    static func statusJSON(_ s: StatusView) -> JSON {
        ["name": .string(s.name), "color": .array(s.color.map { .int(Int64($0)) }), "kind": .string(s.kind.rawValue)]
    }

    static func listJSON(id: UInt64, name: String) -> JSON {
        ["id": .id(id), "name": .string(name), "color": nil, "start": nil, "end": nil, "priority": 0, "owner": nil, "description": "", "archived": false]
    }

    // MARK: create

    /// A new space with the default statuses and one list called "List".
    @discardableResult
    public static func createSpace(_ data: inout JSON, name: String, color: [Int], id: UInt64, listId: UInt64) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        data.append(at: "spaces", [
            "id": .id(id), "name": .string(clean.isEmpty ? "Space" : clean),
            "color": .array(color.prefix(3).map { .int(Int64($0)) }),
            "statuses": .array(StatusView.defaults.map(statusJSON)),
            "folders": [], "lists": [listJSON(id: listId, name: "List")],
            "expanded": true, "private_owner": nil, "archived": false, "hidden": false, "icon": "",
        ])
        return true
    }

    /// A list in a space, or in one of its folders.
    @discardableResult
    public static func createList(_ data: inout JSON, space: UInt64, folder: UInt64?, name: String, id: UInt64) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let item = listJSON(id: id, name: clean.isEmpty ? "List" : clean)
        var done = false
        data.updateItem(at: "spaces", id: space) { s in
            s["expanded"] = true
            if let folder {
                done = s.updateItem(at: "folders", id: folder) { f in
                    f["expanded"] = true
                    f.append(at: "lists", item)
                }
            } else {
                s.append(at: "lists", item)
                done = true
            }
        }
        return done
    }

    @discardableResult
    public static func createFolder(_ data: inout JSON, space: UInt64, name: String, id: UInt64) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        return data.updateItem(at: "spaces", id: space) { s in
            s["expanded"] = true
            s.append(at: "folders", ["id": .id(id), "name": .string(clean.isEmpty ? "Folder" : clean), "lists": [], "expanded": true])
        }
    }

    // MARK: rename, colour, archive

    @discardableResult
    public static func renameSpace(_ data: inout JSON, space: UInt64, to name: String) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return false }
        return data.updateItem(at: "spaces", id: space) { $0["name"] = .string(clean) }
    }

    @discardableResult
    public static func setSpaceColor(_ data: inout JSON, space: UInt64, to color: [Int]) -> Bool {
        data.updateItem(at: "spaces", id: space) { $0["color"] = .array(color.prefix(3).map { .int(Int64($0)) }) }
    }

    @discardableResult
    public static func setSpaceArchived(_ data: inout JSON, space: UInt64, _ archived: Bool) -> Bool {
        data.updateItem(at: "spaces", id: space) { $0["archived"] = .bool(archived) }
    }

    @discardableResult
    public static func renameFolder(_ data: inout JSON, folder: UInt64, to name: String) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return false }
        return changeFolder(&data, folder) { $0["name"] = .string(clean) }
    }

    /// Archived folders leave the sidebar with their lists until restored.
    @discardableResult
    public static func setFolderArchived(_ data: inout JSON, folder: UInt64, _ archived: Bool) -> Bool {
        changeFolder(&data, folder) { $0["archived"] = archived ? .bool(true) : nil }
    }

    @discardableResult
    public static func renameList(_ data: inout JSON, list: UInt64, to name: String) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return false }
        return changeList(&data, list) { $0["name"] = .string(clean) }
    }

    @discardableResult
    public static func setListArchived(_ data: inout JSON, list: UInt64, _ archived: Bool) -> Bool {
        changeList(&data, list) { $0["archived"] = .bool(archived) }
    }

    private static func changeFolder(_ data: inout JSON, _ folder: UInt64, _ change: (inout JSON) -> Void) -> Bool {
        var spaces = data["spaces"]?.array ?? []
        for i in spaces.indices where spaces[i].updateItem(at: "folders", id: folder, change) {
            data["spaces"] = .array(spaces)
            return true
        }
        return false
    }

    private static func changeList(_ data: inout JSON, _ list: UInt64, _ change: (inout JSON) -> Void) -> Bool {
        var spaces = data["spaces"]?.array ?? []
        for i in spaces.indices {
            if spaces[i].updateItem(at: "lists", id: list, change) {
                data["spaces"] = .array(spaces)
                return true
            }
            var folders = spaces[i]["folders"]?.array ?? []
            for k in folders.indices where folders[k].updateItem(at: "lists", id: list, change) {
                spaces[i]["folders"] = .array(folders)
                data["spaces"] = .array(spaces)
                return true
            }
        }
        return false
    }

    // MARK: statuses (the status editor in src/app.rs)

    /// The space's statuses as they are read (the defaults when it has none
    /// written), and the ids of its lists.
    static func statusContext(_ data: JSON, space: UInt64) -> (statuses: [StatusView], lists: [UInt64])? {
        guard let s = (data["spaces"]?.array ?? []).first(where: { $0["id"]?.uint == space }) else { return nil }
        let st = (s["statuses"]?.array ?? []).map(StatusView.init)
        let lists = ((s["lists"]?.array ?? []) + (s["folders"]?.array ?? []).flatMap { $0["lists"]?.array ?? [] }).compactMap { $0["id"]?.uint }
        return (st.isEmpty ? StatusView.defaults : st, lists)
    }

    private static func writeStatuses(_ data: inout JSON, space: UInt64, _ sts: [StatusView]) {
        data.updateItem(at: "spaces", id: space) { $0["statuses"] = .array(sts.map(statusJSON)) }
    }

    /// retarget_automations_all: automations on `old` in this space or its
    /// lists always follow; workspace-wide ones only when no other space has
    /// a status called `old`.
    static func retargetAutomations(_ data: inout JSON, space: UInt64, lists: [UInt64], old: String, new: String) {
        let elsewhere = (data["spaces"]?.array ?? []).contains { s in
            s["id"]?.uint != space && (s["statuses"]?.array ?? []).contains { $0["name"]?.string == old }
        }
        guard var rules = data.at("extra.automations")?.array else { return }
        var changed = false
        for i in rules.indices where rules[i]["trigger_status"]?.string == old {
            let here: Bool
            if let l = rules[i]["list"]?.uint { here = l == space || lists.contains(l) } else { here = !elsewhere }
            if here {
                rules[i]["trigger_status"] = .string(new)
                changed = true
            }
        }
        if changed { data.set("extra.automations", .array(rules)) }
    }

    /// Renames status `index`; its tasks and automations follow. An empty
    /// name or one another status has is turned down.
    @discardableResult
    public static func renameStatus(_ data: inout JSON, space: UInt64, index: Int, to name: String, ts: Int64) -> Bool {
        let clean = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty, let (sts, lists) = statusContext(data, space: space), sts.indices.contains(index) else { return false }
        let old = sts[index].name
        guard old != clean, !sts.contains(where: { $0.name == clean }) else { return false }
        var new = sts
        new[index] = StatusView(name: clean, color: sts[index].color, kind: sts[index].kind)
        writeStatuses(&data, space: space, new)
        var tasks = data["tasks"]?.array ?? []
        for i in tasks.indices where tasks[i]["status"]?.string == old && lists.contains(tasks[i]["list_id"]?.uint ?? 0) {
            tasks[i]["status"] = .string(clean)
            tasks[i]["updated"] = .int(ts)
        }
        data["tasks"] = .array(tasks)
        retargetAutomations(&data, space: space, lists: lists, old: old, new: clean)
        return true
    }

    /// "+ Add status": a custom status before the done and closed ones.
    /// Returns its index.
    @discardableResult
    public static func addStatus(_ data: inout JSON, space: UInt64) -> Int? {
        guard let (sts, _) = statusContext(data, space: space) else { return nil }
        var name = "new status"
        var n = 2
        while sts.contains(where: { $0.name == name }) { name = "new status \(n)"; n += 1 }
        let color = statusPalette[(sts.count * 2 + 1) % statusPalette.count]
        let pos = sts.firstIndex { $0.isClosed } ?? sts.count
        var new = sts
        new.insert(StatusView(name: name, color: color, kind: .custom), at: pos)
        writeStatuses(&data, space: space, new)
        return pos
    }

    @discardableResult
    public static func setStatusColor(_ data: inout JSON, space: UInt64, index: Int, to color: [Int]) -> Bool {
        guard let (sts, _) = statusContext(data, space: space), sts.indices.contains(index) else { return false }
        var new = sts
        new[index] = StatusView(name: sts[index].name, color: color, kind: sts[index].kind)
        writeStatuses(&data, space: space, new)
        return true
    }

    @discardableResult
    public static func setStatusKind(_ data: inout JSON, space: UInt64, index: Int, to kind: StatusKind) -> Bool {
        guard let (sts, _) = statusContext(data, space: space), sts.indices.contains(index), sts[index].kind != kind else { return false }
        var new = sts
        new[index] = StatusView(name: sts[index].name, color: sts[index].color, kind: kind)
        writeStatuses(&data, space: space, new)
        return true
    }

    /// move_off_status: the status goes (never the last one); its tasks
    /// move to a status of the same kind, else the first, with a history
    /// line, and its automations follow them.
    @discardableResult
    public static func deleteStatus(_ data: inout JSON, space: UInt64, index: Int, me: UInt64, ts: Int64) -> Bool {
        guard let (sts, lists) = statusContext(data, space: space), sts.count > 1, sts.indices.contains(index) else { return false }
        let gone = sts[index]
        var rest = sts
        rest.remove(at: index)
        let to = rest.first { $0.kind == gone.kind } ?? rest[0]
        writeStatuses(&data, space: space, rest)
        var tasks = data["tasks"]?.array ?? []
        for i in tasks.indices where tasks[i]["status"]?.string == gone.name && lists.contains(tasks[i]["list_id"]?.uint ?? 0) {
            tasks[i]["status"] = .string(to.name)
            log(&tasks[i], me: me, text: "changed status from \(gone.name.uppercased()) to \(to.name.uppercased())", ts: ts)
            tasks[i]["updated"] = .int(ts)
        }
        data["tasks"] = .array(tasks)
        retargetAutomations(&data, space: space, lists: lists, old: gone.name, new: to.name)
        return true
    }
}
