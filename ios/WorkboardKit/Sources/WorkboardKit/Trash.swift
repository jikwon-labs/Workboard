// Trash on the iPhone. Deleting a task or a doc still takes it out of the
// shared workspace (the sync tombstones it, so it is gone for everyone), but
// a copy goes to this iPhone's own Trash first, from where Restore puts it
// back (the sync then records a revival, so it comes back for everyone too).
//
// Like the Mac app's extra.trash, Trash is personal and never shared: it is
// kept in its own file next to the workspace copy (trash.json), not inside
// the copy that is pushed to the server. Items have the Mac's TrashItem
// shape (src/model.rs): {id, kind, title, deleted, payload}, where payload is
// the JSON text of the removed items ("Task": the task and its subtasks,
// "Doc": the page and its sub pages).

import Foundation

public struct TrashItem: Equatable, Identifiable, Sendable {
    public var id: UInt64
    /// "Task" or "Doc" (the Mac app has more kinds; the iPhone makes these).
    public var kind: String
    public var title: String
    /// When it was deleted (seconds).
    public var deleted: Int64
    /// The removed items as JSON text.
    public var payload: String

    public init(id: UInt64, kind: String, title: String, deleted: Int64, payload: String) {
        self.id = id
        self.kind = kind
        self.title = title
        self.deleted = deleted
        self.payload = payload
    }

    public var json: JSON {
        ["id": .id(id), "kind": .string(kind), "title": .string(title), "deleted": .int(deleted), "payload": .string(payload)]
    }

    public init?(_ j: JSON) {
        guard let id = j["id"]?.uint, let kind = j["kind"]?.string else { return nil }
        self.init(id: id, kind: kind, title: j["title"]?.string ?? "", deleted: j["deleted"]?.int ?? 0, payload: j["payload"]?.string ?? "")
    }

    /// The removed items (tasks or doc pages).
    public var items: [JSON] {
        guard let j = try? JSON(parsing: Data(payload.utf8)) else { return [] }
        if let a = j.array { return a }
        // an older Mac item holding a single page
        return j.object != nil ? [j] : []
    }
}

/// What a restore did.
public enum RestoreResult: Equatable, Sendable {
    case restored
    /// The task's list is gone, so it went to this list instead.
    case movedTo(String)
    /// There is nowhere to put it (no list at all); it stays in Trash.
    case nowhere(String)
}

extension Edits {
    /// The ids of an item and everything under it (`parent` links).
    static func subtree(_ items: [JSON], of root: UInt64) -> [UInt64] {
        var all: [UInt64] = [root]
        var i = 0
        while i < all.count {
            let k = all[i]
            all += items.filter { $0["parent"]?.uint == k }.compactMap { $0["id"]?.uint }
            i += 1
        }
        return all
    }

    /// Action::DeleteTask: the task and its subtasks leave the workspace and
    /// come back as a Trash item. Nil when there is no such task.
    public static func trashTask(_ data: inout JSON, task: UInt64, id: UInt64, ts: Int64) -> TrashItem? {
        let tasks = data["tasks"]?.array ?? []
        guard let first = tasks.first(where: { $0["id"]?.uint == task }) else { return nil }
        let kill = Set(subtree(tasks, of: task))
        let removed = tasks.filter { kill.contains($0["id"]?.uint ?? 0) }
        guard deleteTask(&data, task: task) else { return nil }
        return TrashItem(id: id, kind: "Task", title: first["name"]?.string ?? "", deleted: ts, payload: JSON.array(removed).serialized())
    }

