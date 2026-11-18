import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Spaces, folders, lists and statuses, changed the way the Mac app does.
final class SpaceEditTests: XCTestCase {
    let spaceRequired = ["id", "name", "color", "statuses", "folders", "lists"]
    func space(_ d: JSON, _ id: UInt64) -> JSON { d["spaces"]!.array!.first { $0["id"]?.uint == id }! }

    func testNewSpaceHasDefaultStatusesAndAList() {
        var d = SampleWorkspace.make()
        Edits.createSpace(&d, name: " Ops ", color: [16, 144, 224], id: 700, listId: 701)
        let s = space(d, 700)
        for k in spaceRequired { XCTAssertNotNil(s[k], k) }
        XCTAssertEqual(s["name"], "Ops")
        XCTAssertEqual(s["statuses"]!.array!.map { $0["name"]! }, ["to do", "in progress", "complete"])
        XCTAssertEqual(s["lists"]![0]!["name"], "List")
        XCTAssertNotNil(WorkspaceView(d).list(701))
    }

    func testListsAndFoldersAreCreatedRenamedAndArchived() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.createFolder(&d, space: 100, name: "Q1", id: 710))
        XCTAssertTrue(Edits.createList(&d, space: 100, folder: 710, name: "Ideas", id: 711))
        XCTAssertTrue(Edits.createList(&d, space: 100, folder: nil, name: "", id: 712))
        var v = WorkspaceView(d)
        XCTAssertEqual(v.spaces[0].folders.last?.lists.map(\.name), ["Ideas"])
        XCTAssertEqual(v.list(712)?.name, "List")
        XCTAssertTrue(Edits.renameList(&d, list: 711, to: "Big ideas"))
        XCTAssertTrue(Edits.renameList(&d, list: 112, to: "Mobile"))
        XCTAssertFalse(Edits.renameList(&d, list: 711, to: " "))
        XCTAssertTrue(Edits.renameFolder(&d, folder: 710, to: "Q2"))
        v = WorkspaceView(d)
        XCTAssertEqual(v.list(711)?.name, "Big ideas")
        XCTAssertEqual(v.list(112)?.name, "Mobile")
        XCTAssertEqual(v.spaces[0].folders.last?.name, "Q2")
        XCTAssertTrue(Edits.setListArchived(&d, list: 712, true))
        XCTAssertTrue(Edits.setFolderArchived(&d, folder: 710, true))
        v = WorkspaceView(d)
        XCTAssertNil(v.list(712))
        XCTAssertNil(v.list(711), "an archived folder hides its lists")
        XCTAssertTrue(Edits.setFolderArchived(&d, folder: 710, false))
        XCTAssertNil(space(d, 100)["folders"]!.array!.last!["archived"], "left out when not archived, as the Mac writes it")
    }

    func testSpaceRenameColourAndArchive() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.renameSpace(&d, space: 100, to: "Make"))
        XCTAssertTrue(Edits.setSpaceColor(&d, space: 100, to: [1, 2, 3]))
        XCTAssertEqual(WorkspaceView(d).spaces[0].name, "Make")
        XCTAssertEqual(WorkspaceView(d).spaces[0].color, [1, 2, 3])
        XCTAssertTrue(Edits.setSpaceArchived(&d, space: 100, true))
        XCTAssertTrue(WorkspaceView(d).spaces.isEmpty)
    }

    func testRenamingAStatusTakesItsTasksAndAutomations() {
        var d = SampleWorkspace.make()
        d.set("extra.automations", [
            ["id": 1, "enabled": true, "trigger_status": "review", "action": "priority", "value": "1", "list": 112],
            ["id": 2, "enabled": true, "trigger_status": "review", "action": "priority", "value": "1", "list": nil],
        ])
        XCTAssertFalse(Edits.renameStatus(&d, space: 100, index: 2, to: "to do", ts: 5), "taken")
        XCTAssertFalse(Edits.renameStatus(&d, space: 100, index: 2, to: " ", ts: 5))
        XCTAssertTrue(Edits.renameStatus(&d, space: 100, index: 2, to: "qa", ts: 5))
        XCTAssertEqual(task(d, 203)["status"], "qa")
        XCTAssertEqual(d.at("extra.automations")!.array!.map { $0["trigger_status"]! }, ["qa", "qa"])
    }

    func testAWorkspaceWideAutomationStaysWhenAnotherSpaceHasTheName() {
        var d = SampleWorkspace.make()
        Edits.createSpace(&d, name: "Other", color: [1, 1, 1], id: 700, listId: 701)
        Edits.renameStatus(&d, space: 700, index: 1, to: "review", ts: 1)
        d.set("extra.automations", [["id": 2, "enabled": true, "trigger_status": "review", "action": "priority", "value": "1", "list": nil]])
        XCTAssertTrue(Edits.renameStatus(&d, space: 100, index: 2, to: "qa", ts: 5))
        XCTAssertEqual(d.at("extra.automations")![0]!["trigger_status"], "review")
    }

    func testAddedStatusGoesBeforeTheClosedOnesWithAFreeName() {
        var d = SampleWorkspace.make()
        XCTAssertEqual(Edits.addStatus(&d, space: 100), 3)
        XCTAssertEqual(Edits.addStatus(&d, space: 100), 4)
        let names = WorkspaceView(d).spaces[0].statuses.map(\.name)
        XCTAssertEqual(names, ["to do", "in progress", "review", "new status", "new status 2", "complete"])
        XCTAssertTrue(Edits.setStatusKind(&d, space: 100, index: 3, to: .done))
        XCTAssertTrue(Edits.setStatusColor(&d, space: 100, index: 3, to: [9, 9, 9]))
        XCTAssertEqual(WorkspaceView(d).spaces[0].statuses[3].kind, .done)
        XCTAssertEqual(WorkspaceView(d).spaces[0].statuses[3].color, [9, 9, 9])
    }

    func testADeletedStatusMovesItsTasksToOneOfTheSameKind() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.deleteStatus(&d, space: 100, index: 2, me: 1, ts: 9)) // review (custom)
        XCTAssertEqual(task(d, 203)["status"], "in progress")
        XCTAssertEqual(task(d, 203)["activity"]!.array!.last!["text"], "changed status from REVIEW to IN PROGRESS")
        XCTAssertEqual(WorkspaceView(d).spaces[0].statuses.map(\.name), ["to do", "in progress", "complete"])
    }

    func testAStatusesListFromTheDefaultsIsWrittenOut() {
        var d = SampleWorkspace.make()
        d.updateItem(at: "spaces", id: 100) { $0["statuses"] = [] }
        XCTAssertTrue(Edits.renameStatus(&d, space: 100, index: 1, to: "doing", ts: 1))
        XCTAssertEqual(space(d, 100)["statuses"]!.array!.map { $0["name"]! }, ["to do", "doing", "complete"])
    }
}
