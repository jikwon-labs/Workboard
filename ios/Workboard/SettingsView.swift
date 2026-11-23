import SwiftUI
import WorkboardKit

struct SettingsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @Environment(\.dismiss) private var dismiss
    @State private var address = ""
    @State private var forgetting: SavedWorkspace?
    @AppStorage(Place.upcoming.shownKey) private var showUpcoming = true
    @AppStorage(Place.docs.shownKey) private var showDocs = true
    @AppStorage(Place.tools.shownKey) private var showTools = true

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Toggle("Upcoming", isOn: $showUpcoming).accessibilityIdentifier("showOnHome-upcoming")
                    Toggle("Docs", isOn: $showDocs).accessibilityIdentifier("showOnHome-docs")
                    Toggle("Tools", isOn: $showTools).accessibilityIdentifier("showOnHome-tools")
                } header: {
                    CalmHeader("Show on Home")
                } footer: {
                    Text("Inbox, Today and Chat are always there.")
                }
                workspacesSection
                Section("Server") {
                    TextField("https://wb.example.com", text: $address)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onSubmit(apply)
                        .accessibilityIdentifier("settingsServerField")
                    if address.trimmingCharacters(in: .whitespaces) != client.settings.server {
                        Button("Connect to this address", action: apply)
                    }
                    LabeledContent("Status") { stateText }
                        .accessibilityIdentifier("connectionStatus")
                    if client.waitingCount > 0 {
                        LabeledContent("Waiting to sync", value: client.waitingCount == 1 ? String(localized: "1 change") : String(localized: "\(client.waitingCount) changes"))
                            .accessibilityIdentifier("waitingCount")
                    }
                    if let e = client.lastError, !client.state.isConnected {
                        Text(e).font(.footnote).foregroundStyle(.secondary)
                    }
                    if client.state.isConnected {
                        Button("Disconnect", role: .destructive) { client.disconnect() }
                    } else if !client.settings.server.isEmpty {
                        Button("Connect now") { client.connect() }
                    }
                }
                Section {
                    if client.settings.meFromServer {
                        HStack(spacing: 12) {
                            Avatar(member: client.me.flatMap { client.view.member($0) })
                            VStack(alignment: .leading) {
                                Text("Signed in as \(client.me.flatMap { client.view.member($0)?.name } ?? "member \(client.me ?? 0)")")
                                    .accessibilityIdentifier("signedInAs")
                                Text(client.settings.accountEmail ?? "No password yet")
                                    .font(.caption).foregroundStyle(.secondary)
                                    .accessibilityIdentifier("accountEmail")
                            }
                        }
                        .accessibilityElement(children: .combine)
                        profileLinks
                        NavigationLink {
                            PasswordView()
                        } label: {
                            Text(client.settings.accountEmail == nil ? "Set a password" : "Change password")
                        }
                        .accessibilityIdentifier("passwordLink")
                        Button("Sign out", role: .destructive) {
                            client.signOut()
                            dismiss()
                        }
                        .accessibilityIdentifier("signOutButton")
                    } else {
                        NavigationLink {
                            ChooseMemberView(isFirstRun: false)
                        } label: {
                            HStack(spacing: 12) {
                                Avatar(member: client.me.flatMap { client.view.member($0) })
                                Text(client.me.flatMap { client.view.member($0)?.name } ?? "Choose who you are")
                            }
                        }
                        if client.me != nil { profileLinks }
                    }
                } header: {
                    Text("You")
                } footer: {
                    if !client.settings.meFromServer {
                        Text("This server has no sign-in, so you pick yourself. Kept on this iPhone only.")
                    } else if client.settings.accountEmail == nil {
                        Text("This iPhone was signed in with an invite code. Set a password to sign in with your email on other devices.")
                    }
                }
                Section {
                    switch notifier.permission {
                    case .allowed:
                        Label("On for new messages", systemImage: "bell.badge")
                    case .denied:
                        Button("Turned off. Open iOS Settings") {
                            if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                        }
                    case .unknown:
                        Button("Turn on notifications") { Task { await notifier.requestPermission() } }
                            .accessibilityIdentifier("notifyButton")
                    }
                    NavigationLink {
                        NotificationSettingsView()
                    } label: {
                        Label("What to notify about", systemImage: "slider.horizontal.3")
                    }
                    .accessibilityIdentifier("notificationSettingsLink")
                } header: {
                    Text("Notifications")
                } footer: {
                    Text("Messages from others show up while the app is open or was just closed. Push notifications also need the team server's Apple push key and an app build with push turned on; until both are set up, nothing arrives while the app has been closed for a while.")
                        .accessibilityIdentifier("notifyFooter")
                }
                if client.settings.meFromServer, let role = client.myRole, role.manages {
                    Section {
                        NavigationLink {
                            PeopleView()
                        } label: {
                            Label("People and invite codes", systemImage: "person.2")
                        }
                        .accessibilityIdentifier("peopleLink")
                        if client.accessRules.seesBackups {
                            NavigationLink {
                                BackupsView()
                            } label: {
                                Label("Backups", systemImage: "clock.arrow.circlepath")
                            }
                            .accessibilityIdentifier("backupsLink")
                        }
                        if client.canDeleteWorkspace {
                            NavigationLink {
                                // gone: Settings closes with it, onto the sign-in screen
                                DeleteWorkspaceView(onDeleted: { dismiss() })
                            } label: {
                                Label("Delete workspace", systemImage: "trash.slash")
                                    .foregroundStyle(.red)
                            }
                            .accessibilityIdentifier("deleteWorkspaceLink")
                        }
                    } header: {
                        Text("Team")
                    } footer: {
                        Text(role == .owner ? "You're the owner of this server." : "You're an admin on this server.")
                    }
                }
                Section("Look") {
                    NavigationLink {
                        ThemeSettingsView()
                    } label: {
                        Label("Theme", systemImage: "paintpalette")
                    }
                    .accessibilityIdentifier("themeLink")
                }
                Section("Workspace") {
                    NavigationLink {
                        TrashView()
                    } label: {
                        LabeledContent {
                            Text(client.trash.isEmpty ? "" : "\(client.trash.count)")
                        } label: {
                            Label("Trash", systemImage: "trash")
                        }
                    }
                    .accessibilityIdentifier("trashLink")
                    NavigationLink {
                        AutomationsView()
                    } label: {
                        Label("Automations", systemImage: "bolt")
                    }
                    .accessibilityIdentifier("automationsLink")
                    LabeledContent("Name", value: client.view.name)
                    LabeledContent("Members", value: "\(client.view.members.count)")
                    LabeledContent("Server revision", value: "\(client.rev)")
                    LabeledContent("This device's id tag", value: client.settings.tag.map(String.init) ?? "Not given yet")
                }
            }
            .scrollContentBackground(.hidden)
            .background(Calm.page)
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
            .onAppear { address = client.settings.server }
            .confirmationDialog(forgetting.map { String(localized: "Remove \($0.title) from this iPhone?") } ?? "", isPresented: Binding(isPresent: $forgetting), titleVisibility: .visible, presenting: forgetting) { w in
                Button("Remove", role: .destructive) { client.forget(w) }
                    .accessibilityIdentifier("confirmForgetWorkspace")
                Button("Cancel", role: .cancel) {}
            } message: { w in
                Text(w.waiting == 0 ? "Its copy and sign-in are removed from this iPhone. Nothing changes on the server." : w.waiting == 1 ? "1 change made there hasn't reached its server yet and will be lost. Nothing else changes on the server." : "\(w.waiting) changes made there haven't reached its server yet and will be lost. Nothing else changes on the server.")
            }
        }
    }

    @ViewBuilder var stateText: some View {
        switch client.state {
        case .connected: Label("Connected", systemImage: "circle.fill").foregroundStyle(.green).labelStyle(.titleAndIcon).imageScale(.small)
        case .connecting: Text("Connecting…")
        case .signIn: Text("Needs sign-in")
        case .waiting(_, let n): Text("Reconnecting (try \(n))")
        case .idle: Text("Disconnected")
        }
    }

    /// Edit profile and Set status (with the status now), as in the Mac's
    /// profile menu.
    @ViewBuilder var profileLinks: some View {
        NavigationLink {
            ProfileEditView()
        } label: {
            Label("Edit profile", systemImage: "person.crop.circle")
        }
        .accessibilityIdentifier("editProfileLink")
        NavigationLink {
            StatusEditView()
        } label: {
            HStack {
                Label("Set status", systemImage: "face.smiling")
                Spacer()
                if let status = client.me.flatMap({ client.view.member($0)?.statusText }), !status.isEmpty {
                    Text(verbatim: status).foregroundStyle(.secondary).lineLimit(1)
                }
            }
        }
        .accessibilityIdentifier("setStatusLink")
    }

    /// The open workspace and the ones kept on this iPhone, like the Mac
    /// app's Switch Workspace menu: tap one to open it, swipe to remove it.
    var workspacesSection: some View {
        Section {
            HStack(spacing: 12) {
                WorkspaceRow(title: client.view.name.isEmpty ? client.settings.server : client.view.name,
                             detail: client.settings.server)
                Spacer()
                Image(systemName: "checkmark").foregroundStyle(.tint)
            }
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isSelected)
            .accessibilityIdentifier("currentWorkspace")
            ForEach(client.savedWorkspaces) { w in
                Button {
                    client.switchTo(w)
                    dismiss()
                } label: {
                    WorkspaceRow(title: w.title, detail: w.detail)
                }
                .foregroundStyle(.primary)
                .accessibilityIdentifier("savedWorkspace-\(w.server)")
                .swipeActions {
                    Button("Remove", role: .destructive) { forgetting = w }
                        .accessibilityIdentifier("forgetWorkspace")
                }
            }
            NavigationLink {
                FindWorkspacesView(onOpened: { dismiss() })
            } label: {
                Label("Find workspaces on this server", systemImage: "magnifyingglass")
            }
            .accessibilityIdentifier("findWorkspacesLink")
        } header: {
            Text("Workspaces")
        } footer: {
            Text("Find the other workspaces you have on this server, or connect to another server's address below. Each workspace stays on this iPhone with its sign-in and the changes it hasn't sent yet.")
        }
    }

    /// A different server has its own workspace: the one open now is kept on
    /// this iPhone (see Workspaces) and the new one starts from its own copy.
    func apply() {
        client.setServer(address)
        if client.state == .idle { client.connect() }
    }
}

