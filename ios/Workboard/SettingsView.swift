import SwiftUI
import WorkboardKit

struct SettingsView: View {
    @Environment(SyncClient.self) private var client
    @Environment(Notifier.self) private var notifier
    @Environment(\.dismiss) private var dismiss
    @State private var address = ""
    @State private var confirmSwitch = false

    var body: some View {
        NavigationStack {
            Form {
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
                        LabeledContent("Waiting to sync", value: "\(client.waitingCount) \(client.waitingCount == 1 ? "change" : "changes")")
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
                    } header: {
                        Text("Team")
                    } footer: {
                        Text(role == .owner ? "You're the owner of this server." : "You're an admin on this server.")
                    }
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
                    LabeledContent("Name", value: client.view.name)
                    LabeledContent("Members", value: "\(client.view.members.count)")
                    LabeledContent("Server revision", value: "\(client.rev)")
                    LabeledContent("This device's id tag", value: client.settings.tag.map(String.init) ?? "Not given yet")
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
            .onAppear { address = client.settings.server }
            .alert("Switch servers?", isPresented: $confirmSwitch) {
                Button("Switch", role: .destructive) { apply(confirmed: true) }
                    .accessibilityIdentifier("confirmServerSwitch")
                Button("Cancel", role: .cancel) { address = client.settings.server }
            } message: {
                Text("\(client.waitingCount) \(client.waitingCount == 1 ? "change hasn't" : "changes haven't") reached \(client.settings.server) yet. A different server has its own workspace, so this iPhone starts over from that one and those changes are lost.")
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

    /// A different server has its own workspace: the copy here starts over
    /// from it (never merged into it). Ask first when edits would be lost.
    func apply() { apply(confirmed: false) }

    func apply(confirmed: Bool) {
        if !confirmed, client.isDifferentServer(address), client.waitingCount > 0 {
            confirmSwitch = true
            return
        }
        client.setServer(address)
        if client.state == .idle { client.connect() }
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
                    SecureField("Current password", text: $old)
                        .passwordField(.current)
                        .accessibilityIdentifier("oldPasswordField")
                }
                SecureField("New password (at least \(minPasswordLength) characters)", text: $password)
                    .passwordField(.new)
                    .accessibilityIdentifier("accountPasswordField")
                SecureField("New password again", text: $confirm)
                    .passwordField(.new)
                    .onSubmit(save)
                    .accessibilityIdentifier("accountConfirmField")
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
        guard password == confirm else { localError = "The two passwords don't match."; return }
        localError = client.setPassword(email: email, password: password, oldPassword: hasAccount ? old : nil)
    }
}
