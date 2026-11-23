import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// My reading state and notification choices kept in step between my
/// devices (Mine.swift, wb_sync::mine on the Mac and the server).
@MainActor
final class MineTests: FakeServerTestCase {
    override var devicePushDelay: TimeInterval? { 0.05 }

    /// The same case as the_shape_both_apps_give in crates/wb-sync/src/mine.rs:
    /// both apps must give exactly this.
    func testTheSameStateAsTheMac() {
        var v = MineView()
        v.read[10] = 500
        v.repliesSeen = 7
        v.inboxCleared = ["a"]
        v.inboxLater = ["b"]
        v.repliesRead = ["r"]
        v.settings["muted_until"] = 9
        var got = Mine.collect(v, known: ["replies_seen": 0], now: 1000)
        XCTAssertEqual(got, [
            "read": ["10": ["ts": 500, "at": 1000]],
            "replies_seen": 7,
            "inbox": ["a": ["tab": "cleared", "at": 1000], "b": ["tab": "later", "at": 1000]],
            "replies_read": ["r": ["on": true, "at": 1000]],
            "set": ["muted_until": ["value": 9, "at": 1000]],
        ])
        Mine.merge(&got, ["inbox": ["a": ["tab": "", "at": 2000]], "read": ["10": ["ts": 400, "at": 900], "11": ["ts": 3, "at": 900]]])
        let r = Mine.resolve(got)
        XCTAssertEqual(r.inboxCleared, [])
        XCTAssertEqual(r.inboxLater, ["b"])
        XCTAssertEqual(r.read, [10: 500, 11: 3])
        // nothing new: the same state goes nowhere
        XCTAssertEqual(Mine.collect(r, known: got, now: 5000), got)
        // nothing agreed yet: stamped 0
        XCTAssertEqual(Mine.collect(v, known: [:], now: 1000).at("set.muted_until.at"), 0)
    }

    func testMarkUnreadHoldsAndNewerPartsPassAlong() {
        var mac = MineView(); mac.read[10] = 900
        let known = Mine.collect(mac, known: ["replies_seen": 0], now: 1000)
        var server: JSON = [:]
        Mine.merge(&server, known)
        mac.read[10] = 700
        Mine.merge(&server, Mine.collect(mac, known: known, now: 2000))
        var phone = MineView(); phone.read[10] = 900
        Mine.merge(&server, Mine.collect(phone, known: known, now: 3000))
        XCTAssertEqual(Mine.resolve(server).read[10], 700, "marked unread later: it holds")
        var d = SampleWorkspace.make()
        Edits.markRead(&d, channel: 301, ts: 5000)
        Mine.apply(Mine.resolve(server).withRead(301, 10), to: &d)
        XCTAssertEqual(WorkspaceView(d).channels.first { $0.id == 301 }?.lastRead, 10, "lower positions are taken too")
        // a part from a newer app goes round once and stops
        var s2: JSON = [:]
        Mine.merge(&s2, ["read": [:], "pins": ["a": 1]])
        XCTAssertEqual(s2["pins"], ["a": 1])
        let back = Mine.collect(Mine.resolve(s2), known: s2, now: 9000)
        XCTAssertEqual(back, s2)
        XCTAssertFalse(Mine.merge(&s2, back))
    }

    func testChangesOnBothDevicesBothStayAndTiesKeepTheFirst() {
        var a = MineView(); a.inboxCleared = ["x"]; a.settings["muted_until"] = 0
        var b = MineView(); b.inboxLater = ["y"]; b.settings["muted_until"] = 99
        var server: JSON = [:]
        Mine.merge(&server, Mine.collect(a, known: ["replies_seen": 0], now: 1000))
        Mine.merge(&server, Mine.collect(b, known: ["replies_seen": 0], now: 1000))
        let r = Mine.resolve(server)
        XCTAssertEqual(r.inboxCleared, ["x"])
        XCTAssertEqual(r.inboxLater, ["y"])
        XCTAssertEqual(r.settings["muted_until"], 0, "a tie keeps the one already there")
    }

    func testTheMacsNotifyShapeRoundTrips() {
        var p = NotifyPrefs()
        p.direct = false
        p.taskChanges = false
        p.dueTime = "08:30"
        p.meetingMinutes = 5
        p.badge = false
        p.channelDefault = "all"
        p.channelModes = [301: "nothing"]
        p.keywords = "deploy, 배포"
        p.mutedUntil = 42
        // what only the Mac has stays
        let mac: JSON = ["desktop": false, "chat": true, "fired": ["x"]]
        let n = p.mac(base: mac)
        XCTAssertEqual(n["desktop"], false)
        XCTAssertEqual(n["dock_badge"], false)
        XCTAssertEqual(n["task_changes"], false)
        XCTAssertEqual(n["channel_modes"], [[301, "nothing"]])
        XCTAssertEqual(NotifyPrefs(mac: n, mutedUntil: 42), p)
    }

