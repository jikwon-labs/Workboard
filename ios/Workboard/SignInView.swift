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
    /// What the forms hold, kept while switching between them.
    @State private var form = SignInForm()
    @State private var scanning = false
    @State private var editServer = false
    @State private var address = ""

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
                switch form.mode {
                case .password: PasswordSignInForm(form: form)
                case .code: CodeSignInForm(form: form, scanning: $scanning)
                case .invite: InviteSignInForm(form: form)
                case .forgot: ForgotPasswordForm(form: form)
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
        switch form.mode {
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
        if form.emptyServer { return AuthAPI.emptyServerNote }
        switch client.signInReason {
        case .signedOut: return client.signInMessage ?? "This iPhone isn't signed in on the server any more."
        case .gone: return (client.signInMessage ?? WorkspaceAPI.goneMessage) + " Sign in to another workspace, or type another server's address."
        case .needed, nil: return form.mode == .password ? String(localized: "Sign in with your email and password, with the code your computer shows, or with the invite code you were given.") : nil
        case .failed: return nil
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
        guard !server.isEmpty else { form.emptyServer = false; return }
        let has = await AuthAPI.hasWorkspace(server: server)
        if server == client.settings.server { form.emptyServer = has == false }
    }

    /// An invite link opened from outside the app: fill in the invite form.
    /// A sign-in link from the computer was sent already (RootView.use):
    /// show its code, and the server's answer under it.
    func takeInvite() {
        guard let link = nav.invite else { return }
        nav.invite = nil
        if link.fromComputer {
            form.linkCode = link.code
            form.switchTo(.code)
            form.attemptMode = .code
            return
        }
        form.code = link.code
        if let e = link.email { form.inviteEmail = e }
        form.switchTo(.invite)
    }
}

/// What is typed in the sign-in forms, and how the last attempt went.
@MainActor
@Observable
final class SignInForm {
    var mode: SignInView.Mode = .password
    // sign in
    var email = ""
    var password = ""
    // code from the computer
    var linkCode = ""
    // invite
    var code = ""
    var inviteEmail = ""
    var newPassword = ""
    var confirm = ""
    // forgot
    var resetEmail = ""
    var codeSent = false
    var resetCode = ""
    var resetPassword = ""
    var resetConfirm = ""
    var forgotInfo: String?
    var asking = false
    // shared
    var localError: String?
    /// The form the last attempt came from, so its error shows only there.
    var attemptMode: SignInView.Mode?
    /// GET /health says the server has no workspace yet.
    var emptyServer = false

    func switchTo(_ m: SignInView.Mode) {
        localError = nil
        forgotInfo = nil
        mode = m
    }

    func send(_ r: SignInRequest, client: SyncClient, nav: Nav) {
        attemptMode = mode
        localError = client.signIn(r)
        if localError == nil { nav.openingWorkspace = true }
    }

    /// The error under the form that was sent (local check or the server's answer).
    func shownError(_ client: SyncClient) -> String? {
        if let localError { return localError }
        if emptyServer { return nil } // the note above says what to do instead
        if client.signInReason == .failed, attemptMode == mode, !client.signingIn { return client.signInMessage }
        return nil
    }
}

/// The error under a sign-in form.
private struct SignInError: View {
    let error: String?

    var body: some View {
        if let e = error {
            // the server's own words, in Korean when the table has them
            Label(L(e), systemImage: "exclamationmark.triangle")
                .foregroundStyle(.red)
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("signInError")
        }
    }
}

/// A form's button: its title, or a spinner while signing in.
private struct BusyLabel: View {
    @Environment(SyncClient.self) private var client
    let text: String
    var busy: Bool?

    var body: some View {
        if busy ?? client.signingIn {
            ProgressView().accessibilityLabel("Signing in")
        } else {
            Text(L(text))
        }
    }
}

/// Email and password.
private struct PasswordSignInForm: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Bindable var form: SignInForm

    var body: some View {
        Section {
            TextField("Email", text: $form.email)
                .keyboardType(.emailAddress)
                .usernameField()
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .accessibilityIdentifier("emailField")
            PasswordField("Password", text: $form.password, kind: .current, id: "passwordField")
                .onSubmit(signIn)
            Button(action: signIn) { BusyLabel(text: "Sign in") }
                .disabled(client.signingIn || form.email.trimmingCharacters(in: .whitespaces).isEmpty || form.password.isEmpty)
                .accessibilityIdentifier("signInButton")
        } header: {
            Text("Account")
        } footer: {
            SignInError(error: form.shownError(client))
        }
        Section {
            Button("Sign in with a code from your computer") { form.switchTo(.code) }
                .accessibilityIdentifier("codeModeButton")
            Button("I have an invite code") { form.switchTo(.invite) }
                .accessibilityIdentifier("inviteModeButton")
            Button("Forgot password?") { form.resetEmail = form.email; form.switchTo(.forgot) }
                .accessibilityIdentifier("forgotButton")
        }
    }

    func signIn() {
        form.send(.password(email: form.email, password: form.password), client: client, nav: nav)
    }
}

