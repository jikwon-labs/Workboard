import XCTest
@testable import WorkboardKit

/// The messages in Protocol.swift are a hand copy of wb_net::Msg. Rust writes
/// one file per message this app sends or reads (crates/wb-net/tests/protocol.rs
/// into crates/wb-net/tests/protocol); these tests read them, so a field added,
/// renamed or dropped on either side fails here or there.
final class ProtocolGoldenTests: XCTestCase {
    var repo: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()  // WorkboardKitTests
            .deletingLastPathComponent()  // Tests
            .deletingLastPathComponent()  // WorkboardKit
            .deletingLastPathComponent()  // ios
            .deletingLastPathComponent()  // repo
    }

    /// name -> (file text, parsed) for every file in one folder.
    func files(_ folder: String) throws -> [String: (String, JSON)] {
        let dir = repo.appendingPathComponent("crates/wb-net/tests/protocol/\(folder)")
        let names = try FileManager.default.contentsOfDirectory(atPath: dir.path).filter { $0.hasSuffix(".json") }
        XCTAssertFalse(names.isEmpty, "no files in \(dir.path)")
        var out: [String: (String, JSON)] = [:]
        for n in names {
            let text = try String(contentsOf: dir.appendingPathComponent(n), encoding: .utf8)
            out[String(n.dropLast(5))] = (text, try JSON(parsing: text))
        }
        return out
    }

    func testEveryMessageToTheServerReadsAndWritesBackTheSame() throws {
        for (name, (text, want)) in try files("to_server") {
            guard let m = ClientMessage(text: text) else {
                XCTFail("\(name): ClientMessage can't read it")
                continue
            }
            XCTAssertEqual(m.json.serialized(), want.serialized(), name)
        }
    }

    func testEveryMessageFromTheServerReadsAndWritesBackTheSame() throws {
        for (name, (text, want)) in try files("to_app") {
            let m = ServerMessage(text: text)
            if case .unknown = m { XCTFail("\(name): ServerMessage can't read it") }
            XCTAssertEqual(m.json.serialized(), want.serialized(), name)
        }
    }

    /// What this app builds is what the server's own type writes, field for
    /// field. Every file in to_server needs a line here, so a message the
    /// server learns shows up as missing.
    func testMessagesThisAppBuildsMatchTheServer() throws {
        func hello(_ f: (inout Hello) -> Void) -> ClientMessage {
            var h = Hello(device: "dev-1")
            f(&h)
            return .hello(h)
        }
        var prefs = NotifyPrefs()
        prefs.mentions = false
        prefs.channelDefault = "all"
        prefs.channelModes = [301: "nothing"]
        prefs.keywords = "launch, deploy"
        prefs.mutedUntil = 1763009999
        let item = ScheduledItem(id: 1763000000123, channel: 301, parent: 5001, text: "Standup in 5 @Sam", at: 1763003600, mentions: [3])
        let built: [String: ClientMessage] = [
            "hello_token": hello {
                $0.tag = 5; $0.token = "tok-123"; $0.app = "0.1.23"; $0.sync = 3; $0.deltas = true
            },
            "hello_reconnect": hello {
                $0.tag = 5; $0.token = "tok-123"; $0.app = "0.1.23"; $0.sync = 5; $0.deltas = true
                $0.have = Have(epoch: 4_503_599_627_370_497, rev: 12); $0.mineParts = true
            },
            "hello_password": hello { $0.email = "sam@example.com"; $0.password = "secret-pass" },
            "hello_invite": hello { $0.invite = "ABCD-EFGH-JKLM"; $0.email = "sam@example.com"; $0.newPassword = "new-secret" },
            "hello_reset": hello { $0.resetCode = "RSET-CODE-0001"; $0.email = "sam@example.com"; $0.newPassword = "new-secret" },
            "hello_link": hello { $0.linkCode = "K3P9QX" },
            "push": .push(data: ["channels": [["id": 301, "name": "general"]], "next_id": 7]),
            "resync": .resync,
            "set_password": .setPassword(email: "sam@example.com", password: "new-secret", oldPassword: "old-secret"),
            "set_password_first": .setPassword(email: "sam@example.com", password: "new-secret", oldPassword: nil),
            "push_token": .pushToken(token: "apns-token", platform: "ios", env: "production", workspace: "macmini:8787", prefs: prefs.forServer),
            "push_token_stop": .pushToken(token: "", platform: "ios", env: "development"),
            "call_join": .callJoin(channel: 301),
            "typing": .typing(channel: 301, thread: 5001),
            "create_invite": .createInvite(member: 2, role: .admin),
            "set_role": .setRole(member: 2, role: .owner),
            "revoke": .revoke(member: 2),
            "access_list": .accessList,
            "backup_list": .backupList,
            "restore": .restore(name: "hourly-20261115-03.json"),
            "schedule": .schedule(item: item.json),
            "unschedule": .unschedule(id: 1763000000123),
            "scheduled_list": .scheduledList,
            "mine": .mine(data: ["read": ["301": 5001]]),
            "integrations": .integrations,
            // the server fills in a new hook's token and creator
            "hook_add": .hookAdd(hook: {
                var h = HookItem.request(kind: "outgoing", channel: 301, name: " Deploy bot ", url: "https://example.com/hook", trigger: "deploy")
                h["token"] = "hk_abc"
                h["creator"] = 2
                return h
            }()),
            "hook_remove": .hookRemove(token: "hk_abc"),
            "command_add": .commandAdd(command: CommandItem.request(name: "weather", url: "https://example.com/weather", description: "Today's weather")),
            "command_remove": .commandRemove(name: "weather"),
            "command": .command(channel: 301, parent: 5001, name: "weather", text: "Seoul"),
            "ping": .ping,
            "pong": .pong,
        ]
        let golden = try files("to_server")
        XCTAssertEqual(Set(built.keys), Set(golden.keys))
        for (name, m) in built {
            guard let (_, want) = golden[name] else { continue }
            XCTAssertEqual(m.json.serialized(), want.serialized(), name)
        }
    }

    /// The typed views of what the server sends read every field.
    func testServerMessagesReadIntoTheirFields() throws {
        let g = try files("to_app")
        func msg(_ n: String) -> ServerMessage { ServerMessage(text: g[n]?.0 ?? "") }
        guard case let .welcome(w) = msg("welcome") else { return XCTFail("welcome") }
        XCTAssertEqual(w.tag, 5)
        XCTAssertEqual(w.rev, 12)
        XCTAssertEqual(w.member, 2)
        XCTAssertEqual(w.role, "admin")
        XCTAssertEqual(w.token, "tok-123")
        XCTAssertEqual(w.email, "sam@example.com")
        XCTAssertEqual(w.hidden, [9001])
        XCTAssertTrue(w.deltas)
        XCTAssertNotNil(w.snapshot?["channels"])
        XCTAssertEqual(w.epoch, 4_503_599_627_370_497)
        XCTAssertNil(w.since)
        guard case let .welcome(empty) = msg("welcome_empty") else { return XCTFail("welcome_empty") }
        XCTAssertNil(empty.snapshot)
        XCTAssertNil(empty.epoch)
        guard case let .welcome(caught) = msg("welcome_caught_up") else { return XCTFail("welcome_caught_up") }
        XCTAssertNil(caught.snapshot)
        XCTAssertEqual(caught.since, 12)
        XCTAssertEqual(caught.rev, 15)
        XCTAssertNotNil(caught.part?["channels"])
        guard case let .minePart(part) = msg("mine_part") else { return XCTFail("mine_part") }
        XCTAssertNotNil(part["read"]?["301"])
        XCTAssertEqual(msg("error_gone"), .error(code: "auth", message: WorkspaceAPI.goneMessage, gone: true))
        XCTAssertEqual(msg("error_put_back"), .error(code: "denied", message: "Some changes were turned down.", putBack: true))
        XCTAssertEqual(msg("error_call"), .error(code: "denied", message: "Huddles aren't set up.", channel: 301))
        guard case let .integrationList(hooks, commands) = msg("integration_list") else { return XCTFail("integration_list") }
        let hook = try XCTUnwrap(hooks.first.flatMap(HookItem.init))
        XCTAssertEqual([hook.token, hook.kind, hook.name, hook.url, hook.trigger], ["hk_abc", "outgoing", "Deploy bot", "https://example.com/hook", "deploy"])
        XCTAssertEqual(hook.channel, 301)
        let command = try XCTUnwrap(commands.first.flatMap(CommandItem.init))
        XCTAssertEqual([command.name, command.description, command.url], ["weather", "Today's weather", "https://example.com/weather"])
        guard case let .scheduled(items) = msg("scheduled") else { return XCTFail("scheduled") }
        XCTAssertEqual(items.compactMap(ScheduledItem.init),
                       [ScheduledItem(id: 1763000000123, channel: 301, parent: 5001, text: "Standup in 5 @Sam", at: 1763003600, mentions: [3])])
        guard case let .access(people, role, answer) = msg("access") else { return XCTFail("access") }
        XCTAssertEqual(people, [Person(member: 1, role: .owner, devices: 2, account: true),
                                Person(member: 2, role: .member, devices: 0, invite: "ABCD-EFGH-JKLM")])
        XCTAssertEqual(role, .owner)
        XCTAssertEqual(answer, true)
        XCTAssertEqual(msg("backups"), .backups([Backup(name: "hourly-20261115-03.json", bytes: 123456)]))
        XCTAssertEqual(msg("invite_mail_failed"), .invite(member: 2, code: "ABCD-EFGH-JKLM", mailError: "no mail server"))
        XCTAssertEqual(msg("typing"), .typing(channel: 301, member: 3))
    }

    /// The fixed workspace of crates/wb-sync/tests/golden through the read
    /// models: odd values (a null due time, numbers past i64, a checklist
    /// line without an id, custom values that aren't text) read without
    /// losing the rest.
    func testTheGoldenWorkspaceReadsIntoTheViews() throws {
        let url = repo.appendingPathComponent("crates/wb-sync/tests/golden/workspace.json")
        let v = WorkspaceView(try JSON(parsing: Data(contentsOf: url)))
        XCTAssertEqual(v.name, "Team 팀 \"Q\"")

        XCTAssertEqual(v.members.map(\.name), ["Ji 지원", "Bo"])
        XCTAssertEqual(v.members.map(\.role), ["Owner", "Member"])
        XCTAssertEqual(v.activeMembers.map(\.id), [2])
        XCTAssertEqual(v.member(2)?.email, "ji@example.com")

        let space = try XCTUnwrap(v.spaces.first)
        XCTAssertEqual(space.name, "Engineering")
        XCTAssertEqual(space.color, [123, 104, 238])
        XCTAssertEqual(space.folders.map(\.name), ["Q4 계획"])
        XCTAssertEqual(space.folders.first?.lists.map(\.name), ["Sprint 1"])
        XCTAssertEqual(space.lists.map(\.name), ["Backlog"])
        XCTAssertEqual(space.allListIds, [120, 111])
        XCTAssertEqual(v.list(111)?.name, "Sprint 1")
        XCTAssertEqual(space.statuses, StatusView.defaults)

        XCTAssertEqual(v.tasks.map(\.id), [1001, 1003])
        let t = try XCTUnwrap(v.tasks.first)
        XCTAssertEqual(t.name, "Ship the \\ release\ttoday\nplease")
        XCTAssertEqual(t.status, "in progress")
        XCTAssertEqual(v.status(of: t).name, "in progress")
        XCTAssertEqual(t.priority, 2)
        XCTAssertEqual(t.assignees, [2, 3])
        XCTAssertEqual(t.tags, ["urgent", "백엔드"])
        XCTAssertEqual(t.due, "2026-11-20")
        XCTAssertNil(t.dueTime)
        XCTAssertEqual(t.custom, [7001: "Gold"])
        XCTAssertEqual(t.timeEntries.map(\.member), [2, 3, 2])
        XCTAssertEqual(v.trackedSecs(t, now: 1763100000), 3600 + 1800 + 600)
        XCTAssertEqual(t.timers, [RunningTimerView(member: 2, start: 1763100000), RunningTimerView(member: 3, start: 1763100500)])
        XCTAssertEqual(v.myTimerStart(t, me: 3), 1763100500)
        XCTAssertEqual(t.activity.map(\.text), ["made it"])
        XCTAssertEqual(t.checklist.map(\.text), ["loose item", "write tests"])
        XCTAssertEqual(t.checklist.map(\.done), [false, true])
        XCTAssertEqual(v.myTasks(me: 3).map(\.id), [1001])
        XCTAssertEqual(v.tasks(inList: 120).map(\.name), ["Tiny"])
        XCTAssertEqual(v.allTags, ["urgent", "백엔드"])

        XCTAssertEqual(v.docs.map(\.title), ["Spec"])
        XCTAssertEqual(v.docs.first?.space, 100)

        let c = try XCTUnwrap(v.channels.first)
        XCTAssertEqual(c.kind, .channel)
        XCTAssertEqual(v.title(of: c, me: 2), "general")
        XCTAssertEqual(c.messages.map(\.text), ["hi 👋", ""])
        XCTAssertEqual(c.messages.first?.reactions, [ReactionView(emoji: "👍", users: [2, 3, 2]), ReactionView(emoji: "🎉", users: [3])])
        XCTAssertEqual(c.unread(me: 2), 1)
        XCTAssertEqual(c.firstUnread(me: 2), 3003)
        XCTAssertEqual(v.visibleChannels(me: 3).map(\.id), [3001])

        XCTAssertEqual(v.fields.map(\.name), ["Tier"])
        XCTAssertEqual(v.fields.first?.options, ["Gold", "Silver"])
    }
}
