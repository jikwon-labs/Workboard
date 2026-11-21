// Ids worked out from contents for task activity lines, whiteboard
// strokes and notes, and goal targets and history lines, bit for bit like
// the Rust code. Part of the port in Sync.swift.

import Foundation

extension Sync {
    // MARK: activity ids (wb_sync::activity_id / fill_activity_ids)

    /// The id a task comment or activity line gets when it was written
    /// without one. Worked out from what the entry says, so every device
    /// gives the same entry the same id; bit 62 set.
    public static func activityId(task: UInt64, ts: Int64, author: UInt64, comment: Bool, text: String, nth: Int) -> UInt64 {
        let h = fnv("\(task)|\(ts)|\(author)|\(comment)|\(nth)|\(text)".utf8)
        return (h & ((1 << 62) - 1)) | (1 << 62)
    }

    /// Gives every task activity entry without an id one, and drops repeats
    /// of the same id. True when anything changed.
    @discardableResult
    public static func fillActivityIds(_ data: inout JSON) -> Bool {
        guard var tasks = data["tasks"]?.array else { return false }
        var changed = false
        for ti in tasks.indices {
            let tid = idOf(tasks[ti]) ?? 0
            guard let acts = tasks[ti]["activity"]?.array else { continue }
            if acts.allSatisfy({ (idOf($0) ?? 0) != 0 }) {
                var seen = Set<UInt64?>()
                if acts.allSatisfy({ seen.insert(idOf($0)).inserted }) { continue }
            }
            struct Key: Hashable { let ts: Int64; let author: UInt64; let comment: Bool; let text: String }
            var counts: [Key: Int] = [:]
            var seen = Set<UInt64>()
            var out: [JSON] = []
            for var a in acts {
                guard a.object != nil else { continue }
                let ts = a["ts"]?.int ?? 0
                let author = a["author"]?.uint ?? 0
                let comment = a["comment"]?.bool ?? false
                let text = a["text"]?.string ?? ""
                let key = Key(ts: ts, author: author, comment: comment, text: text)
                let nth = counts[key, default: 0]
                counts[key] = nth + 1
                var id = a["id"]?.uint ?? 0
                if id == 0 {
                    id = activityId(task: tid, ts: ts, author: author, comment: comment, text: text, nth: nth)
                    a["id"] = .id(id)
                    changed = true
                }
                if seen.insert(id).inserted { out.append(a) } else { changed = true }
            }
            tasks[ti]["activity"] = .array(out)
        }
        if changed { data["tasks"] = .array(tasks) }
        return changed
    }

    // MARK: whiteboard ids (wb_sync::board_item_id / fill_board_ids)

    /// The id a whiteboard stroke or note gets when it was drawn without one,
    /// worked out from what it looks like; bit 62 set.
    public static func boardItemId(board: UInt64, kind: String, sig: String, nth: Int) -> UInt64 {
        let h = fnv("wb|\(board)|\(kind)|\(nth)|\(sig)".utf8)
        return (h & ((1 << 62) - 1)) | (1 << 62)
    }

    static func boardSig(_ kind: String, _ item: JSON) -> String {
        func r10(_ v: JSON?) -> Int64 { Int64(((v?.double ?? 0) * 10).rounded()) }
        let color = (item["color"]?.array ?? []).map { String($0.uint ?? 0) }.joined(separator: ",")
        if kind == "strokes" {
            let pts = (item["points"]?.array ?? []).map { p in "\(r10(p[0])),\(r10(p[1]))" }
            return "\(color)|\(pts.joined(separator: ";"))"
        }
        let pos = item["pos"]
        return "\(color)|\(item["text"]?.string ?? "")|\(r10(pos?[0]))|\(r10(pos?[1]))"
    }

    /// Gives every whiteboard stroke and note without an id one, so they
    /// merge one by one. True when anything changed.
    @discardableResult
    public static func fillBoardIds(_ data: inout JSON) -> Bool {
        guard var boards = data.at("extra.whiteboards")?.array else { return false }
        var changed = false
        for bi in boards.indices {
            let bid = idOf(boards[bi]) ?? 0
            for kind in ["strokes", "notes"] {
                guard var items = boards[bi][kind]?.array else { continue }
                var counts: [String: Int] = [:]
                var touched = false
                for i in items.indices {
                    guard items[i].object != nil, (idOf(items[i]) ?? 0) == 0 else { continue }
                    let sig = boardSig(kind, items[i])
                    let n = counts[sig, default: 0]
                    items[i]["id"] = .id(boardItemId(board: bid, kind: kind, sig: sig, nth: n))
                    counts[sig] = n + 1
                    touched = true
                }
                if touched {
                    boards[bi][kind] = .array(items)
                    changed = true
                }
            }
        }
        if changed { data.set("extra.whiteboards", .array(boards)) }
        return changed
    }

    // MARK: goal ids (wb_sync::goal_item_id / goal_sig / fill_goal_ids)

    /// The id a goal's target or history line gets when it has none,
    /// worked out from what it is; bit 62 set.
    public static func goalItemId(goal: UInt64, kind: String, sig: String, nth: Int) -> UInt64 {
        let h = fnv("goal|\(goal)|\(kind)|\(nth)|\(sig)".utf8)
        return (h & ((1 << 62) - 1)) | (1 << 62)
    }

    /// A target by its name and kind ("Signups|Number"); a history line by
    /// when, who, the progress after and what happened.
    static func goalSig(_ kind: String, _ item: JSON) -> String {
        func variant(_ v: JSON?) -> String {
            if let s = v?.string { return s }
            guard let (k, inner) = v?.object?.first else { return "" }
            switch inner {
            case .int(let n): return "\(k):\(n)"
            case .uint(let n): return "\(k):\(n)"
            default: return k
            }
        }
        if kind == "targets" {
            return "\(item["name"]?.string ?? "")|\(variant(item["kind"]))"
        }
        return "\(item["ts"]?.int ?? 0)|\(item["who"]?.int ?? 0)|\(item["pct"]?.int ?? 0)|\(variant(item["what"]))"
    }

    /// Gives every goal target and history line without an id one, passing
    /// over ids already in the list or deleted, so they merge one by one.
    /// True when anything changed.
    @discardableResult
    public static func fillGoalIds(_ data: inout JSON) -> Bool {
        guard var goals = data.at("extra.goals")?.array else { return false }
        let tombs = Set((data.at("extra.sync.tombs")?.object ?? [:]).keys.compactMap { UInt64($0) })
        var changed = false
        for gi in goals.indices {
            let gid = idOf(goals[gi]) ?? 0
            for kind in ["targets", "log"] {
                guard var items = goals[gi][kind]?.array else { continue }
                func missing(_ it: JSON) -> Bool { it.object != nil && (idOf(it) ?? 0) == 0 }
                guard items.contains(where: missing) else { continue }
                var used = Set(items.compactMap { idOf($0) }.filter { $0 != 0 })
                var counts: [String: Int] = [:]
                for i in items.indices where missing(items[i]) {
                    let sig = goalSig(kind, items[i])
                    var n = counts[sig, default: 0]
                    var id: UInt64
                    repeat {
                        id = goalItemId(goal: gid, kind: kind, sig: sig, nth: n)
                        n += 1
                    } while used.contains(id) || tombs.contains(id)
                    counts[sig] = n
                    used.insert(id)
                    items[i]["id"] = .id(id)
                }
                goals[gi][kind] = .array(items)
                changed = true
            }
        }
        if changed { data.set("extra.goals", .array(goals)) }
        return changed
    }
}
