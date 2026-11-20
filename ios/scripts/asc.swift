// App Store Connect API calls for TestFlight, used by ios/scripts/testflight.sh.
//
//   swift ios/scripts/asc.swift check
//   swift ios/scripts/asc.swift distribute --version 0.1.20 --notes-file FILE \
//       [--since 2026-11-20T10:00:00Z] [--group Team]
//
// "distribute" waits until the newest build of that version (uploaded after
// --since, if given) has finished processing, sets its "What to Test" text,
// adds it to the external group and submits it for beta review.
//
// Needs ASC_KEY_ID, ASC_ISSUER_ID and the .p8 key file (ASC_KEY_PATH, default
// ~/.appstoreconnect/private_keys/AuthKey_<ASC_KEY_ID>.p8). See
// docs/ios-design.md, "TestFlight".

import CryptoKit
import Foundation

let appID = "6835523119"
/// ASC_API_URL points the script at a stand-in server for testing.
let api =
    ProcessInfo.processInfo.environment["ASC_API_URL"] ?? "https://api.appstoreconnect.apple.com"

typealias JSON = [String: Any]

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data(("asc: " + message + "\n").utf8))
    exit(1)
}

// MARK: - Token

struct Key {
    let id: String
    let issuer: String
    let key: P256.Signing.PrivateKey
}

func loadKey() -> Key {
    let env = ProcessInfo.processInfo.environment
    guard let id = env["ASC_KEY_ID"], !id.isEmpty,
        let issuer = env["ASC_ISSUER_ID"], !issuer.isEmpty
    else {
        fail(
            "ASC_KEY_ID and ASC_ISSUER_ID are not set (put them in ~/.appstoreconnect/workboard.env)"
        )
    }
    let path =
        env["ASC_KEY_PATH"].flatMap { $0.isEmpty ? nil : $0 }
        ?? "\(NSHomeDirectory())/.appstoreconnect/private_keys/AuthKey_\(id).p8"
    guard let pem = try? String(contentsOfFile: path, encoding: .utf8) else {
        fail("cannot read the API key file \(path)")
    }
    guard let key = try? P256.Signing.PrivateKey(pemRepresentation: pem) else {
        fail("\(path) is not an App Store Connect .p8 key")
    }
    return Key(id: id, issuer: issuer, key: key)
}

func base64url(_ data: Data) -> String {
    data.base64EncodedString()
        .replacingOccurrences(of: "+", with: "-")
        .replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
}

/// ES256 JWT, valid for 20 minutes (the most Apple allows).
func token(_ k: Key) -> String {
    let now = Int(Date().timeIntervalSince1970)
    let header: JSON = ["alg": "ES256", "kid": k.id, "typ": "JWT"]
    let claims: JSON = [
        "iss": k.issuer, "iat": now, "exp": now + 1200, "aud": "appstoreconnect-v1",
    ]
    let input =
        base64url(try! JSONSerialization.data(withJSONObject: header)) + "."
        + base64url(try! JSONSerialization.data(withJSONObject: claims))
    guard let sig = try? k.key.signature(for: Data(input.utf8)) else {
        fail("could not sign the API token")
    }
    return input + "." + base64url(sig.rawRepresentation)
}

// MARK: - HTTP

/// Loaded on first use, so a usage error does not ask for the key.
enum Auth { static let key = loadKey() }

/// Calls the API. Fails on any status outside 2xx unless it is in `allow`.
func call(_ method: String, _ path: String, _ body: JSON? = nil, allow: Set<Int> = [])
    async -> (Int, JSON)
{
    guard let url = URL(string: api + path) else { fail("bad URL \(path)") }
    var req = URLRequest(url: url)
    req.httpMethod = method
    req.setValue("Bearer " + token(Auth.key), forHTTPHeaderField: "Authorization")
    if let body {
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try! JSONSerialization.data(withJSONObject: body)
    }
    let data: Data
    let response: URLResponse
    do {
        (data, response) = try await URLSession.shared.data(for: req)
    } catch {
        fail("\(method) \(path): \(error.localizedDescription)")
    }
    let status = (response as? HTTPURLResponse)?.statusCode ?? 0
    let json = (try? JSONSerialization.jsonObject(with: data)) as? JSON ?? [:]
    if !(200..<300).contains(status) && !allow.contains(status) {
        fail("\(method) \(path): HTTP \(status)\n\(errorText(json))")
    }
    return (status, json)
}

func errorText(_ json: JSON) -> String {
    let errors = json["errors"] as? [JSON] ?? []
    return errors.map { e in
        [e["title"], e["detail"]].compactMap { $0 as? String }.joined(separator: ": ")
    }.joined(separator: "\n")
}

func items(_ json: JSON) -> [JSON] { json["data"] as? [JSON] ?? [] }
func attrs(_ item: JSON) -> JSON { item["attributes"] as? JSON ?? [:] }
func str(_ item: JSON, _ name: String) -> String { attrs(item)[name] as? String ?? "" }

// MARK: - Commands

func check() async {
    let (_, app) = await call("GET", "/v1/apps/\(appID)")
    let data = app["data"] as? JSON ?? [:]
    print("app: \(str(data, "name")) (\(str(data, "bundleId")), \(str(data, "primaryLocale")))")
    let (_, groups) = await call("GET", "/v1/apps/\(appID)/betaGroups?limit=50")
    for g in items(groups) {
        let a = attrs(g)
        let kind = (a["isInternalGroup"] as? Bool ?? false) ? "internal" : "external"
        let link = (a["publicLinkEnabled"] as? Bool ?? false) ? ", public link on" : ""
        print("group: \(str(g, "name")) (\(kind)\(link))")
    }
}

