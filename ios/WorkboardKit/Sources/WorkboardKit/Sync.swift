// Change stamps and merging, ported from crates/wb-sync/src/lib.rs (the
// same code the Mac app and workboard-server use) so that a copy pushed from
// the phone merges exactly like a Mac's copy.
//
// Every item with an `id` remembers when its own fields last changed
// (`stamps`, in milliseconds). The newest version of an item wins; the items
// inside it (messages in a channel, lists in a space) are merged one by one,
// and an item that moved (a list to another space) sits only where its newest
// copy puts it. Deleted items leave a tombstone (`tombs`); restoring one
// records a revival. A few single settings (SHARED_VALUES) are shared too,
// newest change wins. Task comments and activity lines get ids worked out
// from their contents when they have none (`activityId`), bit for bit like
// the Rust code, since those ids are shared.
//
// Fingerprints (`hashes`, `value_hashes`) are this device's own and need not
// match the Rust bytes; they only need to be stable here, which the
// sorted-key serializer in JSON.swift gives.

import Foundation

public enum Sync {
    /// Collections that belong to the whole team (SHARED).
    public static let shared: [String] = [
        "spaces", "tasks", "members", "docs", "channels", "extra.goals", "extra.forms", "extra.whiteboards", "extra.meetings", "extra.fields", "extra.automations", "extra.templates", "extra.dashboards", "extra.teams",
    ]
    /// Single settings that belong to the whole team (SHARED_VALUES).
    public static let sharedValues: [String] = ["workspace", "extra.workspace_color", "extra.apps"]
    /// Fields that are each person's own even inside shared items (LOCAL_KEYS).
    public static let localKeys: Set<String> = ["last_read", "saved", "expanded"]
    /// Lists that only ever grow (APPEND_KEYS).
    public static let appendKeys: Set<String> = ["activity"]
}

/// Change bookkeeping kept at `extra.sync` (wb_sync::SyncMeta).
public struct SyncMeta: Equatable, Sendable {
    public var stamps: [UInt64: Int64] = [:]
    public var tombs: [UInt64: Int64] = [:]
    public var revived: [UInt64: Int64] = [:]
    public var hashes: [UInt64: UInt64] = [:]
    /// How `hashes` were made: 0 = before an item's place counted, 1 = since.
    public var v: UInt32 = 0
    /// Shared setting -> when it last changed.
    public var values: [String: Int64] = [:]
    /// Shared setting -> fingerprint when last saved (this device only).
    public var valueHashes: [String: UInt64] = [:]

    public init(stamps: [UInt64: Int64] = [:], tombs: [UInt64: Int64] = [:], revived: [UInt64: Int64] = [:], hashes: [UInt64: UInt64] = [:],
                v: UInt32 = 0, values: [String: Int64] = [:], valueHashes: [String: UInt64] = [:]) {
        self.stamps = stamps
        self.tombs = tombs
        self.revived = revived
        self.hashes = hashes
        self.v = v
        self.values = values
        self.valueHashes = valueHashes
    }

    /// Reads `extra.sync` of a workspace; missing or broken parts read as empty.
    public init(of workspace: JSON) {
        self.init(json: workspace.at("extra.sync") ?? .null)
    }

    public init(json: JSON) {
        func map(_ key: String) -> [UInt64: Int64] {
            var out: [UInt64: Int64] = [:]
            for (k, v) in json[key]?.object ?? [:] {
                if let id = UInt64(k), let t = v.int { out[id] = t }
            }
            return out
        }
        stamps = map("stamps")
        tombs = map("tombs")
        revived = map("revived")
        for (k, v) in json["hashes"]?.object ?? [:] {
            if let id = UInt64(k), let h = v.uint { hashes[id] = h }
        }
        v = UInt32(clamping: json["v"]?.uint ?? 0)
        for (k, x) in json["values"]?.object ?? [:] {
            if let t = x.int { values[k] = t }
        }
        for (k, x) in json["value_hashes"]?.object ?? [:] {
            if let h = x.uint { valueHashes[k] = h }
        }
    }

