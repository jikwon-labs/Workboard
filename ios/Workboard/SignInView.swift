// Signing in to a server that refused this device (error code "auth"):
// with an email and password, with the code the person's computer shows
// (typed, or its QR code scanned), with an invite code (choosing a
// password), or with a reset code from "Forgot password?". Each attempt is a fresh
// connection; the server's message is shown as it is.

import SwiftUI
import WorkboardKit

struct SignInView: View {
    enum Mode: Hashable { case password, code, invite, forgot }

    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @State private var mode: Mode = .password
    // sign in
    @State private var email = ""
    @State private var password = ""
    // code from the computer
    @State private var linkCode = ""
    @State private var scanning = false
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
    /// GET /health says the server has no workspace yet.
    @State private var emptyServer = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 12) {
                        WMark().frame(width: 56, height: 56)
                        Text(L(title)).font(.largeTitle.bold())
                        if let note { Text(note).foregroundStyle(.secondary).accessibilityIdentifier("signInNote") }
                    }
                    .listRowBackground(Color.clear)
                    .padding(.vertical, 8)
                }
                switch mode {
                case .password: passwordForm
                case .code: codeForm
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
            .sheet(isPresented: $scanning) {
                // RootView acts on it like a link opened from the Camera app
                QRScanView { link in nav.openedLink = link }
            }
            .task(id: "\(client.settings.server)|\(client.signInMessage ?? "")") { await checkEmpty() }
        }
    }

    var title: String {
        switch mode {
        case .forgot: return "Reset password"
        case .invite: return "Join"
        case .code: return "Sign in"
        case .password:
            switch client.signInReason {
            case .signedOut: return "Signed out"
            case .gone: return "Workspace deleted"
            default: return "Sign in"
            }
        }
    }

    /// Why the server asked (its own words), above the forms.
    var note: String? {
        if emptyServer { return AuthAPI.emptyServerNote }
        switch client.signInReason {
        case .signedOut: return client.signInMessage ?? "This iPhone isn't signed in on the server any more."
        case .gone: return (client.signInMessage ?? WorkspaceAPI.goneMessage) + " Sign in to another workspace, or type another server's address."
        case .needed, nil: return mode == .password ? String(localized: "Sign in with your email and password, with the code your computer shows, or with the invite code you were given.") : nil
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
            // the server's own words, in Korean when the table has them
            Label(L(e), systemImage: "exclamationmark.triangle")
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
            PasswordField("Password", text: $password, kind: .current, id: "passwordField")
                .onSubmit(signIn)
            Button(action: signIn) { busyLabel("Sign in") }
                .disabled(client.signingIn || email.trimmingCharacters(in: .whitespaces).isEmpty || password.isEmpty)
                .accessibilityIdentifier("signInButton")
        } header: {
            Text("Account")
        } footer: {
            errorFooter
        }
        Section {
            Button("Sign in with a code from your computer") { switchTo(.code) }
                .accessibilityIdentifier("codeModeButton")
            Button("I have an invite code") { switchTo(.invite) }
                .accessibilityIdentifier("inviteModeButton")
            Button("Forgot password?") { resetEmail = email; switchTo(.forgot) }
                .accessibilityIdentifier("forgotButton")
        }
    }

    @ViewBuilder var codeForm: some View {
        Section {
            TextField("ABC123", text: $linkCode)
                .textInputAutocapitalization(.characters)
                .autocorrectionDisabled()
                .keyboardType(.asciiCapable)
                .font(.title2.monospaced())
                .accessibilityIdentifier("linkCodeField")
                .onChange(of: linkCode) { _, new in
                    let t = LinkCode.typed(new)
                    if t != new { linkCode = t }
                }
                .onSubmit(signInWithCode)
            Button(action: signInWithCode) { busyLabel("Sign in") }
                .disabled(client.signingIn || linkCode.count < LinkCode.length)
                .accessibilityIdentifier("linkSignInButton")
        } header: {
            Text("Code from your computer")
        } footer: {
            if shownError != nil { errorFooter } else { Text("On your computer, open Work together > Sign in on your phone. The code works once, for 5 minutes.") }
        }
        Section {
            Button { scanning = true } label: { Label("Scan QR code", systemImage: "qrcode.viewfinder") }
                .accessibilityIdentifier("scanButton")
        }
        Section {
            Button("I already have a password") { switchTo(.password) }
                .accessibilityIdentifier("signInModeButton")
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
            PasswordField("Password (at least \(minPasswordLength) characters)", text: $newPassword, kind: .new, id: "newPasswordField")
            PasswordField("Password again", text: $confirm, kind: .new, id: "confirmPasswordField")
                .onSubmit(join)
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
                PasswordField("New password (at least \(minPasswordLength) characters)", text: $resetPassword, kind: .new, id: "resetPasswordField")
                PasswordField("New password again", text: $resetConfirm, kind: .new, id: "resetConfirmField")
                    .onSubmit(reset)
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
                    .onSubmit { useAddress() }
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
            Text(L(text))
        }
    }

    // MARK: actions

    /// A different server starts from its own workspace; the one here is
    /// kept on this iPhone (see SettingsView.apply).
    func useAddress() {
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
        if localError == nil { nav.openingWorkspace = true }
    }

    func signIn() {
        send(.password(email: email, password: password))
    }

    func signInWithCode() {
        send(.link(code: linkCode))
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
    /// A sign-in link from the computer was sent already (RootView.use):
    /// show its code, and the server's answer under it.
    func takeInvite() {
        guard let link = nav.invite else { return }
        nav.invite = nil
        if link.fromComputer {
            linkCode = link.code
            switchTo(.code)
            attemptMode = .code
            return
        }
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
