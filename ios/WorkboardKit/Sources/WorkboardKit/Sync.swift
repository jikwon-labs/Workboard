// Change stamps and merging, ported from the Mac app's src/sync.rs so that a
// copy pushed from the phone merges on the server exactly like a Mac's copy.
//
// Every item with an `id` remembers when its own fields last changed
// (`stamps`). The newest version of an item wins; the items inside it
// (messages in a channel, lists in a space) are merged one by one. Deleted
// items leave a tombstone (`tombs`); restoring one records a revival.
//
// Fingerprints (`hashes`) are this device's own and need not match the Rust
// bytes; they only need to be stable here, which the sorted-key serializer in
// JSON.swift gives.

import Foundation

public enum Sync {
    /// Collections that belong to the whole team (src/sync.rs SHARED).
    public static let shared: [String] = [
        "spaces", "tasks", "members", "docs", "channels", "extra.goals", "extra.forms", "extra.whiteboards", "extra.meetings", "extra.fields", "extra.automations", "extra.templates", "extra.dashboards", "extra.teams",
    ]
    /// Fields that are each person's own even inside shared items (LOCAL_KEYS).
    public static let localKeys: Set<String> = ["last_read", "saved", "expanded"]
    /// Lists that only ever grow (APPEND_KEYS).
    public static let appendKeys: Set<String> = ["activity"]
}

/// Change bookkeeping kept at `extra.sync` (src/sync.rs SyncMeta).
public struct SyncMeta: Equatable, Sendable {
    public var stamps: [UInt64: Int64] = [:]
    public var tombs: [UInt64: Int64] = [:]
    public var revived: [UInt64: Int64] = [:]
    public var hashes: [UInt64: UInt64] = [:]

    public init(stamps: [UInt64: Int64] = [:], tombs: [UInt64: Int64] = [:], revived: [UInt64: Int64] = [:], hashes: [UInt64: UInt64] = [:]) {
        self.stamps = stamps
        self.tombs = tombs
        self.revived = revived
        self.hashes = hashes
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
    }

