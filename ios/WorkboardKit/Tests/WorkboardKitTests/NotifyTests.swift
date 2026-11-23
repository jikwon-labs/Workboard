import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// The Mac's notify_candidates rules, on the phone.
final class NotifyTests: XCTestCase {
    func keys(_ d: JSON, _ p: NotifyPrefs = NotifyPrefs(), manager: Bool = false) -> [String] {
        Notify.candidates(d, prefs: p, me: 1, manager: manager).map(\.key)
    }

    func testDirectMessagesAlwaysButChannelsOnlyWhenMentionedByDefault() {
        var d = SampleWorkspace.make()
        Edits.sendMessage(&d, channel: 303, text: "dm", me: 2, id: 900, ts: 10)
        Edits.sendMessage(&d, channel: 301, text: "plain", me: 2, id: 901, ts: 11)
        Edits.sendMessage(&d, channel: 301, text: "@Jamie Park look", me: 2, id: 902, ts: 12)
        Edits.sendMessage(&d, channel: 301, text: "mine", me: 1, id: 903, ts: 13)
        let k = keys(d)
        XCTAssertTrue(k.contains("chat:303:900"))
        XCTAssertFalse(k.contains("chat:301:901"), "channels default to mentions only")
        XCTAssertTrue(k.contains("chat:301:902"))
        XCTAssertFalse(k.contains("chat:301:903"))
        var all = NotifyPrefs()
        all.setChannelMode(301, "all")
        XCTAssertTrue(keys(d, all).contains("chat:301:901"))
        var off = NotifyPrefs()
        off.setChannelMode(301, "nothing")
        XCTAssertFalse(keys(d, off).contains("chat:301:902"), "a channel set to nothing says nothing, mentions too")
        var noDM = NotifyPrefs()
        noDM.direct = false
        XCTAssertFalse(keys(d, noDM).contains("chat:303:900"))
    }

    func testAttachmentOnlyMessagesSayWhatWasSent() {
        let one = { (k: String) -> JSON in .object(["file": .string("ab"), "name": .string("a"), "kind": .string(k)]) }
        XCTAssertEqual(Notify.sentWhat([]), "")
        XCTAssertEqual(Notify.sentWhat([one("Image")]), "Sent a photo")
        XCTAssertEqual(Notify.sentWhat([one("Video")]), "Sent a video")
        XCTAssertEqual(Notify.sentWhat([one("Image"), one("Video")]), "Sent photos and videos")
        XCTAssertEqual(Notify.sentWhat([one("Image"), one("File")]), "Sent a file")
    }

    func testTaskNewsFollowTheChoices() {
        var d = SampleWorkspace.make()
        Edits.toggleAssignee(&d, task: 205, member: 1, me: 2, ts: 10)       // assigned to me
        Edits.addComment(&d, task: 201, text: "nice", me: 2, ts: 11)        // on my task
        Edits.setStatus(&d, task: 201, to: "review", me: 2, ts: 12)         // a change on my task
        let notes = Notify.candidates(d, prefs: NotifyPrefs(), me: 1, manager: false)
        XCTAssertEqual(notes.filter { $0.kind == .assigned }.count, 1)
        XCTAssertEqual(notes.first { $0.kind == .comment }?.title, "Alex Kim commented on Write the launch post")
        XCTAssertEqual(notes.first { $0.kind == .taskChange }?.body, "Alex Kim changed status from IN PROGRESS to REVIEW")
        var p = NotifyPrefs()
        p.assigned = false; p.comments = false; p.taskChanges = false
        XCTAssertTrue(Notify.candidates(d, prefs: p, me: 1, manager: false).filter { [.assigned, .comment, .taskChange].contains($0.kind) }.isEmpty)
    }

    func testATaskWhoseUpdatesITurnedOffStaysQuiet() {
        var d = SampleWorkspace.make()
        let comments = { (d: JSON, every: Bool) in
            Notify.candidates(d, prefs: NotifyPrefs(), me: 1, manager: false, everyTask: every).filter { $0.kind == .comment }.count
        }
        XCTAssertTrue(WorkspaceView(d).tasks.first { $0.id == 201 }!.watchedBy(1), "my own task tells me at first")
        Edits.toggleWatcher(&d, task: 201, member: 1, ts: 10)
        XCTAssertEqual(d["tasks"]?.array?.first { $0["id"]?.uint == 201 }?["unwatched"], [1])
        XCTAssertFalse(WorkspaceView(d).tasks.first { $0.id == 201 }!.watchedBy(1))
        Edits.addComment(&d, task: 201, text: "nice", me: 2, ts: 11)
        XCTAssertEqual(comments(d, false), 0)
        XCTAssertEqual(comments(d, true), 1, "its keys are still remembered as seen")
        Edits.toggleWatcher(&d, task: 201, member: 1, ts: 12)
        XCTAssertEqual(d["tasks"]?.array?.first { $0["id"]?.uint == 201 }?["unwatched"], [], "stays, empty")
        XCTAssertEqual(comments(d, false), 1)
    }

