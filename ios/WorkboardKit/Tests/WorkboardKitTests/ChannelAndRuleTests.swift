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

    /// The screens read automations and workflows through AutomationView.
    func testAutomationsAndWorkflowsReadIntoTheirFields() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addAutomation(&d, trigger: "review", action: "assign", value: "2", list: 112, id: 50))
        XCTAssertTrue(Edits.addWorkflow(&d, channel: 301, keyword: " deploy ", action: "post", text: "{user} said {text}", target: 302, list: nil, id: 51))
        XCTAssertTrue(Edits.addWorkflow(&d, channel: 301, keyword: "", action: "task", text: "", target: nil, list: 111, id: 52))
        XCTAssertTrue(Edits.addWorkflow(&d, channel: 302, keyword: "", action: "reply", text: "Thanks!", target: nil, list: nil, id: 53))
        let v = WorkspaceView(d)
        XCTAssertEqual(v.automations.map(\.id), [50, 51, 52, 53])
        let rule = v.automations[0]
        XCTAssertFalse(rule.isWorkflow)
        XCTAssertTrue(rule.enabled)
        XCTAssertEqual([rule.triggerStatus, rule.action, rule.value], ["review", "assign", "2"])
        XCTAssertEqual(rule.list, 112)
        XCTAssertNil(rule.channel)

        XCTAssertEqual(v.workflows(in: 301).map(\.id), [51, 52])
        XCTAssertEqual(v.workflows(in: 302).map(\.id), [53])
        XCTAssertEqual(v.workflows(in: 303), [])
        let post = v.automations[1]
        XCTAssertTrue(post.isWorkflow)
        XCTAssertEqual([post.action, post.keyword, post.value, post.triggerStatus], ["post", "deploy", "{user} said {text}", ""])
        XCTAssertEqual(post.target, 302)
        XCTAssertNil(post.list)
        XCTAssertEqual(v.automations[2].value, "111")
        XCTAssertEqual(v.automations[2].keyword, "")
        XCTAssertNil(v.automations[2].target)

        // fields left out read as their defaults (an older file)
        let bare = AutomationView(["id": 9])
        XCTAssertFalse(bare.enabled)
        XCTAssertEqual([bare.action, bare.value, bare.trigger, bare.keyword], ["", "", "", ""])
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
        XCTAssertFalse(posts.first!.message.hasDefaultPostTitle, "a typed title is shown as typed")
        // the desktop saves "Update" for a post without a title; screens translate it
        XCTAssertTrue(Edits.sendPost(&d, channel: 302, title: MessageView.defaultPostTitle, text: "Doors at 9", me: 1, id: 9003, ts: 7))
        XCTAssertTrue(WorkspaceView(d).posts(me: 1).contains { $0.message.id == 9003 && $0.message.hasDefaultPostTitle })
        // only text written: the post gets that default title too, as on the Mac
        XCTAssertTrue(Edits.sendPost(&d, channel: 302, title: "  ", text: "Coffee is on", me: 1, id: 9004, ts: 8))
        XCTAssertTrue(WorkspaceView(d).posts(me: 1).contains { $0.message.id == 9004 && $0.message.hasDefaultPostTitle })
        // a post in a direct message I'm in is listed, as on the Mac's Posts page; not for others
        XCTAssertTrue(Edits.sendPost(&d, channel: 303, title: "Lunch", text: "Pizza", me: 1, id: 9005, ts: 9))
        XCTAssertEqual(WorkspaceView(d).posts(me: 1).first?.message.id, 9005)
        XCTAssertFalse(WorkspaceView(d).posts(me: 3).contains { $0.message.id == 9005 })
        XCTAssertTrue(Edits.remindAboutMessage(&d, channel: 301, message: 313, at: 100, me: 1, id: 9002))
        XCTAssertEqual(d.at("extra.reminders")![0]!["message"], [301, 313])
    }
}
