import SwiftUI
import WorkboardKit

struct RootView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav

    @Environment(CallManager.self) private var calls

    var body: some View {
        @Bindable var nav = nav
        @Bindable var calls = calls
        Group {
            if client.needsSignIn {
                SignInView()
            } else if client.data == nil {
                if nav.openingWorkspace { OpeningWorkspaceView() } else { SetupView() }
            } else if client.me == nil || client.view.member(client.me!) == nil {
                ChooseMemberView(isFirstRun: true)
            } else {
                MainTabs()
            }
        }
        .sheet(isPresented: $nav.showSettings) { SettingsView() }
        .fullScreenCover(isPresented: $calls.showCall) { CallView() }
        .onChange(of: nav.openedLink) { if let l = nav.openedLink { nav.openedLink = nil; open(l) } }
        .onAppear { if let l = nav.openedLink { nav.openedLink = nil; open(l) } }
        // a sign-in code that worked needs no sign-in screen
        .onChange(of: client.state.isConnected) { if client.state.isConnected, nav.invite?.fromComputer == true { nav.invite = nil } }
        // the spinner after a sign-in lasts until the workspace is here, or
        // the sign-in screen comes back to say why it didn't work
        // (a copy kept on this iPhone opens at once)
        .onChange(of: !client.needsSignIn && client.data != nil) { _, open in if open { nav.openingWorkspace = false } }
        .onChange(of: client.signInReason) { if client.signInReason == .failed { nav.openingWorkspace = false } }
        .alert("Switch servers?", isPresented: Binding(isPresent: $nav.confirmSwitch), presenting: nav.confirmSwitch) { link in
            Button("Switch") { use(link, fresh: true) }
                .accessibilityIdentifier("confirmSwitchButton")
            Button("Cancel", role: .cancel) {}
        } message: { link in
            if link.fromComputer {
                Text("This sign-in code is for \(link.server ?? "another server"). You're signed in to \(client.settings.server), which stays on this iPhone: switch back to it in Settings > Workspaces.")
            } else {
                Text("This invite is for \(link.server ?? "another server"). You're signed in to \(client.settings.server), which stays on this iPhone: switch back to it in Settings > Workspaces.")
            }
        }
        .alert("Already signed in", isPresented: Binding(isPresent: $nav.confirmSignOut), presenting: nav.confirmSignOut) { link in
            Button("Sign out and use it", role: .destructive) {
                client.signOut()
                nav.invite = link
                if link.fromComputer { signIn(link) }
            }
            Button("Keep me signed in", role: .cancel) {}
        } message: { link in
            if link.fromComputer {
                Text("This iPhone is already signed in to this server. Sign out and sign in with the code from the computer?")
            } else {
                Text("This iPhone is already signed in to this server. Use the invite to sign in as someone else?")
            }
        }
    }

    /// An invite link (workboard://join?...): set the server and fill in the
    /// invite form, asking first when that means leaving a signed-in server.
    /// A sign-in link from the person's computer (workboard://link?...,
    /// scanned here or with the Camera app) signs in with its code at once.
    func open(_ link: InviteLink) {
        nav.showSettings = false
        let signedIn = client.settings.meFromServer && !client.needsSignIn
        let sameServer = link.server.map { ServerAddress.same($0, client.settings.server) } ?? true
        if signedIn {
            if sameServer { nav.confirmSignOut = link } else { nav.confirmSwitch = link }
            return
        }
        use(link, fresh: !sameServer && !client.settings.server.isEmpty)
    }

    func use(_ link: InviteLink, fresh: Bool) {
        nav.invite = link
        if let s = link.server, !ServerAddress.same(s, client.settings.server) {
            client.setServer(s, freshCopy: fresh)
        } else if client.state == .idle, !link.fromComputer {
            client.connect()
        }
        if link.fromComputer { signIn(link) }
    }

    func signIn(_ link: InviteLink) {
        if client.signIn(.link(code: link.code)) == nil { nav.openingWorkspace = true }
    }
}