    /// trash_doc in src/app/docs.rs: a page and its sub pages as one item,
    /// so restoring brings the whole tree back.
    public static func trashDoc(_ data: inout JSON, doc: UInt64, id: UInt64, ts: Int64) -> TrashItem? {
        let docs = data["docs"]?.array ?? []
        guard let root = docs.first(where: { $0["id"]?.uint == doc }) else { return nil }
        let ids = subtree(docs, of: doc)
        let pages = ids.compactMap { i in docs.first { $0["id"]?.uint == i } }
        guard deleteDoc(&data, doc: doc) else { return nil }
        return TrashItem(id: id, kind: "Doc", title: root["title"]?.string ?? "", deleted: ts, payload: JSON.array(pages).serialized())
    }

    /// restore_trash in src/app/menus.rs for tasks and docs. A task whose
    /// list is gone goes to the first list (with a status that list has); a
    /// page whose parent page is gone comes back at the top level. Items
    /// still in the workspace are left as they are.
    public static func restore(_ data: inout JSON, _ item: TrashItem, me: UInt64, ts: Int64) -> RestoreResult {
        switch item.kind {
        case "Task":
            var tasks = item.items
            let lists = rawLists(data)
            var result = RestoreResult.restored
            let gone = Set(tasks.compactMap { $0["list_id"]?.uint }.filter { lists[$0] == nil })
            if !gone.isEmpty {
                guard let to = firstList(data, me: me), let target = lists[to] else {
                    return .nowhere("There is no list to restore it into. Make a list first.")
                }
                for i in tasks.indices where gone.contains(tasks[i]["list_id"]?.uint ?? 0) {
                    tasks[i]["list_id"] = .id(to)
                    // fit_statuses: keep a status the list has, else the first
                    let old = tasks[i]["status"]?.string ?? ""
                    if !target.statuses.contains(where: { $0.name == old }) {
                        let first = target.statuses[0].name
                        tasks[i]["status"] = .string(first)
                        log(&tasks[i], me: me, text: "changed status from \(old.uppercased()) to \(first.uppercased())", ts: ts)
                    }
                }
                result = .movedTo(target.name)
            }
            var all = data["tasks"]?.array ?? []
            let here = Set(all.compactMap { $0["id"]?.uint })
            all += tasks.filter { !here.contains($0["id"]?.uint ?? 0) }
            data["tasks"] = .array(all)
            return result
        case "Doc":
            // restore_doc_pages in src/app/docs.rs
            let pages = item.items
            let ids = Set(pages.compactMap { $0["id"]?.uint })
            var docs = data["docs"]?.array ?? []
            for var d in pages {
                if let p = d["parent"]?.uint, !ids.contains(p), !docs.contains(where: { $0["id"]?.uint == p }) {
                    d["parent"] = .null
                }
                if !docs.contains(where: { $0["id"]?.uint == d["id"]?.uint }) { docs.append(d) }
            }
            data["docs"] = .array(docs)
            return .restored
        default:
            return .nowhere("This kind of item can only be restored in Workboard on the Mac.")
        }
    }

    /// first_list in src/model.rs: the first open list I can see.
    static func firstList(_ data: JSON, me: UInt64) -> UInt64? {
        for s in data["spaces"]?.array ?? [] {
            if s["archived"]?.bool == true { continue }
            if let o = s["private_owner"]?.uint, o != me { continue }
            for f in s["folders"]?.array ?? [] {
                if let l = (f["lists"]?.array ?? []).first(where: { $0["archived"]?.bool != true }) { return l["id"]?.uint }
            }
            if let l = (s["lists"]?.array ?? []).first(where: { $0["archived"]?.bool != true }) { return l["id"]?.uint }
        }
        return nil
    }
}

extension Storage {
    var trashURL: URL { folder.appendingPathComponent("trash.json") }

    public func loadTrash() -> [TrashItem] {
        guard let d = try? Data(contentsOf: trashURL), let j = try? JSON(parsing: d) else { return [] }
        return (j.array ?? []).compactMap(TrashItem.init)
    }

    public func save(trash: [TrashItem]) {
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? JSON.array(trash.map(\.json)).data().write(to: trashURL, options: .atomic)
    }
}

