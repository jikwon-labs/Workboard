// A command-line client for checking a server by hand.
//
//   wb-cli URL dump
//   wb-cli URL say CHANNEL_ID TEXT [--as MEMBER_ID]
//   wb-cli URL reply CHANNEL_ID MESSAGE_ID TEXT [--as MEMBER_ID]
//   wb-cli URL status TASK_ID STATUS [--as MEMBER_ID]
//   wb-cli URL doc DOC_ID BODY [--as MEMBER_ID]
//   wb-cli URL add LIST_ID NAME [--as MEMBER_ID]
//   wb-cli URL watch
//
// On a server that wants a sign-in, add --email E --password P (an account),
// or --code INVITE_CODE (signs in without setting a password, like older
// apps). The member then comes from the server and --as is ignored.
//
// It is a separate device each run (its own temporary folder).

import Foundation
import WorkboardKit

setvbuf(stdout, nil, _IOLBF, 0)
var argv = Array(CommandLine.arguments.dropFirst())
var asMember: UInt64 = 2
if let i = argv.firstIndex(of: "--as"), i + 1 < argv.count {
    asMember = UInt64(argv[i + 1]) ?? 2
    argv.removeSubrange(i...(i + 1))
}
func option(_ name: String) -> String? {
    guard let i = argv.firstIndex(of: name), i + 1 < argv.count else { return nil }
    let v = argv[i + 1]
    argv.removeSubrange(i...(i + 1))
    return v
}
let inviteCode = option("--code")
let email = option("--email")
let password = option("--password")
let request: SignInRequest? = inviteCode.map { .invite(code: $0, email: email, newPassword: password) }
    ?? email.flatMap { e in password.map { .password(email: e, password: $0) } }
guard argv.count >= 2 else {
    print("usage: wb-cli URL dump|watch|say|reply|status|doc|add ...")
    exit(2)
}

func summary(_ v: WorkspaceView) -> String {
    var out = ["workspace: \(v.name)"]
    for c in v.channels {
        let last = c.messages.suffix(3).map { m in "    [\(m.id)] \(v.member(m.author)?.name ?? "?"): \(m.text)" + (m.replies.isEmpty ? "" : " (+\(m.replies.count) replies: \(m.replies.last!.text))") }
        out.append("  #\(c.name.isEmpty ? "dm" : c.name) (\(c.id))")
        out += last
    }
    for t in v.tasks { out.append("  task \(t.id) [\(t.status)] \(t.name)") }
    for d in v.docs { out.append("  doc \(d.id) \(d.title): \(d.body.replacingOccurrences(of: "\n", with: " / ").prefix(60))") }
    return out.joined(separator: "\n")
}

@MainActor
func run() async {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("wb-cli-\(UUID().uuidString)")
    let client = SyncClient(storage: Storage(folder: dir), tokens: MemoryTokenStore())
    client.setMe(asMember)
    client.setServer(argv[0])
    if let request, let why = client.signIn(request) { print(why); exit(2) }
    let deadline = Date().addingTimeInterval(10)
    while !client.state.isConnected || client.data == nil {
        if ProcessInfo.processInfo.environment["WB_DEBUG"] != nil { print("state \(client.state) data \(client.data != nil) err \(client.lastError ?? "-")") }
        if client.state == .signIn {
            print(request == nil ? "this server wants a sign-in: add --email E --password P, or --code INVITE_CODE" : "refused: \(client.signInMessage ?? "")")
            exit(1)
        }
        if Date() > deadline { print("could not connect: \(client.lastError ?? "timeout")"); exit(1) }
        try? await Task.sleep(nanoseconds: 50_000_000)
    }
    let cmd = argv[1]
    let rest = Array(argv.dropFirst(2))
    var ok = true
    switch cmd {
    case "dump":
        print(summary(client.view)); exit(0)
    case "watch":
        print(summary(client.view))
        var seen = client.updatesReceived
        while true {
            try? await Task.sleep(nanoseconds: 200_000_000)
            if client.updatesReceived != seen {
                seen = client.updatesReceived
                print("--- update rev \(client.rev) (\(client.state.isConnected ? "connected" : "offline"))")
                print(summary(client.view))
            }
        }
    case "say" where rest.count >= 2:
        ok = client.edit { d, me in Edits.sendMessage(&d, channel: UInt64(rest[0]) ?? 0, text: rest[1], me: me, id: client.newId(), ts: nowTs()) }
    case "reply" where rest.count >= 3:
        ok = client.edit { d, me in Edits.sendReply(&d, channel: UInt64(rest[0]) ?? 0, to: UInt64(rest[1]) ?? 0, text: rest[2], me: me, id: client.newId(), ts: nowTs()) }
    case "status" where rest.count >= 2:
        ok = client.edit { d, me in Edits.setStatus(&d, task: UInt64(rest[0]) ?? 0, to: rest[1], me: me, ts: nowTs()) }
    case "add" where rest.count >= 2:
        ok = client.edit { d, me in Edits.addTask(&d, list: UInt64(rest[0]) ?? 0, name: rest[1], assignees: [me], me: me, id: client.newId(), ts: nowTs()) }
    case "doc" where rest.count >= 2:
        ok = client.edit { d, _ in Edits.setDocBody(&d, doc: UInt64(rest[0]) ?? 0, to: rest[1], ts: nowTs()) }
    default:
        print("unknown command"); exit(2)
    }
    guard ok else { print("edit refused"); exit(1) }
    let before = client.updatesReceived
    let end = Date().addingTimeInterval(5)
    while client.updatesReceived == before && Date() < end {
        try? await Task.sleep(nanoseconds: 50_000_000)
    }
    print(client.updatesReceived > before ? "sent; server rev \(client.rev)" : "sent; no update came back")
    exit(0)
}

Task { await run() }
dispatchMain()
