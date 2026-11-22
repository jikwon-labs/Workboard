// Fingerprints kept between changes. Stamping, merging and pushing each need
// every shared item's fingerprint, its field fingerprints and the item it
// sits in; working them out from scratch took several passes over the whole
// workspace on every edit and every message that came in. The cache keeps
// the items it last saw and works out again only the ones that differ.
//
// An item is the same as before when its JSON is equal to the copy kept
// here and it sits in the same item. JSON values share storage until they
// are changed (copy on write), so an item nobody touched compares equal at
// once, without looking inside it; only the items on the way from the top
// of the workspace to a change are looked at.

import Foundation

/// Not to be used from two threads at once; handed from one to another
/// (made off the main thread when a copy is opened) it is fine.
public final class FingerprintCache: @unchecked Sendable {
    private struct Node {
        var value: JSON
        var parent: UInt64?
    }

    /// Every shared item last seen, with the item it sat in.
    private var nodes: [UInt64: Node] = [:]
    /// Each shared collection as last seen.
    private var top: [String: JSON] = [:]
    /// Sync.fingerprints of the workspace last seen.
    public private(set) var hashes: [UInt64: UInt64] = [:]
    /// Sync.partHashes of it.
    public private(set) var parts: [UInt64: [String: UInt32]] = [:]
    /// Sync.parentsOf of it.
    public private(set) var parents: [UInt64: UInt64] = [:]

    public init() {}

    /// Everything as for a workspace never seen.
    public func reset() {
        nodes = [:]
        top = [:]
        hashes = [:]
        parts = [:]
        parents = [:]
    }

    /// Bring the fingerprints up to date with `data`. Afterwards they are
    /// exactly Sync.fingerprints(data), Sync.partHashes(data) and
    /// Sync.parentsOf(data).
    public func update(_ data: JSON) {
        var seen = Set<UInt64>()
        var gone: [UInt64] = []
        for path in Sync.shared {
            let now = data.at(path) ?? .null
            let was = top[path] ?? .null
            if now == was { continue }
            // items at the top that left the collection
            var ids = Set<UInt64>()
            for item in now.array ?? [] { if let id = Sync.idOf(item) { ids.insert(id) } }
            for item in was.array ?? [] {
                if let id = Sync.idOf(item), !ids.contains(id) { gone.append(id) }
            }
            walk(now, parent: nil, &seen, &gone)
            top[path] = now
        }
        // gone: unless it turned up somewhere else (an item moved), with
        // everything inside it that didn't
        for id in gone where !seen.contains(id) { drop(id, seen) }
    }

    private func walk(_ arr: JSON, parent: UInt64?, _ seen: inout Set<UInt64>, _ gone: inout [UInt64]) {
        for item in arr.array ?? [] {
            guard let id = Sync.idOf(item), let obj = item.object else { continue }
            seen.insert(id)
            let old = nodes[id]
            if let old, old.parent == parent, old.value == item { continue }
            var h = Sync.ownHash(obj)
            if let p = parent { h = Sync.fnv("\(h)/\(p)".utf8) }
            hashes[id] = h
            var m: [String: UInt32] = [:]
            for (p, v) in Sync.partsOf(obj) { m[p] = Sync.partHash(v) }
            parts[id] = m
            parents[id] = parent
            nodes[id] = Node(value: item, parent: parent)
            var inner = Set<UInt64>()
            for (_, child) in obj where Sync.isIdArray(child) {
                for c in child.array ?? [] { if let cid = Sync.idOf(c) { inner.insert(cid) } }
                walk(child, parent: id, &seen, &gone)
            }
            // items that were inside it and aren't now
            for (_, child) in old?.value.object ?? [:] where Sync.isIdArray(child) {
                for c in child.array ?? [] {
                    if let cid = Sync.idOf(c), !inner.contains(cid) { gone.append(cid) }
                }
            }
        }
    }

    private func drop(_ id: UInt64, _ seen: Set<UInt64>) {
        guard let n = nodes.removeValue(forKey: id) else { return }
        hashes[id] = nil
        parts[id] = nil
        parents[id] = nil
        for (_, child) in n.value.object ?? [:] where Sync.isIdArray(child) {
            for c in child.array ?? [] {
                if let cid = Sync.idOf(c), !seen.contains(cid) { drop(cid, seen) }
            }
        }
    }
}

extension Sync {
    /// Fingerprints, field fingerprints and places of every shared item:
    /// from the cache when there is one.
    static func prints(_ data: JSON, _ cache: FingerprintCache?) -> (hashes: [UInt64: UInt64], parts: [UInt64: [String: UInt32]], parents: [UInt64: UInt64]) {
        if let cache {
            cache.update(data)
            return (cache.hashes, cache.parts, cache.parents)
        }
        return (fingerprints(data), partHashes(data), parentsOf(data))
    }
}
