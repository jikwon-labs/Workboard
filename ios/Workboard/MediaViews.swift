// Pictures, videos and other files in chat messages. Files come from the team server
// (GET /files/<sha256> with this device's token, see WorkboardKit Files.swift)
// and are kept in the caches folder. Pictures show in the message and open
// large on a tap; a video shows as a tile with its still (`thumb`, a small
// JPEG, when the sender made one) and downloads the video only when it's
// played, in the system player. Sending: photos picked in the message box go
// up first (PUT /files) and the message names them, the same shape the Mac
// app writes; a video goes with a still made here. Any other file (a PDF, a
// Markdown note) goes up as it is and shows as its name and size; a tap
// downloads it and opens it in Quick Look, which can share or save it. A
// sound (a voice message, or an audio file picked in Files) plays in the
// message (VoiceViews.swift).

import AVFoundation
import AVKit
import CoreTransferable
import ImageIO
import PhotosUI
import QuickLook
import SwiftUI
import UniformTypeIdentifiers
import WorkboardKit

// MARK: showing

struct AttachmentsView: View {
    let attachments: [AttachmentView]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(attachments, id: \.file) { a in
                if a.isAudio {
                    VoiceCard(attachment: a)
                } else if a.isFile {
                    FileTile(attachment: a)
                } else if a.isVideo {
                    VideoTile(attachment: a)
                } else {
                    PictureView(attachment: a)
                }
            }
        }
    }
}

/// Size of a picture or video in the message: its own shape, at most 240 points.
private func boxSize(_ a: AttachmentView) -> CGSize {
    let w = CGFloat(max(a.w, 1)), h = CGFloat(max(a.h, 1))
    let maxSide: CGFloat = 240
    if a.w == 0 || a.h == 0 { return CGSize(width: maxSide, height: maxSide * 0.75) }
    let k = min(maxSide / w, maxSide / h)
    return CGSize(width: max(80, w * k), height: max(80, h * k))
}

/// The longest side, in pixels, a picture needs to fill `box` (scaledToFill
/// crops the rest), so it can be decoded at that size instead of whole.
private func fillPixels(_ a: AttachmentView, box: CGSize, scale: CGFloat) -> Int {
    var side = max(box.width, box.height)
    if a.w > 0, a.h > 0 {
        let fill = max(box.width / CGFloat(a.w), box.height / CGFloat(a.h))
        side = CGFloat(max(a.w, a.h)) * fill
    }
    return max(1, Int((side * scale).rounded(.up)))
}

/// Pictures decoded at the size they are shown, kept while there is memory
/// for them. A phone photo decoded whole takes tens of megabytes for a
/// bubble of 240 points, and scrolling back used to decode it again.
@MainActor
enum PictureCache {
    static let cache: NSCache<NSString, UIImage> = {
        let c = NSCache<NSString, UIImage>()
        c.totalCostLimit = 64 * 1024 * 1024
        return c
    }()

    /// The picture `key` already decoded at `maxPixels`, nil when it isn't
    /// (or was let go of).
    static func cached(key: String, maxPixels: Int) -> UIImage? {
        cache.object(forKey: "\(key)-\(maxPixels)" as NSString)
    }

    /// The picture at `url` with its longest side at most `maxPixels`,
    /// decoded off the main thread. `key` names the file (its sha256).
    static func thumbnail(_ url: URL, key: String, maxPixels: Int) async -> UIImage? {
        let k = "\(key)-\(maxPixels)" as NSString
        if let hit = cache.object(forKey: k) { return hit }
        let made = await Task.detached(priority: .userInitiated) { () -> (UIImage, Int)? in
            guard let src = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary) else { return nil }
            let opts: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
                kCGImageSourceThumbnailMaxPixelSize: maxPixels,
            ]
            guard let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) else { return nil }
            return (UIImage(cgImage: cg), cg.bytesPerRow * cg.height)
        }.value
        guard let (image, cost) = made else { return nil }
        cache.setObject(image, forKey: k, cost: cost)
        return image
    }
}

