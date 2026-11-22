// Deltas: only what changed since the server's summary (wb_sync::Summary /
// delta). Part of the port in Sync.swift.

import Foundation

extension Sync {
    // MARK: deltas (wb_sync::Summary / delta)

    /// The part of `data` (a whole copy) that differs from what `since` says
    /// the server has: changed items inside the items they sit in, their
    /// bookkeeping, new deletes and restores, changed shared settings.
    /// Merging it is the same as merging the whole copy. nil when nothing changed.
    public static func delta(since: SyncSummary, _ data: JSON, cache: FingerprintCache? = nil, meta known: SyncMeta? = nil) -> JSON? {
        let cur = cache.map { $0.update(data); return $0.hashes } ?? fingerprints(data)
        let meta = known ?? SyncMeta(of: data)
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
        // shared map entries set or taken away since
        let entries = Set(vh.keys).union(meta.values.keys).filter {
            mapEntry($0) != nil && (since.valueHashes[$0] != vh[$0] || since.values[$0] != meta.values[$0])
        }.sorted(by: utf8Less)
        if changed.isEmpty && tombs.isEmpty && revived.isEmpty && values.isEmpty && entries.isEmpty { return nil }
        var kept: [UInt64] = []
        func prune(_ arr: [JSON]) -> [JSON] {
            var out: [JSON] = []
            for item in arr {
                guard let id = idOf(item), let obj = item.object else { continue }
                var o: [String: JSON] = [:]
                var inner = false
                for (k, v) in obj {
                    if isIdArray(v) {
                        var sub = prune(v.array ?? [])
                        if !sub.isEmpty { inner = true }
                        // entries without an id are the item's own
                        sub += looseEntries(v)
                        if !sub.isEmpty { o[k] = .array(sub) }
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
        for p in entries {
            // one taken away is only its stamp
            guard let e = mapEntry(p), let v = data.at(e.path)?.object?[e.key] else { continue }
            var m = out.at(e.path)?.object ?? [:]
            m[e.key] = v
            out.set(e.path, .object(m))
        }
        var part = SyncMeta()
        for id in kept {
            part.stamps[id] = meta.stamps[id]
            part.moved[id] = meta.moved[id]
            part.fields[id] = meta.fields[id]
        }
        part.tombs = tombs
        part.revived = revived
        for p in values + entries { part.values[p] = meta.values[p] }
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
        let vh = Sync.valueHashes(part)
        // a shared map's entries are there when they have a fingerprint
        for (path, h) in vh where Sync.mapEntry(path) != nil || part.at(path) != nil {
            valueHashes[path] = h
            if let t = m.values[path] { values[path] = t }
        }
        // shared map entries it took away
        for (p, t) in m.values where Sync.mapEntry(p) != nil && vh[p] == nil {
            valueHashes[p] = nil
            values[p] = t
        }
    }
}