struct MainTabs: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav

    var body: some View {
        @Bindable var nav = nav
        // the banner sits above the tabs (not as a safe-area inset), so it never
        // covers the navigation bar's buttons
        VStack(spacing: 0) {
            ConnectionBanner()
            OngoingCallBar()
            tabs
        }
        .animation(.default, value: client.state.isConnected)
        .overlay(alignment: .top) { NoticeToast() }
    }

    var tabs: some View {
        @Bindable var nav = nav
        return TabView(selection: $nav.tab) {
            NavigationStack(path: $nav.home) { HomeView().wbDestinations() }
                .tabItem { Label("Home", systemImage: "house") }
                .tag(Nav.Tab.home)
            NavigationStack(path: $nav.chat) { ChatListView().wbDestinations() }
                .tabItem { Label("Chat", systemImage: "bubble.left.and.bubble.right") }
                .badge(unreadTotal)
                .tag(Nav.Tab.chat)
            NavigationStack(path: $nav.tasks) { SpacesView().wbDestinations() }
                .tabItem { Label("Tasks", systemImage: "checklist") }
                .tag(Nav.Tab.tasks)
            NavigationStack(path: $nav.docs) { DocsView().wbDestinations() }
                .tabItem { Label("Docs", systemImage: "doc.text") }
                .tag(Nav.Tab.docs)
            NavigationStack(path: $nav.more) { MoreView().wbDestinations() }
                .tabItem { Label("More", systemImage: "square.grid.2x2") }
                .tag(Nav.Tab.more)
        }
    }

    var unreadTotal: Int {
        guard let me = client.me else { return 0 }
        return client.view.visibleChannels(me: me).reduce(0) { $0 + $1.unread(me: me) }
    }
}

extension View {
    func wbDestinations() -> some View {
        navigationDestination(for: Nav.Route.self) { r in
            switch r {
            case .channel(let id): ConversationView(channelId: id)
            case let .thread(c, m): ThreadView(channelId: c, messageId: m)
            case .space(let id): SpaceDetailView(spaceId: id)
            case .list(let id): ListTasksView(listId: id)
            case .task(let id): TaskDetailView(taskId: id)
            case .doc(let id): DocDetailView(docId: id)
            case .statuses(let id): StatusEditorView(spaceId: id)
            case .app(let name): AppPage(name: name)
            case .goal(let id): GoalDetailView(goalId: id)
            case .meeting(let id): MeetingDetailView(meetingId: id)
            }
        }
    }
}

/// The gear in each tab's top bar.
struct SettingsButton: View {
    @Environment(Nav.self) private var nav
    @Environment(SyncClient.self) private var client

    var body: some View {
        Button { nav.showSettings = true } label: {
            Avatar(member: client.me.flatMap { client.view.member($0) }, size: 28)
        }
        .accessibilityLabel("Settings")
        .accessibilityIdentifier("settingsButton")
    }
}

/// A slim bar while not connected: edits are off until the server is back.
struct ConnectionBanner: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        switch client.state {
        case .connected, .signIn:
            EmptyView()
        case .connecting:
            bar(icon: "arrow.triangle.2.circlepath", text: "Connecting…" + waiting, retry: false)
        case .idle:
            bar(icon: "wifi.slash", text: "Offline" + waiting, retry: false)
        case let .waiting(at, _):
            TimelineView(.periodic(from: .now, by: 1)) { ctx in
                let secs = max(0, Int(at.timeIntervalSince(ctx.date).rounded(.up)))
                bar(icon: "wifi.slash", text: "Offline" + waiting + (secs > 0 ? " · retry in \(secs)s" : " · retrying…"), retry: true)
            }
        }
    }

    /// Edits keep working offline; say how many are waiting to go up.
    var waiting: String {
        let n = client.waitingCount
        return n == 0 ? String(localized: " · edits sync later") : n == 1 ? String(localized: " · 1 change waiting") : String(localized: " · \(n) changes waiting")
    }

    func bar(icon: String, text: String, retry: Bool) -> some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
            Text(text).lineLimit(1)
            Spacer(minLength: 0)
            if retry {
                Button("Retry") { client.retryNow() }
                    .font(.footnote.weight(.semibold))
            }
        }
        .font(.footnote)
        .foregroundStyle(.primary)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(.bar)
        .overlay(alignment: .bottom) { Divider() }
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("offlineBanner")
    }
}