/// Downloads a file once per view and keeps where it landed.
@MainActor
@Observable
final class FileLoad {
    var url: URL?
    var error: String?

    func load(_ a: AttachmentView, client: SyncClient) async {
        guard url == nil else { return }
        do {
            url = try await FileAPI.download(server: client.settings.server, auth: client.fileAuth, sha: a.file, ext: (a.name as NSString).pathExtension.lowercased())
            error = nil
        } catch let e as FileError {
            error = e.explanation
        } catch {
            self.error = error.localizedDescription
        }
    }
}

/// A file that isn't a picture or video: its kind, name and size. A tap
/// downloads it and opens it in Quick Look (which shows it without running
/// anything, and can share or save it).
struct FileTile: View {
    @Environment(SyncClient.self) private var client
    let attachment: AttachmentView
    /// Shown after the size, and the tile takes the whole width (the
    /// conversation's Other files: who sent it and when).
    var detail: String?
    @State private var file = FileLoad()
    @State private var loading = false
    @State private var shown: URL?

    /// "PDF", "MD"; "FILE" without an extension (as on the Mac).
    static func label(_ name: String) -> String {
        let e = (name as NSString).pathExtension.uppercased()
        return e.isEmpty || e.count > 5 || !e.allSatisfy({ $0.isLetter || $0.isNumber }) ? "FILE" : e
    }

    var body: some View {
        Button {
            Task { await open() }
        } label: {
            HStack(spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Color.accentColor.opacity(0.15))
                    Text(verbatim: Self.label(attachment.name))
                        .font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(Color.accentColor)
                }
                .frame(width: 36, height: 36)
                VStack(alignment: .leading, spacing: 2) {
                    Text(verbatim: attachment.name)
                        .font(.subheadline)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .foregroundStyle(.primary)
                    if let e = file.error {
                        Text(verbatim: e).font(.caption).foregroundStyle(.red).lineLimit(2)
                    } else {
                        let size = ByteCountFormatter.string(fromByteCount: Int64(attachment.bytes), countStyle: .file)
                        Text(verbatim: detail.map { "\(size) · \($0)" } ?? size)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer(minLength: 0)
                if loading { ProgressView() }
            }
            .padding(8)
            .frame(maxWidth: detail == nil ? 260 : .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color(.secondarySystemBackground)))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(Text(verbatim: attachment.name))
        .accessibilityHint("Opens the file")
        .accessibilityIdentifier("fileAttachment")
        .quickLookPreview($shown)
    }

    func open() async {
        guard !loading else { return }
        loading = true
        defer { loading = false }
        file.error = nil
        await file.load(attachment, client: client)
        guard let url = file.url else { return }
        // under its own name, so Quick Look and Save show that, not the hash
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("open-\(attachment.file)", isDirectory: true)
        let dest = dir.appendingPathComponent(FileAPI.safeName(attachment.name))
        if !FileManager.default.fileExists(atPath: dest.path) {
            do {
                try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                try FileManager.default.copyItem(at: url, to: dest)
            } catch {
                file.error = error.localizedDescription
                return
            }
        }
        shown = dest
    }
}

struct PictureView: View {
    @Environment(SyncClient.self) private var client
    let attachment: AttachmentView
    /// The box it fills (a square in the conversation's files); its own
    /// shape when nil.
    var box: CGSize?
    @Environment(\.displayScale) private var displayScale
    @State private var file = FileLoad()
    /// Decoded at the size of the box; the full picture only opens large.
    @State private var image: UIImage?
    @State private var showLarge = false

