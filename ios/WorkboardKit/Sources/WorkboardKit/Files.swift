// Pictures and videos in chat (server/src/files.rs, crates/wb-net/src/http.rs).
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
        case .tooLarge: return "The file is larger than the server takes (90 MB)."
        case .missing: return "The server doesn't have this file."
        case .unsupported: return "The server doesn't take pictures and videos yet."
        case .failed(let why): return "Couldn't reach the server: \(why)"
        }
    }
}

public enum FileAPI {
    /// The largest file the server takes (MAX_FILE).
    public static let maxBytes = 90 << 20

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

    /// The sha256 of a file, read a piece at a time (a video can be hundreds
    /// of megabytes; it is never held in memory whole).
    public static func sha256(file: URL, chunk: Int = 1 << 20) throws -> String {
        let h = try FileHandle(forReadingFrom: file)
        defer { try? h.close() }
        var hasher = SHA256()
        while let piece = try h.read(upToCount: chunk), !piece.isEmpty {
            hasher.update(data: piece)
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }

    /// Store a file on the server straight from disk (videos); returns its
    /// sha256. URLSession streams the body from the file.
    public static func upload(server: String, auth: String?, file: URL, contentType: String, session: URLSession = .shared) async throws -> String {
        let size = (try? file.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
        guard size <= maxBytes else { throw FileError.tooLarge }
        let sha: String
        do {
            sha = try await Task.detached(priority: .userInitiated) { try sha256(file: file) }.value
        } catch {
            throw FileError.failed(error.localizedDescription)
        }
        guard let url = url(server: server, sha: sha) else { throw FileError.failed("that server address doesn't look right") }
        var req = request(url, method: "PUT", auth: auth)
        req.setValue(contentType, forHTTPHeaderField: "Content-Type")
        req.timeoutInterval = 600
        let (body, resp): (Data, URLResponse)
        do {
            (body, resp) = try await session.upload(for: req, fromFile: file)
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

    /// An Attachment object as the Mac app writes it (src/model.rs). A
    /// video's `thumb` names its still (left out when there is none).
    public static func attachment(sha: String, name: String, isVideo: Bool, bytes: Int, w: Int, h: Int, secs: Double, thumb: String? = nil) -> JSON {
        var o: JSON = [
            "file": .string(sha), "name": .string(name), "kind": .string(isVideo ? "Video" : "Image"),
            "bytes": .int(Int64(bytes)), "w": .int(Int64(w)), "h": .int(Int64(h)), "secs": .double(secs),
        ]
        if isVideo, let thumb, isFileName(thumb) { o["thumb"] = .string(thumb) }
        return o
    }

    /// An Attachment object for a file that isn't a picture or video
    /// (MediaKind::File on the Mac): sent as it is, with no size to show.
    public static func fileAttachment(sha: String, name: String, bytes: Int) -> JSON {
        [
            "file": .string(sha), "name": .string(name), "kind": .string("File"),
            "bytes": .int(Int64(bytes)), "w": .int(0), "h": .int(0), "secs": .double(0),
        ]
    }

    /// An Attachment object for a sound (MediaKind::Audio on the Mac): a
    /// voice message recorded in the message box, or an audio file. Only
    /// its length goes with it; no picture size, no still.
    public static func audioAttachment(sha: String, name: String, bytes: Int, secs: Double) -> JSON {
        [
            "file": .string(sha), "name": .string(name), "kind": .string("Audio"),
            "bytes": .int(Int64(bytes)), "secs": .double(secs.isFinite ? max(0, secs) : 0),
        ]
    }

    /// A name a file from someone else's message may be saved under here:
    /// its last part only, never empty, never hidden.
    public static func safeName(_ name: String) -> String {
        let last = (name as NSString).lastPathComponent
            .replacingOccurrences(of: ":", with: "-")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmed = String(last.drop(while: { $0 == "." }))
        return trimmed.isEmpty ? "file" : trimmed
    }

    /// Keep a copy of a file sent from here in the cache, under the name a
    /// download would give it, so it never comes down again and can go up
    /// again after a restore from Trash.
    public static func keep(_ data: Data, sha: String, ext: String = "", folder: URL = defaultCache) {
        var dest = cached(sha, folder: folder)
        if !ext.isEmpty { dest.appendPathExtension(ext) }
        guard !FileManager.default.fileExists(atPath: dest.path) else { return }
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? data.write(to: dest, options: .atomic)
    }

    /// keep(_:sha:ext:folder:) for a file on disk (a video), copied, not read.
    public static func keep(file: URL, sha: String, ext: String = "", folder: URL = defaultCache) {
        var dest = cached(sha, folder: folder)
        if !ext.isEmpty { dest.appendPathExtension(ext) }
        guard !FileManager.default.fileExists(atPath: dest.path) else { return }
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? FileManager.default.copyItem(at: file, to: dest)
    }

    /// How much the downloaded files may take on this device before the
    /// ones looked at longest ago are let go of (they come down again when
    /// needed). iOS clears Caches too when space runs out, but only then.
    public static let cacheLimit = 1_000_000_000

    /// Take the files used longest ago out of the cache until it holds at
    /// most `limit` bytes. Files whose sha is in `keep` (in this iPhone's
    /// Trash, or waiting to go back to the server) stay. Returns how many
    /// bytes were let go of.
    @discardableResult
    public static func trimCache(folder: URL = defaultCache, limit: Int = cacheLimit, keep: Set<String> = []) -> Int {
        let keys: [URLResourceKey] = [.fileSizeKey, .contentAccessDateKey, .contentModificationDateKey, .isRegularFileKey]
        guard let urls = try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: keys) else { return 0 }
        var files: [(url: URL, size: Int, used: Date)] = []
        var total = 0
        for u in urls {
            guard let v = try? u.resourceValues(forKeys: Set(keys)), v.isRegularFile == true else { continue }
            let size = v.fileSize ?? 0
            total += size
            files.append((u, size, v.contentAccessDate ?? v.contentModificationDate ?? .distantPast))
        }
        guard total > limit else { return 0 }
        var freed = 0
        for f in files.sorted(by: { $0.used < $1.used }) where total > limit {
            let sha = String(f.url.lastPathComponent.prefix { $0 != "." })
            if keep.contains(sha) { continue }
            if (try? FileManager.default.removeItem(at: f.url)) != nil {
                total -= f.size
                freed += f.size
            }
        }
        return freed
    }

    /// This device's copy of a file, whatever extension it was kept with.
    public static func localCopy(_ sha: String, folder: URL = defaultCache) -> URL? {
        guard isFileName(sha) else { return nil }
        let plain = cached(sha, folder: folder)
        if FileManager.default.fileExists(atPath: plain.path) { return plain }
        let names = (try? FileManager.default.contentsOfDirectory(atPath: folder.path)) ?? []
        return names.first { $0.hasPrefix(sha + ".") }.map { folder.appendingPathComponent($0) }
    }

    /// The size of a file on the server, nil when it isn't there (HEAD,
    /// like wb_net::files::remote_size).
    public static func remoteSize(server: String, auth: String?, sha: String, session: URLSession = .shared) async throws -> Int? {
        guard let url = url(server: server, sha: sha) else { throw FileError.failed("that server address doesn't look right") }
        let resp: URLResponse
        do {
            (_, resp) = try await session.data(for: request(url, method: "HEAD", auth: auth))
        } catch {
            throw FileError.failed(error.localizedDescription)
        }
        let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
        switch status {
        case 200:
            let len = (resp as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Length").flatMap { Int($0) }
            return len ?? Int(resp.expectedContentLength)
        // HEAD has no body to tell a missing file from an older server
        case 404: return nil
        default: throw Self.error(status: status, body: Data())
        }
    }

    /// Put a file this device has back on the server unless the server has
    /// it (asked fresh every time: a file may have been cleaned up while
    /// only someone's Trash named it). True when it went up.
    @discardableResult
    public static func reupload(server: String, auth: String?, sha: String, folder: URL = defaultCache, session: URLSession = .shared) async throws -> Bool {
        guard let local = localCopy(sha, folder: folder) else { return false }
        let size = (try? local.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? -1
        if try await remoteSize(server: server, auth: auth, sha: sha, session: session) == size { return false }
        let up = try await upload(server: server, auth: auth, file: local, contentType: contentType(ext: local.pathExtension), session: session)
        return up == sha
    }

    public static func contentType(ext: String) -> String {
        switch ext.lowercased() {
        case "jpg", "jpeg": return "image/jpeg"
        case "png": return "image/png"
        case "gif": return "image/gif"
        case "heic": return "image/heic"
        case "webp": return "image/webp"
        case "mov": return "video/quicktime"
        case "mp4", "m4v": return "video/mp4"
        case "m4a": return "audio/mp4"
        case "mp3": return "audio/mpeg"
        case "wav": return "audio/wav"
        case "aac": return "audio/aac"
        case "caf": return "audio/x-caf"
        default: return "application/octet-stream"
        }
    }

    /// The files (and video stills) named by the attachments anywhere in
    /// some JSON text, like trash_attachments in src/app/files.rs with
    /// parts(): a Trash item's payload.
    public static func files(inPayload payload: String) -> [String] {
        guard let j = try? JSON(parsing: Data(payload.utf8)) else { return [] }
        var out: [String] = []
        func add(_ s: String?) { if let s, isFileName(s), !out.contains(s) { out.append(s) } }
        func walk(_ v: JSON) {
            if let o = v.object {
                for (k, x) in o.sorted(by: { $0.key < $1.key }) {
                    if k == "attachments", let list = x.array {
                        for a in list {
                            add(a["file"]?.string)
                            add(a["thumb"]?.string)
                        }
                    } else {
                        walk(x)
                    }
                }
            } else if let a = v.array {
                a.forEach(walk)
            }
        }
        walk(j)
        return out
    }

    /// A sha256 in hex, the only names files have on the server.
    public static func isFileName(_ s: String) -> Bool {
        s.count == 64 && s.allSatisfy { ("0"..."9").contains($0) || ("a"..."f").contains($0) }
    }

    /// Longer side of the still sent along with a video (media::STILL).
    public static let stillSide = 480
}

extension SyncClient {
    /// The Authorization header for /files on the current server, nil when
    /// this device has no token there.
    public var fileAuth: String? {
        guard let t = deviceToken, !t.isEmpty else { return nil }
        return FileAPI.authHeader(device: settings.device, token: t)
    }
}