    func testHuddlesReactionsFormsAndJoins() {
        var d = SampleWorkspace.make()
        Edits.postHuddle(&d, channel: 303, me: 2, id: 900, ts: 10)
        Edits.toggleReaction(&d, channel: 302, message: 321, emoji: "👍", me: 2)
        Edits.addForm(&d, list: 120, me: 1, id: 830)
        _ = Edits.submitForm(&d, form: 830, answers: [0: "Laptop"], me: 2, id: 9000, ts: 11)
        let notes = Notify.candidates(d, prefs: NotifyPrefs(), me: 1, manager: true)
        XCTAssertEqual(notes.first { $0.kind == .huddle }?.title, "Alex Kim is calling you")
        XCTAssertNil(notes.first { $0.kind == .reaction }, "reactions are off by default")
        XCTAssertNotNil(Notify.candidates(d, prefs: .everything, me: 1, manager: true).first { $0.kind == .reaction })
        XCTAssertEqual(notes.first { $0.kind == .form }?.title, "New answer to New Form")
        XCTAssertEqual(notes.filter { $0.kind == .member }.count, 2)
        XCTAssertTrue(Notify.candidates(d, prefs: NotifyPrefs(), me: 1, manager: false).filter { $0.kind == .member }.isEmpty)
    }

    /// Two forms can share a name (two made from one template): an answer
    /// tells the owner of the form the task says it answers, and an answer
    /// from an app that doesn't record it goes by the name, then by the
    /// form's list when several share it (as on the Mac).
    func testAFormAnswerTellsOnlyThatFormsOwner() {
        var d = SampleWorkspace.make()
        Edits.addForm(&d, list: 120, me: 1, id: 830)
        Edits.addForm(&d, list: 111, me: 3, id: 831)
        _ = Edits.submitForm(&d, form: 831, answers: [0: "Theirs"], me: 2, id: 9000, ts: 11)
        func forms(_ d: JSON) -> [Note] { Notify.candidates(d, prefs: NotifyPrefs(), me: 1, manager: false).filter { $0.kind == .form } }
        XCTAssertTrue(forms(d).isEmpty, "an answer to someone else's form of the same name")
        _ = Edits.submitForm(&d, form: 830, answers: [0: "Mine"], me: 2, id: 9001, ts: 12)
        XCTAssertEqual(forms(d).map(\.body), ["Alex Kim answered: Mine"])
        // answers older apps sent, without the form: told apart by list
        for (tid, list) in [(UInt64(9002), UInt64(120)), (9003, 111)] {
            Edits.addTask(&d, list: list, name: "Old \(tid)", assignees: [], due: nil, me: 2, id: tid, ts: 13)
            d.updateItem(at: "tasks", id: tid) { t in Edits.log(&t, me: 2, text: "submitted the form New Form", ts: 13) }
        }
        XCTAssertEqual(Set(forms(d).map(\.body)), ["Alex Kim answered: Mine", "Alex Kim answered: Old 9002"])
    }

    func testOnlyFreshNewsOnceAndHuddlesShortly() {
        let now: Int64 = 100_000
        let notes = [
            Note(key: "a", kind: .direct, title: "", body: "", target: .people, ts: now - 10),
            Note(key: "b", kind: .direct, title: "", body: "", target: .people, ts: now - Notify.freshSecs - 1),
            Note(key: "h", kind: .huddle, title: "", body: "", target: .people, ts: now - 600),
            Note(key: "r", kind: .reaction, title: "", body: "", target: .people, ts: nil),
        ]
        var seen: Set<String> = []
        XCTAssertEqual(Notify.fresh(notes, all: ["a", "b", "h", "r", "x"], seen: &seen, now: now).map(\.key), ["a", "r"])
        XCTAssertTrue(Notify.fresh(notes, all: [], seen: &seen, now: now).isEmpty, "told once")
        XCTAssertTrue(seen.contains("x"), "what exists with every kind on isn't news later")
    }