    var body: some View {
        let size = box ?? boxSize(attachment)
        Button { if image != nil { showLarge = true } } label: {
            ZStack {
                RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color(.secondarySystemFill))
                if let image {
                    Image(uiImage: image).resizable().scaledToFill()
                } else if let e = file.error {
                    VStack(spacing: 4) {
                        Image(systemName: "photo").font(.title3)
                        Text(e).font(.caption2).multilineTextAlignment(.center).lineLimit(3)
                    }
                    .foregroundStyle(.secondary)
                    .padding(8)
                } else {
                    ProgressView()
                }
            }
            .frame(width: size.width, height: size.height)
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Picture \(attachment.name)")
        .accessibilityIdentifier("picture-\(attachment.file.prefix(12))")
        .task(id: attachment.file) {
            await file.load(attachment, client: client)
            if let u = file.url {
                image = await PictureCache.thumbnail(u, key: attachment.file, maxPixels: fillPixels(attachment, box: box ?? boxSize(attachment), scale: displayScale))
            }
        }
        .fullScreenCover(isPresented: $showLarge) {
            if let image, let u = file.url { LargePicture(url: u, preview: image, name: attachment.name) }
        }
    }
}

private struct LargePicture: View {
    @Environment(\.dismiss) private var dismiss
    let url: URL
    /// The small copy from the message, shown until the full one is ready.
    let preview: UIImage
    let name: String
    @State private var full: UIImage?
    @State private var scale: CGFloat = 1

    var body: some View {
        let image = full ?? preview
        NavigationStack {
            Image(uiImage: image)
                .resizable()
                .scaledToFit()
                .scaleEffect(scale)
                .gesture(MagnifyGesture().onChanged { scale = max(1, $0.magnification) }.onEnded { _ in withAnimation { scale = 1 } })
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color.black)
                .navigationTitle(name)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
                    ToolbarItem(placement: .topBarLeading) {
                        ShareLink(item: Image(uiImage: image), preview: SharePreview(name, image: Image(uiImage: image)))
                    }
                }
        }
        .task {
            // decoded here, not on the main thread when first drawn
            let path = url.path
            full = await Task.detached(priority: .userInitiated) { () -> UIImage? in
                let i = UIImage(contentsOfFile: path)
                return i?.preparingForDisplay() ?? i
            }.value
        }
    }
}

struct VideoTile: View {
    @Environment(SyncClient.self) private var client
    @Environment(\.displayScale) private var displayScale
    let attachment: AttachmentView
    /// As in PictureView.
    var box: CGSize?
    @State private var file = FileLoad()
    @State private var loading = false
    @State private var player: AVPlayer?
    /// The still the sender made (only the small JPEG is fetched for it).
    @State private var still: UIImage?

    var body: some View {
        let size = box ?? boxSize(attachment)
        Button(action: play) {
            ZStack {
                RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.black.opacity(0.85))
                if let still {
                    Image(uiImage: still).resizable().scaledToFill()
                        .frame(width: size.width, height: size.height)
                        .accessibilityIdentifier("videoStill")
                }
                VStack(spacing: 6) {
                    if loading {
                        ProgressView().tint(.white)
                    } else {
                        Image(systemName: "play.circle.fill").font(.system(size: 40))
                    }
                    if let e = file.error {
                        Text(e).font(.caption2).multilineTextAlignment(.center).lineLimit(3).padding(.horizontal, 8)
                    }
                }
                .foregroundStyle(.white)
                VStack {
                    Spacer()
                    HStack {
                        Text(attachment.name).lineLimit(1)
                        Spacer()
                        if attachment.secs > 0 { Text(clock(attachment.secs)).monospacedDigit() }
                    }
                    .font(.caption2)
                    .foregroundStyle(.white.opacity(0.9))
                    .padding(8)
                }
            }
            .frame(width: size.width, height: size.height)
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Video \(attachment.name)")
        .accessibilityIdentifier("video-\(attachment.file.prefix(12))")
        .task(id: attachment.thumb) {
            guard let t = attachment.thumb, still == nil else { return }
            if let u = try? await FileAPI.download(server: client.settings.server, auth: client.fileAuth, sha: t, ext: "jpg") {
                still = await PictureCache.thumbnail(u, key: t, maxPixels: fillPixels(attachment, box: box ?? boxSize(attachment), scale: displayScale))
            }
        }
        .fullScreenCover(isPresented: Binding(get: { player != nil }, set: { if !$0 { player?.pause(); player = nil } })) {
            if let player {
                NavigationStack {
                    VideoPlayer(player: player)
                        .ignoresSafeArea(edges: .bottom)
                        .background(Color.black)
                        .navigationTitle(attachment.name)
                        .navigationBarTitleDisplayMode(.inline)
                        .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { self.player?.pause(); self.player = nil } } }
                        .onAppear { player.play() }
                }
            }
        }
    }

    func play() {
        guard !loading else { return }
        loading = true
        Task {
            await file.load(attachment, client: client)
            loading = false
            if let u = file.url { player = AVPlayer(url: u) }
        }
    }

    /// m:ss like media::clock in the Mac app.
    func clock(_ secs: Double) -> String {
        let s = Int(secs.rounded())
        return String(format: "%d:%02d", s / 60, s % 60)
    }
}

