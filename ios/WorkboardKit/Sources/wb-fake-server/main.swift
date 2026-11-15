// Runs the test-only fake server:
//   swift run wb-fake-server [--port 8787] [--host 127.0.0.1] [--seed FILE] [--save FILE]
//                            [--sign-in] [--invite MEMBER_ID ...] [--no-mail] [--admin-http]
//                            [--livekit-dev | --livekit-url URL --livekit-key K --livekit-secret S]
//   swift run wb-fake-server --write-sample FILE   (writes the sample and exits)
//
// Without --seed it starts from SampleWorkspace. With --save the snapshot is
// written there after every push, so a restart can continue from it.
// --sign-in makes it ask for a sign-in like the real server (invite code,
// email + password, reset code, device token); each --invite prints a
// one-time code. Reset codes from POST /auth/forgot are printed here instead
// of mailed; --no-mail makes /auth/forgot answer mail: false.
// While running, type "invite 2" or "revoke 2" and press return.
// --admin-http (tests only) also takes POST /admin/invite?member=N,
// /admin/revoke?member=N, /admin/say?channel=C&author=A&text=T,
// /admin/reset-code?email=E, /admin/drop-account?member=N,
// /admin/offline?secs=N (drop the app connections and turn new ones away),
// /admin/online, /admin/find?text=T and /admin/drop-answer (the next access
// or backup request gets no answer) over HTTP, for UI tests running in the
// simulator.

import Foundation
import WorkboardFakeServer
import WorkboardKit

setvbuf(stdout, nil, _IOLBF, 0)
var port: UInt16 = 8787
var host = "127.0.0.1"
var seed: String?
var save: String?
var signIn = false
var inviteFor: [UInt64] = []
var lk: LiveKitConfig?
var adminHTTP = false
var noMail = false
var args = CommandLine.arguments.dropFirst()
func usage() -> Never {
    print("usage: wb-fake-server [--port N] [--host ADDR] [--seed FILE] [--save FILE] [--sign-in] [--invite MEMBER] [--no-mail] [--admin-http] [--livekit-dev | --livekit-url U --livekit-key K --livekit-secret S] | --write-sample FILE")
    exit(2)
}
while let a = args.popFirst() {
    switch a {
    case "--port": port = UInt16(args.popFirst() ?? "") ?? port
    case "--host": host = args.popFirst() ?? host
    case "--seed": seed = args.popFirst()
    case "--save": save = args.popFirst()
    case "--sign-in": signIn = true
    case "--admin-http": adminHTTP = true
    case "--no-mail": noMail = true
    case "--invite": inviteFor.append(UInt64(args.popFirst() ?? "") ?? 1)
    case "--livekit-dev": lk = .dev
    case "--livekit-url": lk = LiveKitConfig(url: args.popFirst() ?? "", key: lk?.key ?? "devkey", secret: lk?.secret ?? "secret")
    case "--livekit-key": lk = LiveKitConfig(url: lk?.url ?? LiveKitConfig.dev.url, key: args.popFirst() ?? "", secret: lk?.secret ?? "secret")
    case "--livekit-secret": lk = LiveKitConfig(url: lk?.url ?? LiveKitConfig.dev.url, key: lk?.key ?? "devkey", secret: args.popFirst() ?? "")
    case "--write-sample":
        let path = args.popFirst() ?? "sample.json"
        try! SampleWorkspace.make().data().write(to: URL(fileURLWithPath: path))
        print("wrote \(path)")
        exit(0)
    default: usage()
    }
}

var start: JSON = SampleWorkspace.make()
for path in [save, seed].compactMap({ $0 }) {
    if let d = FileManager.default.contents(atPath: path), let j = try? JSON(parsing: d) {
        start = j
        print("loaded \(path)")
        break
    }
}

let server = FakeServer(snapshot: start, requireSignIn: signIn, livekit: lk)
server.log = { print($0) }
server.adminHTTP = adminHTTP
server.canMail = !noMail
server.onPush = { s in
    if let save { try? s.data().write(to: URL(fileURLWithPath: save), options: .atomic) }
}
Task {
    do {
        let p = try await server.start(host: host, port: port)
        print("fake workboard-server listening on ws://\(host):\(p)" + (signIn ? " (sign-in required)" : "") + (lk.map { ", calls via \($0.url)" } ?? ""))
        for m in inviteFor { print("invite code for member \(m): \(server.invite(member: m))") }
    } catch {
        print("could not listen: \(error)")
        exit(1)
    }
}
// admin commands on stdin
Thread {
    while let line = readLine() {
        let w = line.split(separator: " ")
        guard w.count == 2, let m = UInt64(w[1]) else { print("commands: invite MEMBER_ID | revoke MEMBER_ID"); continue }
        switch w[0] {
        case "invite": print("invite code for member \(m): \(server.invite(member: m))")
        case "revoke": server.revoke(member: m); print("revoked member \(m)")
        default: print("commands: invite MEMBER_ID | revoke MEMBER_ID")
        }
    }
}.start()
dispatchMain()
