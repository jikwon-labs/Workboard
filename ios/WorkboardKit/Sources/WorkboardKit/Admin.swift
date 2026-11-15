// Managing access from the iPhone, like Work together > People and Backups in
// the Mac app (src/app/together.rs). The server decides (Hub::request in
// server/src/lib.rs); `AccessRules` mirrors its rules so the screens only
// offer what the person may do:
//
//   - owners and admins see everyone's access, change roles (not the
//     owner's, and nobody becomes owner) and remove access (not the owner's);
//   - an invite code signs in as its person, so admins make codes only for
//     plain members who haven't set up a password, and only the owner makes
//     one for the owner;
//   - only the owner sees and restores backups.

import Foundation

/// A new invite code from the server.
public struct InviteResult: Equatable, Sendable {
    public var member: UInt64
    public var code: String
    /// The address the server emailed it to.
    public var emailed: String?
    /// Why mailing it failed (mail is set up but sending didn't work).
    public var mailError: String?
}

/// What someone with `role` may do to `person` (the server's rules).
public struct AccessRules: Sendable {
    public let role: Role
    public let me: UInt64?

    public init(role: Role, me: UInt64?) {
        self.role = role
        self.me = me
    }

    public var seesPeople: Bool { role.manages }
    public var seesBackups: Bool { role == .owner }

    /// Make admin / Make member.
    public func mayChangeRole(of p: Person) -> Bool {
        role.manages && p.role != .owner && p.member != me
    }

    public func mayRemoveAccess(of p: Person) -> Bool {
        role.manages && p.role != .owner && p.member != me
    }

    /// admin_may_invite: an admin only for plain members without a password.
    public func mayInvite(_ p: Person) -> Bool {
        guard role.manages, p.member != me else { return false }
        if role == .owner { return true }
        return p.role == .member && !p.account
    }
}

extension SyncClient {
    /// My role on this server (nil: no sign-in, or not said yet).
    public var myRole: Role? { settings.role.flatMap(Role.init(rawValue:)) }

    public var accessRules: AccessRules { AccessRules(role: myRole ?? .member, me: me) }

    /// Ask for everyone's access (owners and admins).
    public func requestAccessList() { adminSend(.accessList) }

    /// A fresh one-time code for `member` (replacing an earlier one); the
    /// server also emails it when it can.
    public func createInvite(member: UInt64) {
        lastInvite = nil
        adminSend(.createInvite(member: member, role: .member))
    }

    public func setRole(member: UInt64, role: Role) { adminSend(.setRole(member: member, role: role)) }

    /// Sign out every device of this person, cancel their code and remove
    /// their password.
    public func revoke(member: UInt64) { adminSend(.revoke(member: member)) }

    /// The server's backups, newest first (owner only).
    public func requestBackups() { adminSend(.backupList) }

    /// Go back to a backup: deleted items come back, newer ones stay (owner only).
    public func restore(backup name: String) {
        adminNotice = nil
        adminSend(.restore(name: name))
    }

    public func clearAdminMessages() {
        adminError = nil
        adminNotice = nil
    }

    private func adminSend(_ m: ClientMessage) {
        adminError = nil
        guard state.isConnected, let ws = socket else {
            adminError = "Connect to the server first."
            return
        }
        adminPending += 1
        send(m, on: ws)
    }

    /// The server's answers (and the access lists it sends unasked after
    /// any change).
    func handleAdmin(_ m: ServerMessage) {
        switch m {
        case let .invite(member, code, emailed, mailError):
            adminPending = max(0, adminPending - 1)
            lastInvite = InviteResult(member: member, code: code, emailed: emailed, mailError: mailError)
            if var list = people, let i = list.firstIndex(where: { $0.member == member }) {
                list[i].invite = code
                people = list
            }
        case let .access(list, role):
            // also an answer to set_role, revoke and access_list
            adminPending = max(0, adminPending - 1)
            people = list
            if settings.role != role.rawValue {
                settings.role = role.rawValue
                storage.save(settings)
            }
            if role != .owner { backups = nil }
        case let .backups(list):
            adminPending = max(0, adminPending - 1)
            backups = list
        case let .restored(name):
            adminPending = max(0, adminPending - 1)
            adminNotice = "Restored the team's workspace to \(Backup(name: name, bytes: 0).label). Everyone gets it now."
            // the copy from before the restore is a new backup
            requestBackups()
        default:
            break
        }
    }

    func clearAdmin() {
        people = nil
        backups = nil
        lastInvite = nil
        adminPending = 0
        adminError = nil
        adminNotice = nil
    }
}