/// Settings > Workspaces > Find workspaces on this server: every workspace
/// the account signs in to on this server (POST /auth/workspaces), as the
/// Mac lists them when signing in. Tapping one opens it, signing in there
/// with the same email and password; the one open now is kept on this iPhone.
struct FindWorkspacesView: View {
    @Environment(SyncClient.self) private var client
    let onOpened: () -> Void
    @State private var email = ""
    @State private var password = ""
    @State private var asking = false
    @State private var found: [FoundWorkspace]?
    @State private var error: String?

    var body: some View {
        Form {
            Section {
                TextField("Email", text: $email)
                    .keyboardType(.emailAddress)
                    .usernameField()
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("findEmailField")
                PasswordField("Password", text: $password, kind: .current, id: "findPasswordField")
                    .onSubmit(find)
                Button(action: find) {
                    if asking { ProgressView() } else { Text("Find workspaces") }
                }
                .disabled(asking || email.trimmed.isEmpty || password.isEmpty)
                .accessibilityIdentifier("findWorkspacesButton")
            } header: {
                Text("Account")
            } footer: {
                if let error {
                    Label(error, systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.red)
                        .accessibilityElement(children: .combine)
                        .accessibilityIdentifier("findWorkspacesError")
                } else {
                    Text("Your email and password on this server. They list your workspaces here and sign you in to the one you open.")
                }
            }
            if let found {
                Section {
                    ForEach(found) { w in
                        let here = ServerAddress.same(w.address, client.settings.server)
                        Button {
                            client.open(w, email: email, password: password)
                            onOpened()
                        } label: {
                            HStack(spacing: 12) {
                                WorkspaceRow(title: w.name.isEmpty ? w.address : w.name, detail: w.address)
                                Spacer()
                                if here { Image(systemName: "checkmark").foregroundStyle(.tint) }
                            }
                        }
                        .foregroundStyle(.primary)
                        .disabled(here)
                        .accessibilityAddTraits(here ? .isSelected : [])
                        .accessibilityIdentifier("foundWorkspace-\(ServerAddress.workspaceID(w.address) ?? "first")")
                    }
                } header: {
                    Text("Workspaces on this server")
                } footer: {
                    if found.allSatisfy({ ServerAddress.same($0.address, client.settings.server) }) {
                        Text("This account has no other workspace on this server.")
                            .accessibilityIdentifier("noOtherWorkspaces")
                    }
                }
            }
        }
        .navigationTitle("Find workspaces")
        .onAppear { if email.isEmpty { email = client.settings.accountEmail ?? "" } }
    }

