// Notifications for new messages.
//
// Local: when a message or reply from someone else arrives over the
// connection and the person is not looking at that conversation, a
// notification is shown (also briefly in the background, while iOS keeps the
// connection alive).
//
// Remote (APNs): after permission is given the app registers for remote
// notifications and sends the device token to the server (`push_token`).
// The server sends them once its apns.json is set up (server/src/push.rs);
// the app needs a signed build with the aps-environment entitlement (Config/Workboard.entitlements); see docs/ios-design.md.
// The payload the app understands:
//   {"aps": {"alert": {"title": "...", "body": "..."}, "thread-id": "ch-301"},
//    "channel": 301, "parent": 311}
// ("parent" only for thread replies). Tapping it opens the conversation.

import SwiftUI
import UIKit
import UserNotifications
import WorkboardKit

final class AppDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    /// Set by the app once its state exists.
    @MainActor var onDeviceToken: (String) -> Void = { _ in }
    @MainActor var onOpen: (UInt64, UInt64?) -> Void = { _, _ in }
    @MainActor var onOpenTask: (UInt64) -> Void = { _ in }
    @MainActor var currentChannel: () -> UInt64? = { nil }
    /// A tap that arrived before the app was ready.
    @MainActor var pendingOpen: (UInt64, UInt64?)?

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        UNUserNotificationCenter.current().delegate = self
        return true
    }

    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        let hex = deviceToken.map { String(format: "%02x", $0) }.joined()
        MainActor.assumeIsolated { onDeviceToken(hex) }
    }

    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        // expected in the simulator and in unsigned builds (no aps-environment entitlement)
        print("remote notifications not available: \(error.localizedDescription)")
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions {
        let info = notification.request.content.userInfo
        let ch = Notifier.channel(in: info)
        // UIApplication is main-thread only; this delegate method is not
        let (current, active) = await MainActor.run { (currentChannel(), UIApplication.shared.applicationState == .active) }
        if let ch, ch == current, active { return [] }
        return [.banner, .list, .sound]
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        let info = response.notification.request.content.userInfo
        if let task = Notifier.number(info["task"]) {
            await MainActor.run { onOpenTask(task) }
            return
        }
        guard let ch = Notifier.channel(in: info) else { return }
        let parent = Notifier.number(info["parent"])
        await MainActor.run {
            pendingOpen = (ch, parent)
            onOpen(ch, parent)
        }
    }
}

@MainActor
@Observable
final class Notifier {
    enum Permission { case unknown, allowed, denied }
    private(set) var permission: Permission = .unknown

    func refresh() async {
        let s = await UNUserNotificationCenter.current().notificationSettings()
        switch s.authorizationStatus {
        case .authorized, .provisional, .ephemeral: permission = .allowed
        case .denied: permission = .denied
        default: permission = .unknown
        }
        if permission == .allowed { UIApplication.shared.registerForRemoteNotifications() }
    }