    public var json: JSON {
        func obj(_ m: [UInt64: Int64]) -> JSON { .object(Dictionary(uniqueKeysWithValues: m.map { (String($0.key), JSON.int($0.value)) })) }
        return .object([
            "stamps": obj(stamps),
            "tombs": obj(tombs),
            "revived": obj(revived),
            "hashes": .object(Dictionary(uniqueKeysWithValues: hashes.map { (String($0.key), JSON.id($0.value)) })),
            "v": .int(Int64(v)),
            "values": .object(values.mapValues { JSON.int($0) }),
            "value_hashes": .object(valueHashes.mapValues { JSON.id($0) }),
        ])
    }
}

extension Sync {
    public static func fnv(_ bytes: some Sequence<UInt8>) -> UInt64 {
        var h: UInt64 = 0xcbf29ce484222325
        for b in bytes {
            h ^= UInt64(b)
            h = h &* 0x100000001b3
        }
        return h
    }

    static func idOf(_ v: JSON) -> UInt64? { v["id"]?.uint }

    /// A non-empty array whose entries are all objects with an id.
    static func isIdArray(_ v: JSON) -> Bool {
        guard let a = v.array, !a.isEmpty else { return false }
        return a.allSatisfy { $0.object != nil && idOf($0) != nil }
    }

    /// Fingerprint of an item's own fields (not the items inside it, not personal fields).
    static func ownHash(_ obj: [String: JSON]) -> UInt64 {
        var own: [String: JSON] = [:]
        for (k, v) in obj {
            if localKeys.contains(k) { continue }
            if let a = v.array, a.isEmpty { continue }
            if isIdArray(v) { continue }
            own[k] = v
        }
        return fnv(JSON.object(own).serialized().utf8)
    }

    /// `parent` is the item this array sits in (nil at the top). With
    /// `placed`, an inner item's fingerprint includes its parent, so moving a
    /// list to another space counts as a change to the list.
    private static func walk(_ v: JSON, parent: UInt64?, placed: Bool, _ out: inout [UInt64: UInt64]) {
        for item in v.array ?? [] {
            guard let id = idOf(item), let obj = item.object else { continue }
            var h = ownHash(obj)
            if placed, let p = parent { h = fnv("\(h)/\(p)".utf8) }
            out[id] = h
            for (_, child) in obj where isIdArray(child) {
                walk(child, parent: id, placed: placed, &out)
            }
        }
    }

    static func fingerprints(_ data: JSON, placed: Bool) -> [UInt64: UInt64] {
        var out: [UInt64: UInt64] = [:]
        for path in shared {
            if let v = data.at(path) { walk(v, parent: nil, placed: placed, &out) }
        }
        return out
    }

    /// Fingerprints of every shared item in a workspace.
    public static func fingerprints(_ data: JSON) -> [UInt64: UInt64] {
        fingerprints(data, placed: true)
    }

    static func valueHashes(_ data: JSON) -> [String: UInt64] {
        var out: [String: UInt64] = [:]
        for p in sharedValues { out[p] = fnv((data.at(p) ?? .null).serialized().utf8) }
        return out
    }

