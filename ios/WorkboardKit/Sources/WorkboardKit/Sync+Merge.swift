// Merging copies field by field (wb_sync::merge / merge_arr). Part of the
// port in Sync.swift.

import Foundation

extension Sync {
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
        let f = partField(part)
        // a stamp for a whole field from before it was split stands for each element
        if part != f, let t = sd.listed?[f] { return t }
        var counts = present
        if !counts {
            if f == "reactions" && part != f {
                counts = sd.omits || (sd.obj["reactions"].map { !isEmptyArr($0) } ?? false)
            } else if f == "time_entries" && part != f {
                counts = sd.omits || (sd.obj["time_entries"].map { !isEmptyArr($0) } ?? false)
            } else if f == "custom" && part != f {
                counts = sd.omits || (sd.obj["custom"]?.object.map { !$0.isEmpty } ?? false)
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
    /// (wb_sync merge_fields). Returns the fields, the field names decided,
    /// and for lists of items the entries without an id that go after them.
    private static func mergeFields(_ cx: Ctx, _ id: UInt64, _ list: [Cand], stamp: Int64, newest: [String: JSON]) -> ([String: JSON], Set<String>, [String: JSON]) {
        var child = Set<String>()
        for c in list {
            for (k, v) in c.value.object ?? [:] where isIdArray(v) { child.insert(k) }
        }
        let sides: [Side] = list.compactMap { c in
            guard let obj = c.value.object else { return nil }
            let s = cx.stamps[c.src][id] ?? 0
            let listed = cx.fields.indices.contains(c.src) ? cx.fields[c.src][id] : nil
            // a list of items keeps only its entries without an id here
            return Side(obj: obj, parts: partsOf(obj).filter { !child.contains(partField($0.0)) || child.contains($0.0) }, base: listed?[""] ?? s, listed: listed,
                        omits: cx.omitsDefaults.indices.contains(c.src) && cx.omitsDefaults[c.src], src: c.src)
        }
        let anyEntry = sides.contains { $0.listed != nil }
        // parts in the order the newest copy has them, so every side ends up with the same order
        var order: [String] = []
        var seen = Set<String>()
        let first = sides.firstIndex(where: { $0.obj == newest }) ?? 0
        for sd in Array(sides[first...]) + Array(sides[..<first]) {
            for (p, _) in sd.parts where seen.insert(p).inserted { order.append(p) }
            // (a whole split field's stamp is read for each of its elements)
            for p in (sd.listed?.keys ?? [String: Int64]().keys).sorted(by: utf8Less)
            where !p.isEmpty && (!child.contains(partField(p)) || child.contains(p)) && !splitLater.contains(p) && seen.insert(p).inserted {
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
        var entries: [(String, JSON)] = []
        var custom: [String: JSON] = [:]
        var loose: [String: JSON] = [:]
        for p in order {
            var win: (Int64, Side, JSON?)?
            for sd in sides {
                let val = sd.value(p)
                guard let t = readPart(sd, p, present: val != nil, inVec: inVec) else { continue }
                if win == nil || t > win!.0 { win = (t, sd, val) }
            }
            guard let (t, sd, val) = win else { continue }
            if anyEntry && (val == nil || t != baseOut) { entry[p] = t }
            if child.contains(p) {
                // the entries without an id of a list of items
                if let v = val { loose[p] = v }
                continue
            }
            let f = partField(p)
            if f == "time_entries" && p != f {
                decided.insert("time_entries")
                if let v = val { entries.append((p, v)) }
            } else if f == "custom" && p != f {
                decided.insert("custom")
                if let v = val { custom[String(p.dropFirst("custom/".count))] = v }
            } else if f == "reactions" && p != f {
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
            // the same order on every device: by emoji, then by person
            reactions.sort { l, r in l.0 != r.0 ? utf8Less(l.0, r.0) : l.1 < r.1 }
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
        if decided.contains("time_entries") {
            // the same order everywhere: by start, then whose
            entries.sort { l, r in
                let kl = (l.1["start"]?.int ?? 0, l.1["member"]?.uint ?? 0), kr = (r.1["start"]?.int ?? 0, r.1["member"]?.uint ?? 0)
                return kl != kr ? kl < kr : utf8Less(l.0, r.0)
            }
            if !entries.isEmpty || sides.contains(where: { $0.obj["time_entries"] != nil }) {
                obj["time_entries"] = .array(entries.map(\.1))
            }
        }
        if decided.contains("custom") && (!custom.isEmpty || sides.contains(where: { $0.obj["custom"] != nil })) {
            obj["custom"] = .object(custom)
        }
        if anyEntry {
            if baseOut != stamp { entry[""] = baseOut }
            cx.outFields[id] = entry
        }
        return (obj, decided, loose)
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
            var (obj, decided, loose) = mergeFields(cx, id, list, stamp: bestStamp, newest: wobj)
            for k in keys {
                if decided.contains(k) || obj[k] != nil { continue }
                let vals: [(Int, JSON)] = list.compactMap { c in c.value[k].map { (c.src, $0) } }
                let value: JSON
                if vals.contains(where: { isIdArray($0.1) }) {
                    let subs: [(Int, [JSON])] = vals.compactMap { s, v in v.array.map { (s, $0) } }
                    // entries without an id stay as they are, after the items
                    value = .array(mergeArr(cx, subs, parent: id) + (loose[k]?.array ?? []))
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
            // this device's entries without an id stay as they are
            let loose = sources[0].1.filter { idOf($0) == nil }
            merged.append((path, .array(mergeArr(cx, sources, parent: nil) + loose)))
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
        mergeMaps(&local, &lmeta, others, metas)
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

    /// Shared maps (wb_sync merge_maps): per key the newest change wins (0:
    /// a copy that never had it changed); on a tie an entry beats none, then
    /// the larger value, so every copy ends up the same; an entry left out
    /// with the newest stamp was taken away.
    static func mergeMaps(_ local: inout JSON, _ lmeta: inout SyncMeta, _ others: [JSON], _ metas: [SyncMeta]) {
        for path in sharedMaps {
            let mine = local.at(path)?.object
            var sides: [(values: [String: Int64], obj: [String: JSON]?)] = [(lmeta.values, mine)]
            for (o, m) in zip(others, metas) { sides.append((m.values, o.at(path)?.object)) }
            var keys = Set<String>()
            for sd in sides {
                if let o = sd.obj { keys.formUnion(o.keys) }
                for p in sd.values.keys {
                    if let e = mapEntry(p), e.path == path { keys.insert(e.key) }
                }
            }
            var out = mine ?? [:]
            for key in keys.sorted(by: utf8Less) {
                let part = mapPart(path, key)
                var best: (stamp: Int64, value: JSON?)?
                for sd in sides {
                    let s = sd.values[part] ?? 0
                    let v = sd.obj?[key]
                    let better: Bool
                    if let b = best {
                        if s != b.stamp {
                            better = s > b.stamp
                        } else if (v != nil) != (b.value != nil) {
                            better = v != nil
                        } else if let x = v, let y = b.value {
                            better = utf8Less(y.serialized(), x.serialized())
                        } else {
                            better = false
                        }
                    } else {
                        better = true
                    }
                    if better { best = (s, v) }
                }
                guard let b = best else { continue }
                out[key] = b.value
                if b.stamp > 0 { lmeta.values[part] = b.stamp }
            }
            if mine != nil || !out.isEmpty { local.set(path, .object(out)) }
        }
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
}