// MARK: sending

/// One picture, video or file picked in the message box, on its way to the
/// server.
@MainActor
@Observable
final class PendingFile: Identifiable {
    let id = UUID()
    /// The Attachment object once stored on the server.
    var attachment: JSON?
    var preview: UIImage?
    var error: String?
    /// A file from Files (not a picture or video), and its name.
    var isFile = false
    var name = ""
    /// A voice message recorded in the box: the recording on this device
    /// (played before sending) and its length.
    var voice: URL?
    var secs: Double = 0
}

struct PendingChip: View {
    let file: PendingFile
    let remove: () -> Void

    var body: some View {
        ZStack(alignment: .topTrailing) {
            ZStack {
                RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color(.secondarySystemFill))
                if let p = file.preview {
                    Image(uiImage: p).resizable().scaledToFill()
                } else if file.isFile && file.error == nil {
                    VStack(spacing: 2) {
                        Image(systemName: "doc.fill")
                        Text(verbatim: FileTile.label(file.name)).font(.system(size: 9, weight: .semibold))
                    }
                    .foregroundStyle(.secondary)
                    .opacity(file.attachment == nil ? 0.35 : 1)
                    .overlay { if file.attachment == nil { ProgressView() } }
                } else if file.attachment != nil {
                    Image(systemName: "video.fill").foregroundStyle(.secondary)
                } else if file.error != nil {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.red)
                } else {
                    ProgressView()
                }
            }
            .frame(width: 56, height: 56)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            Button(action: remove) {
                Image(systemName: "xmark.circle.fill").symbolRenderingMode(.palette).foregroundStyle(.white, .black.opacity(0.6))
            }
            .offset(x: 6, y: -6)
            .accessibilityLabel("Remove")
        }
        .padding(.top, 6)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(file.error ?? (file.attachment == nil ? "Uploading" : "Ready to send"))
        .accessibilityIdentifier("pendingFile")
    }
}

/// A video from the photo library as a file (videos are too big to load as Data).
struct PickedMovie: Transferable {
    let url: URL
    static var transferRepresentation: some TransferRepresentation {
        FileRepresentation(contentType: .movie) { SentTransferredFile($0.url) } importing: { received in
            let dest = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + "." + (received.file.pathExtension.isEmpty ? "mov" : received.file.pathExtension))
            try FileManager.default.copyItem(at: received.file, to: dest)
            return PickedMovie(url: dest)
        }
    }
}

enum MediaUpload {
    struct Prepared {
        /// A picture's bytes; nil for a video, which goes up from `file`.
        let data: Data?
        /// A video's file (removed after the upload).
        var file: URL? = nil
        let bytes: Int
        let name: String
        let contentType: String
        let isVideo: Bool
        let w: Int
        let h: Int
        let secs: Double
        let preview: UIImage?
        /// A video's still as JPEG (sent as its own file, named by `thumb`).
        var still: Data? = nil
    }

    /// A still from early in a video, no larger than FileAPI.stillSide, as
    /// JPEG (the very first frame is often black, so a little way in).
    static func still(_ asset: AVURLAsset, secs: Double) async -> Data? {
        let gen = AVAssetImageGenerator(asset: asset)
        gen.appliesPreferredTrackTransform = true
        let side = CGFloat(FileAPI.stillSide)
        gen.maximumSize = CGSize(width: side, height: side)
        let at = CMTime(seconds: min(max(secs, 0) / 10, 1), preferredTimescale: 600)
        guard let (cg, _) = try? await gen.image(at: at) else { return nil }
        return UIImage(cgImage: cg).jpegData(compressionQuality: 0.75)
    }