    func testTheIconCountsLikeTheMacsDock() {
        var d = SampleWorkspace.make()
        Edits.sendReply(&d, channel: 302, to: 321, text: "agreed", me: 2, id: 901, ts: 103) // my message's thread
        let view = WorkspaceView(d)
        let before = Notify.badge(d, view: view, me: 1)
        Edits.markThreadRead(&d, channel: 302, parent: 321, me: 1)
        XCTAssertEqual(d.at("extra.replies_read"), ["chat:302:901"])
        XCTAssertEqual(Notify.badge(d, view: WorkspaceView(d), me: 1), before - 1, "a reply read in its thread no longer counts")
        // my due reminders count, as on the Mac, unless its choice is off
        d.set("extra.reminders", [["id": 1, "text": "call", "due_ts": 10, "owner": 1]])
        XCTAssertEqual(Notify.badge(d, view: WorkspaceView(d), me: 1, now: 20), before)
        d.set("extra.notify.reminders", false)
        XCTAssertEqual(Notify.badge(d, view: WorkspaceView(d), me: 1, now: 20), before - 1)
    }

    func testMyOtherDeviceGetsWhatIReadClearAndChoose() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = "ws://127.0.0.1:\(try await server.start(port: 0))"
        defer { server.stop() }
        let phone = device(url)
        await until("sign-in screen") { phone.needsSignIn }
        phone.signIn(.invite(code: server.invite(member: 1), email: nil, newPassword: nil))
        let ipad = device(url)
        await until("sign-in screen") { ipad.needsSignIn }
        ipad.signIn(.invite(code: server.invite(member: 1), email: nil, newPassword: nil))
        await until("both signed in as me") { phone.canEdit && ipad.canEdit && phone.mineOK && ipad.mineOK }

        // on the phone: clear a row, read #general, choose fewer notifications
        phone.editLocal { d in
            Inbox.move(&d, keys: ["201:100:2"], to: .cleared)
            Edits.markRead(&d, channel: 301, ts: 4_000_000_000)
        }
        var p = NotifyPrefs()
        p.direct = false
        p.mutedUntil = 4_000_000_000
        phone.setNotifyPrefs(p)
        await until("the other device takes it") {
            ipad.data?.at("extra.inbox_cleared") == ["201:100:2"]
                && ipad.view.channels.first { $0.id == 301 }?.lastRead == 4_000_000_000
                && ipad.syncedNotifyPrefs == p
        }

        // on the other one: open a thread, put the row back
        ipad.markThreadRead(channel: 302, parent: 321)
        ipad.editLocal { Inbox.move(&$0, keys: ["201:100:2"], to: nil) }
        await until("the phone takes it back") {
            phone.data?.at("extra.inbox_cleared") == []
        }
        // never part of the team's copy
        XCTAssertFalse(Sync.shared.contains("extra.mine_sync"))
        XCTAssertNil(Sync.outgoing(phone.data!).at("extra.mine_sync"))
    }

    /// Read on the Mac while the phone wasn't connected: the server's quiet
    /// push (read_payload in server/src/push.rs) is taken in as a
    /// `mine_part` is, only for its own workspace, and the next connection
    /// agrees with it.
    func testAQuietPushBringsWhatIReadOnTheMac() async throws {
        let server = FakeServer(snapshot: SampleWorkspace.make(), requireSignIn: true)
        let url = try await start(server)
        defer { server.stop() }
        let phone = device(url)
        await until("sign-in screen") { phone.needsSignIn }
        phone.signIn(.invite(code: server.invite(member: 1), email: nil, newPassword: nil))
        await until("signed in, state kept") { phone.canEdit && phone.mineOK }
        phone.disconnect()
        await until("away") { !phone.state.isConnected }
        let before = phone.view.channels.first { $0.id == 301 }!.lastRead

        let part: JSON = ["read": ["301": ["ts": 4_000_000_000, "at": 1_800_000_000_000]]]
        XCTAssertFalse(phone.takeReadElsewhere(part, workspace: "another"), "another workspace's")
        XCTAssertEqual(phone.view.channels.first { $0.id == 301 }?.lastRead, before)
        var updated = false
        phone.onUpdate = { updated = true }
        XCTAssertTrue(phone.takeReadElsewhere(part, workspace: phone.pushKey))
        XCTAssertEqual(phone.view.channels.first { $0.id == 301 }?.lastRead, 4_000_000_000)
        XCTAssertTrue(updated, "the icon's number is worked out again")
        XCTAssertFalse(phone.mineOK, "a push isn't the server saying it keeps my state")
        XCTAssertEqual(phone.data?.at("extra.mine_sync.read.301.ts"), 4_000_000_000)
    }
}

extension MineView {
    func withRead(_ channel: UInt64, _ ts: Int64) -> MineView { var v = self; v.read[channel] = ts; return v }
}
