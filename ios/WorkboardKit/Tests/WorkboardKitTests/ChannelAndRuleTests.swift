import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

final class ChannelAndRuleTests: XCTestCase {
    func channel(_ d: JSON, _ id: UInt64) -> JSON { d["channels"]!.array!.first { $0["id"]?.uint == id }! }

    func testWhoMayChangeAChannelsPeople() {
        var d = SampleWorkspace.make()
        Edits.createChannel(&d, name: "Plans", isPrivate: true, me: 2, id: 900, ts: 1)
        XCTAssertTrue(Edits.mayManageChannel(d, channel: 900, me: 2, role: "member"), "its maker")
        XCTAssertFalse(Edits.mayManageChannel(d, channel: 900, me: 3, role: "member"))
        XCTAssertTrue(Edits.mayManageChannel(d, channel: 900, me: 3, role: "admin"))
        // #general has no maker: anyone in it
        XCTAssertTrue(Edits.mayManageChannel(d, channel: 301, me: 3, role: nil))
    }

    func testAPrivateChannelWithChosenPeopleIsAGroupConversation() {
        var d = SampleWorkspace.make()
        Edits.createChannel(&d, name: "Launch crew", isPrivate: true, me: 1, id: 900, ts: 1)
        XCTAssertTrue(Edits.setChannelMember(&d, channel: 900, member: 2, true))
        XCTAssertTrue(Edits.setChannelMember(&d, channel: 900, member: 3, true))
        XCTAssertFalse(Edits.setChannelMember(&d, channel: 900, member: 3, true))
        XCTAssertEqual(channel(d, 900)["members"], [1, 2, 3])
        XCTAssertTrue(WorkspaceView(d).visibleChannels(me: 3).contains { $0.id == 900 })
        XCTAssertTrue(Edits.setChannelMember(&d, channel: 900, member: 3, false))
        XCTAssertFalse(WorkspaceView(d).visibleChannels(me: 3).contains { $0.id == 900 })
    }

    func testChannelNameAndTopic() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.renameChannel(&d, channel: 302, to: "Brand Design"))
        XCTAssertEqual(channel(d, 302)["name"], "brand-design")
        XCTAssertFalse(Edits.renameChannel(&d, channel: 302, to: "  "))
        XCTAssertTrue(Edits.setChannelTopic(&d, channel: 302, to: " Logos "))
        XCTAssertEqual(channel(d, 302)["topic"], "Logos")
    }

    func testAutomationsAreAddedTurnedOffAndRemovedAndRun() {
        var d = SampleWorkspace.make()
        XCTAssertFalse(Edits.addAutomation(&d, trigger: "review", action: "priority", value: " ", list: nil, id: 1))
        XCTAssertTrue(Edits.addAutomation(&d, trigger: "review", action: "priority", value: "1", list: 112, id: 50))
        XCTAssertTrue(Edits.setStatus(&d, task: 204, to: "review", me: 1, ts: 5))
        XCTAssertEqual(d["tasks"]!.array!.first { $0["id"] == 204 }!["priority"], 1)
        XCTAssertTrue(Edits.setAutomationEnabled(&d, automation: 50, false))
        Edits.setPriority(&d, task: 205, to: 4, me: 1, ts: 6)
        Edits.moveTasks(&d, ids: [205], to: 112, me: 1, ts: 6)
        Edits.setStatus(&d, task: 205, to: "review", me: 1, ts: 7)
        XCTAssertEqual(d["tasks"]!.array!.first { $0["id"] == 205 }!["priority"], 4, "a turned-off rule doesn't run")
        XCTAssertTrue(Edits.deleteAutomation(&d, automation: 50))
        XCTAssertEqual(d.at("extra.automations"), [])
    }
}

final class DocPageTests: XCTestCase {
    func testSubPagesFollowTheirParent() {
        var d = SampleWorkspace.make()
        Edits.setDocPrivate(&d, doc: 401, true, ts: 1)
        XCTAssertTrue(Edits.addPage(&d, parent: 401, me: 1, id: 900, ts: 2))
        XCTAssertTrue(Edits.addPage(&d, parent: 401, me: 1, id: 901, ts: 3))
        let page = d["docs"]!.array!.first { $0["id"] == 901 }!
        XCTAssertEqual(page["title"], "Page 2")
        XCTAssertEqual(page["parent"], 401)
        XCTAssertEqual(page["private"], true)
        XCTAssertEqual(page["space"], 100)
        XCTAssertTrue(Edits.setDocArchived(&d, doc: 402, true, ts: 4))
        XCTAssertFalse(WorkspaceView(d).docs.contains { $0.id == 402 })
    }
}

final class MessageExtrasTests: XCTestCase {
    func testSaveTaskPostAndRemind() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.toggleSaved(&d, channel: 301, message: 312)) // a reply
        XCTAssertEqual(WorkspaceView(d).savedMessages(d, me: 1).map(\.message.id), [312])
        XCTAssertTrue(Edits.messageToTask(&d, channel: 301, message: 313, list: 120, me: 1, id: 9000, ts: 5))
        let t = d["tasks"]!.array!.first { $0["id"] == 9000 }!
        XCTAssertEqual(t["name"], "Can someone review the pricing copy?")
        XCTAssertEqual(t["assignees"], [1])
        XCTAssertEqual(d["channels"]![0]!["messages"]!.array!.first { $0["id"] == 313 }!["task"], 9000)
        XCTAssertTrue(Edits.sendPost(&d, channel: 302, title: "Launch day", text: "We ship Friday", me: 1, id: 9001, ts: 6))
        let posts = WorkspaceView(d).posts(me: 1)
        XCTAssertEqual(posts.first?.message.postTitle, "Launch day")
        XCTAssertTrue(Edits.remindAboutMessage(&d, channel: 301, message: 313, at: 100, me: 1, id: 9002))
        XCTAssertEqual(d.at("extra.reminders")![0]!["message"], [301, 313])
    }
}