/// The code the person's computer shows, typed or scanned.
private struct CodeSignInForm: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Bindable var form: SignInForm
    @Binding var scanning: Bool

    var body: some View {
        let error = form.shownError(client)
        Section {
            TextField("ABC123", text: $form.linkCode)
                .textInputAutocapitalization(.characters)
                .autocorrectionDisabled()
                .keyboardType(.asciiCapable)
                .font(.title2.monospaced())
                .accessibilityIdentifier("linkCodeField")
                .onChange(of: form.linkCode) { _, new in
                    let t = LinkCode.typed(new)
                    if t != new { form.linkCode = t }
                }
                .onSubmit(signInWithCode)
            Button(action: signInWithCode) { BusyLabel(text: "Sign in") }
                .disabled(client.signingIn || form.linkCode.count < LinkCode.length)
                .accessibilityIdentifier("linkSignInButton")
        } header: {
            Text("Code from your computer")
        } footer: {
            if error != nil { SignInError(error: error) } else { Text("On your computer, open Work together > Sign in on your phone. The code works once, for 5 minutes.") }
        }
        Section {
            Button { scanning = true } label: { Label("Scan QR code", systemImage: "qrcode.viewfinder") }
                .accessibilityIdentifier("scanButton")
        }
        Section {
            Button("I already have a password") { form.switchTo(.password) }
                .accessibilityIdentifier("signInModeButton")
        }
    }

    func signInWithCode() {
        form.send(.link(code: form.linkCode), client: client, nav: nav)
    }
}

/// An invite code and a new password.
private struct InviteSignInForm: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Bindable var form: SignInForm

    var body: some View {
        let error = form.shownError(client)
        Section {
            TextField("XXXX-XXXX-XXXX", text: $form.code)
                .textInputAutocapitalization(.characters)
                .autocorrectionDisabled()
                .font(.title3.monospaced())
                .accessibilityIdentifier("inviteField")
                .onChange(of: form.code) { old, new in readPastedLink(old: old, new: new) }
            TextField("Email (optional)", text: $form.inviteEmail)
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
            PasswordField("Password (at least \(minPasswordLength) characters)", text: $form.newPassword, kind: .new, id: "newPasswordField")
            PasswordField("Password again", text: $form.confirm, kind: .new, id: "confirmPasswordField")
                .onSubmit(join)
            Button(action: join) { BusyLabel(text: "Join") }
                .disabled(client.signingIn || form.code.trimmingCharacters(in: .whitespaces).isEmpty || form.newPassword.isEmpty)
                .accessibilityIdentifier("joinButton")
        } header: {
            Text("Choose a password")
        } footer: {
            if error != nil { SignInError(error: error) } else { Text("You sign in with your email and this password on your other devices.") }
        }
        Section {
            Button("I already have a password") { form.switchTo(.password) }
                .accessibilityIdentifier("signInModeButton")
        }
    }

    func join() {
        if let why = newPasswordProblem(form.newPassword, confirm: form.confirm) { form.localError = why; return }
        let e = form.inviteEmail.trimmingCharacters(in: .whitespaces)
        form.send(.invite(code: form.code, email: e.isEmpty ? nil : e, newPassword: form.newPassword), client: client, nav: nav)
        // the account email to fill in next time
        if !e.isEmpty { form.email = e }
    }

    /// Pasting the whole link (or an email with it) into the code field.
    func readPastedLink(old: String, new: String) {
        guard new.count - old.count > 1, let link = InviteLink.find(in: new) else { return }
        form.code = link.code
        if let e = link.email { form.inviteEmail = e }
        if let s = link.server, !ServerAddress.same(s, client.settings.server) {
            client.setServer(s, freshCopy: true)
        }
    }
}