/// Right after a sign-in (most often the QR code from the computer): a
/// spinner while it connects and the workspace comes down, so the screen
/// doesn't sit still on the setup form.
struct OpeningWorkspaceView: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        VStack(spacing: 16) {
            WMark().frame(width: 64, height: 64)
            ProgressView().controlSize(.large)
            Text(L(client.state.isConnected ? "Opening your workspace…" : "Signing in…"))
                .font(.headline)
            if case .waiting = client.state {
                Text(L(client.lastError ?? "Can't reach the server. Retrying…"))
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .multilineTextAlignment(.center)
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("openingWorkspace")
    }
}

/// First launch: where is the server? Or scan the QR code the computer
/// shows, which has the address and a sign-in code.
struct SetupView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @State private var address = ""
    @State private var scanning = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 12) {
                        WMark().frame(width: 64, height: 64)
                        Text("Workboard").font(.largeTitle.bold())
                        Text("Connect to your team's server to see the same tasks, chat and docs as on the Mac.")
                            .foregroundStyle(.secondary)
                    }
                    .listRowBackground(Color.clear)
                    .padding(.vertical, 8)
                }
                Section {
                    TextField("https://wb.example.com", text: $address)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .accessibilityIdentifier("serverField")
                        .onSubmit(connect)
                    Button("Connect", action: connect)
                        .disabled(address.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("connectButton")
                } header: {
                    Text("Server address")
                } footer: {
                    status
                }
                Section {
                    Button { scanning = true } label: {
                        Label("Scan QR code from your computer", systemImage: "qrcode.viewfinder")
                    }
                    .accessibilityIdentifier("setupScanButton")
                } footer: {
                    Text("Signed in on your computer? Open Work together > Sign in on your phone there, and scan its QR code to sign in here without a password.")
                }
            }
            .onAppear { address = client.settings.server }
            .sheet(isPresented: $scanning) {
                QRScanView { link in nav.openedLink = link }
            }
        }
    }

    @ViewBuilder var status: some View {
        switch client.state {
        case .connecting: Label("Connecting…", systemImage: "arrow.triangle.2.circlepath")
        case .connected: Label("Connected. Waiting for the workspace…", systemImage: "checkmark.circle")
        case .signIn: Label("This server needs a sign-in.", systemImage: "lock")
        case .waiting:
            Label(client.lastError ?? "Can't reach the server. Retrying…", systemImage: "exclamationmark.triangle")
        case .idle:
            if let e = client.lastError { Label(e, systemImage: "exclamationmark.triangle") } else { Text("Use the address of the Mac running workboard-server.") }
        }
    }

    func connect() {
        client.setServer(address)
        if client.state == .idle { client.connect() }
    }
}

/// Phase 1 has no login: pick yourself from the members, like the Mac app's
/// "who are you" step. Remembered on this device only.
struct ChooseMemberView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    let isFirstRun: Bool

    var body: some View {
        let list = List {
            Section {
                ForEach(client.view.members.filter { !$0.invited || $0.id == client.me }) { m in
                    Button {
                        client.setMe(m.id)
                        if !isFirstRun { dismiss() }
                    } label: {
                        HStack(spacing: 12) {
                            Avatar(member: m)
                            VStack(alignment: .leading) {
                                Text(m.name).foregroundStyle(.primary)
                                if !m.role.isEmpty { Text(m.role).font(.caption).foregroundStyle(.secondary) }
                            }
                            Spacer()
                            if m.id == client.me { Image(systemName: "checkmark").foregroundStyle(Color.wbAccent) }
                        }
                    }
                    .accessibilityIdentifier("member-\(m.id)")
                }
            } footer: {
                Text("This server has no sign-in, so you pick yourself. Kept on this iPhone only.")
            }
        }
        .navigationTitle("Who are you?")
        if isFirstRun {
            VStack(spacing: 0) {
                ConnectionBanner()
                NavigationStack { list }
            }
        } else {
            list
        }
    }
}

/// A request the server turned down (error code "denied"), shown for a few seconds.
struct NoticeToast: View {
    @Environment(SyncClient.self) private var client

    var body: some View {
        if let n = client.notice {
            Text(n)
                .font(.footnote)
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
                .background(.regularMaterial, in: Capsule())
                .padding(.top, 8)
                .accessibilityIdentifier("noticeToast")
                .onTapGesture { client.clearNotice() }
                .task(id: n) {
                    try? await Task.sleep(nanoseconds: 4_000_000_000)
                    if client.notice == n { client.clearNotice() }
                }
        }
    }
}