    /// Record what changed since the last save: new stamps for edited items,
    /// tombstones for removed ones, revivals for restored ones (wb_sync::stamp).
    /// `now` is in milliseconds. A new stamp is always later than the item's
    /// last one, so an edit made after seeing a version beats that version
    /// even when this device's clock runs behind.
    public static func stamp(_ data: JSON, _ meta: inout SyncMeta, now: Int64) {
        let cur = fingerprints(data)
        if meta.v < 1 {
            // fingerprints saved before an item's place counted: carry over the
            // unchanged ones so updating the app doesn't mark everything as edited
            for (id, old) in fingerprints(data, placed: false) where meta.hashes[id] == old {
                if let h = cur[id] { meta.hashes[id] = h }
            }
            meta.v = 1
        }
        for (id, h) in cur {
            let next = meta.stamps[id].map { max(now, $0 + 1) } ?? now
            if let old = meta.hashes[id] {
                if old != h { meta.stamps[id] = next }
            } else {
                meta.stamps[id] = next
                if let t = meta.tombs[id], meta.revived[id].map({ $0 < t }) ?? true {
                    meta.revived[id] = max(now, t + 1)
                }
            }
        }
        for id in meta.hashes.keys where cur[id] == nil {
            // a delete always wins over the item's last known change or
            // restore, even when this device's clock is behind
            var t = now
            if let s = meta.stamps[id] { t = max(t, s + 1) }
            if let r = meta.revived[id] { t = max(t, r + 1) }
            meta.tombs[id] = t
            meta.stamps[id] = nil
        }
        meta.hashes = cur
        // shared settings: a changed one gets a new stamp; one seen for the
        // first time keeps none, so it wins only once someone changes it
        for (path, h) in valueHashes(data) {
            if let old = meta.valueHashes[path], old != h {
                meta.values[path] = meta.values[path].map { max(now, $0 + 1) } ?? now
            }
            meta.valueHashes[path] = h
        }
    }

    // MARK: merge (wb_sync::merge / merge_arr)

    private typealias Cand = (src: Int, value: JSON)

    /// wb_sync DEFAULT_WHEN_MISSING: fields the Mac leaves out at their
    /// default that can go back to it; any other missing field is kept from
    /// the other copies so an app that doesn't know it can't erase it.
    static let defaultWhenMissing: Set<String> = [
        "archived", "assignee", "reactions", "resolved", "timer_by", "timers", "milestone", "watchers", "waiting_on", "linked", "tags", "private", "required", "options", "cards", "mentions",
    ]

    private final class Ctx {
        let stamps: [[UInt64: Int64]]
        let tombs: [UInt64: Int64]
        let revived: [UInt64: Int64]
        var out: [UInt64: Int64] = [:]
        /// id -> every copy of the item, in every source and every place.
        var cands: [UInt64: [Cand]] = [:]
        /// id -> the item it sits in, in the newest copy (nil at the top).
        var home: [UInt64: UInt64?] = [:]
        /// source -> whether that copy leaves fields at their default out of
        /// the file (the Mac app from SyncMeta.v 2 on). When such a copy wins
        /// and lacks a field, the field is at its default there (a stopped
        /// timer, an unpinned message) and is not filled back in from another
        /// copy (wb_sync Ctx::omits_defaults).
        var omitsDefaults: [Bool] = []

        init(stamps: [[UInt64: Int64]], tombs: [UInt64: Int64], revived: [UInt64: Int64], all: [UInt64: [(src: Int, parent: UInt64?, value: JSON)]]) {
            self.stamps = stamps
            self.tombs = tombs
            self.revived = revived
            for (id, list) in all {
                // the newest copy decides where the item is; on a tie this device's
                var best = list[0]
                var bestStamp = stamps[best.src][id] ?? 0
                for c in list.dropFirst() {
                    let s = stamps[c.src][id] ?? 0
                    if s > bestStamp {
                        best = c
                        bestStamp = s
                    }
                }
                home[id] = .some(best.parent)
                cands[id] = list.map { (src: $0.src, value: $0.value) }
            }
        }

        func alive(_ id: UInt64) -> Bool {
            guard let t = tombs[id] else { return true }
            return revived[id].map { $0 > t } ?? false
        }
    }

    /// Every item with an id in one source, with the item it sits in.
    private static func collect(_ src: Int, _ arr: [JSON], parent: UInt64?, _ out: inout [UInt64: [(src: Int, parent: UInt64?, value: JSON)]], _ order: inout [UInt64]) {
        for item in arr {
            guard let id = idOf(item), let obj = item.object else { continue }
            if out[id] == nil { order.append(id) }
            out[id, default: []].append((src: src, parent: parent, value: item))
            for (_, child) in obj where isIdArray(child) {
                collect(src, child.array ?? [], parent: id, &out, &order)
            }
        }
    }

