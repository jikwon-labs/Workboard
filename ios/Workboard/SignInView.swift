// Signing in to a server that refused this device (error code "auth"):
// with an email and password, with an invite code (choosing a password),
// or with a reset code from "Forgot password?". Each attempt is a fresh
// connection; the server's message is shown as it is.

import SwiftUI
import WorkboardKit

struct SignInView: View {
    enum Mode: Hashable { case password, invite, forgot }

    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @State private var mode: Mode = .password
    // sign in
    @State private var email = ""
    @State private var password = ""
    // invite
    @State private var code = ""
    @State private var inviteEmail = ""
    @State private var newPassword = ""
    @State private var confirm = ""
    // forgot
    @State private var resetEmail = ""
    @State private var codeSent = false
    @State private var resetCode = ""
    @State private var resetPassword = ""
    @State private var resetConfirm = ""
    @State private var forgotInfo: String?
    @State private var asking = false
    // shared
    @State private var localError: String?
    /// The form the last attempt came from, so its error shows only there.
    @State private var attemptMode: Mode?
    @State private var editServer = false
    @State private var address = ""
    @State private var confirmSwitch = false
    /// GET /health says the server has no workspace yet.
    @State private var emptyServer = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 12) {
                        WMark().frame(width: 56, height: 56)
                        Text(title).font(.largeTitle.bold())
                        if let note { Text(note).foregroundStyle(.secondary).accessibilityIdentifier("signInNote") }
                    }
                    .listRowBackground(Color.clear)
                    .padding(.vertical, 8)
                }
                switch mode {
                case .password: passwordForm
                case .invite: inviteForm
                case .forgot: forgotForm
                }
                serverSection
            }
            .onAppear {
                address = client.settings.server
                takeInvite()
            }
            .onChange(of: nav.invite) { takeInvite() }
            .task(id: "\(client.settings.server)|\(client.signInMessage ?? "")") { await checkEmpty() }
            .alert("Switch servers?", isPresented: $confirmSwitch) {
                Button("Switch", role: .destructive) { useAddress(confirmed: true) }
                Button("Cancel", role: .cancel) { address = client.settings.server }
            } message: {
                Text(client.waitingCount == 1 ? "1 change hasn't reached \(client.settings.server) yet. A different server has its own workspace, so this iPhone starts over from that one and that change is lost." : "\(client.waitingCount) changes haven't reached \(client.settings.server) yet. A different server has its own workspace, so this iPhone starts over from that one and those changes are lost.")
            }
        }
    }

    var title: String {
        switch mode {
        case .forgot: return "Reset password"
        case .invite: return "Join"
        case .password: return client.signInReason == .signedOut ? "Signed out" : "Sign in"
        }
    }

    /// Why the server asked (its own words), above the forms.
    var note: String? {
        if emptyServer { return AuthAPI.emptyServerNote }
        switch client.signInReason {
        case .signedOut: return client.signInMessage ?? "This iPhone isn't signed in on the server any more."
        case .needed, nil: return mode == .password ? "Sign in with your email and password, or use the invite code you were given." : nil
        case .failed: return nil
        }
    }

    /// The error under the form that was sent (local check or the server's answer).
    var shownError: String? {
        if let localError { return localError }
        if emptyServer { return nil } // the note above says what to do instead
        if client.signInReason == .failed, attemptMode == mode, !client.signingIn { return client.signInMessage }
        return nil
    }

    @ViewBuilder var errorFooter: some View {
        if let e = shownError {
            Label(e, systemImage: "exclamationmark.triangle")
                .foregroundStyle(.red)
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("signInError")
        }
    }

    // MARK: forms

    @ViewBuilder var passwordForm: some View {
        Section {
            TextField("Email", text: $email)
                .keyboardType(.emailAddress)
                .usernameField()
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .accessibilityIdentifier("emailField")
            SecureField("Password", text: $password)
                .passwordField(.current)
                .onSubmit(signIn)
                .accessibilityIdentifier("passwordField")
            Button(action: signIn) { busyLabel("Sign in") }
                .disabled(client.signingIn || email.trimmingCharacters(in: .whitespaces).isEmpty || password.isEmpty)
                .accessibilityIdentifier("signInButton")
        } header: {
            Text("Account")
        } footer: {
            errorFooter
        }
        Section {
            Button("I have an invite code") { switchTo(.invite) }
                .accessibilityIdentifier("inviteModeButton")
            Button("Forgot password?") { resetEmail = email; switchTo(.forgot) }
                .accessibilityIdentifier("forgotButton")
        }
    }

    @ViewBuilder var inviteForm: some View {
        Section {
            TextField("XXXX-XXXX-XXXX", text: $code)
                .textInputAutocapitalization(.characters)
                .autocorrectionDisabled()
                .font(.title3.monospaced())
                .accessibilityIdentifier("inviteField")
                .onChange(of: code) { old, new in readPastedLink(old: old, new: new) }
            TextField("Email (optional)", text: $inviteEmail)
                .keyboardType(.emailAddress)
                .usernameField()
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .accessibilityIdentifier("inviteEmailField")
        } header: {
            Text("Invite code")
        } footer: {
            Text("The email you were invited with. Leave it empty if the invite came to your email. You can also paste the whole invite link or email here.")
        }
        Section {
            SecureField("Password (at least \(minPasswordLength) characters)", text: $newPassword)
                .passwordField(.new)
                .accessibilityIdentifier("newPasswordField")
            SecureField("Password again", text: $confirm)
                .passwordField(.new)
                .onSubmit(join)
                .accessibilityIdentifier("confirmPasswordField")
            Button(action: join) { busyLabel("Join") }
                .disabled(client.signingIn || code.trimmingCharacters(in: .whitespaces).isEmpty || newPassword.isEmpty)
                .accessibilityIdentifier("joinButton")
        } header: {
            Text("Choose a password")
        } footer: {
            if shownError != nil { errorFooter } else { Text("You sign in with your email and this password on your other devices.") }
        }
        Section {
            Button("I already have a password") { switchTo(.password) }
                .accessibilityIdentifier("signInModeButton")
        }
    }

    @ViewBuilder var forgotForm: some View {
        Section {
            TextField("Email", text: $resetEmail)
                .keyboardType(.emailAddress)
                .usernameField()
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .accessibilityIdentifier("forgotEmailField")
            Button(action: askForCode) { busyLabel(codeSent ? "Send another code" : "Send code", busy: asking) }
                .disabled(asking || resetEmail.trimmingCharacters(in: .whitespaces).isEmpty)
                .accessibilityIdentifier("sendCodeButton")
        } header: {
            Text("Your account email")
        } footer: {
            if let forgotInfo {
                Text(forgotInfo).accessibilityIdentifier("forgotInfo")
            } else if !codeSent, shownError != nil {
                errorFooter
            } else {
                Text("The server emails you a code to set a new password.")
            }
        }
        if codeSent {
            Section {
                TextField("XXXX-XXXX-XXXX", text: $resetCode)
                    .textInputAutocapitalization(.characters)
                    .autocorrectionDisabled()
                    .font(.title3.monospaced())
                    .accessibilityIdentifier("resetCodeField")
                SecureField("New password (at least \(minPasswordLength) characters)", text: $resetPassword)
                    .passwordField(.new)
                    .accessibilityIdentifier("resetPasswordField")
                SecureField("New password again", text: $resetConfirm)
                    .passwordField(.new)
                    .onSubmit(reset)
                    .accessibilityIdentifier("resetConfirmField")
                Button(action: reset) { busyLabel("Set password and sign in") }
                    .disabled(client.signingIn || resetCode.trimmingCharacters(in: .whitespaces).isEmpty || resetPassword.isEmpty)
                    .accessibilityIdentifier("resetButton")
            } header: {
                Text("Code from the email")
            } footer: {
                errorFooter
            }
        }
        Section {
            Button("Back to sign in") { switchTo(.password) }
                .accessibilityIdentifier("signInModeButton")
        }
    }

    var serverSection: some View {
        Section {
            if editServer {
                TextField("https://wb.example.com", text: $address)
                    .keyboardType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onSubmit { useAddress(confirmed: false) }
            } else {
                LabeledContent(client.settings.server) {
                    Button("Change") { address = client.settings.server; editServer = true }
                }
            }
        } header: {
            Text("Server")
        } footer: {
            switch client.state {
            case .waiting, .idle:
                if let e = client.lastError { Text(e) }
            case .connecting where !client.signingIn:
                Text("Connecting…")
            default:
                EmptyView()
            }
        }
    }

    @ViewBuilder func busyLabel(_ text: String, busy: Bool? = nil) -> some View {
        if busy ?? client.signingIn {
            ProgressView().accessibilityLabel("Signing in")
        } else {
            Text(text)
        }
    }

    // MARK: actions

    /// A different server starts from its own workspace (see SettingsView.apply).
    func useAddress(confirmed: Bool) {
        if !confirmed, client.isDifferentServer(address), client.waitingCount > 0 {
            confirmSwitch = true
            return
        }
        client.setServer(address)
        editServer = false
    }

    /// The server turned us away: is it because it's still empty?
    func checkEmpty() async {
        let server = client.settings.server
        guard !server.isEmpty else { emptyServer = false; return }
        let has = await AuthAPI.hasWorkspace(server: server)
        if server == client.settings.server { emptyServer = has == false }
    }

    func switchTo(_ m: Mode) {
        localError = nil
        forgotInfo = nil
        mode = m
    }

    func send(_ r: SignInRequest) {
        attemptMode = mode
        localError = client.signIn(r)
    }

    func signIn() {
        send(.password(email: email, password: password))
    }

    func join() {
        if let why = passwordProblem(newPassword) { localError = why; return }
        guard newPassword == confirm else { localError = "The two passwords don't match."; return }
        let e = inviteEmail.trimmingCharacters(in: .whitespaces)
        send(.invite(code: code, email: e.isEmpty ? nil : e, newPassword: newPassword))
        // the account email to fill in next time
        if !e.isEmpty { email = e }
    }

    func reset() {
        if let why = passwordProblem(resetPassword) { localError = why; return }
        guard resetPassword == resetConfirm else { localError = "The two passwords don't match."; return }
        send(.reset(email: resetEmail, code: resetCode, newPassword: resetPassword))
    }

    func askForCode() {
        localError = nil
        forgotInfo = nil
        asking = true
        let (server, mail) = (client.settings.server, resetEmail)
        Task {
            defer { asking = false }
            do {
                if try await AuthAPI.forgot(server: server, email: mail) {
                    codeSent = true
                    forgotInfo = "If \(mail.trimmingCharacters(in: .whitespaces)) has an account here, a code is on its way. It works for 30 minutes."
                } else {
                    codeSent = false
                    forgotInfo = "This server can't send email. Ask the owner or an admin for a new invite code, then use \"I have an invite code\"."
                }
            } catch AuthError.failed(let why) {
                localError = why
            } catch {
                localError = "That server address doesn't look right."
            }
        }
    }

    /// An invite link opened from outside the app: fill in the invite form.
    func takeInvite() {
        guard let link = nav.invite else { return }
        nav.invite = nil
        code = link.code
        if let e = link.email { inviteEmail = e }
        switchTo(.invite)
    }

    /// Pasting the whole link (or an email with it) into the code field.
    func readPastedLink(old: String, new: String) {
        guard new.count - old.count > 1, let link = InviteLink.find(in: new) else { return }
        code = link.code
        if let e = link.email { inviteEmail = e }
        if let s = link.server, !ServerAddress.same(s, client.settings.server) {
            client.setServer(s, freshCopy: true)
        }
    }
}