    func testEachWorkspaceRemembersWhatWasToldOnItsOwn() {
        let now: Int64 = 100_000
        func note(_ k: String) -> Note { Note(key: k, kind: .direct, title: "", body: "", target: .people, ts: now - 10) }
        var told = Notify.Told()
        XCTAssertTrue(told.fresh([note("a1")], all: ["a1"], workspace: "A", now: now).isEmpty, "the first look only remembers")
        XCTAssertEqual(told.fresh([note("a1"), note("a2")], all: [], workspace: "A", now: now).map(\.key), ["a2"])
        // another workspace opened: its last day is not news
        XCTAssertTrue(told.fresh([note("b1"), note("b2")], all: ["b1", "b2"], workspace: "B", now: now).isEmpty)
        XCTAssertEqual(told.fresh([note("b1"), note("b3")], all: [], workspace: "B", now: now).map(\.key), ["b3"])
        // back to the first: only what came since it was left
        XCTAssertEqual(told.fresh([note("a1"), note("a2"), note("a3")], all: [], workspace: "A", now: now).map(\.key), ["a3"])
    }

    func testPrefsSaveAndReadBackAndOldFilesKeepDefaults() throws {
        var p = NotifyPrefs()
        p.setChannelMode(301, "all")
        p.mutedUntil = 5
        let back = try JSONDecoder().decode(NotifyPrefs.self, from: JSONEncoder().encode(p))
        XCTAssertEqual(back, p)
        let old = try JSONDecoder().decode(NotifyPrefs.self, from: Data(#"{"direct": false}"#.utf8))
        XCTAssertFalse(old.direct)
        XCTAssertTrue(old.mentions)
        XCTAssertEqual(old.channelDefault, "mentions")
        p.setChannelMode(301, "mentions")
        XCTAssertNil(p.channelModes[301], "the default isn't kept as an exception")
    }

    func testDueSummaryAndBadge() {
        var d = SampleWorkspace.make()
        let v = WorkspaceView(d)
        let today = dayString()
        // 201 due today, 202 two days late (both mine); 204 later
        XCTAssertEqual(Notify.dueSummary(v, me: 1, day: today), "1 due today, 1 overdue")
        Edits.addComment(&d, task: 201, text: "x", me: 2, ts: 5)
        XCTAssertEqual(Notify.badge(d, view: WorkspaceView(d), me: 1), 2, "one Inbox row and one unread DM")
    }

    /// Keywords notify like a mention (NotifySettings::keywords), not in a
    /// channel set to nothing, and once when the message also names me.
    func testKeywordsNotifyLikeAMention() {
        var d = SampleWorkspace.make()
        Edits.sendMessage(&d, channel: 301, text: "lunch", me: 2, id: 900, ts: 10)
        Edits.sendMessage(&d, channel: 301, text: "The Deploy is out", me: 2, id: 901, ts: 11)
        Edits.sendReply(&d, channel: 301, to: 900, text: "배포 언제?", me: 3, id: 902, ts: 12)
        Edits.sendMessage(&d, channel: 301, text: "@Jamie Park deploy", me: 2, id: 903, ts: 13)
        var p = NotifyPrefs()
        p.keywords = "deploy, 배포 ,"
        XCTAssertEqual(p.keywordList, ["deploy", "배포"])
        let notes = Notify.candidates(d, prefs: p, me: 1, manager: false)
        let k = notes.map(\.key)
        XCTAssertFalse(k.contains("chat:301:900"))
        XCTAssertTrue(k.contains("chat:301:901"))
        XCTAssertTrue(k.contains("chat:301:902"), "in a thread too")
        XCTAssertEqual(notes.first { $0.key == "chat:301:901" }?.title, "Alex Kim said \"deploy\" in #general")
        XCTAssertEqual(notes.filter { $0.key.hasSuffix(":903") }.count, 1)
        XCTAssertFalse(keys(d).contains("chat:301:901"), "no keywords, no notice")
        p.setChannelMode(301, "nothing")
        XCTAssertFalse(keys(d, p).contains("chat:301:901"))
    }

    /// What the server gets with the push token (wb_net::PushPrefs).
    func testChoicesForTheServer() {
        var p = NotifyPrefs()
        p.keywords = "deploy"
        p.setChannelMode(301, "all")
        p.mutedUntil = 99
        p.replies = false
        XCTAssertEqual(p.forServer, [
            "direct": true, "mentions": true, "replies": false, "channel_default": "mentions",
            "channel_modes": [["channel": 301, "mode": "all"]], "keywords": ["deploy"], "muted_until": 99,
        ])
    }

    /// A scan builds notes only for the last day (and keys only for the
    /// list of everything): what it tells is the same as from all of history.
    func testAScanOfTheLastDayTellsWhatAFullScanTells() {
        let now: Int64 = 1_800_000_000
        var d = SampleWorkspace.make()
        var id: UInt64 = 9_000
        for (k, age) in [Int64(3 * 86_400), 90_000, 86_000, 3_600, 60].enumerated() {
            id += 10
            Edits.sendMessage(&d, channel: 303, text: "dm \(k)", me: 2, id: id, ts: now - age)
            Edits.sendMessage(&d, channel: 301, text: "@Jamie Park look \(k) deploy", me: 3, id: id + 1, ts: now - age)
            Edits.sendMessage(&d, channel: 302, text: "plain \(k) deploy", me: 2, id: id + 2, ts: now - age)
            Edits.sendReply(&d, channel: 301, to: id + 1, text: "reply \(k) deploy", me: 2, id: id + 3, ts: now - age + 1)
            Edits.addComment(&d, task: 201, text: "comment \(k)", me: 2, ts: now - age)
        }
        var prefs = [NotifyPrefs(), .everything]
        var p = NotifyPrefs()
        p.keywords = "deploy"
        p.reactions = true
        p.setChannelMode(302, "all")
        prefs.append(p)
        for pr in prefs {
            let inbox = Inbox.items(d, me: 1)
            let full = Notify.candidates(d, prefs: pr, me: 1, manager: true, inbox: inbox)
            let fullAll = Notify.candidates(d, prefs: .everything, me: 1, manager: true, inbox: inbox, everyTask: true).map(\.key)
            let since = now - Notify.freshSecs
            let quick = Notify.candidates(d, prefs: pr, me: 1, manager: true, inbox: inbox, since: since)
            let quickAll = Notify.candidates(d, prefs: .everything, me: 1, manager: true, inbox: inbox, everyTask: true, since: since, keysOnly: true).map(\.key)
            var seenA = Set<String>(), seenB = Set<String>()
            let a = Notify.fresh(full, all: fullAll, seen: &seenA, now: now)
            let b = Notify.fresh(quick, all: quickAll, seen: &seenB, now: now)
            XCTAssertEqual(a, b)
            XCTAssertFalse(a.isEmpty)
            // later, with new messages: still the same news
            var d2 = d
            Edits.sendMessage(&d2, channel: 303, text: "new dm", me: 2, id: 99_001, ts: now + 100)
            Edits.sendMessage(&d2, channel: 302, text: "new deploy", me: 2, id: 99_002, ts: now + 100)
            let inbox2 = Inbox.items(d2, me: 1)
            let a2 = Notify.fresh(Notify.candidates(d2, prefs: pr, me: 1, manager: true, inbox: inbox2),
                                  all: Notify.candidates(d2, prefs: .everything, me: 1, manager: true, inbox: inbox2, everyTask: true).map(\.key), seen: &seenA, now: now + 100)
            let s2 = now + 100 - Notify.freshSecs
            let b2 = Notify.fresh(Notify.candidates(d2, prefs: pr, me: 1, manager: true, inbox: inbox2, since: s2),
                                  all: Notify.candidates(d2, prefs: .everything, me: 1, manager: true, inbox: inbox2, everyTask: true, since: s2, keysOnly: true).map(\.key), seen: &seenB, now: now + 100)
            XCTAssertEqual(a2, b2)
        }
    }

    /// What I read on any of my devices takes its notifications away: up
    /// to the conversation's read position, thread replies once marked read.
    func testNotificationsOfWhatIReadGoAway() {
        var d = SampleWorkspace.make()
        d.updateItem(at: "channels", id: 301) { $0["last_read"] = 1_000 }
        d.set("extra.replies_read", ["chat:302:901"])
        let shown = [
            Notify.Shown(id: "read", channel: 301, message: 5, thread: nil, ts: 900, shownAt: 2_000),
            Notify.Shown(id: "newer", channel: 301, message: 6, thread: nil, ts: 1_100, shownAt: 1_100),
            Notify.Shown(id: "old server", channel: 301, message: nil, thread: nil, ts: nil, shownAt: 990),
            Notify.Shown(id: "reply read", channel: 302, message: 901, thread: 321, ts: 50, shownAt: 50),
            Notify.Shown(id: "reply not read", channel: 302, message: 902, thread: 321, ts: 50, shownAt: 50),
            Notify.Shown(id: "no place", channel: nil, message: nil, thread: nil, ts: 1, shownAt: 1),
        ]
        XCTAssertEqual(Notify.readOnes(shown, data: d), ["read", "old server", "reply read"])
    }
}
