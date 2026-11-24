// A doc's Version history, as on the Mac (src/model/doc_versions.rs): earlier
// texts kept inside the doc ("versions"), each an item with an id, so they
// merge one by one like any item inside an item. Saving an edit (Done in the
// editor) keeps the text it started from when no version has it yet and
// the new text as a version; edits by one person closer together than
// `gap` make one version; Restore this version brings a text back as a new
// version. Field names and rules follow the Mac's `DocVersion`.

import Foundation

public struct DocVersionView: Identifiable, Hashable, Sendable {
    public let id: UInt64
    /// Who wrote it; 0 for text from before the doc had versions.
    public let by: UInt64
    /// When it was written (seconds).
    public let at: Int64
    public let title: String
    public let body: String
    /// Made by Restore this version: when the version it brought back was written.
    public let restored: Int64?

    init(_ j: JSON) {
        id = j["id"]?.uint ?? 0
        by = j["by"]?.uint ?? 0
        at = j["at"]?.int ?? 0
        title = j["title"]?.string ?? ""
        body = j["body"]?.string ?? ""
        restored = j["restored"]?.int
    }
}

public enum DocVersions {
    /// Edits by one person closer together than this make one version.
    public static let gap: Int64 = 5 * 60
    /// The most versions a doc keeps (the newest ones).
    public static let kept = 50
    /// Versions older than this many days go (the newest one always stays).
    public static let days: Int64 = 30

    /// A doc's versions, newest first.
    public static func newest(_ versions: [DocVersionView]) -> [DocVersionView] {
        versions.sorted { ($0.at, $0.id) > ($1.at, $1.id) }
    }

    static func newestRaw(_ doc: JSON) -> [JSON] {
        (doc["versions"]?.array ?? []).sorted {
            (($0["at"]?.int ?? 0), ($0["id"]?.uint ?? 0)) > (($1["at"]?.int ?? 0), ($1["id"]?.uint ?? 0))
        }
    }

    /// Drop versions past `kept` and older than `days` before `now`, keeping the newest.
    static func trim(_ doc: inout JSON, now: Int64) {
        let keep = Set(newestRaw(doc).enumerated().filter { i, v in
            i == 0 || (i < kept && (v["at"]?.int ?? 0) >= now - days * 24 * 3600)
        }.compactMap { $0.element["id"]?.uint })
        doc["versions"] = .array((doc["versions"]?.array ?? []).filter { keep.contains($0["id"]?.uint ?? 0) })
    }

    static func item(id: UInt64, by: UInt64, at: Int64, title: String, body: String, restored: Int64? = nil) -> JSON {
        var v: JSON = ["id": .id(id), "by": .id(by), "at": .int(at), "title": .string(title), "body": .string(body)]
        if let r = restored { v["restored"] = .int(r) }
        return v
    }

    /// keep_doc_text: keep `title` and `body` as a version of `doc` by `by`
    /// at `at`. Nothing happens when the newest version reads the same; a
    /// newer edit by the same person within `gap` of their last version
    /// updates that one instead. Returns whether the versions changed.
    @discardableResult
    public static func keepText(_ data: inout JSON, doc: UInt64, by: UInt64, at: Int64, title: String, body: String, newId: () -> UInt64) -> Bool {
        data.tryUpdateItem(at: "docs", id: doc) { d in
            let latest = newestRaw(d).first
            if let l = latest, l["title"]?.string == title, l["body"]?.string == body { return false }
            var versions = d["versions"]?.array ?? []
            if let l = latest, by != 0, l["by"]?.uint == by, l["restored"] == nil,
               abs(at - (l["at"]?.int ?? 0)) < gap,
               let i = versions.firstIndex(where: { $0["id"] == l["id"] }) {
                versions[i]["title"] = .string(title)
                versions[i]["body"] = .string(body)
                versions[i]["at"] = .int(max(l["at"]?.int ?? 0, at))
                d["versions"] = .array(versions)
                return true
            }
            versions.append(item(id: newId(), by: by, at: at, title: title, body: body))
            d["versions"] = .array(versions)
            trim(&d, now: at)
            return true
        }
    }