    private static func mergeArr(_ cx: Ctx, _ sources: [(Int, [JSON])], parent: UInt64?) -> [JSON] {
        var order: [UInt64] = []
        var seen = Set<UInt64>()
        for (_, arr) in sources {
            for v in arr {
                guard let id = idOf(v) else { continue }
                if seen.insert(id).inserted { order.append(id) }
            }
        }
        var out: [JSON] = []
        for id in order {
            if !cx.alive(id) { continue }
            if parent != nil, let h = cx.home[id], h != parent { continue }
            guard let list = cx.cands[id], !list.isEmpty else { continue }
            // newest own fields win; on a tie this device's version stays
            var best = list[0]
            var bestStamp = cx.stamps[best.src][id] ?? 0
            for c in list.dropFirst() {
                let s = cx.stamps[c.src][id] ?? 0
                if s > bestStamp {
                    best = c
                    bestStamp = s
                }
            }
            cx.out[id] = bestStamp
            guard let wobj = best.value.object else { continue }
            let local = list.first(where: { $0.src == 0 })?.value.object
            var keys = Set(wobj.keys)
            for c in list {
                for k in c.value.object?.keys ?? [String: JSON]().keys { keys.insert(k) }
            }
            var obj: [String: JSON] = [:]
            for k in keys {
                let vals: [(Int, JSON)] = list.compactMap { c in c.value[k].map { (c.src, $0) } }
                let value: JSON
                if vals.contains(where: { isIdArray($0.1) }) {
                    let subs: [(Int, [JSON])] = vals.compactMap { s, v in v.array.map { (s, $0) } }
                    value = .array(mergeArr(cx, subs, parent: id))
                } else if appendKeys.contains(k) {
                    var all: [JSON] = []
                    for (_, v) in vals {
                        for x in v.array ?? [] where !all.contains(x) { all.append(x) }
                    }
                    value = .array(stableSorted(all) { $0["ts"]?.int ?? 0 })
                } else if localKeys.contains(k), let l = local, let lv = l[k] {
                    value = lv
                } else if let v = wobj[k] {
                    value = v
                } else if defaultWhenMissing.contains(k) && cx.omitsDefaults.indices.contains(best.src) && cx.omitsDefaults[best.src] {
                    // the newest copy has it at its default (see omitsDefaults)
                    continue
                } else {
                    value = vals[0].1
                }
                obj[k] = value
            }
            out.append(.object(obj))
        }
        // messages and replies read in time order
        if !out.isEmpty, out.allSatisfy({ if case .int = $0["ts"] { return true } else { return false } }) {
            out = stableSorted(out) { $0["ts"]?.int ?? 0 }
        }
        return out
    }

    private static func stableSorted(_ a: [JSON], by key: (JSON) -> Int64) -> [JSON] {
        a.enumerated().sorted { l, r in
            let kl = key(l.element), kr = key(r.element)
            return kl != kr ? kl < kr : l.offset < r.offset
        }.map(\.element)
    }

