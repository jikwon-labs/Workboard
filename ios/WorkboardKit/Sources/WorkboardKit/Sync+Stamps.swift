// Field stamps: which of an item's fields changed and when (wb_sync
// parts_of / note_fields / stamp). Part of the port in Sync.swift.

import Foundation

extension Sync {
    // MARK: field stamps (wb_sync parts_of / note_fields)

    /// The fields that together hold a task's running timers (TIMER_KEYS).
    static let timerKeys: Set<String> = ["timer_started", "timer_by", "timers"]

    static func isEmptyArr(_ v: JSON) -> Bool { v.array?.isEmpty == true }

    /// Fields split into parts since field stamps began (wb_sync SPLIT_LATER).
    static let splitLater: [String] = ["time_entries", "custom"]

    static func utf8Less(_ a: String, _ b: String) -> Bool { Array(a.utf8).lexicographicallyPrecedes(Array(b.utf8)) }

    /// The parts of a time_entries field, one per entry by whose it is and
    /// when it started (wb_sync entry_parts). nil when it isn't a list of entries.
    static func entryParts(_ v: JSON) -> [(String, JSON)]? {
        guard let a = v.array else { return nil }
        var out: [(String, JSON)] = []
        struct Key: Hashable { let m: UInt64; let s: Int64 }
        var seen: [Key: Int] = [:]
        for e in a {
            guard e.object != nil, let start = e["start"]?.int else { return nil }
            let member = e["member"]?.uint ?? 0
            let n = seen[Key(m: member, s: start), default: 0]
            seen[Key(m: member, s: start)] = n + 1
            out.append((n == 0 ? "time_entries/\(member)/\(start)" : "time_entries/\(member)/\(start)/\(n)", e))
        }
        return out
    }

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
        for k in obj.keys.sorted(by: utf8Less) {
            let v = obj[k]!
            if localKeys.contains(k) || appendKeys.contains(k) || timerKeys.contains(k) || isEmptyArr(v) { continue }
            if isIdArray(v) {
                let loose = looseEntries(v)
                if !loose.isEmpty { out.append((k, .array(loose))) }
                continue
            }
            if k == "reactions", let pairs = reactionPairs(v) {
                for (e, u) in pairs { out.append(("reactions/\(e)/\(u)", .bool(true))) }
                continue
            }
            if k == "time_entries", let parts = entryParts(v) {
                out += parts
                continue
            }
            if k == "custom", let o = v.object {
                for key in o.keys.sorted(by: utf8Less) { out.append(("custom/\(key)", o[key]!)) }
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

    /// Fingerprints saved before time entries and custom field values were
    /// split have one for the whole field: when it still matches, it is
    /// swapped for today's parts (wb_sync upgrade_split_hashes).
    static func upgradeSplitHashes(_ meta: inout SyncMeta, _ data: JSON, _ parts: [UInt64: [String: UInt32]]) {
        guard meta.fieldHashes.values.contains(where: { e in splitLater.contains { e[$0] != nil } }) else { return }
        var fh = meta.fieldHashes
        visit(data) { id, _, obj in
            guard var old = fh[id] else { return }
            let now = parts[id]
            var touched = false
            for k in splitLater {
                guard let h = old[k] else { continue }
                // a value that doesn't split is still one part
                if now?[k] != nil { continue }
                if let v = obj[k], partHash(v) == h {
                    old[k] = nil
                    for (p, ph) in now ?? [:] where partField(p) == k && p.count > k.count { old[p] = ph }
                    touched = true
                }
            }
            if touched { fh[id] = old }
        }
        meta.fieldHashes = fh
    }

    static func tidyFields(_ e: inout [String: Int64], present: (String) -> Bool, base: Int64, stamp: Int64) {
        e = e.filter { $0.key.isEmpty || !(present($0.key) && $0.value == base) }
        if base == stamp { e[""] = nil } else { e[""] = base }
    }

    /// Fingerprints of the shared settings and of each shared map entry
    /// there is (wb_sync::value_hashes).
    static func valueHashes(_ data: JSON) -> [String: UInt64] {
        var out: [String: UInt64] = [:]
        for p in sharedValues { out[p] = fnv((data.at(p) ?? .null).serialized().utf8) }
        for p in sharedMaps {
            for (k, v) in data.at(p)?.object ?? [:] { out[mapPart(p, k)] = fnv(v.serialized().utf8) }
        }
        return out
    }

    /// An entry an upgrade put into a shared map rather than anyone's
    /// decision: noted as saved already, without a stamp, so `stamp`
    /// doesn't take it as set now (wb_sync::carry_map_entry).
    public static func carryMapEntry(_ meta: inout SyncMeta, _ path: String, _ key: String, _ value: JSON) {
        meta.valueHashes[mapPart(path, key)] = fnv(value.serialized().utf8)
    }

    /// After opening a saved copy: a shared setting or shared map entry with
    /// no fingerprint yet gets today's, without a stamp, so the next edit
    /// doesn't take entries an older build kept (a map the team's copy
    /// brought in) as set just now (wb_sync::fill_value_hashes).
    public static func fillValueHashes(_ meta: inout SyncMeta, _ data: JSON) {
        for (part, h) in valueHashes(data) where meta.valueHashes[part] == nil {
            meta.valueHashes[part] = h
        }
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
        upgradeSplitHashes(&meta, data, parts)
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
        // first time keeps none, so it wins only once someone changes it. A
        // shared map entry new since the last save was just set (on a copy
        // never stamped before it keeps none), one gone was taken away.
        let vh = valueHashes(data)
        let stampedBefore = sharedValues.contains { meta.valueHashes[$0] != nil }
        func bump(_ part: String) { meta.values[part] = meta.values[part].map { max(now, $0 + 1) } ?? now }
        for (part, h) in vh {
            let changed: Bool
            if let old = meta.valueHashes[part] {
                changed = old != h
            } else {
                changed = stampedBefore && mapEntry(part) != nil
            }
            if changed { bump(part) }
        }
        for part in meta.valueHashes.keys where mapEntry(part) != nil && vh[part] == nil {
            bump(part)
        }
        meta.valueHashes = meta.valueHashes.filter { mapEntry($0.key) == nil }
        meta.valueHashes.merge(vh) { _, new in new }
    }
}
