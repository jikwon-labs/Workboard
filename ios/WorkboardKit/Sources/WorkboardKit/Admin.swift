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

/// A request sent to the server whose answer hasn't come yet. The server
/// answers the requests on one connection one by one, in order, but it also
/// sends access lists nobody asked for (after anyone's change, the asker's
/// own included) and refusals of pushed changes, so each answer is matched
/// to the kind of request it can answer, never to what it shows: an answer
/// is worked out after the change, so another admin's change to the same
/// person can already be in it.
public enum PendingAsk: Equatable, Sendable {
    case password
    case accessList
    case invite(member: UInt64)
    case setRole(member: UInt64, role: Role)
    case revoke(member: UInt64)
    case backups
    case restore(name: String)

    public var isAdmin: Bool { self != .password }

    /// Asks the server answers with an access list (Hub::request answers
    /// set_role and revoke with access_for).
    var wantsAccess: Bool {
        switch self {
        case .accessList, .setRole, .revoke: return true
        default: return false
        }
    }

    /// How long to wait for the answer before giving up on it: making a
    /// code may email it and a restore puts a whole workspace back, so
    /// those get longer.
    func patience(_ base: TimeInterval) -> TimeInterval {
        switch self {
        case .invite, .restore: return base * 4
        default: return base
        }
    }

    /// Whether `m` is of the kind that answers this request. Invites and
    /// restores say which one they answer (only the asker gets them).
    func answered(by m: ServerMessage) -> Bool {
        switch (self, m) {
        case (.password, .account): return true
        case let (.invite(who), .invite(member, _, _, _)): return who == member
        case (.accessList, .access), (.setRole, .access), (.revoke, .access): return true
        case (.backups, .backups): return true
        case let (.restore(name), .restored(n)): return name == n
        default: return false
        }
    }

    /// Answers the server only ever sends to the one who asked.
    static func onlyAnswers(_ m: ServerMessage) -> Bool {
        switch m {
        case .account, .invite, .backups, .restored: return true
        default: return false
        }
    }
}

/// A request on its way, with when it was sent.
struct WaitingAsk: Equatable, Sendable {
    var ask: PendingAsk
    var sent: Date
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
    public func requestAccessList() { adminSend(.accessList, .accessList) }

    /// A fresh one-time code for `member` (replacing an earlier one); the
    /// server also emails it when it can.
    public func createInvite(member: UInt64) {
        lastInvite = nil
        adminSend(.createInvite(member: member, role: .member), .invite(member: member))
    }

    public func setRole(member: UInt64, role: Role) { adminSend(.setRole(member: member, role: role), .setRole(member: member, role: role)) }

    /// Sign out every device of this person, cancel their code and remove
    /// their password.
    public func revoke(member: UInt64) { adminSend(.revoke(member: member), .revoke(member: member)) }

    /// The server's backups, newest first (owner only).
    public func requestBackups() { adminSend(.backupList, .backups) }

    /// Go back to a backup: deleted items come back, newer ones stay (owner only).
    public func restore(backup name: String) {
        adminNotice = nil
        adminSend(.restore(name: name), .restore(name: name))
    }

    public func clearAdminMessages() {
        adminError = nil
        adminNotice = nil
        adminUnconfirmed = nil
    }

    /// Said when the server didn't answer a request in time.
    nonisolated static let unconfirmedNote = "The server didn't confirm that in time. What you see here is the latest it sent; check again in a moment."
    nonisolated static let passwordUnconfirmed = "The server didn't answer in time. Check your connection and try again."

    private func adminSend(_ m: ClientMessage, _ ask: PendingAsk) {
        adminError = nil
        adminUnconfirmed = nil
        guard state.isConnected, let ws = socket else {
            adminError = "Connect to the server first."
            return
        }
        wait(for: ask)
        send(m, on: ws)
    }

    /// Put `ask` on the waiting list and give up on it if no answer comes
    /// in time, so a lost answer can't take a later request's refusal.
    func wait(for ask: PendingAsk) {
        asked.append(WaitingAsk(ask: ask, sent: Date()))
        let wait = ask.patience(askTimeout)
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64((wait + 0.05) * 1_000_000_000))
            self?.expireAsks()
        }
    }

    /// Drop the requests that waited too long, with a gentle note: the
    /// change may well have been made, but the answer never came.
    func expireAsks(now: Date = Date()) {
        let late = asked.filter { now.timeIntervalSince($0.sent) >= $0.ask.patience(askTimeout) }
        guard !late.isEmpty else { return }
        asked.removeAll { late.contains($0) }
        if late.contains(where: { $0.ask == .password }), accountRequest == .sending {
            accountRequest = .failed(Self.passwordUnconfirmed)
        }
        if late.contains(where: \.ask.isAdmin) { adminUnconfirmed = Self.unconfirmedNote }
    }

    /// Take the request `m` answers off the waiting list. Answers come in
    /// order, so an access list answers the oldest request only when that
    /// one asked for one (otherwise it was sent unasked after someone's
    /// change); what the list shows doesn't matter. An answer only ever
    /// sent to the asker also shows the ones before it were answered.
    func settle(_ m: ServerMessage) {
        if PendingAsk.onlyAnswers(m) {
            guard let i = asked.firstIndex(where: { $0.ask.answered(by: m) }) else { return }
            asked.removeFirst(i + 1)
        } else if let first = asked.first, first.ask.wantsAccess, first.ask.answered(by: m) {
            asked.removeFirst()
        }
    }

    /// The server's answers (and the access lists it sends unasked after
    /// any change).
    func handleAdmin(_ m: ServerMessage) {
        switch m {
        case let .invite(member, code, emailed, mailError):
            settle(m)
            lastInvite = InviteResult(member: member, code: code, emailed: emailed, mailError: mailError)
            if var list = people, let i = list.firstIndex(where: { $0.member == member }) {
                list[i].invite = code
                people = list
            }
        case let .access(list, role):
            // also an answer to set_role, revoke and access_list
            settle(m)
            people = list
            if settings.role != role.rawValue {
                settings.role = role.rawValue
                storage.save(settings)
            }
            if role != .owner { backups = nil }
        case let .backups(list):
            settle(m)
            backups = list
        case let .restored(name):
            settle(m)
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
        asked.removeAll { $0.ask.isAdmin }
        adminError = nil
        adminNotice = nil
        adminUnconfirmed = nil
    }
}
