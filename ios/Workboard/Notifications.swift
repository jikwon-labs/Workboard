// Notifications for new messages.
//
// Local: when a message or reply from someone else arrives over the
// connection and the person is not looking at that conversation, a
// notification is shown (also briefly in the background, while iOS keeps the
// connection alive).
//
// Remote (APNs): after permission is given the app registers for remote
// notifications and sends the device token to the server (`push_token`).
// The server side and the aps-environment entitlement are not set up yet; see
// docs/ios-design.md. The payload the app understands:
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
        // expected until the app is signed with the aps-environment entitlement
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

    /// Show a notification for messages that arrived over the connection.
    func announce(_ msgs: [IncomingMessage], view: WorkspaceView, me: UInt64, currentChannel: UInt64?) {
        guard permission == .allowed else { return }
        let active = UIApplication.shared.applicationState == .active
        for m in msgs.suffix(5) {
            if active && m.channel == currentChannel { continue }
            guard let c = view.channels.first(where: { $0.id == m.channel }) else { continue }
            let content = UNMutableNotificationContent()
            let who = view.member(m.author)?.name ?? "Someone"
            let place = c.kind == .channel ? "#\(c.name)" : nil
            content.title = place.map { "\(who) in \($0)" } ?? who
            if m.parent != nil { content.subtitle = "Thread reply" }
            content.body = m.text.isEmpty ? "Sent a photo or video" : m.text
            content.sound = .default
            content.threadIdentifier = "ch-\(m.channel)"
            var info: [String: Any] = ["channel": m.channel]
            if let p = m.parent { info["parent"] = p }
            content.userInfo = info
            UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: "msg-\(m.id)", content: content, trigger: nil))
        }
    }

    nonisolated static func channel(in info: [AnyHashable: Any]) -> UInt64? { number(info["channel"]) }

    nonisolated static func number(_ v: Any?) -> UInt64? {
        if let n = v as? NSNumber { return n.uint64Value }
        if let s = v as? String { return UInt64(s) }
        return nil
    }
}
