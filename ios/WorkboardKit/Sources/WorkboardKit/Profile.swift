// Edit profile and Set status, the way the Mac app writes them
// (src/app/profile.rs, set_status_body in src/app/menus/workspace.rs): a
// member's full name, display name, title, photo and status. Only the fields
// that change are written: members merge field by field, so a change another
// device made meanwhile to the rest still counts.

import Foundation

extension Edits {
    /// The name a profile photo goes under (as the Mac names it).
    public static let profilePhotoName = "Profile photo.png"

    /// Save Edit profile for `member`: the names and title trimmed, and the
    /// photo (an Attachment, as FileAPI.attachment makes it) or nil for none.
    /// False without a full name or when the member isn't there.
    @discardableResult
    public static func setProfile(_ data: inout JSON, member: UInt64, name: String, displayName: String, title: String, photo: JSON?) -> Bool {
        let full = name.trimmed
        guard !full.isEmpty else { return false }
        return data.updateItem(at: "members", id: member) { m in
            func put(_ key: String, _ value: String) {
                if (m[key]?.string ?? "") != value { m[key] = .string(value) }
            }
            put("name", full)
            put("display_name", displayName.trimmed)
            put("title", title.trimmed)
            // none is null, as serde writes Option::None
            let new = photo ?? .null
            if (m["photo"] ?? .null) != new { m["photo"] = new }
        }
    }

    /// Set status for `member`: a few words, trimmed; empty clears it.
    @discardableResult
    public static func setStatusText(_ data: inout JSON, member: UInt64, to text: String) -> Bool {
        let clean = text.trimmed
        return data.updateItem(at: "members", id: member) { m in
            if (m["status_text"]?.string ?? "") != clean { m["status_text"] = .string(clean) }
        }
    }
}
