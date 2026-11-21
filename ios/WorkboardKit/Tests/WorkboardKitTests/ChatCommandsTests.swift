import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Slash commands, webhook addresses and workflows, as the Mac app does them.
final class ChatCommandsTests: XCTestCase {
    let deploy = CommandItem(["name": "deploy", "description": "Ship it"])!

    func testSlashCommandsAreReadLikeTheMac() {
        typealias P = SlashCommands.Parsed
        XCTAssertEqual(SlashCommands.parse("/shrug oh well", commands: []), P.shrug("oh well ¯\\_(ツ)_/¯"))
        XCTAssertEqual(SlashCommands.parse("/me waves", commands: []), P.me("*waves*"))
        XCTAssertEqual(SlashCommands.parse("/topic Launch week", commands: []), P.topic("Launch week"))
        XCTAssertEqual(SlashCommands.parse("/MUTE", commands: []), P.mute)
        XCTAssertEqual(SlashCommands.parse("/remind me to call Sam in 30 minutes", commands: []), P.remind("to call Sam", seconds: 1800))
        XCTAssertEqual(SlashCommands.parse("/remind sometime", commands: []), P.remindHelp)
        XCTAssertEqual(SlashCommands.parse("/deploy v2", commands: [deploy]), P.server(name: "deploy", text: "v2", known: true))
        XCTAssertEqual(SlashCommands.parse("/nope", commands: [deploy]), P.server(name: "nope", text: "", known: false))
        XCTAssertEqual(SlashCommands.parse("/usr/bin is full", commands: []), P.plain)
        XCTAssertEqual(SlashCommands.parse("hello", commands: []), P.plain)
        XCTAssertEqual(SlashCommands.choices("/de", commands: [deploy]).map(\.0), ["deploy"])
        XCTAssertEqual(SlashCommands.choices("/", commands: [deploy]).count, 7)
        XCTAssertTrue(SlashCommands.choices("/deploy v2", commands: [deploy]).isEmpty)
    }

    func testWebhookAddressesAndTheWire() {
        XCTAssertEqual(ServerAddress.httpAddress("https://wb.example.com/w/ab12", path: "/hooks/t1"), "https://wb.example.com/w/ab12/hooks/t1")
        XCTAssertEqual(ServerAddress.httpAddress("macmini", path: "/hooks/t1"), "http://macmini:8787/hooks/t1")
        for m: ClientMessage in [.integrations, .hookAdd(hook: ["kind": "in", "channel": 301, "name": "CI"]), .hookRemove(token: "t"),
                                 .commandAdd(command: ["name": "deploy", "url": "https://x"]), .commandRemove(name: "deploy"),
                                 .command(channel: 301, parent: nil, name: "deploy", text: "v2"), .command(channel: 301, parent: 5, name: "d", text: "")] {
            XCTAssertEqual(ClientMessage(text: m.text), m)
        }
        XCTAssertEqual(ClientMessage.command(channel: 301, parent: nil, name: "deploy", text: "v2").json,
                       ["type": "command", "channel": 301, "name": "deploy", "text": "v2"])
        let list = ServerMessage.integrationList(hooks: [["token": "t", "kind": "in", "channel": 301, "name": "CI"]], commands: [["name": "deploy"]])
        XCTAssertEqual(ServerMessage(text: list.text), list)
        XCTAssertEqual(list.json["type"], "integration_list")
    }

    func testWorkflowsPostReplyAndMakeTasksOnce() {
        var d = SampleWorkspace.make()
        let list = WorkspaceView(d).spaces.flatMap(\.allListIds).first!
        XCTAssertTrue(Edits.addWorkflow(&d, channel: 301, keyword: "outage", action: "post", text: "{user} reported: {text}", target: 302, list: nil, id: 980))
        XCTAssertTrue(Edits.addWorkflow(&d, channel: 301, keyword: "", action: "reply", text: "Thanks, {user}!", target: nil, list: nil, id: 981))
        XCTAssertTrue(Edits.addWorkflow(&d, channel: 301, keyword: "bug", action: "task", text: "", target: nil, list: list, id: 982))
        XCTAssertFalse(Edits.addWorkflow(&d, channel: 301, keyword: "", action: "post", text: "", target: 302, list: nil, id: 983), "a post needs text")
        var next: UInt64 = 5000
        Edits.sendMessage(&d, channel: 301, text: "Big **outage** in the bug tracker", me: 1, id: 990, ts: 10)
        Edits.runWorkflows(&d, channel: 301, message: 990, me: 1, ts: 10) { next += 1; return next }
        let v = WorkspaceView(d)
        let posted = v.channels.first { $0.id == 302 }!.messages.last!
        XCTAssertEqual(posted.text, "Jamie Park reported: Big outage in the bug tracker")
        XCTAssertEqual(posted.bot, "Workflow")
        let mine = v.channels.first { $0.id == 301 }!.messages.first { $0.id == 990 }!
        XCTAssertEqual(mine.replies.map(\.text), ["Thanks, Jamie Park!"])
        XCTAssertTrue(v.tasks.contains { $0.name == "Big outage in the bug tracker" })
        // a workflow's own message starts nothing
        let before = d
        Edits.runWorkflows(&d, channel: 302, message: posted.id, me: 1, ts: 11) { next += 1; return next }
        XCTAssertEqual(d, before)
    }

    func testAPostStartsWorkflowsByItsTitleAsOnTheMac() {
        var d = SampleWorkspace.make()
        XCTAssertTrue(Edits.addWorkflow(&d, channel: 301, keyword: "outage", action: "post", text: "{user} posted: {text}", target: 302, list: nil, id: 980))
        var next: UInt64 = 5000
        // the keyword in the title; {text} is the post's text
        XCTAssertTrue(Edits.sendPost(&d, channel: 301, title: "Outage tonight", text: "DB at 10", me: 1, id: 990, ts: 10))
        Edits.runWorkflows(&d, channel: 301, message: 990, me: 1, ts: 10) { next += 1; return next }
        XCTAssertEqual(WorkspaceView(d).channels.first { $0.id == 302 }!.messages.last!.text, "Jamie Park posted: DB at 10")
        // no text: the title
        XCTAssertTrue(Edits.sendPost(&d, channel: 301, title: "Outage over", text: "", me: 1, id: 991, ts: 11))
        Edits.runWorkflows(&d, channel: 301, message: 991, me: 1, ts: 11) { next += 1; return next }
        XCTAssertEqual(WorkspaceView(d).channels.first { $0.id == 302 }!.messages.last!.text, "Jamie Park posted: Outage over")
    }
}