func distribute(version: String, since: Date?, groupName: String, notes: String) async {
    // The external group.
    let encoded = groupName.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? ""
    let (_, groups) = await call(
        "GET", "/v1/apps/\(appID)/betaGroups?filter%5Bname%5D=\(encoded)&limit=50")
    guard
        let group = items(groups).first(where: {
            str($0, "name") == groupName && !(attrs($0)["isInternalGroup"] as? Bool ?? false)
        }), let groupID = group["id"] as? String
    else { fail("no external TestFlight group named \(groupName)") }

    // Wait for the build to show up and finish processing (usually 10-30 minutes).
    let iso = ISO8601DateFormatter()
    let deadline = Date().addingTimeInterval(90 * 60)
    var build: JSON = [:]
    var lastNote = ""
    while true {
        let (_, builds) = await call(
            "GET",
            "/v1/builds?filter%5Bapp%5D=\(appID)&filter%5BpreReleaseVersion.version%5D=\(version)"
                + "&sort=-uploadedDate&limit=1")
        var note = "waiting for build \(version) to appear"
        if let b = items(builds).first {
            let uploaded = iso.date(from: str(b, "uploadedDate")) ?? .distantPast
            if since == nil || uploaded >= since!.addingTimeInterval(-120) {
                let state = str(b, "processingState")
                note = "build \(version) (\(str(b, "version"))): \(state)"
                if state == "VALID" {
                    build = b
                    print(note)
                    break
                }
                if state == "FAILED" || state == "INVALID" { fail(note) }
            }
        }
        if note != lastNote {
            print(note)
            lastNote = note
        }
        if Date() > deadline { fail("gave up after 90 minutes: \(note)") }
        try? await Task.sleep(nanoseconds: 30_000_000_000)
    }
    guard let buildID = build["id"] as? String else { fail("build has no id") }
    let number = str(build, "version")

    // "What to Test": update every existing localization and make sure the
    // app's primary language has one.
    let (_, app) = await call("GET", "/v1/apps/\(appID)")
    let locale = str(app["data"] as? JSON ?? [:], "primaryLocale")
    let (_, locs) = await call("GET", "/v1/builds/\(buildID)/betaBuildLocalizations")
    var hasPrimary = false
    for l in items(locs) {
        guard let id = l["id"] as? String else { continue }
        if str(l, "locale") == locale { hasPrimary = true }
        _ = await call(
            "PATCH", "/v1/betaBuildLocalizations/\(id)",
            [
                "data": [
                    "type": "betaBuildLocalizations", "id": id,
                    "attributes": ["whatsNew": notes],
                ]
            ])
    }
    if !hasPrimary && !locale.isEmpty {
        _ = await call(
            "POST", "/v1/betaBuildLocalizations",
            [
                "data": [
                    "type": "betaBuildLocalizations",
                    "attributes": ["locale": locale, "whatsNew": notes],
                    "relationships": ["build": ["data": ["type": "builds", "id": buildID]]],
                ]
            ])
    }
    print("what to test: set")

    // Add to the group; testers get it once beta review approves it.
    _ = await call(
        "POST", "/v1/betaGroups/\(groupID)/relationships/builds",
        ["data": [["type": "builds", "id": buildID]]])
    print("added \(version) (\(number)) to \(groupName)")

    // Submit for beta review unless it is already in or past review.
    let (_, detail) = await call("GET", "/v1/builds/\(buildID)/buildBetaDetail")
    let state = str(detail["data"] as? JSON ?? [:], "externalBuildState")
    let done: Set<String> = [
        "WAITING_FOR_BETA_REVIEW", "IN_BETA_REVIEW", "BETA_APPROVED", "IN_BETA_TESTING",
    ]
    if done.contains(state) {
        print("beta review: already \(state)")
    } else {
        let (status, result) = await call(
            "POST", "/v1/betaAppReviewSubmissions",
            ["data": ["type": "betaAppReviewSubmissions", "relationships": ["build": ["data": ["type": "builds", "id": buildID]]]]],
            allow: [409])
        if status == 409 {
            print("beta review: not submitted\n\(errorText(result))")
        } else {
            print("beta review: submitted")
        }
    }
}

// MARK: - Arguments

var args = Array(CommandLine.arguments.dropFirst())
func option(_ name: String) -> String? {
    guard let i = args.firstIndex(of: name), i + 1 < args.count else { return nil }
    return args[i + 1]
}

switch args.first {
case "check":
    await check()
case "distribute":
    guard let version = option("--version") else { fail("--version is required") }
    guard let notesFile = option("--notes-file"),
        let notes = try? String(contentsOfFile: notesFile, encoding: .utf8)
    else { fail("--notes-file is required and must be readable") }
    var since: Date? = nil
    if let s = option("--since") {
        guard let d = ISO8601DateFormatter().date(from: s) else { fail("bad --since \(s)") }
        since = d
    }
    await distribute(
        version: version, since: since, groupName: option("--group") ?? "Team",
        notes: String(notes.trimmingCharacters(in: .whitespacesAndNewlines).prefix(4000)))
default:
    fail("usage: asc.swift check | distribute --version V --notes-file F [--since ISO] [--group G]")
}
