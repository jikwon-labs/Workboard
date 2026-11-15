// Owners and admins: everyone's access, invite codes and roles; the owner:
// the server's backups. Like Work together > People and Backups in the Mac
// app. Only what the person may do is offered (AccessRules mirrors the
// server's rules; the server checks again).

import SwiftUI
import WorkboardKit

struct PeopleView: View {
    @Environment(SyncClient.self) private var client
    @State private var confirmRemove: Person?

    var rules: AccessRules { client.accessRules }

    var body: some View {
        List {
            if !rules.seesPeople {
                Text("Only the owner or an admin can manage access.")
                    .foregroundStyle(.secondary)
            } else if let people = client.people {
                if let invite = client.lastInvite {
                    Section { inviteNote(invite) }
                }
                Section {
                    // like the Mac: people the workspace knows
                    ForEach(people.filter { client.view.member($0.member) != nil }) { p in
                        row(p)
                    }
                } footer: {
                    Text("An invite code works once, for 7 days, and signs in as that person. Add people to the workspace from the Mac first.")
                }
            } else if client.state.isConnected {
                HStack { ProgressView(); Text("Asking the server…").foregroundStyle(.secondary) }
            } else {
                Text("Connect to the server to see who has access.").foregroundStyle(.secondary)
            }
            if let e = client.adminError {
                Section {
                    Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                        .accessibilityElement(children: .combine)
                        .accessibilityIdentifier("adminError")
                }
            }
        }
        .navigationTitle("People")
        .onAppear {
            client.clearAdminMessages()
            if rules.seesPeople { client.requestAccessList() }
        }
        .confirmationDialog("Remove access?", isPresented: Binding(get: { confirmRemove != nil }, set: { if !$0 { confirmRemove = nil } }),
                            titleVisibility: .visible, presenting: confirmRemove) { p in
            Button("Remove access", role: .destructive) { client.revoke(member: p.member) }
                .accessibilityIdentifier("confirmRemoveAccess")
        } message: { p in
            Text("\(name(p)) is signed out on every device, and their invite code and password stop working.")
        }
    }

    func name(_ p: Person) -> String { client.view.member(p.member)?.name ?? "Member \(p.member)" }

    func devices(_ n: Int) -> String {
        switch n {
        case 0: return "not signed in"
        case 1: return "1 device"
        default: return "\(n) devices"
        }
    }

    @ViewBuilder func row(_ p: Person) -> some View {
        let member = client.view.member(p.member)
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 12) {
                Avatar(member: member)
                VStack(alignment: .leading, spacing: 2) {
                    Text(name(p) + (p.member == client.me ? " (you)" : ""))
                    Text([p.role.label, devices(p.devices), p.account ? "password set" : nil].compactMap { $0 }.joined(separator: " · "))
                        .font(.caption).foregroundStyle(.secondary)
                        .accessibilityIdentifier("personRole-\(p.member)")
                }
                Spacer()
                actions(p)
            }
            if let code = p.invite {
                HStack {
                    Text(code).font(.callout.monospaced()).textSelection(.enabled)
                        .accessibilityIdentifier("inviteCode-\(p.member)")
                    Spacer()
                    Button { UIPasteboard.general.string = code } label: { Image(systemName: "doc.on.doc") }
                        .buttonStyle(.borderless)
                        .accessibilityLabel("Copy code")
                    if let url = InviteLink(server: client.settings.server, code: code, email: member?.email.nilIfBlank).url {
                        ShareLink(item: url, message: Text("Join \(client.view.name) on Workboard with this link, or type the code \(code). It works once, for 7 days.")) {
                            Image(systemName: "square.and.arrow.up")
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel("Share invite")
                    }
                }
                .padding(.leading, 44)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("person-\(p.member)")
    }

    @ViewBuilder func actions(_ p: Person) -> some View {
        let invite = rules.mayInvite(p), role = rules.mayChangeRole(of: p), remove = rules.mayRemoveAccess(of: p)
        if invite || role || remove {
            Menu {
                if invite {
                    Button { client.createInvite(member: p.member) } label: {
                        Label(p.invite == nil ? "Invite code" : "New code", systemImage: "key")
                    }
                }
                if role {
                    if p.role == .admin {
                        Button { client.setRole(member: p.member, role: .member) } label: { Label("Make member", systemImage: "person") }
                    } else {
                        Button { client.setRole(member: p.member, role: .admin) } label: { Label("Make admin", systemImage: "person.badge.shield.checkmark") }
                    }
                }
                if remove {
                    Button(role: .destructive) { confirmRemove = p } label: { Label("Remove access", systemImage: "person.slash") }
                }
            } label: {
                Image(systemName: "ellipsis.circle").imageScale(.large)
            }
            .disabled(!client.state.isConnected)
            .accessibilityLabel("Actions for \(name(p))")
            .accessibilityIdentifier("personMenu-\(p.member)")
        }
    }

    @ViewBuilder func inviteNote(_ i: InviteResult) -> some View {
        let who = client.view.member(i.member)?.name ?? "Member \(i.member)"
        if let to = i.emailed {
            Label("Emailed \(who)'s invite code to \(to).", systemImage: "envelope")
                .accessibilityIdentifier("inviteNote")
        } else if let why = i.mailError {
            Label("Couldn't email \(who)'s invite code (\(why)). Copy or share it below.", systemImage: "envelope.badge")
                .accessibilityIdentifier("inviteNote")
        } else {
            Label("New invite code for \(who). Copy or share it below.", systemImage: "key")
                .accessibilityIdentifier("inviteNote")
        }
    }
}

struct BackupsView: View {
    @Environment(SyncClient.self) private var client
    @State private var confirm: Backup?