extension SyncClient {
    /// Delete a task (with its subtasks) for everyone, keeping a copy in
    /// this iPhone's Trash. False when editing isn't allowed.
    @discardableResult
    public func trashTask(_ task: UInt64) -> Bool {
        let id = newId()
        var item: TrashItem?
        let ok = edit { d, _ in item = Edits.trashTask(&d, task: task, id: id, ts: nowTs()) }
        guard ok, let item else { return false }
        addToTrash(item)
        return true
    }

    /// Delete a doc (with its sub pages) for everyone, keeping a copy here.
    @discardableResult
    public func trashDoc(_ doc: UInt64) -> Bool {
        let id = newId()
        var item: TrashItem?
        let ok = edit { d, _ in item = Edits.trashDoc(&d, doc: doc, id: id, ts: nowTs()) }
        guard ok, let item else { return false }
        addToTrash(item)
        return true
    }

    /// Put a Trash item back into the workspace (for everyone).
    @discardableResult
    public func restoreFromTrash(_ id: UInt64) -> RestoreResult? {
        guard let item = trash.first(where: { $0.id == id }) else { return nil }
        var result: RestoreResult = .restored
        let ok = edit { d, me in result = Edits.restore(&d, item, me: me, ts: nowTs()) }
        guard ok else { return nil }
        if case .nowhere = result { return result }
        setTrash(trash.filter { $0.id != id })
        reuploadFiles(of: item)
        return result
    }

    /// Items came back from Trash: their pictures, videos and stills (the
    /// ones this iPhone has) go back where the team can get them, checking
    /// with the server rather than trusting it still has them (the Mac's
    /// reupload_files). Kept in the settings until the server could be
    /// asked, so a restore made offline sends them after the next connect.
    func reuploadFiles(of item: TrashItem) {
        let shas = FileAPI.files(inPayload: item.payload).filter { !settings.reuploads.contains($0) }
        guard !shas.isEmpty else { return }
        settings.reuploads += shas
        storage.save(settings)
        flushReuploads()
    }

    /// Try the files waiting to go back up (see reuploadFiles).
    func flushReuploads() {
        let shas = settings.reuploads.filter { !reuploading.contains($0) }
        guard !shas.isEmpty else { return }
        reuploading.formUnion(shas)
        let (server, auth, folder) = (settings.server, fileAuth, fileCache)
        lastReupload = Task { [weak self] in
            var sent = 0
            for sha in shas {
                // the server's answer settles it, whatever it was; no answer: next connect
                let done: Bool
                do {
                    if try await FileAPI.reupload(server: server, auth: auth, sha: sha, folder: folder) { sent += 1 }
                    done = true
                } catch {
                    done = false
                }
                guard let self else { continue }
                self.reuploading.remove(sha)
                if done, self.settings.server == server {
                    self.settings.reuploads.removeAll { $0 == sha }
                    self.storage.save(self.settings)
                }
            }
            return sent
        }
    }

    /// Delete a Trash item for good (only this iPhone's copy).
    public func deleteFromTrash(_ id: UInt64) {
        setTrash(trash.filter { $0.id != id })
    }

    public func emptyTrash() { setTrash([]) }

    func addToTrash(_ item: TrashItem) {
        // Trash belongs to whoever deleted the items
        if trash.isEmpty || settings.trashOwner != settings.me { trash = [] }
        settings.trashOwner = settings.me
        storage.save(settings)
        setTrash(trash + [item])
    }

    func setTrash(_ items: [TrashItem]) {
        trash = items
        storage.save(trash: items)
        if items.isEmpty, settings.trashOwner != nil {
            settings.trashOwner = nil
            storage.save(settings)
        }
    }

    /// Someone else is signed in now (or picked by hand): what the one
    /// before deleted isn't theirs to see or restore.
    func trashFollowsMe() {
        guard !trash.isEmpty, let me = settings.me, settings.trashOwner != me else { return }
        setTrash([])
    }
}