    /// Ask once (the "Turn on" buttons); afterwards Settings is the only way.
    func requestPermission() async {
        let ok = (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge])) ?? false
        permission = ok ? .allowed : .denied
        if ok { UIApplication.shared.registerForRemoteNotifications() }
    }

    // MARK: choices (each person's own, kept on this iPhone like the Mac's)

    var prefs: NotifyPrefs = Notifier.loadPrefs() {
        didSet { if let d = try? JSONEncoder().encode(prefs) { UserDefaults.standard.set(d, forKey: "notifyPrefs") } }
    }

    static func loadPrefs() -> NotifyPrefs {
        guard let d = UserDefaults.standard.data(forKey: "notifyPrefs"), let p = try? JSONDecoder().decode(NotifyPrefs.self, from: d) else { return NotifyPrefs() }
        return p
    }

    /// Keys already told (or that existed at the first look) in this run.
    @ObservationIgnored private var seen: Set<String> = []
    @ObservationIgnored private var seeded = false

    /// After the team's changes came in (notify_scan): tell what is new, keep
    /// the icon's number and the clock notices up to date.
    func scan(client: SyncClient, nav: Nav) {
        guard let data = client.data, let me = client.me else { return }
        let manager = ["owner", "admin"].contains(client.settings.role ?? "")
        let found = Notify.candidates(data, prefs: prefs, me: me, manager: manager)
        let all = Notify.candidates(data, prefs: .everything, me: me, manager: manager).map(\.key)
        if !seeded {
            seen = Set(all).union(found.map(\.key))
            seeded = true
        } else {
            var fresh = Notify.fresh(found, all: all, seen: &seen, now: nowTs())
            let active = UIApplication.shared.applicationState == .active
            fresh.removeAll { active && visible($0.target, nav: nav) }
            deliver(fresh, view: client.view)
        }
        updateBadge(data, view: client.view, me: me)
        scheduleClock(client: client)
    }

    /// The conversation or task already on screen needs no notification.
    func visible(_ t: Note.Target, nav: Nav) -> Bool {
        switch t {
        case .channel(let c, _): return nav.currentChannel == c
        case .task(let id): return nav.currentTask == id
        case .people: return false
        }
    }

    func deliver(_ notes: [Note], view: WorkspaceView) {
        guard permission == .allowed, !notes.isEmpty, !prefs.muted(now: nowTs()) else { return }
        if notes.count > Notify.burst {
            // huddles still ring on their own; the rest as one summary
            let calls = notes.filter { $0.kind == .huddle }
            let rest = notes.filter { $0.kind != .huddle }
            calls.forEach { post($0) }
            if !rest.isEmpty {
                post(key: "burst-\(nowTs())", title: String(localized: "\(rest.count) new notifications"),
                     body: rest.prefix(3).map(title).joined(separator: "\n"), info: [:])
            }
        } else {
            notes.forEach { post($0) }
        }
    }

    /// The note's title in the phone's language (the Mac's words).
    func title(_ n: Note) -> String {
        switch n.kind {
        case .direct: return n.who
        case .channel: return String(localized: "\(n.who) in \(n.subject)")
        case .huddle: return n.subject.isEmpty ? String(localized: "\(n.who) is calling you") : String(localized: "\(n.who) started a huddle in \(n.subject)")
        case .reaction: return String(localized: "\(n.who) reacted \(n.subject) to your message")
        case .mention: return String(localized: "\(n.who) mentioned you in \(n.subject)")
        case .reply: return String(localized: "\(n.who) replied in \(n.subject)")
        case .assigned: return String(localized: "\(n.who) assigned you a task")
        case .comment: return String(localized: "\(n.who) commented on \(n.subject)")
        case .taskChange: return n.title
        case .form: return String(localized: "New answer to \(n.subject)")
        case .member: return String(localized: "\(n.who) joined \(n.subject)")
        }
    }

    func body(_ n: Note) -> String {
        switch n.kind {
        case .taskChange: return History.line(who: n.who, text: n.subject)
        case .huddle: return String(localized: "Join from the conversation")
        case .member: return String(localized: "Say hello in a direct message")
        default: return n.body == "Sent a photo or video" ? String(localized: "Sent a photo or video") : n.body
        }
    }

    func post(_ n: Note) {
        var info: [String: Any] = [:]
        switch n.target {
        case .channel(let c, let thread):
            info["channel"] = c
            if let thread { info["parent"] = thread }
        case .task(let id): info["task"] = id
        case .people: break
        }
        post(key: n.key, title: title(n), body: body(n), info: info, thread: n.target)
    }

    func post(key: String, title: String, body: String, info: [String: Any], thread: Note.Target? = nil) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        if prefs.sound { content.sound = .default }
        if case .channel(let c, _)? = thread { content.threadIdentifier = "ch-\(c)" }
        content.userInfo = info
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: key, content: content, trigger: nil))
    }

    func updateBadge(_ data: JSON, view: WorkspaceView, me: UInt64) {
        let n = prefs.badge ? Notify.badge(data, view: view, me: me) : 0
        UNUserNotificationCenter.current().setBadgeCount(n)
    }

    // MARK: the clock: tasks due, meetings about to start, goals due

    /// Notify.dueSummary in the phone's language.
    static func dueText(_ view: WorkspaceView, me: UInt64, day: String) -> String? {
        let mine = view.myTasks(me: me)
        let today = mine.filter { $0.due == day }.count
        let late = mine.filter { ($0.due ?? "9999") < day }.count
        switch (today, late) {
        case (0, 0): return nil
        case (1, 0): return String(localized: "1 task is due today")
        case (let d, 0): return String(localized: "\(d) tasks are due today")
        case (0, 1): return String(localized: "1 overdue task")
        case (0, let l): return String(localized: "\(l) overdue tasks")
        case (let d, let l): return String(localized: "\(d) due today, \(l) overdue")
        }
    }

    func scheduleClock(client: SyncClient) {
        let center = UNUserNotificationCenter.current()
        center.getPendingNotificationRequests { reqs in
            let old = reqs.map(\.identifier).filter { $0.hasPrefix("clock-") }
            center.removePendingNotificationRequests(withIdentifiers: old)
        }
        guard permission == .allowed, let data = client.data, let me = client.me else { return }
        let view = client.view
        let cal = Calendar.current
        let now = Date()
        let parts = prefs.dueTime.split(separator: ":").compactMap { Int($0) }
        func at(_ day: Date, _ h: Int, _ m: Int) -> Date? { cal.date(bySettingHour: h, minute: m, second: 0, of: day) }
        func add(_ id: String, _ title: String, _ body: String, _ when: Date, _ info: [String: Any] = [:]) {
            guard when > now, !prefs.muted(now: Int64(when.timeIntervalSince1970)) else { return }
            let c = UNMutableNotificationContent()
            c.title = title
            c.body = body
            if prefs.sound { c.sound = .default }
            c.userInfo = info
            let trig = UNCalendarNotificationTrigger(dateMatching: cal.dateComponents([.year, .month, .day, .hour, .minute], from: when), repeats: false)
            center.add(UNNotificationRequest(identifier: "clock-" + id, content: c, trigger: trig))
        }
        // my tasks due today and overdue, once a day at the chosen time (the next two days)
        if prefs.due, parts.count == 2 {
            for offset in 0..<2 {
                guard let day = cal.date(byAdding: .day, value: offset, to: now), let when = at(day, parts[0], parts[1]),
                      let text = Self.dueText(view, me: me, day: dayString(day)) else { continue }
                add("due-\(dayString(day))", text, String(localized: "Open Home to see them."), when)
            }
        }
        // meetings I attend, a few minutes before
        if prefs.meetings {
            let f = DateFormatter()
            f.locale = Locale(identifier: "en_US_POSIX")
            f.dateFormat = "yyyy-MM-dd HH:mm"
            for m in view.meetings(data) where m.attendees.contains(me) {
                guard let start = f.date(from: "\(m.date) \(m.time)"), start > now, start < now.addingTimeInterval(7 * 86400) else { continue }
                add("meeting-\(m.id)", m.title, String(localized: "Starts in \(prefs.meetingMinutes) minutes"), start.addingTimeInterval(-Double(prefs.meetingMinutes) * 60))
            }
        }
        // my goals due today, at the chosen time
        if prefs.goals, parts.count == 2 {
            for g in view.goals(data) where g.owner == me {
                guard let due = g.due, let day = CustomFieldRow.day(due), let when = at(day, parts[0], parts[1]) else { continue }
                add("goal-\(g.id)", String(localized: "Goal due today"), g.name, when)
            }
        }
    }

    nonisolated static func channel(in info: [AnyHashable: Any]) -> UInt64? { number(info["channel"]) }

    nonisolated static func number(_ v: Any?) -> UInt64? {
        if let n = v as? NSNumber { return n.uint64Value }
        if let s = v as? String { return UInt64(s) }
        return nil
    }
}