    func find() {
        guard !asking, !email.trimmed.isEmpty, !password.isEmpty else { return }
        asking = true
        error = nil
        let (server, mail, pw) = (client.settings.server, email, password)
        Task {
            defer { asking = false }
            do {
                found = try await WorkspaceAPI.list(server: server, email: mail, password: pw)
            } catch AuthError.failed(let why) {
                found = nil
                error = why
            } catch {
                found = nil
                self.error = "That server address doesn't look right."
            }
        }
    }
}

/// One workspace in Settings > Workspaces.
struct WorkspaceRow: View {
    let title: String
    let detail: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "building.2")
                .foregroundStyle(.secondary)
                .frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).lineLimit(1)
                Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
    }
}

extension SavedWorkspace {
    var title: String { name.isEmpty ? server : name }

    /// Where it is, who is signed in there and what still waits to go up.
    var detail: String {
        var parts = [server]
        if let who { parts.append(who) }
        if waiting == 1 { parts.append(String(localized: "1 change waiting")) } else if waiting > 1 { parts.append(String(localized: "\(waiting) changes waiting")) }
        return parts.joined(separator: " · ")
    }
}

/// Set up an account for this signed-in device (email + password), or
/// change its password (needs the current one).
struct PasswordView: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.dismiss) private var dismiss
    @State private var email = ""
    @State private var old = ""
    @State private var password = ""
    @State private var confirm = ""
    @State private var localError: String?

    var hasAccount: Bool { client.settings.accountEmail != nil }

    var body: some View {
        Form {
            Section {
                TextField("Email", text: $email)
                    .keyboardType(.emailAddress)
                    .usernameField()
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("accountEmailField")
                if hasAccount {
                    PasswordField("Current password", text: $old, kind: .current, id: "oldPasswordField")
                }
                PasswordField("New password (at least \(minPasswordLength) characters)", text: $password, kind: .new, id: "accountPasswordField")
                PasswordField("New password again", text: $confirm, kind: .new, id: "accountConfirmField")
                    .onSubmit(save)
            } footer: {
                if let e = localError ?? failure {
                    Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                        .accessibilityElement(children: .combine)
                        .accessibilityIdentifier("passwordError")
                } else if client.accountRequest == .done {
                    Label("Saved. Sign in with \(client.settings.accountEmail ?? email) on your other devices.", systemImage: "checkmark.circle")
                        .accessibilityElement(children: .combine)
                        .accessibilityIdentifier("passwordSaved")
                } else {
                    Text(hasAccount ? "Your other devices stay signed in." : "Then you can sign in with your email and password on any device.")
                }
            }
            Section {
                Button(action: save) {
                    if client.accountRequest == .sending { ProgressView() } else { Text("Save password") }
                }
                .disabled(client.accountRequest == .sending || password.isEmpty)
                .accessibilityIdentifier("savePasswordButton")
            }
        }
        .navigationTitle(hasAccount ? "Change password" : "Set a password")
        .onAppear {
            client.resetAccountRequest()
            email = client.settings.accountEmail ?? client.me.flatMap { client.view.member($0)?.email } ?? ""
        }
        .onChange(of: client.accountRequest) {
            if client.accountRequest == .done { old = ""; password = ""; confirm = "" }
        }
    }

    var failure: String? {
        if case .failed(let m) = client.accountRequest { return m }
        return nil
    }

    func save() {
        client.resetAccountRequest()
        localError = nil
        if let why = newPasswordProblem(password, confirm: confirm) { localError = why; return }
        localError = client.setPassword(email: email, password: password, oldPassword: hasAccount ? old : nil)
    }
}

