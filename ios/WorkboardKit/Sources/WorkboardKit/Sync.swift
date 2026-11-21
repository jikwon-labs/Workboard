// Change stamps and merging, ported from crates/wb-sync/src/lib.rs (the
// same code the Mac app and workboard-server use) so that a copy pushed from
// the phone merges exactly like a Mac's copy.
//
// Every item with an `id` remembers when it last changed (`stamps`, in
// milliseconds) and when each of its own fields did (`fields`): for each
// field the newest change wins, so two people changing different fields of
// one task both keep theirs. Reactions merge per (emoji, person), running
// timers per person, time entries per (person, start), custom field values
// per field; whiteboard strokes and notes have ids (`fillBoardIds`), as
// do a goal's targets and history lines (`fillGoalIds`), and
// entries without an id in a list of items stay as they are beside them.
// The items inside an item (messages in a channel, lists in a space) are merged one by one,
// and an item that moved (a list to another space) sits only where its newest
// copy puts it. Deleted items leave a tombstone (`tombs`); restoring one
// records a revival. A few single settings (SHARED_VALUES) are shared too,
// newest change wins, and a few maps entry by entry (SHARED_MAPS, newest
// change per key wins). Task comments and activity lines get ids worked out
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
    /// default may be left out, 3 = field stamps are kept, 4 = time entries
    /// and custom field values each have field stamps of their own
    /// ("time_entries/<member>/<start>", "custom/<field id>"). Sent in the
    /// hello as `sync`; the server takes no changes below
    /// wb_net::OLDEST_SYNC (4).
    public static let metaV: UInt32 = 4
    /// Single settings that belong to the whole team (SHARED_VALUES).
    public static let sharedValues: [String] = ["workspace", "extra.workspace_color", "extra.apps"]
    /// Shared maps merged entry by entry, newest change per key wins
    /// (SHARED_MAPS): `values` keeps a stamp per entry ("<path>/<key>").
    /// `extra.slack_map`: Slack user id -> the member a Mac's Import from
    /// Slack brings them in as.
    public static let sharedMaps: [String] = ["extra.slack_map"]

    /// The name of one entry of a shared map in `values` / `valueHashes`
    /// (wb_sync::map_part).
    public static func mapPart(_ path: String, _ key: String) -> String { path + "/" + key }

    /// The shared map and key an entry name stands for (wb_sync map_entry).
    static func mapEntry(_ part: String) -> (path: String, key: String)? {
        for p in sharedMaps where part.hasPrefix(p + "/") {
            return (p, String(part.dropFirst(p.count + 1)))
        }
        return nil
    }
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

    /// An item's id; 0 is no id (wb_sync id_of).
    static func idOf(_ v: JSON) -> UInt64? {
        guard let i = v["id"]?.uint, i != 0 else { return nil }
        return i
    }

    /// A non-empty array of objects of which at least one has an id: its
    /// entries with ids are items, merged one by one; entries without one
    /// stay as they are, part of the item the array sits in (wb_sync
    /// is_id_array / loose_entries).
    static func isIdArray(_ v: JSON) -> Bool {
        guard let a = v.array, !a.isEmpty else { return false }
        return a.allSatisfy { $0.object != nil } && a.contains { idOf($0) != nil }
    }

    /// The entries without an id in an array of items.
    static func looseEntries(_ v: JSON) -> [JSON] {
        (v.array ?? []).filter { idOf($0) == nil }
    }

    /// Fingerprint of an item's own fields (not the items inside it, not personal fields).
    static func ownHash(_ obj: [String: JSON]) -> UInt64 {
        var own: [String: JSON] = [:]
        for (k, v) in obj {
            if localKeys.contains(k) { continue }
            if let a = v.array, a.isEmpty { continue }
            if isIdArray(v) {
                // entries without an id are the item's own
                let loose = looseEntries(v)
                if !loose.isEmpty { own[k] = .array(loose) }
                continue
            }
            own[k] = v
        }
        return fnv(JSON.object(own).serialized().utf8)
    }

    /// `parent` is the item this array sits in (nil at the top). With
    /// `placed`, an inner item's fingerprint includes its parent, so moving a
    /// list to another space counts as a change to the list.
    static func walk(_ v: JSON, parent: UInt64?, placed: Bool, _ out: inout [UInt64: UInt64]) {
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
}
