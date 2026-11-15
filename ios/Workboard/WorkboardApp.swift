// Workboard for iPhone: the same workspace as the Mac app, through the team's
// workboard-server. Screens only; data, merging and the connection live in
// WorkboardKit.
//
// Launch arguments, for UI tests and screenshots (all optional):
//   -wbReset YES         start from nothing (removes this app's saved copy and token)
//   -wbServer ws://...   server address
//   -wbMe 1              who I am (servers without sign-in)
//   -wbTab chat          home | chat | tasks | docs
//   -wbOpen channel:301  also task:ID, doc:ID, thread:CHANNEL:MESSAGE,
//                        list:SPACE:LIST, settings
//   -wbOpenURL URL       open a link at launch, as if tapped (workboard://join?...)
//   -wbNoPasswordAutoFill YES  no saved or strong passwords offered in the
//                        sign-in fields (UI tests type passwords themselves)
//   -wbAskTimeout 2      seconds to wait for the server's answer to an access
//                        or backup request (15 by default)
//
// Links: workboard://join?server=HOST[:PORT]&code=XXXX-XXXX-XXXX&email=...
// (the URL scheme is registered in Config/Info.plist) opens the invite form.

import SwiftUI
import WorkboardKit

@main
struct WorkboardApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var client: SyncClient
    @State private var nav: Nav
    @State private var calls = CallManager()
    @State private var notifier = Notifier()

    init() {
        let args = UserDefaults.standard
        let storage = Storage.standard
        let tokens = KeychainTokenStore()
        if args.bool(forKey: "wbReset") {
            let old = storage.loadSettings().server
            if !old.isEmpty { tokens.setToken(nil, for: old) }
            if let s = args.string(forKey: "wbServer") { tokens.setToken(nil, for: s) }
            try? FileManager.default.removeItem(at: storage.folder)
        }
        let c = SyncClient(storage: storage, tokens: tokens)
        if let me = args.string(forKey: "wbMe"), let id = UInt64(me) { c.setMe(id) }
        if let server = args.string(forKey: "wbServer") { c.setServer(server) }
        if args.double(forKey: "wbAskTimeout") > 0 { c.askTimeout = args.double(forKey: "wbAskTimeout") }
        _client = State(initialValue: c)
        let nav = Nav(tab: args.string(forKey: "wbTab"), open: args.string(forKey: "wbOpen"))
        if let u = args.string(forKey: "wbOpenURL") { nav.openedLink = InviteLink.parse(u) }
        _nav = State(initialValue: nav)
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(client)
                .environment(nav)
                .environment(calls)
                .environment(notifier)
                .tint(.wbAccent)
                .onAppear(perform: wire)
                .onOpenURL { url in
                    if let link = InviteLink.parse(url.absoluteString) { nav.openedLink = link }
                }
                .task { await notifier.refresh() }
        }
    }

    func wire() {
        if client.state == .idle, !client.settings.server.isEmpty { client.connect() }
        let (c, n, nv) = (client, notifier, nav)
        c.onIncoming = { msgs in
            n.announce(msgs, view: c.view, me: c.me ?? 0, currentChannel: nv.currentChannel)
        }
        appDelegate.currentChannel = { nv.currentChannel }
        appDelegate.onOpen = { ch, parent in nv.open(channel: ch, parent: parent) }
        if let (ch, parent) = appDelegate.pendingOpen { nav.open(channel: ch, parent: parent) }
        appDelegate.onDeviceToken = { hex in
            #if DEBUG
            c.setPushToken(hex, env: "development")
            #else
            c.setPushToken(hex, env: "production")
            #endif
        }
    }
}

/// Which tab is shown and what is open in each, so links, notifications and
/// launch arguments can jump straight to a screen.
@MainActor
@Observable
final class Nav {
    enum Tab: String, Hashable { case home, chat, tasks, docs }

    enum Route: Hashable {
        case channel(UInt64)
        case thread(channel: UInt64, message: UInt64)
        case space(UInt64)
        case list(UInt64)
        case task(UInt64)
        case doc(UInt64)
    }

    var tab: Tab = .home
    var home: [Route] = []
    var chat: [Route] = []
    var tasks: [Route] = []
    var docs: [Route] = []
    var showSettings = false
    /// An invite link just opened, waiting for RootView to act on it.
    var openedLink: InviteLink?
    /// An invite link for the sign-in screen to fill in.
    var invite: InviteLink?
    /// Asking before leaving a signed-in server for an invite link.
    var confirmSwitch: InviteLink?
    var confirmSignOut: InviteLink?
    /// A doc made with the New button, to open in edit mode.
    var newDoc: UInt64?

    init(tab: String? = nil, open: String? = nil) {
        if let t = tab.flatMap(Tab.init(rawValue:)) { self.tab = t }
        guard let open else { return }
        let parts = open.split(separator: ":").map(String.init)
        let ids = parts.dropFirst().compactMap { UInt64($0) }
        switch (parts.first, ids.count) {
        case ("channel", 1): self.tab = .chat; chat = [.channel(ids[0])]
        case ("thread", 2): self.tab = .chat; chat = [.channel(ids[0]), .thread(channel: ids[0], message: ids[1])]
        case ("task", 1): self.tab = .tasks; tasks = [.task(ids[0])]
        case ("list", 2): self.tab = .tasks; tasks = [.space(ids[0]), .list(ids[1])]
        case ("doc", 1): self.tab = .docs; docs = [.doc(ids[0])]
        case ("settings", _): showSettings = true
        default: break
        }
    }

    /// Open a conversation (and a thread in it) from a notification.
    func open(channel: UInt64, parent: UInt64?) {
        showSettings = false
        tab = .chat
        chat = [.channel(channel)] + (parent.map { [.thread(channel: channel, message: $0)] } ?? [])
    }

    /// The conversation on screen, if any (no notification for it).
    var currentChannel: UInt64? {
        guard tab == .chat else { return nil }
        for r in chat.reversed() {
            switch r {
            case .channel(let c): return c
            case .thread(let c, _): return c
            default: continue
            }
        }
        return nil
    }
}
