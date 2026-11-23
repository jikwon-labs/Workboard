// Workboard for iPhone: the same workspace as the Mac app, through the team's
// workboard-server. Screens only; data, merging and the connection live in
// WorkboardKit.
//
// Launch arguments, for UI tests and screenshots (all optional):
//   -wbReset YES         start from nothing (removes this app's saved copy, token and drafts)
//   -wbServer ws://...   server address
//   -wbMe 1              who I am (servers without sign-in)
//   -wbOpen channel:301  a place (inbox, today, upcoming, chats, docs,
//                        tools, search), or task:ID, doc:ID,
//                        thread:CHANNEL:MESSAGE, list:SPACE:LIST, app:NAME,
//                        goal:ID, meeting:ID, settings
//   -wbOpenURL URL       open a link at launch, as if tapped (workboard://join?...)
//   -wbNoPasswordAutoFill YES  no saved or strong passwords offered in the
//                        sign-in fields (UI tests type passwords themselves)
//   -wbAskTimeout 2      seconds to wait for the server's answer to an access
//                        or backup request (15 by default)
//
// Links: workboard://join?server=HOST[:PORT]&code=XXXX-XXXX-XXXX&email=...
// (the URL scheme is registered in Config/Info.plist) opens the invite form.

import SwiftUI
import UIKit
import WorkboardKit

@main
struct WorkboardApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var client: SyncClient
    @State private var nav: Nav
    @State private var calls = CallManager()
    @State private var notifier: Notifier

    init() {
        let args = UserDefaults.standard
        let storage = Storage.standard
        let tokens = KeychainTokenStore()
        if args.bool(forKey: "wbReset") {
            let old = storage.loadSettings().server
            if !old.isEmpty { tokens.setToken(nil, for: old) }
            if let s = args.string(forKey: "wbServer") { tokens.setToken(nil, for: s) }
            for w in storage.loadSaved() { tokens.setToken(nil, for: w.server) }
            try? FileManager.default.removeItem(at: storage.folder)
            // unsent messages are kept apart from the saved copy
            ChatDrafts.removeAll()
        }
        // the saved copy is read after the first frame (openSaved), except
        // when launch arguments change things that need it at once
        let now = args.bool(forKey: "wbReset") || args.string(forKey: "wbServer") != nil
        let c = SyncClient(storage: storage, tokens: tokens, openLater: !now)
        if let me = args.string(forKey: "wbMe"), let id = UInt64(me) { c.setMe(id) }
        if let server = args.string(forKey: "wbServer") { c.setServer(server) }
        if args.double(forKey: "wbAskTimeout") > 0 { c.askTimeout = args.double(forKey: "wbAskTimeout") }
        _client = State(initialValue: c)
        let n = Notifier()
        _notifier = State(initialValue: n)
        // what I read on my other devices while away (a quiet push; iOS may
        // start the app in the background for it, with no screen)
        AppDelegate.quietPush = { part, ws in
            if c.data == nil { await c.openSaved() }
            guard c.takeReadElsewhere(part, workspace: ws) else { return false }
            await n.settle(client: c)
            await withCheckedContinuation { (k: CheckedContinuation<Void, Never>) in c.flushSoon { k.resume() } }
            return true
        }
        let nav = Nav(open: args.string(forKey: "wbOpen"))
        if let u = args.string(forKey: "wbOpenURL") { nav.openedLink = InviteLink.parse(u) }
        _nav = State(initialValue: nav)
    }

    @Environment(\.scenePhase) private var scenePhase
    @AppStorage("appearance") private var appearance = "system"
    @AppStorage("accent") private var accent = "workboard"

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(client)
                .environment(nav)
                .environment(calls)
                .environment(notifier)
                .tint(Appearance.color(accent))
                .preferredColorScheme(Appearance.scheme(appearance))
                .onAppear(perform: wire)
                .onOpenURL { url in
                    if let link = InviteLink.parse(url.absoluteString) { nav.openedLink = link }
                }
                .task { await client.openSaved() }
                .task { await notifier.refresh() }
                // the copy is saved a moment after changes; leaving the app
                // writes what isn't saved yet now, since iOS may stop the app
                // before that: off the main thread, and going to the
                // background with time asked of iOS to finish
                .onChange(of: scenePhase) { _, p in
                    if p == .background { saveInBackground() } else if p == .inactive { client.flushSoon {} }
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willTerminateNotification)) { _ in client.flushSnapshot() }
        }
    }

    func saveInBackground() {
        let task = BackgroundTask()
        task.id = UIApplication.shared.beginBackgroundTask(withName: "Save the workspace") { task.end() }
        client.flushSoon { DispatchQueue.main.async { task.end() } }
    }

    func wire() {
        if client.state == .idle, !client.settings.server.isEmpty { client.connect() }
        let (c, n, nv) = (client, notifier, nav)
        c.onUpdate = { n.scanSoon(client: c, nav: nv) }
        n.scan(client: c, nav: nv) // the first look only remembers what is there
        appDelegate.onOpenTask = { id in nv.open(task: id) }
        appDelegate.currentChannel = { nv.currentChannel }
        appDelegate.otherWorkspace = { ws in c.savedWorkspace(pushKey: ws) != nil }
        // a push from a workspace kept on this iPhone opens that workspace first
        let open: (UInt64, UInt64?, String?) -> Void = { ch, parent, ws in
            if let w = c.savedWorkspace(pushKey: ws) { c.switchTo(w) }
            nv.open(channel: ch, parent: parent)
        }
        appDelegate.onOpen = open
        if let (ch, parent, ws) = appDelegate.pendingOpen { open(ch, parent, ws) }
        appDelegate.onDeviceToken = { hex in
            #if DEBUG
            c.setPushToken(hex, env: "development")
            #else
            c.setPushToken(hex, env: "production")
            #endif
        }
    }
}

