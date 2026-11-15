// Pictures and videos in chat (server/src/files.rs, crates/wb-net/src/files.rs).
// A message only names its files by the sha256 of their contents; the files
// live on the team server:
//
//   PUT /files/<sha256>   store a file (the contents must hash to the name)
//   GET /files/<sha256>   the file
//
// Every request carries `Authorization: Bearer <device>:<token>`, the device
// id and the token this device signed in with. Downloads are kept in the
// app's caches folder by name, so each file comes down once.

import CryptoKit
import Foundation

public enum FileError: Error, Equatable, Sendable {
    case notSignedIn
    case tooLarge
    case missing
    /// The server doesn't take pictures and videos (an older server).
    case unsupported
    case failed(String)

    public var explanation: String {
        switch self {
        case .notSignedIn: return "You're not signed in to the server."
        case .tooLarge: return "The file is larger than the server takes (500 MB)."
        case .missing: return "The server doesn't have this file."
        case .unsupported: return "The server doesn't take pictures and videos yet."
        case .failed(let why): return "Couldn't reach the server: \(why)"
        }
    }
}

public enum FileAPI {
    /// The largest file the server takes (MAX_FILE).
    public static let maxBytes = 500 << 20

    public static func url(server: String, sha: String) -> URL? {
        ServerAddress.http(server, path: "/files/" + sha)
    }

    public static func sha256(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    /// "Bearer <device>:<token>"
    public static func authHeader(device: String, token: String) -> String { "Bearer \(device):\(token)" }

    static func request(_ url: URL, method: String, auth: String?) -> URLRequest {
        var r = URLRequest(url: url, timeoutInterval: 60)
        r.httpMethod = method
        if let auth { r.setValue(auth, forHTTPHeaderField: "Authorization") }
        return r
    }

    static func error(status: Int, body: Data) -> FileError {
        let text = String(decoding: body.prefix(4096), as: UTF8.self)
        switch status {
        case 401: return .notSignedIn
        case 413: return .tooLarge
        case 404, 405: return text.contains("no such file") ? .missing : .unsupported
        default: return .failed("the server answered \(status)")
        }
    }

    /// Store `data` on the server; returns its sha256 (the attachment's `file`).
    public static func upload(server: String, auth: String?, data: Data, contentType: String, session: URLSession = .shared) async throws -> String {
        guard data.count <= maxBytes else { throw FileError.tooLarge }
        let sha = sha256(data)
        guard let url = url(server: server, sha: sha) else { throw FileError.failed("that server address doesn't look right") }
        var req = request(url, method: "PUT", auth: auth)
        req.setValue(contentType, forHTTPHeaderField: "Content-Type")
        let (body, resp): (Data, URLResponse)
        do {
            (body, resp) = try await session.upload(for: req, from: data)
        } catch {
            throw FileError.failed(error.localizedDescription)
        }
        let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
        guard status == 200 || status == 201 else { throw Self.error(status: status, body: body) }
        return sha
    }

    /// Where a downloaded file is kept.
    public static func cached(_ sha: String, folder: URL = defaultCache) -> URL {
        folder.appendingPathComponent(sha)
    }

    public static var defaultCache: URL {
        FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("WorkboardFiles", isDirectory: true)
    }

    /// The file on this device, downloading it first when needed. `ext`
    /// (like "mov") is added to the local name so players know the type.
    public static func download(server: String, auth: String?, sha: String, ext: String = "", folder: URL = defaultCache, session: URLSession = .shared) async throws -> URL {
        var dest = cached(sha, folder: folder)
        if !ext.isEmpty { dest.appendPathExtension(ext) }
        if FileManager.default.fileExists(atPath: dest.path) { return dest }
        guard let url = url(server: server, sha: sha) else { throw FileError.failed("that server address doesn't look right") }
        let (tmp, resp): (URL, URLResponse)
        do {
            (tmp, resp) = try await session.download(for: request(url, method: "GET", auth: auth))
        } catch {
            throw FileError.failed(error.localizedDescription)
        }
        let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
        guard status == 200 else {
            let body = (try? Data(contentsOf: tmp)) ?? Data()
            throw Self.error(status: status, body: body)
        }
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? FileManager.default.removeItem(at: dest)
        do {
            try FileManager.default.moveItem(at: tmp, to: dest)
        } catch {
            throw FileError.failed(error.localizedDescription)
        }
        return dest
    }

    /// An Attachment object as the Mac app writes it (src/model.rs).
    public static func attachment(sha: String, name: String, isVideo: Bool, bytes: Int, w: Int, h: Int, secs: Double) -> JSON {
        [
            "file": .string(sha), "name": .string(name), "kind": .string(isVideo ? "Video" : "Image"),
            "bytes": .int(Int64(bytes)), "w": .int(Int64(w)), "h": .int(Int64(h)), "secs": .double(secs),
        ]
    }
}

extension SyncClient {
    /// The Authorization header for /files on the current server, nil when
    /// this device has no token there.
    public var fileAuth: String? {
        guard let t = deviceToken, !t.isEmpty else { return nil }
        return FileAPI.authHeader(device: settings.device, token: t)
    }
}
