import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Edit profile and Set status, written as the Mac writes them
/// (src/app/profile.rs, set_status_body in src/app/menus/workspace.rs).
final class ProfileTests: XCTestCase {
    func member(_ d: JSON, _ id: UInt64) -> JSON { d["members"]!.array!.first { $0["id"]?.uint == id }! }

    let photo = FileAPI.attachment(sha: String(repeating: "a", count: 64), name: Edits.profilePhotoName, isVideo: false, bytes: 1234, w: 512, h: 512, secs: 0)

    func testEditProfileWritesTheNamesTitleAndPhoto() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.setProfile(&d, member: 2, name: "  Alex Kim ", displayName: " Alex ", title: " Designer ", photo: photo))
        let m = member(d, 2)
        XCTAssertEqual(m["name"], "Alex Kim")
        XCTAssertEqual(m["display_name"], "Alex")
        XCTAssertEqual(m["title"], "Designer")
        XCTAssertEqual(m["photo"], photo)
        // others see the display name, with the photo and title
        let v = WorkspaceView(d).member(2)!
        XCTAssertEqual(v.name, "Alex")
        XCTAssertEqual(v.fullName, "Alex Kim")
        XCTAssertEqual(v.title, "Designer")
        XCTAssertEqual(v.initials, "A")
        XCTAssertEqual(v.photo?.file, String(repeating: "a", count: 64))
        XCTAssertEqual(v.photo?.name, "Profile photo.png")
        // Remove photo writes none the way serde writes Option::None
        XCTAssertTrue(Edits.setProfile(&d, member: 2, name: "Alex Kim", displayName: "", title: "Designer", photo: nil))
        XCTAssertEqual(member(d, 2)["photo"], .null)
        XCTAssertNil(WorkspaceView(d).member(2)!.photo)
        XCTAssertEqual(WorkspaceView(d).member(2)!.name, "Alex Kim", "no display name: the full name")
    }

    func testEditProfileNeedsAFullNameAndTheMember() {
        var d = SampleWorkspace.make()
        let before = d
        XCTAssertFalse(Edits.setProfile(&d, member: 2, name: "   ", displayName: "A", title: "", photo: nil))
        XCTAssertFalse(Edits.setProfile(&d, member: 99, name: "Nobody", displayName: "", title: "", photo: nil))
        XCTAssertEqual(d, before)
    }

    /// Saving without a change leaves every field as it was, so it counts as
    /// no edit at all (a missing field stays missing).
    func testSavingWithoutChangesWritesNothing() {
        var d = SampleWorkspace.make()
        let before = d
        let m = WorkspaceView(d).member(3)!
        XCTAssertTrue(Edits.setProfile(&d, member: 3, name: m.fullName, displayName: m.displayName, title: m.title, photo: nil))
        XCTAssertEqual(d, before)
        XCTAssertNil(member(d, 3)["display_name"])
        XCTAssertNil(member(d, 3)["photo"])
    }

    /// Only the fields that changed get a stamp: a rename made meanwhile on
    /// another device isn't undone by a new title from here.
    func testOnlyTheChangedFieldsAreStamped() {
        var d = SampleWorkspace.make()
        var meta = SyncMeta()
        Sync.stamp(d, &meta, now: 1_000)
        // field stamps, without "" (the stamp the others are as of)
        let stamped = { (meta.fields[2] ?? [:]).filter { !$0.key.isEmpty } }
        XCTAssertEqual(stamped(), [:])
        let m = WorkspaceView(d).member(2)!
        Edits.setProfile(&d, member: 2, name: m.fullName, displayName: "", title: "Engineer", photo: nil)
        Sync.stamp(d, &meta, now: 2_000)
        XCTAssertEqual(stamped(), ["title": 2_000])
        Edits.setStatusText(&d, member: 2, to: "🏠 Working remotely")
        Sync.stamp(d, &meta, now: 3_000)
        XCTAssertEqual(meta.fields[2]?["status_text"], 3_000)
        XCTAssertEqual(meta.fields[2]?["title"], 2_000)
    }

    func testSetStatusAndClearIt() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.setStatusText(&d, member: 1, to: "  📅 In a meeting \n"))
        XCTAssertEqual(member(d, 1)["status_text"], "📅 In a meeting")
        XCTAssertEqual(WorkspaceView(d).member(1)!.statusText, "📅 In a meeting")
        XCTAssertTrue(Edits.setStatusText(&d, member: 1, to: ""))
        XCTAssertEqual(member(d, 1)["status_text"], "")
        XCTAssertEqual(WorkspaceView(d).member(1)!.statusText, "")
        XCTAssertFalse(Edits.setStatusText(&d, member: 99, to: "Away"))
    }

    /// A photo the Mac wrote reads back; one that doesn't name a file on
    /// the server (an older or broken copy) shows initials instead.
    func testAPhotoMustNameAFile() {
        var d = SampleWorkspace.make()
        d.updateItem(at: "members", id: 2) { $0["photo"] = ["file": "../secret", "name": "x.png", "kind": "Image"] }
        XCTAssertNil(WorkspaceView(d).member(2)!.photo)
        d.updateItem(at: "members", id: 2) { $0["photo"] = "not an attachment" }
        XCTAssertNil(WorkspaceView(d).member(2)!.photo)
        XCTAssertEqual(MemberView.initials(of: "Jamie Park"), "JP")
        XCTAssertEqual(MemberView.initials(of: ""), "?")
    }
}
