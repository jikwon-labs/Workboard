// Change stamps and merging, ported from crates/wb-sync/src/lib.rs (the
// same code the Mac app and workboard-server use) so that a copy pushed from
// the phone merges exactly like a Mac's copy.
//
// Every item with an `id` remembers when it last changed (`stamps`, in
// milliseconds) and when each of its own fields did (`fields`): for each
// field the newest change wins, so two people changing different fields of
// one task both keep theirs. Reactions merge per (emoji, person), running
// timers per person, whiteboard strokes and notes have ids (`fillBoardIds`).
// The items inside an item (messages in a channel, lists in a space) are merged one by one,
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
        "spaces", "tasks", "members", "docs", "channels", "extra.goals", "extra.forms", "extra.whiteboards", "extra.meetings", "extra.fields", "extra.automations", "extra.templates", "extra.dashboards", "extra.teams", "extra.bookmarks",
    ]
    /// The SyncMeta.v this version writes (META_V): 1 = an item's place
    /// counts in its fingerprint, 2 = `moved` is kept and fields at their
    /// default may be left out, 3 = field stamps are kept.
    public static let metaV: UInt32 = 3
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
    /// How `hashes` were made and what the copy keeps (Sync.metaV).
    public var v: UInt32 = 0
    /// Shared setting -> when it last changed.
    public var values: [String: Int64] = [:]
    /// Shared setting -> fingerprint when last saved (this device only).
    public var valueHashes: [String: UInt64] = [:]
    /// id -> part -> when that field last changed (wb_sync SyncMeta::fields;
    /// "" is the item's base).
    public var fields: [UInt64: [String: Int64]] = [:]
    /// id -> part -> fingerprint when last saved (this device only).
    public var fieldHashes: [UInt64: [String: UInt32]] = [:]
    /// id -> when it last moved into another item (a list into another
    /// space). Where an item sits follows this, not `stamps` (from v 2).
    public var moved: [UInt64: Int64] = [:]
    /// id -> the item it sat in when last saved (this device only).
    public var parents: [UInt64: UInt64] = [:]

    public init(stamps: [UInt64: Int64] = [:], tombs: [UInt64: Int64] = [:], revived: [UInt64: Int64] = [:], hashes: [UInt64: UInt64] = [:],
                v: UInt32 = 0, values: [String: Int64] = [:], valueHashes: [String: UInt64] = [:],
                fields: [UInt64: [String: Int64]] = [:], fieldHashes: [UInt64: [String: UInt32]] = [:]) {
        self.stamps = stamps
        self.tombs = tombs
        self.revived = revived
        self.hashes = hashes
        self.v = v
        self.values = values
        self.valueHashes = valueHashes
        self.fields = fields
        self.fieldHashes = fieldHashes
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
        moved = map("moved")
        for (k, x) in json["parents"]?.object ?? [:] {
            if let id = UInt64(k), let p = x.uint { parents[id] = p }
        }
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
        for (k, x) in json["fields"]?.object ?? [:] {
            guard let id = UInt64(k), let o = x.object else { continue }
            var e: [String: Int64] = [:]
            for (p, t) in o { if let t = t.int { e[p] = t } }
            fields[id] = e
        }
        for (k, x) in json["field_hashes"]?.object ?? [:] {
            guard let id = UInt64(k), let o = x.object else { continue }
            var e: [String: UInt32] = [:]
            for (p, h) in o { if let h = h.uint { e[p] = UInt32(truncatingIfNeeded: h) } }
            fieldHashes[id] = e
        }
    }

    public var json: JSON {
        func obj(_ m: [UInt64: Int64]) -> JSON { .object(Dictionary(uniqueKeysWithValues: m.map { (String($0.key), JSON.int($0.value)) })) }
        var o: [String: JSON] = [
            "stamps": obj(stamps),
            "tombs": obj(tombs),
            "revived": obj(revived),
            "hashes": .object(Dictionary(uniqueKeysWithValues: hashes.map { (String($0.key), JSON.id($0.value)) })),
            "v": .int(Int64(v)),
            "values": .object(values.mapValues { JSON.int($0) }),
            "value_hashes": .object(valueHashes.mapValues { JSON.id($0) }),
        ]
        if !moved.isEmpty { o["moved"] = obj(moved) }
        if !parents.isEmpty {
            o["parents"] = .object(Dictionary(uniqueKeysWithValues: parents.map { (String($0.key), JSON.id($0.value)) }))
        }
        if !fields.isEmpty {
            o["fields"] = .object(Dictionary(uniqueKeysWithValues: fields.map { (String($0.key), JSON.object($0.value.mapValues { JSON.int($0) })) }))
        }
        if !fieldHashes.isEmpty {
            o["field_hashes"] = .object(Dictionary(uniqueKeysWithValues: fieldHashes.map { (String($0.key), JSON.object($0.value.mapValues { JSON.int(Int64($0)) })) }))
        }
        return .object(o)
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

    // MARK: field stamps (wb_sync parts_of / note_fields)

    /// The fields that together hold a task's running timers (TIMER_KEYS).
    static let timerKeys: Set<String> = ["timer_started", "timer_by", "timers"]

    static func isEmptyArr(_ v: JSON) -> Bool { v.array?.isEmpty == true }

    /// The (emoji, member) pairs of a reactions field, in order.
    static func reactionPairs(_ v: JSON) -> [(String, UInt64)]? {
        guard let a = v.array else { return nil }
        var out: [(String, UInt64)] = []
        for r in a {
            guard let e = r["emoji"]?.string, let users = r["users"]?.array else { return nil }
            for u in users {
                guard let u = u.uint else { return nil }
                if !out.contains(where: { $0.0 == e && $0.1 == u }) { out.append((e, u)) }
            }
        }
        return out
    }

    /// A task's running timers: (member, start, from `timers`).
    static func timerList(_ obj: [String: JSON]) -> [(member: UInt64, start: Int64, inVec: Bool)] {
        var out: [(member: UInt64, start: Int64, inVec: Bool)] = []
        if let st = obj["timer_started"]?.int {
            out.append((obj["timer_by"]?.uint ?? 0, st, false))
        }
        for r in obj["timers"]?.array ?? [] {
            if let m = r["member"]?.uint, let st = r["start"]?.int, !out.contains(where: { $0.member == m }) {
                out.append((m, st, true))
            }
        }
        return out
    }

    /// The parts of an item's own fields with their values (wb_sync parts_of).
    static func partsOf(_ obj: [String: JSON]) -> [(String, JSON)] {
        var out: [(String, JSON)] = []
        for k in obj.keys.sorted(by: { Array($0.utf8).lexicographicallyPrecedes(Array($1.utf8)) }) {
            let v = obj[k]!
            if localKeys.contains(k) || appendKeys.contains(k) || timerKeys.contains(k) || isEmptyArr(v) || isIdArray(v) { continue }
            if k == "reactions", let pairs = reactionPairs(v) {
                for (e, u) in pairs { out.append(("reactions/\(e)/\(u)", .bool(true))) }
                continue
            }
            out.append((k, v))
        }
        for t in timerList(obj) {
            out.append(("timers/\(t.member)", ["member": .id(t.member), "start": .int(t.start)]))
        }
        return out
    }

    static func partField(_ p: String) -> String {
        if let i = p.firstIndex(of: "/") { return String(p[..<i]) }
        return p
    }

    static func partHash(_ v: JSON) -> UInt32 { UInt32(truncatingIfNeeded: fnv(v.serialized().utf8)) }

    /// Every shared item: id, the item it sits in, its fields.
    static func visit(_ data: JSON, _ f: (UInt64, UInt64?, [String: JSON]) -> Void) {
        func go(_ v: JSON, _ parent: UInt64?) {
            for item in v.array ?? [] {
                guard let id = idOf(item), let obj = item.object else { continue }
                f(id, parent, obj)
                for (_, child) in obj where isIdArray(child) { go(child, id) }
            }
        }
        for path in shared {
            if let v = data.at(path) { go(v, nil) }
        }
    }

    /// Fingerprints of every part of every shared item (field_hashes).
    public static func partHashes(_ data: JSON) -> [UInt64: [String: UInt32]] {
        var out: [UInt64: [String: UInt32]] = [:]
        visit(data) { id, _, obj in
            var m: [String: UInt32] = [:]
            for (p, v) in partsOf(obj) { m[p] = partHash(v) }
            out[id] = m
        }
        return out
    }

    /// id -> the item it sits in, for every shared item inside another (parents_of).
    public static func parentsOf(_ data: JSON) -> [UInt64: UInt64] {
        var out: [UInt64: UInt64] = [:]
        visit(data) { id, parent, _ in if let p = parent { out[id] = p } }
        return out
    }

    /// Only items inside a space (folders, lists) ever move; move times
    /// for anything else are left out (wb_sync::shrink_moved).
    public static func shrinkMoved(_ moved: inout [UInt64: Int64], _ data: JSON) {
        guard !moved.isEmpty else { return }
        var inner: [UInt64: UInt64] = [:]
        if let spaces = data["spaces"] { walk(spaces, parent: nil, placed: false, &inner) }
        for s in data["spaces"]?.array ?? [] { if let id = idOf(s) { inner[id] = nil } }
        moved = moved.filter { inner[$0.key] != nil }
    }

    /// After the shape items are written in changed (`legacy` is the
    /// workspace the old way, `current` the new way): fingerprints that still
    /// match the old way are carried over, so the change in shape alone
    /// doesn't count as an edit (wb_sync::carry_hashes).
    public static func carryHashes(_ meta: inout SyncMeta, legacy: JSON, current: JSON) {
        let cur = fingerprints(current)
        let parts = partHashes(current)
        for (id, old) in fingerprints(legacy) where meta.hashes[id] == old {
            if let h = cur[id] {
                meta.hashes[id] = h
                if let p = parts[id] { meta.fieldHashes[id] = p }
            }
        }
    }

    /// A workspace as copies before SyncMeta.v 2 wrote it, with the fields
    /// that are now left out at their default filled in (the Mac app's
    /// legacy_shape): time entries' `member`, forms', whiteboards' and
    /// dashboards' `owner`, dashboards' `kind`.
    public static func legacyShape(_ v: JSON) -> JSON {
        var v = v
        func fill(_ items: JSON?, _ keys: [(String, JSON)]) -> JSON? {
            guard let a = items?.array else { return items }
            return .array(a.map { it in
                guard var o = it.object else { return it }
                for (k, d) in keys where o[k] == nil { o[k] = d }
                return .object(o)
            })
        }
        if let tasks = v["tasks"]?.array {
            v["tasks"] = .array(tasks.map { t in
                var t = t
                if t.object != nil, let e = fill(t["time_entries"], [("member", 0)]) { t["time_entries"] = e }
                return t
            })
        }
        if var e = v["extra"], e.object != nil {
            if let f = fill(e["forms"], [("owner", 0)]) { e["forms"] = f }
            if let w = fill(e["whiteboards"], [("owner", 0)]) { e["whiteboards"] = w }
            if let d = fill(e["dashboards"], [("owner", 0), ("kind", "")]) { e["dashboards"] = d }
            v["extra"] = e
        }
        return v
    }

    /// An item's own fields changed at `next` (wb_sync note_fields).
    static func noteFields(_ meta: inout SyncMeta, _ id: UInt64, before: Int64, _ nowParts: [String: UInt32]?, next: Int64) {
        let nowParts = nowParts ?? [:]
        guard let oldParts = meta.fieldHashes[id] else {
            if var e = meta.fields[id] {
                e = e.filter { !$0.key.isEmpty && nowParts[$0.key] == nil }
                meta.fields[id] = e
            }
            return
        }
        var changed = nowParts.filter { oldParts[$0.key] != $0.value }.map(\.key)
        changed += oldParts.keys.filter { nowParts[$0] == nil }
        var e = meta.fields[id] ?? [:]
        let base = e[""] ?? before
        for p in changed { e[p] = next }
        tidyFields(&e, present: { nowParts[$0] != nil }, base: base, stamp: next)
        meta.fields[id] = e
    }

    static func tidyFields(_ e: inout [String: Int64], present: (String) -> Bool, base: Int64, stamp: Int64) {
        e = e.filter { $0.key.isEmpty || !(present($0.key) && $0.value == base) }
        if base == stamp { e[""] = nil } else { e[""] = base }
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
        // places: an item now in another item than when last saved moved there
        let places = parentsOf(data)
        for (id, p) in places {
            if let old = meta.parents[id], old != p {
                meta.moved[id] = meta.moved[id].map { max(now, $0 + 1) } ?? now
            }
        }
        meta.parents = places
        shrinkMoved(&meta.moved, data)
        meta.v = metaV
        let parts = partHashes(data)
        for (id, h) in cur {
            let next = meta.stamps[id].map { max(now, $0 + 1) } ?? now
            if let old = meta.hashes[id] {
                if old != h {
                    noteFields(&meta, id, before: meta.stamps[id] ?? 0, parts[id], next: next)
                    meta.stamps[id] = next
                }
            } else {
                meta.stamps[id] = next
                // new (or back): every field changed now
                meta.fields[id] = [:]
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
            meta.fields[id] = nil
            meta.moved[id] = nil
        }
        meta.hashes = cur
        meta.fieldHashes = parts
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
        /// source -> its field stamps
        var fields: [[UInt64: [String: Int64]]] = []
        var outFields: [UInt64: [String: Int64]] = [:]

        /// id -> when it moved where it is (0: not known)
        var homeMoved: [UInt64: Int64] = [:]
        var outMoved: [UInt64: Int64] = [:]

        init(stamps: [[UInt64: Int64]], moved: [[UInt64: Int64]?], tombs: [UInt64: Int64], revived: [UInt64: Int64], all: [UInt64: [(src: Int, parent: UInt64?, value: JSON)]]) {
            self.stamps = stamps
            self.tombs = tombs
            self.revived = revived
            omitsDefaults = moved.map { $0 != nil }
            // the copy that moved it last decides where the item is (copies
            // that don't keep move times count their item's last change as
            // one), then the newest copy; on a tie this device's
            func key(_ src: Int, _ id: UInt64) -> (Int64, Int64) {
                let s = stamps[src][id] ?? 0
                let m = moved[src].map { $0[id] ?? 0 } ?? s
                return (m, s)
            }
            for (id, list) in all {
                var best = list[0]
                var bestKey = key(best.src, id)
                for c in list.dropFirst() {
                    let k = key(c.src, id)
                    if k > bestKey {
                        best = c
                        bestKey = k
                    }
                }
                home[id] = .some(best.parent)
                // a move time worth keeping: one the deciding copy keeps, or
                // a real move (the copies disagree where it is)
                if moved[best.src] != nil || list.contains(where: { $0.parent != best.parent }) {
                    homeMoved[id] = bestKey.0
                } else {
                    homeMoved[id] = list.compactMap { moved[$0.src]?[id] }.max() ?? 0
                }
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

    /// One copy of an item, as mergeFields sees it.
    private struct Side {
        let obj: [String: JSON]
        let parts: [(String, JSON)]
        let base: Int64
        let listed: [String: Int64]?
        let omits: Bool
        let src: Int

        func value(_ p: String) -> JSON? { parts.first(where: { $0.0 == p })?.1 }
    }

    /// When a copy's version of `part` was set, or nil when it can't tell
    /// (wb_sync read_part).
    private static func readPart(_ sd: Side, _ part: String, present: Bool, inVec: Set<UInt64>) -> Int64? {
        if let t = sd.listed?[part] { return t }
        var counts = present
        if !counts {
            let f = partField(part)
            if f == "reactions" && part != f {
                counts = sd.omits || (sd.obj["reactions"].map { !isEmptyArr($0) } ?? false)
            } else if f == "timers" && part != f {
                let m = UInt64(part.dropFirst("timers/".count)) ?? 0
                // an app that doesn't know `timers` can't have stopped one there
                counts = sd.omits || sd.obj["timers"] != nil || (sd.obj["timer_started"] != nil && !inVec.contains(m))
            } else {
                counts = defaultWhenMissing.contains(part) && sd.omits
            }
        }
        return counts ? sd.base : nil
    }

    /// Merge the own fields of one item's copies part by part
    /// (wb_sync merge_fields). Returns the fields and the field names decided.
    private static func mergeFields(_ cx: Ctx, _ id: UInt64, _ list: [Cand], stamp: Int64, newest: [String: JSON]) -> ([String: JSON], Set<String>) {
        var child = Set<String>()
        for c in list {
            for (k, v) in c.value.object ?? [:] where isIdArray(v) { child.insert(k) }
        }
        let sides: [Side] = list.compactMap { c in
            guard let obj = c.value.object else { return nil }
            let s = cx.stamps[c.src][id] ?? 0
            let listed = cx.fields.indices.contains(c.src) ? cx.fields[c.src][id] : nil
            return Side(obj: obj, parts: partsOf(obj).filter { !child.contains(partField($0.0)) }, base: listed?[""] ?? s, listed: listed,
                        omits: cx.omitsDefaults.indices.contains(c.src) && cx.omitsDefaults[c.src], src: c.src)
        }
        let anyEntry = sides.contains { $0.listed != nil }
        // parts in the order the newest copy has them, so every side ends up with the same order
        var order: [String] = []
        var seen = Set<String>()
        let first = sides.firstIndex(where: { $0.obj == newest }) ?? 0
        for sd in Array(sides[first...]) + Array(sides[..<first]) {
            for (p, _) in sd.parts where seen.insert(p).inserted { order.append(p) }
            for p in (sd.listed?.keys ?? [String: Int64]().keys).sorted(by: { Array($0.utf8).lexicographicallyPrecedes(Array($1.utf8)) })
            where !p.isEmpty && !child.contains(partField(p)) && seen.insert(p).inserted {
                order.append(p)
            }
        }
        var inVec = Set<UInt64>()
        for sd in sides { for t in timerList(sd.obj) where t.inVec { inVec.insert(t.member) } }
        let baseOut = sides.map(\.base).min() ?? stamp
        var entry: [String: Int64] = [:]
        var obj: [String: JSON] = [:]
        var decided = Set<String>()
        var reactions: [(String, UInt64)] = []
        var timers: [(UInt64, Int64)] = []
        for p in order {
            var win: (Int64, Side, JSON?)?
            for sd in sides {
                let val = sd.value(p)
                guard let t = readPart(sd, p, present: val != nil, inVec: inVec) else { continue }
                if win == nil || t > win!.0 { win = (t, sd, val) }
            }
            guard let (t, sd, val) = win else { continue }
            if anyEntry && (val == nil || t != baseOut) { entry[p] = t }
            let f = partField(p)
            if f == "reactions" && p != f {
                decided.insert("reactions")
                let rest = p.dropFirst("reactions/".count)
                if val != nil, let slash = rest.lastIndex(of: "/"), let u = UInt64(rest[rest.index(after: slash)...]) {
                    reactions.append((String(rest[..<slash]), u))
                }
            } else if f == "timers" && p != f {
                decided.formUnion(timerKeys)
                if let v = val { timers.append((v["member"]?.uint ?? 0, v["start"]?.int ?? 0)) }
            } else {
                decided.insert(p)
                if let v = val {
                    obj[p] = v
                } else if let v = sd.obj[p] {
                    // taken away there; keep how that copy writes it (an empty list)
                    obj[p] = v
                }
            }
        }
        if decided.contains("reactions") {
            var out: [(String, [UInt64])] = []
            for (e, u) in reactions {
                if let i = out.firstIndex(where: { $0.0 == e }) { out[i].1.append(u) } else { out.append((e, [u])) }
            }
            if !out.isEmpty || newest["reactions"] != nil {
                obj["reactions"] = .array(out.map { ["emoji": .string($0.0), "users": .array($0.1.map { JSON.id($0) })] })
            }
        }
        if decided.contains("timers") {
            // the same order everywhere: an old timer nobody owns first, then by start
            timers.sort { l, r in
                let kl = (l.0 != 0 ? 1 : 0, l.1, l.0), kr = (r.0 != 0 ? 1 : 0, r.1, r.0)
                return kl < kr
            }
            if let (m, st) = timers.first {
                obj["timer_started"] = .int(st)
                if m != 0 { obj["timer_by"] = .id(m) }
            } else if sides.contains(where: { $0.obj["timer_started"] != nil }) {
                obj["timer_started"] = .null
            }
            let rest = timers.dropFirst().map { JSON.object(["member": .id($0.0), "start": .int($0.1)]) }
            if !rest.isEmpty { obj["timers"] = .array(Array(rest)) }
        }
        if anyEntry {
            if baseOut != stamp { entry[""] = baseOut }
            cx.outFields[id] = entry
        }
        return (obj, decided)
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
            if let m = cx.homeMoved[id], m > 0 { cx.outMoved[id] = m }
            guard let wobj = best.value.object else { continue }
            let local = list.first(where: { $0.src == 0 })?.value.object
            var keys = Set(wobj.keys)
            for c in list {
                for k in c.value.object?.keys ?? [String: JSON]().keys { keys.insert(k) }
            }
            // own fields first, field by field (see mergeFields)
            var (obj, decided) = mergeFields(cx, id, list, stamp: bestStamp, newest: wobj)
            for k in keys {
                if decided.contains(k) || obj[k] != nil { continue }
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
        fillBoardIds(&local)
        let others = others.map { o -> JSON in
            var o = o
            fillActivityIds(&o)
            fillBoardIds(&o)
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
        let keeps = { (m: SyncMeta) -> [UInt64: Int64]? in m.v >= 2 ? m.moved : nil }
        let cx = Ctx(stamps: [lmeta.stamps] + metas.map(\.stamps), moved: [keeps(lmeta)] + metas.map(keeps), tombs: tombs, revived: revived, all: all)
        cx.fields = [lmeta.fields] + metas.map(\.fields)
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
        lmeta.moved = cx.outMoved
        shrinkMoved(&lmeta.moved, local)
        lmeta.fields = cx.outFields
        lmeta.hashes = fingerprints(local)
        lmeta.fieldHashes = partHashes(local)
        lmeta.parents = parentsOf(local)
        lmeta.valueHashes = valueHashes(local)
        lmeta.v = metaV
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
        // items marked personal stay on this device (wb_sync::shareable)
        for path in shared {
            guard let a = v.at(path)?.array else { continue }
            let kept = a.filter { $0["personal"]?.bool != true }
            if kept.count != a.count { v.set(path, .array(kept)) }
        }
        if v.at("extra.sync")?.object != nil {
            v.set("extra.sync.hashes", .object([:]))
            v.set("extra.sync.value_hashes", .object([:]))
            var sync = v.at("extra.sync") ?? .object([:])
            sync["field_hashes"] = nil
            sync["parents"] = nil
            v.set("extra.sync", sync)
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
            for key in ["stamps", "hashes", "moved", "parents", "fields", "field_hashes"] {
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

    // MARK: deltas (wb_sync::Summary / delta)

    /// The part of `data` (a whole copy) that differs from what `since` says
    /// the server has: changed items inside the items they sit in, their
    /// bookkeeping, new deletes and restores, changed shared settings.
    /// Merging it is the same as merging the whole copy. nil when nothing changed.
    public static func delta(since: SyncSummary, _ data: JSON) -> JSON? {
        let cur = fingerprints(data)
        let meta = SyncMeta(of: data)
        let changed = Set(cur.filter { id, h in
            since.hashes[id] != h || since.stamps[id] != meta.stamps[id] || since.fields[id] != meta.fields[id] || since.moved[id] != meta.moved[id]
        }.keys)
        func newer(_ a: [UInt64: Int64], _ b: [UInt64: Int64]) -> [UInt64: Int64] {
            a.filter { k, v in b[k].map { $0 < v } ?? true }
        }
        let tombs = newer(meta.tombs, since.tombs)
        let revived = newer(meta.revived, since.revived)
        let vh = valueHashes(data)
        let values = sharedValues.filter { since.valueHashes[$0] != vh[$0] || since.values[$0] != meta.values[$0] }
        if changed.isEmpty && tombs.isEmpty && revived.isEmpty && values.isEmpty { return nil }
        var kept: [UInt64] = []
        func prune(_ arr: [JSON]) -> [JSON] {
            var out: [JSON] = []
            for item in arr {
                guard let id = idOf(item), let obj = item.object else { continue }
                var o: [String: JSON] = [:]
                var inner = false
                for (k, v) in obj {
                    if isIdArray(v) {
                        let sub = prune(v.array ?? [])
                        if !sub.isEmpty {
                            inner = true
                            o[k] = .array(sub)
                        }
                    } else {
                        o[k] = v
                    }
                }
                if inner || changed.contains(id) {
                    kept.append(id)
                    out.append(.object(o))
                }
            }
            return out
        }
        var out: JSON = .object([:])
        for path in shared {
            if let a = data.at(path)?.array {
                let sub = prune(a)
                if !sub.isEmpty { out.set(path, .array(sub)) }
            }
        }
        for p in values {
            if let v = data.at(p) { out.set(p, v) }
        }
        var part = SyncMeta()
        for id in kept {
            part.stamps[id] = meta.stamps[id]
            part.moved[id] = meta.moved[id]
            part.fields[id] = meta.fields[id]
        }
        part.tombs = tombs
        part.revived = revived
        for p in values { part.values[p] = meta.values[p] }
        part.v = meta.v
        out.set("extra.sync", part.json)
        return out
    }
}

/// What the server has of this device's copy (wb_sync::Summary): each
/// item's fingerprint and bookkeeping, deletes, restores, shared settings.
/// Pushes send only what differs from it (Sync.delta).
public struct SyncSummary: Equatable, Sendable {
    var hashes: [UInt64: UInt64] = [:]
    var stamps: [UInt64: Int64] = [:]
    var fields: [UInt64: [String: Int64]] = [:]
    var moved: [UInt64: Int64] = [:]
    var tombs: [UInt64: Int64] = [:]
    var revived: [UInt64: Int64] = [:]
    var values: [String: Int64] = [:]
    var valueHashes: [String: UInt64] = [:]

    /// Everything in a whole copy.
    public init(of data: JSON) {
        let m = SyncMeta(of: data)
        hashes = Sync.fingerprints(data)
        stamps = m.stamps
        fields = m.fields
        moved = m.moved
        tombs = m.tombs
        revived = m.revived
        values = m.values
        valueHashes = Sync.valueHashes(data)
    }

    /// The server has this copy (whole or a delta) too now.
    public mutating func absorb(_ part: JSON) {
        let m = SyncMeta(of: part)
        for (id, h) in Sync.fingerprints(part) {
            hashes[id] = h
            stamps[id] = m.stamps[id]
            moved[id] = m.moved[id]
            fields[id] = m.fields[id]
        }
        for (k, v) in m.tombs { tombs[k] = max(tombs[k] ?? v, v) }
        for (k, v) in m.revived { revived[k] = max(revived[k] ?? v, v) }
        for (path, h) in Sync.valueHashes(part) where part.at(path) != nil {
            valueHashes[path] = h
            if let t = m.values[path] { values[path] = t }
        }
    }
}