    public var json: JSON {
        func obj(_ m: [UInt64: Int64]) -> JSON { .object(Dictionary(uniqueKeysWithValues: m.map { (String($0.key), JSON.int($0.value)) })) }
        return .object([
            "stamps": obj(stamps),
            "tombs": obj(tombs),
            "revived": obj(revived),
            "hashes": .object(Dictionary(uniqueKeysWithValues: hashes.map { (String($0.key), JSON.id($0.value)) })),
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

    private static func walk(_ v: JSON, _ out: inout [UInt64: UInt64]) {
        for item in v.array ?? [] {
            guard let id = idOf(item), let obj = item.object else { continue }
            out[id] = ownHash(obj)
            for (_, child) in obj where isIdArray(child) {
                walk(child, &out)
            }
        }
    }

    /// Fingerprints of every shared item in a workspace.
    public static func fingerprints(_ data: JSON) -> [UInt64: UInt64] {
        var out: [UInt64: UInt64] = [:]
        for path in shared {
            if let v = data.at(path) { walk(v, &out) }
        }
        return out
    }

    /// Record what changed since the last save: new stamps for edited items,
    /// tombstones for removed ones, revivals for restored ones.
    ///
    /// With `pastSeen`, an edited item's stamp is also made later than the
    /// stamp it had, so an edit made after seeing a version always beats that
    /// version even when this device's clock runs behind (the Rust code uses
    /// `now` alone; see docs/ios-design.md).
    public static func stamp(_ data: JSON, _ meta: inout SyncMeta, now: Int64, pastSeen: Bool = false) {
        let cur = fingerprints(data)
        for (id, h) in cur {
            if let old = meta.hashes[id] {
                if old != h {
                    var t = now
                    if pastSeen, let prev = meta.stamps[id] { t = max(now, prev + 1) }
                    meta.stamps[id] = t
                }
            } else {
                meta.stamps[id] = now
                if let t = meta.tombs[id] {
                    if meta.revived[id].map({ $0 < t }) ?? true {
                        meta.revived[id] = max(now, t + 1)
                    }
                }
            }
        }
        for id in meta.hashes.keys where cur[id] == nil {
            meta.tombs[id] = now
            meta.stamps[id] = nil
        }
        meta.hashes = cur
    }

    // MARK: merge (src/sync.rs merge / merge_arr)

    private final class Ctx {
        var stamps: [[UInt64: Int64]]
        var tombs: [UInt64: Int64]
        var revived: [UInt64: Int64]
        var out: [UInt64: Int64] = [:]

        init(stamps: [[UInt64: Int64]], tombs: [UInt64: Int64], revived: [UInt64: Int64]) {
            self.stamps = stamps
            self.tombs = tombs
            self.revived = revived
        }

        func alive(_ id: UInt64) -> Bool {
            guard let t = tombs[id] else { return true }
            return revived[id].map { $0 > t } ?? false
        }
    }

    private static func mergeArr(_ cx: Ctx, _ sources: [(Int, [JSON])]) -> [JSON] {
        var order: [UInt64] = []
        var cands: [UInt64: [(Int, JSON)]] = [:]
        for (src, arr) in sources {
            for v in arr {
                guard let id = idOf(v) else { continue }
                if cands[id] == nil { order.append(id) }
                cands[id, default: []].append((src, v))
            }
        }
        var out: [JSON] = []
        for id in order {
            if !cx.alive(id) { continue }
            let list = cands[id]!
            // newest own fields win; on a tie this device's version stays
            var best = list[0]
            var bestStamp = cx.stamps[best.0][id] ?? 0
            for c in list.dropFirst() {
                let s = cx.stamps[c.0][id] ?? 0
                if s > bestStamp {
                    best = c
                    bestStamp = s
                }
            }
            cx.out[id] = bestStamp
            guard let wobj = best.1.object else { continue }
            let local = list.first(where: { $0.0 == 0 })?.1.object
            var keys = Set(wobj.keys)
            for c in list {
                for k in c.1.object?.keys ?? [String: JSON]().keys { keys.insert(k) }
            }
            var obj: [String: JSON] = [:]
            for k in keys {
                let vals: [(Int, JSON)] = list.compactMap { c in c.1[k].map { (c.0, $0) } }
                let value: JSON
                if vals.contains(where: { isIdArray($0.1) }) {
                    let subs: [(Int, [JSON])] = vals.compactMap { s, v in v.array.map { (s, $0) } }
                    value = .array(mergeArr(cx, subs))
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
        let before = local
        var lmeta = SyncMeta(of: local)
        let metas = others.map { SyncMeta(of: $0) }
        var tombs = lmeta.tombs
        var revived = lmeta.revived
        for m in metas {
            for (k, v) in m.tombs { tombs[k] = max(tombs[k] ?? v, v) }
            for (k, v) in m.revived { revived[k] = max(revived[k] ?? v, v) }
        }
        let cx = Ctx(stamps: [lmeta.stamps] + metas.map(\.stamps), tombs: tombs, revived: revived)
        var merged: [(String, JSON)] = []
        for path in shared {
            var sources: [(Int, [JSON])] = [(0, before.at(path)?.array ?? [])]
            for (i, o) in others.enumerated() {
                if let a = o.at(path)?.array { sources.append((i + 1, a)) }
            }
            merged.append((path, .array(mergeArr(cx, sources))))
        }
        for (path, v) in merged { local.set(path, v) }
        lmeta.tombs = tombs
        lmeta.revived = revived
        lmeta.stamps = cx.out
        lmeta.hashes = fingerprints(local)
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
    /// write_mine does for the shared folder.
    public static func outgoing(_ data: JSON) -> JSON {
        var v = data
        if v.at("extra.sync")?.object != nil {
            v.set("extra.sync.hashes", .object([:]))
        }
        return v
    }
}