    struct Unreadable: LocalizedError {
        var errorDescription: String? { "That picture or video couldn't be read." }
    }

    static func stamp() -> String {
        let f = DateFormatter()
        f.dateFormat = "yyyyMMdd-HHmmss"
        return f.string(from: Date())
    }

    /// A JPEG, PNG, GIF or WebP goes as it is, without where it was taken
    /// and the phone (MediaScrub); any other picture (a HEIC) becomes a JPEG,
    /// or a PNG when see-through, that every device opens (PictureConvert,
    /// as the Mac does). Videos go up as they are.
    static func prepare(_ item: PhotosPickerItem) async throws -> Prepared {
        if item.supportedContentTypes.contains(where: { $0.conforms(to: .movie) }) {
            guard let movie = try await item.loadTransferable(type: PickedMovie.self) else { throw Unreadable() }
            // our own copy goes without where it was filmed and the phone
            // (a picture loses those by being drawn again, below)
            try MediaScrub.scrubFile(at: movie.url)
            // streamed up from the file (FileAPI.upload(file:)), never read whole
            let bytes = (try? movie.url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
            let asset = AVURLAsset(url: movie.url)
            let secs = (try? await asset.load(.duration)).map(CMTimeGetSeconds) ?? 0
            var w = 0, h = 0
            if let track = try? await asset.loadTracks(withMediaType: .video).first,
               let (size, transform) = try? await track.load(.naturalSize, .preferredTransform) {
                let r = CGRect(origin: .zero, size: size).applying(transform)
                w = Int(abs(r.width).rounded())
                h = Int(abs(r.height).rounded())
            }
            let ext = movie.url.pathExtension.lowercased()
            let type = ext == "mp4" || ext == "m4v" ? "video/mp4" : "video/quicktime"
            let length = secs.isFinite ? secs : 0
            let still = await still(asset, secs: length)
            return Prepared(data: nil, file: movie.url, bytes: bytes, name: "Video-\(stamp()).\(ext.isEmpty ? "mov" : ext)", contentType: type, isVideo: true, w: w, h: h, secs: length, preview: still.flatMap(UIImage.init(data:)), still: still)
        }
        guard let raw = try await item.loadTransferable(type: Data.self) else { throw Unreadable() }
        let data: Data, ext: String
        if let common = PictureConvert.commonExt(raw) {
            (data, ext) = (MediaScrub.scrub(raw) ?? raw, common)
        } else if let c = PictureConvert.converted(raw) {
            (data, ext) = c
        } else {
            throw Unreadable()
        }
        // the size as shown: UIImage applies a JPEG's turn
        guard let image = UIImage(data: data) else { throw Unreadable() }
        let size = CGSize(width: image.size.width * image.scale, height: image.size.height * image.scale)
        let preview = image.preparingThumbnail(of: CGSize(width: 120, height: 120 * size.height / max(size.width, 1)))
        let type = ext == "jpg" ? "image/jpeg" : "image/\(ext)"
        return Prepared(data: data, bytes: data.count, name: "Photo-\(stamp()).\(ext)", contentType: type, isVideo: false, w: Int(size.width.rounded()), h: Int(size.height.rounded()), secs: 0, preview: preview)
    }
}

/// Puts a file picked in Files on the server as it is (no picture size, no
/// still), except that a picture or video goes without where it was taken
/// and the phone (MediaScrub), and a HEIC, TIFF or BMP goes as a JPEG or PNG
/// under that name (PictureConvert); the pending file then holds its
/// Attachment. A sound (m4a, mp3, wav, aac, caf) goes as a sound with its
/// length, as the Mac sends it, and plays in the message.
@MainActor
func uploadFile(_ url: URL, server: String, auth: String?, into f: PendingFile) async {
    f.isFile = true
    f.name = url.lastPathComponent
    let scoped = url.startAccessingSecurityScopedResource()
    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
    do {
        let values = try url.resourceValues(forKeys: [.fileSizeKey, .isDirectoryKey])
        if values.isDirectory == true {
            f.error = String(localized: "\(f.name) is a folder.")
            return
        }
        var bytes = values.fileSize ?? 0
        if bytes > FileAPI.maxBytes {
            f.error = FileError.tooLarge.explanation
            return
        }
        // a copy of our own: the picked file is only readable while access lasts
        var ext = url.pathExtension.lowercased()
        var tmp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        if !ext.isEmpty { tmp.appendPathExtension(ext) }
        try FileManager.default.copyItem(at: url, to: tmp)
        defer { try? FileManager.default.removeItem(at: tmp) }
        if PictureConvert.convertedExts.contains(ext),
           let c = PictureConvert.converted(try Data(contentsOf: tmp)) {
            let out = tmp.deletingPathExtension().appendingPathExtension(c.ext)
            try c.data.write(to: out, options: .atomic)
            try? FileManager.default.removeItem(at: tmp)
            tmp = out
            ext = c.ext
            f.name = ((f.name as NSString).deletingPathExtension as NSString).appendingPathExtension(c.ext) ?? f.name
            bytes = c.data.count
        } else if MediaScrub.mediaExts.contains(ext), try MediaScrub.scrubFile(at: tmp) {
            bytes = (try? tmp.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? bytes
        }
        let sound = Voice.isAudio(ext: ext)
        // streamed from the file, never read whole
        let sha = try await FileAPI.upload(server: server, auth: auth, file: tmp, contentType: sound ? FileAPI.contentType(ext: ext) : "application/octet-stream")
        FileAPI.keep(file: tmp, sha: sha, ext: ext)
        if sound {
            let secs = (try? await AVURLAsset(url: tmp).load(.duration)).map(CMTimeGetSeconds) ?? 0
            f.attachment = FileAPI.audioAttachment(sha: sha, name: f.name, bytes: bytes, secs: secs.isFinite ? secs : 0)
        } else {
            f.attachment = FileAPI.fileAttachment(sha: sha, name: f.name, bytes: bytes)
        }
    } catch let e as FileError {
        f.error = e.explanation
    } catch {
        f.error = error.localizedDescription
    }
}

/// Gets a picked picture or video ready and puts it on the server (a
/// video's still first); the pending file then holds its Attachment.
@MainActor
func uploadMedia(_ item: PhotosPickerItem, server: String, auth: String?, into f: PendingFile) async {
    do {
        let a = try await MediaUpload.prepare(item)
        defer { if let file = a.file { try? FileManager.default.removeItem(at: file) } }
        // a video's still goes up first, so it's there when the message
        // is; it's only a preview, so the video goes without one when
        // it can't (as on the Mac)
        var thumb: String?
        if let still = a.still {
            thumb = try? await FileAPI.upload(server: server, auth: auth, data: still, contentType: "image/jpeg")
            if let thumb { FileAPI.keep(still, sha: thumb, ext: "jpg") }
        }
        let sha: String
        let ext = (a.name as NSString).pathExtension.lowercased()
        if let file = a.file {
            // streamed from the file, never read whole
            sha = try await FileAPI.upload(server: server, auth: auth, file: file, contentType: a.contentType)
            FileAPI.keep(file: file, sha: sha, ext: ext)
        } else {
            let data = a.data ?? Data()
            sha = try await FileAPI.upload(server: server, auth: auth, data: data, contentType: a.contentType)
            FileAPI.keep(data, sha: sha, ext: ext)
        }
        f.attachment = FileAPI.attachment(sha: sha, name: a.name, isVideo: a.isVideo, bytes: a.bytes, w: a.w, h: a.h, secs: a.secs, thumb: thumb)
        f.preview = a.preview
    } catch let e as FileError {
        f.error = e.explanation
    } catch {
        f.error = error.localizedDescription
    }
}