/// Delete this workspace from the team server, as its owner, after typing
/// its name. Everyone's apps lose it; the server keeps it 30 days.
struct DeleteWorkspaceView: View {
    @Environment(SyncClient.self) private var client
    var onDeleted: () -> Void = {}
    @State private var typed = ""
    @State private var deleting = false
    @State private var failure: String?

    var name: String { client.view.name }
    var ready: Bool { !deleting && typed.trimmingCharacters(in: .whitespaces) == name.trimmingCharacters(in: .whitespaces) }

    var body: some View {
        Form {
            Section {
                Text("It is deleted from the team server for everyone: their apps lose it too. The server keeps a copy for 30 days, which whoever runs the server can bring back.")
                TextField(name, text: $typed)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .disabled(deleting)
                    .accessibilityIdentifier("deleteWorkspaceName")
            } header: {
                Text("Delete \(name)?")
            } footer: {
                if let failure {
                    Label(failure, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                        .accessibilityElement(children: .combine)
                        .accessibilityIdentifier("deleteWorkspaceError")
                } else {
                    Text("Type the workspace's name to confirm.")
                }
            }
            Section {
                Button(role: .destructive) {
                    deleting = true
                    failure = nil
                    Task {
                        failure = await client.deleteWorkspace()
                        deleting = false
                        if failure == nil { onDeleted() }
                    }
                } label: {
                    if deleting { ProgressView() } else { Text("Delete workspace") }
                }
                .disabled(!ready)
                .accessibilityIdentifier("deleteWorkspaceButton")
            }
        }
        .navigationTitle("Delete workspace")
    }
}