    var body: some View {
        List {
            if !client.accessRules.seesBackups {
                Text("Only the owner can see and restore backups.").foregroundStyle(.secondary)
            } else if let list = client.backups {
                if let n = client.adminNotice {
                    Section {
                        Label(n, systemImage: "checkmark.circle")
                            .accessibilityElement(children: .combine)
                            .accessibilityIdentifier("restoreNotice")
                    }
                }
                Section {
                    if list.isEmpty {
                        Text("No backups yet. The first one is made within a minute of the server having a workspace.")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(list.prefix(80)) { b in
                        HStack {
                            VStack(alignment: .leading) {
                                Text(b.label)
                                Text("\((b.bytes + 1023) / 1024) KB").font(.caption).foregroundStyle(.secondary)
                            }
                            Spacer()
                            Button("Restore") { confirm = b }
                                .buttonStyle(.borderless)
                                .disabled(!client.state.isConnected)
                                .accessibilityIdentifier("restore-\(b.name)")
                        }
                    }
                } footer: {
                    Text("Go back to an earlier copy of the team's workspace. Deleted items come back; anything made since stays. The server keeps a copy from just before.")
                }
            } else if client.state.isConnected {
                HStack { ProgressView(); Text("Asking the server…").foregroundStyle(.secondary) }
            } else {
                Text("Connect to the server to see its backups.").foregroundStyle(.secondary)
            }
            if let e = client.adminError {
                Section {
                    Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(.red)
                        .accessibilityIdentifier("adminError")
                }
            }
        }
        .navigationTitle("Backups")
        .onAppear {
            client.clearAdminMessages()
            if client.accessRules.seesBackups { client.requestBackups() }
        }
        .confirmationDialog("Restore this backup?", isPresented: Binding(get: { confirm != nil }, set: { if !$0 { confirm = nil } }),
                            titleVisibility: .visible, presenting: confirm) { b in
            Button("Yes, restore", role: .destructive) { client.restore(backup: b.name) }
                .accessibilityIdentifier("confirmRestore")
        } message: { b in
            Text("Everyone's workspace goes back to \(b.label). Deleted items come back; anything made since stays.")
        }
    }
}

/// This iPhone's Trash: deleted tasks and docs, to restore or delete for good.
struct TrashView: View {
    @Environment(SyncClient.self) private var client
    @State private var message: String?
    @State private var confirmEmpty = false

    var body: some View {
        List {
            if let m = message {
                Section { Text(m).accessibilityIdentifier("trashMessage") }
            }
            Section {
                if client.trash.isEmpty {
                    Text("Nothing in Trash. Tasks and docs you delete on this iPhone show up here.")
                        .foregroundStyle(.secondary)
                }
                ForEach(client.trash.reversed()) { item in
                    HStack {
                        Image(systemName: item.kind == "Doc" ? "doc.text" : "checkmark.circle")
                            .foregroundStyle(.secondary)
                        VStack(alignment: .leading) {
                            Text(item.title.isEmpty ? "Untitled" : item.title)
                            Text("\(item.kind == "Doc" ? "Doc" : "Task") · deleted \(Fmt.relative(item.deleted))")
                                .font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("Restore") { restore(item) }
                            .buttonStyle(.borderless)
                            .disabled(!client.canEdit)
                            .accessibilityIdentifier("restoreTrash-\(item.title)")
                    }
                    .swipeActions {
                        Button("Delete", role: .destructive) { client.deleteFromTrash(item.id) }
                    }
                    .accessibilityElement(children: .contain)
                }
            } footer: {
                Text("Trash is kept on this iPhone only. Deleted items are gone for everyone; restoring one brings it back for everyone.")
            }
        }
        .navigationTitle("Trash")
        .toolbar {
            if !client.trash.isEmpty {
                Button("Empty", role: .destructive) { confirmEmpty = true }
            }
        }
        .confirmationDialog("Empty Trash?", isPresented: $confirmEmpty, titleVisibility: .visible) {
            Button("Empty Trash", role: .destructive) { client.emptyTrash() }
        } message: {
            Text("The items can't be restored afterwards.")
        }
    }

    func restore(_ item: TrashItem) {
        switch client.restoreFromTrash(item.id) {
        case .restored: message = "Restored \"\(item.title)\"."
        case .movedTo(let list): message = "Restored \"\(item.title)\". Its list is gone, so it went to \(list)."
        case .nowhere(let why): message = why
        case nil: message = "Sign in to restore."
        }
    }
}

extension String {
    var nilIfBlank: String? { trimmingCharacters(in: .whitespaces).isEmpty ? nil : self }
}