/// What is open: one stack of screens on top of Home, so links,
/// notifications and launch arguments can jump straight to a screen and
/// "<" always goes one step up.
@MainActor
@Observable
final class Nav {
    enum Route: Hashable {
        case place(Place)
        case search
        case channel(UInt64)
        case thread(channel: UInt64, message: UInt64)
        case space(UInt64)
        case list(UInt64)
        case task(UInt64)
        case doc(UInt64)
        /// a space's docs (nil: docs in no space)
        case docsIn(space: UInt64?)
        case statuses(UInt64)
        case app(String)
        case goal(UInt64)
        case meeting(UInt64)
    }

    var path: [Route] = []
    var showSettings = false
    /// An invite link just opened, waiting for RootView to act on it.
    var openedLink: InviteLink?
    /// An invite link for the sign-in screen to fill in.
    var invite: InviteLink?
    /// Asking before leaving a signed-in server for an invite link.
    var confirmSwitch: InviteLink?
    var confirmSignOut: InviteLink?
    /// A sign-in was sent (a code from the computer, its QR code, or a
    /// form): show a spinner instead of the setup form until the workspace
    /// arrives or the server turns the sign-in down.
    var openingWorkspace = false
    /// A doc made with the New button, to open in edit mode.
    var newDoc: UInt64?

    /// `open`: inbox, today, upcoming, chats, docs, tools, search,
    /// channel:ID, thread:CHANNEL:MESSAGE, task:ID, list:SPACE:LIST, doc:ID,
    /// app:NAME, goal:ID, meeting:ID or settings.
    init(open: String? = nil) {
        guard let open else { return }
        if let p = Place(rawValue: open) { path = [.place(p)]; return }
        if open == "search" { path = [.search]; return }
        let parts = open.split(separator: ":").map(String.init)
        let ids = parts.dropFirst().compactMap { UInt64($0) }
        switch (parts.first, ids.count) {
        case ("channel", 1): path = [.place(.chats), .channel(ids[0])]
        case ("thread", 2): path = [.place(.chats), .channel(ids[0]), .thread(channel: ids[0], message: ids[1])]
        case ("task", 1): path = [.task(ids[0])]
        case ("list", 2): path = [.space(ids[0]), .list(ids[1])]
        case ("doc", 1): path = [.place(.docs), .doc(ids[0])]
        case ("settings", _): showSettings = true
        case ("app", _) where parts.count == 2: path = [.place(.tools), .app(parts[1])]
        case ("goal", 1): path = [.place(.tools), .app("goals"), .goal(ids[0])]
        case ("meeting", 1): path = [.place(.tools), .app("meetings"), .meeting(ids[0])]
        default: break
        }
    }

    /// Open a conversation (and a thread in it) from a notification.
    func open(channel: UInt64, parent: UInt64?) {
        showSettings = false
        path = [.place(.chats), .channel(channel)] + (parent.map { [.thread(channel: channel, message: $0)] } ?? [])
    }

    /// Open a task from a reminder.
    func open(task: UInt64) {
        showSettings = false
        path = [.task(task)]
    }

    /// The task on screen, if any (no notification for it).
    var currentTask: UInt64? {
        if case .task(let id)? = path.last { return id }
        return nil
    }

    /// The conversation on screen, if any (no notification for it).
    var currentChannel: UInt64? {
        switch path.last {
        case .channel(let c)?: return c
        case .thread(let c, _)?: return c
        default: return nil
        }
    }
}

/// Time asked of iOS to finish something after the app left the screen.
final class BackgroundTask: @unchecked Sendable {
    var id = UIBackgroundTaskIdentifier.invalid

    /// Done (or out of time): give the time back, once.
    func end() {
        guard id != .invalid else { return }
        UIApplication.shared.endBackgroundTask(id)
        id = .invalid
    }
}