    /// keep_doc_start: when someone starts changing `doc`, whose text was
    /// `title` and `body`, and no version has that text, keep it (by
    /// nobody known) as of `at`, so the change can be undone. An empty doc
    /// needs none.
    @discardableResult
    public static func keepStart(_ data: inout JSON, doc: UInt64, title: String, body: String, at: Int64, newId: () -> UInt64) -> Bool {
        guard !body.trimmed.isEmpty else { return false }
        return data.tryUpdateItem(at: "docs", id: doc) { d in
            if let l = newestRaw(d).first, l["title"]?.string == title, l["body"]?.string == body { return false }
            var versions = d["versions"]?.array ?? []
            versions.append(item(id: newId(), by: 0, at: at, title: title, body: body))
            d["versions"] = .array(versions)
            trim(&d, now: at)
            return true
        }
    }

    /// Done in the editor: the doc's text becomes `title` and `body`, the
    /// text it had is kept first when no version has it, and the new text
    /// is kept as `by`'s version. Returns false when nothing changed.
    @discardableResult
    public static func save(_ data: inout JSON, doc: UInt64, title: String, body: String, by: UInt64, now: Int64, newId: () -> UInt64) -> Bool {
        guard let d = (data["docs"]?.array ?? []).first(where: { $0["id"]?.uint == doc }) else { return false }
        let (t0, b0) = (d["title"]?.string ?? "", d["body"]?.string ?? "")
        guard t0 != title || b0 != body else { return false }
        keepStart(&data, doc: doc, title: t0, body: b0, at: now, newId: newId)
        if title != t0 { Edits.setDocTitle(&data, doc: doc, to: title, ts: now) }
        if body != b0 { Edits.setDocBody(&data, doc: doc, to: body, ts: now) }
        keepText(&data, doc: doc, by: by, at: now, title: title, body: body, newId: newId)
        return true
    }

    /// restore_doc_version: the doc's text becomes version `version`'s, kept
    /// as a new version by `by` (so a restore can be undone the same way).
    /// Text no version has is kept first. False when there is no such
    /// version or the doc already reads the same.
    @discardableResult
    public static func restore(_ data: inout JSON, doc: UInt64, version: UInt64, by: UInt64, now: Int64, newId: () -> UInt64) -> Bool {
        guard let d = (data["docs"]?.array ?? []).first(where: { $0["id"]?.uint == doc }),
              let v = (d["versions"]?.array ?? []).first(where: { $0["id"]?.uint == version }) else { return false }
        let (t, b) = (v["title"]?.string ?? "", v["body"]?.string ?? "")
        let (t0, b0) = (d["title"]?.string ?? "", d["body"]?.string ?? "")
        guard t != t0 || b != b0 else { return false }
        keepStart(&data, doc: doc, title: t0, body: b0, at: now, newId: newId)
        let id = newId()
        return data.updateItem(at: "docs", id: doc) { d in
            d["title"] = .string(t)
            d["body"] = .string(b)
            d["updated"] = .int(now)
            var versions = d["versions"]?.array ?? []
            versions.append(item(id: id, by: by, at: now, title: t, body: b, restored: v["at"]?.int ?? 0))
            d["versions"] = .array(versions)
            trim(&d, now: now)
        }
    }
}

extension SyncClient {
    /// Done in the doc editor: the new text, with the doc's versions kept
    /// (`DocVersions.save`). False when editing isn't allowed.
    @discardableResult
    public func saveDoc(_ doc: UInt64, title: String, body: String) -> Bool {
        edit { data, me in
            DocVersions.save(&data, doc: doc, title: title, body: body, by: me, now: nowTs(), newId: { self.newId() })
        }
    }

    /// Restore this version (`DocVersions.restore`).
    @discardableResult
    public func restoreDocVersion(_ doc: UInt64, version: UInt64) -> Bool {
        var done = false
        let ok = edit { data, me in
            done = DocVersions.restore(&data, doc: doc, version: version, by: me, now: nowTs(), newId: { self.newId() })
        }
        return ok && done
    }
}