/// "Forgot password?": a code by email, then a new password.
private struct ForgotPasswordForm: View {
    @Environment(SyncClient.self) private var client
    @Environment(Nav.self) private var nav
    @Bindable var form: SignInForm

    var body: some View {
        let error = form.shownError(client)
        Section {
            TextField("Email", text: $form.resetEmail)
                .keyboardType(.emailAddress)
                .usernameField()
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .accessibilityIdentifier("forgotEmailField")
            Button(action: askForCode) { BusyLabel(text: form.codeSent ? "Send another code" : "Send code", busy: form.asking) }
                .disabled(form.asking || form.resetEmail.trimmingCharacters(in: .whitespaces).isEmpty)
                .accessibilityIdentifier("sendCodeButton")
        } header: {
            Text("Your account email")
        } footer: {
            if let info = form.forgotInfo {
                Text(info).accessibilityIdentifier("forgotInfo")
            } else if !form.codeSent, error != nil {
                SignInError(error: error)
            } else {
                Text("The server emails you a code to set a new password.")
            }
        }
        if form.codeSent {
            Section {
                TextField("XXXX-XXXX-XXXX", text: $form.resetCode)
                    .textInputAutocapitalization(.characters)
                    .autocorrectionDisabled()
                    .font(.title3.monospaced())
                    .accessibilityIdentifier("resetCodeField")
                PasswordField("New password (at least \(minPasswordLength) characters)", text: $form.resetPassword, kind: .new, id: "resetPasswordField")
                PasswordField("New password again", text: $form.resetConfirm, kind: .new, id: "resetConfirmField")
                    .onSubmit(reset)
                Button(action: reset) { BusyLabel(text: "Set password and sign in") }
                    .disabled(client.signingIn || form.resetCode.trimmingCharacters(in: .whitespaces).isEmpty || form.resetPassword.isEmpty)
                    .accessibilityIdentifier("resetButton")
            } header: {
                Text("Code from the email")
            } footer: {
                SignInError(error: error)
            }
        }
        Section {
            Button("Back to sign in") { form.switchTo(.password) }
                .accessibilityIdentifier("signInModeButton")
        }
    }

    func reset() {
        if let why = newPasswordProblem(form.resetPassword, confirm: form.resetConfirm) { form.localError = why; return }
        form.send(.reset(email: form.resetEmail, code: form.resetCode, newPassword: form.resetPassword), client: client, nav: nav)
    }

    func askForCode() {
        form.localError = nil
        form.forgotInfo = nil
        form.asking = true
        let (server, mail) = (client.settings.server, form.resetEmail)
        Task {
            defer { form.asking = false }
            do {
                if try await AuthAPI.forgot(server: server, email: mail) {
                    form.codeSent = true
                    form.forgotInfo = "If \(mail.trimmingCharacters(in: .whitespaces)) has an account here, a code is on its way. It works for 30 minutes."
                } else {
                    form.codeSent = false
                    form.forgotInfo = "This server can't send email. Ask the owner or an admin for a new invite code, then use \"I have an invite code\"."
                }
            } catch AuthError.failed(let why) {
                form.localError = why
            } catch {
                form.localError = "That server address doesn't look right."
            }
        }
    }
}
