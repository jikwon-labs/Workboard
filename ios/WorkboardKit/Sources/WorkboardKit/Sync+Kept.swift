// How the app keeps a copy's bookkeeping (`extra.sync`) while it runs.
//
// Reading the bookkeeping out of the JSON tree and writing it back took two
// passes over hundreds of thousands of entries on every edit and every
// message that came in. So the app keeps it parsed (a SyncMeta) next to the
// copy. In the copy's tree only the parts every device keeps stay, and only
// the entries that changed are written back to them (`patchShared`). The
// parts only this device keeps (fingerprints, places) live in the SyncMeta
// alone and go back into snapshot.json when it is written, so the file reads
// as before (`joined`).

import Foundation

extension Sync {
    /// A whole copy (as snapshot.json or a server keeps it) taken apart: the
    /// copy with only the shared parts of its bookkeeping, and all of it parsed.
    static func split(_ whole: JSON) -> (JSON, SyncMeta) {
        let meta = SyncMeta(of: whole)
        var tree = whole
        tree.set("extra.sync", .object(meta.sharedJSON))
        return (tree, meta)
    }

    /// The copy as snapshot.json keeps it: with this device's parts of the
    /// bookkeeping put back.
    static func joined(_ tree: JSON, _ meta: SyncMeta) -> JSON {
        var whole = tree
        var sync = whole.at("extra.sync")?.object ?? meta.sharedJSON
        sync.merge(meta.deviceJSON) { _, d in d }
        whole.set("extra.sync", .object(sync))
        return whole
    }

    /// Bring the shared parts of the bookkeeping in `tree` from `old` to
    /// `new`, writing only the entries that changed.
    static func patchShared(_ tree: inout JSON, old: SyncMeta, new: SyncMeta) {
        guard old.stamps != new.stamps || old.tombs != new.tombs || old.revived != new.revived || old.v != new.v
            || old.values != new.values || old.moved != new.moved || old.fields != new.fields else { return }
        tree.withObject { root in
            var extra = root.removeValue(forKey: "extra") ?? .object([:])
            if extra.object == nil { extra = .object([:]) }
            extra.withObject { e in
                var sync = e.removeValue(forKey: "sync") ?? .object([:])
                if sync.object == nil { sync = .object([:]) }
                sync.withObject { o in
                    patch(&o, "stamps", old.stamps, new.stamps, keepEmpty: true)
                    patch(&o, "tombs", old.tombs, new.tombs, keepEmpty: true)
                    patch(&o, "revived", old.revived, new.revived, keepEmpty: true)
                    patch(&o, "moved", old.moved, new.moved, keepEmpty: false)
                    if old.values != new.values || o["values"] == nil { o["values"] = SyncMeta.encode(new.values) }
                    o["v"] = .int(Int64(new.v))
                    if new.fields.isEmpty {
                        o["fields"] = nil
                    } else if var f = o.removeValue(forKey: "fields"), f.object != nil {
                        if old.fields != new.fields {
                            f.withObject { fo in
                                for (id, e) in new.fields where old.fields[id] != e { fo[String(id)] = SyncMeta.encode(e) }
                                for id in old.fields.keys where new.fields[id] == nil { fo[String(id)] = nil }
                            }
                        }
                        o["fields"] = f
                    } else {
                        o["fields"] = .object(Dictionary(uniqueKeysWithValues: new.fields.map { (String($0.key), SyncMeta.encode($0.value)) }))
                    }
                }
                e["sync"] = sync
            }
            root["extra"] = extra
        }
    }

    private static func patch(_ o: inout [String: JSON], _ key: String, _ old: [UInt64: Int64], _ new: [UInt64: Int64], keepEmpty: Bool) {
        if new.isEmpty && !keepEmpty {
            o[key] = nil
            return
        }
        guard var m = o.removeValue(forKey: key), m.object != nil else {
            o[key] = SyncMeta.encode(new)
            return
        }
        if old != new {
            m.withObject { mo in
                for (id, t) in new where old[id] != t { mo[String(id)] = .int(t) }
                for id in old.keys where new[id] == nil { mo[String(id)] = nil }
            }
        }
        o[key] = m
    }
}