    /// Merge other copies into this one. Returns true when anything other
    /// than the bookkeeping changed.
    @discardableResult
    public static func merge(_ local: inout JSON, _ others: [JSON]) -> Bool {
        // copies from before activity had ids: give their entries the same ids
        // here as everywhere else, or their comments would not be matched up
        fillActivityIds(&local)
        let others = others.map { o -> JSON in
            var o = o
            fillActivityIds(&o)
            return o
        }
        let before = local
        var lmeta = SyncMeta(of: local)
        let metas = others.map { SyncMeta(of: $0) }
        var tombs = lmeta.tombs
        var revived = lmeta.revived
        for m in metas {
            for (k, v) in m.tombs { tombs[k] = max(tombs[k] ?? v, v) }
            for (k, v) in m.revived { revived[k] = max(revived[k] ?? v, v) }
        }
        var all: [UInt64: [(src: Int, parent: UInt64?, value: JSON)]] = [:]
        var order: [UInt64] = []
        var perPath: [(String, [(Int, [JSON])])] = []
        for path in shared {
            var sources: [(Int, [JSON])] = [(0, before.at(path)?.array ?? [])]
            for (i, o) in others.enumerated() {
                if let a = o.at(path)?.array { sources.append((i + 1, a)) }
            }
            for (src, arr) in sources { collect(src, arr, parent: nil, &all, &order) }
            perPath.append((path, sources))
        }
        let cx = Ctx(stamps: [lmeta.stamps] + metas.map(\.stamps), tombs: tombs, revived: revived, all: all)
        cx.omitsDefaults = [lmeta.v >= 2] + metas.map { $0.v >= 2 }
        var merged: [(String, JSON)] = []
        for (path, sources) in perPath {
            merged.append((path, .array(mergeArr(cx, sources, parent: nil))))
        }
        for (path, v) in merged { local.set(path, v) }
        // shared settings: the newest change wins; on a tie this device's stays
        for path in sharedValues {
            var best: JSON?
            var bestStamp = lmeta.values[path] ?? 0
            for (o, m) in zip(others, metas) {
                let s = m.values[path] ?? 0
                if s > bestStamp, let v = o.at(path) {
                    best = v
                    bestStamp = s
                }
            }
            if let v = best { local.set(path, v) }
            if bestStamp > 0 { lmeta.values[path] = bestStamp }
        }
        lmeta.tombs = tombs
        lmeta.revived = revived
        lmeta.stamps = cx.out
        lmeta.hashes = fingerprints(local)
        lmeta.valueHashes = valueHashes(local)
        lmeta.v = 1
        local.set("extra.sync", lmeta.json)
        return stripSync(before) != stripSync(local)
    }

    static func stripSync(_ v: JSON) -> JSON {
        var v = v
        if var e = v["extra"], e.object != nil {
            e["sync"] = nil
            v["extra"] = e
        }
        return v
    }

    /// The copy sent to the server: this device's fingerprints left out, as
    /// wb_sync::shareable does.
    public static func outgoing(_ data: JSON) -> JSON {
        var v = data
        if v.at("extra.sync")?.object != nil {
            v.set("extra.sync.hashes", .object([:]))
            v.set("extra.sync.value_hashes", .object([:]))
        }
        return v
    }

    /// Takes items the server hides from this person (`hidden` in welcome and
    /// update: others' DMs and private channels, others' personal spaces and
    /// private docs) out of this copy WITHOUT tombstones, so nothing is
    /// deleted for the others, and forgets their stamps and fingerprints
    /// (wb_sync::hide). Returns the ids of everything that left, inner items too.
    @discardableResult
    public static func hide(_ data: inout JSON, ids: [UInt64]) -> Set<UInt64> {
        guard !ids.isEmpty else { return [] }
        let wanted = Set(ids)
        var gone: [JSON] = []
        for path in shared {
            guard let a = data.at(path)?.array else { continue }
            let out = a.filter { idOf($0).map(wanted.contains) ?? false }
            guard !out.isEmpty else { continue }
            gone += out
            data.set(path, .array(a.filter { !(idOf($0).map(wanted.contains) ?? false) }))
        }
        guard !gone.isEmpty else { return [] }
        var inner: [UInt64: UInt64] = [:]
        walk(.array(gone), parent: nil, placed: false, &inner)
        if data.at("extra.sync")?.object != nil {
            for key in ["stamps", "hashes", "moved", "parents"] {
                guard var m = data.at("extra.sync." + key)?.object else { continue }
                for id in inner.keys { m[String(id)] = nil }
                data.set("extra.sync." + key, .object(m))
            }
        }
        return Set(inner.keys)
    }

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
}
