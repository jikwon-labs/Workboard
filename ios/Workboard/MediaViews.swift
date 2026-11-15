// Pictures and videos in chat messages. Files come from the team server
// (GET /files/<sha256> with this device's token, see WorkboardKit Files.swift)
// and are kept in the caches folder. Pictures show in the message and open
// large on a tap; a video shows as a tile with its still (`thumb`, a small
// JPEG, when the sender made one) and downloads the video only when it's
// played, in the system player. Sending: photos picked in the message box go
// up first (PUT /files) and the message names them, the same shape the Mac
// app writes; a video goes with a still made here.

import AVFoundation
import AVKit
import CoreTransferable
import PhotosUI
import SwiftUI
import UniformTypeIdentifiers
import WorkboardKit

// MARK: showing

struct AttachmentsView: View {
    let attachments: [AttachmentView]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(attachments, id: \.file) { a in
                if a.isVideo { VideoTile(attachment: a) } else { PictureView(attachment: a) }
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

struct PictureView: View {
    @Environment(SyncClient.self) private var client
    let attachment: AttachmentView
    @State private var file = FileLoad()
    @State private var image: UIImage?
    @State private var showLarge = false

    var body: some View {
        let size = boxSize(attachment)
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
            if let u = file.url { image = await Task.detached { UIImage(contentsOfFile: u.path) }.value }
        }
        .fullScreenCover(isPresented: $showLarge) {
            if let image { LargePicture(image: image, name: attachment.name) }
        }
    }
}

private struct LargePicture: View {
    @Environment(\.dismiss) private var dismiss
    let image: UIImage
    let name: String
    @State private var scale: CGFloat = 1

    var body: some View {
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
    }
}

struct VideoTile: View {
    @Environment(SyncClient.self) private var client
    let attachment: AttachmentView
    @State private var file = FileLoad()
    @State private var loading = false
    @State private var player: AVPlayer?
    /// The still the sender made (only the small JPEG is fetched for it).
    @State private var still: UIImage?

    var body: some View {
        let size = boxSize(attachment)
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
                still = await Task.detached { UIImage(contentsOfFile: u.path) }.value
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

/// One picture or video picked in the message box, on its way to the server.
@MainActor
@Observable
final class PendingFile: Identifiable {
    let id = UUID()
    /// The Attachment object once stored on the server.
    var attachment: JSON?
    var preview: UIImage?
    var error: String?
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
        let data: Data
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

    /// Pictures become JPEG with their orientation applied (every app reads
    /// that); videos go up as they are.
    static func prepare(_ item: PhotosPickerItem) async throws -> Prepared {
        if item.supportedContentTypes.contains(where: { $0.conforms(to: .movie) }) {
            guard let movie = try await item.loadTransferable(type: PickedMovie.self) else { throw Unreadable() }
            defer { try? FileManager.default.removeItem(at: movie.url) }
            let data = try Data(contentsOf: movie.url, options: .mappedIfSafe)
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
            return Prepared(data: data, name: "Video-\(stamp()).\(ext.isEmpty ? "mov" : ext)", contentType: type, isVideo: true, w: w, h: h, secs: length, preview: still.flatMap(UIImage.init(data:)), still: still)
        }
        guard let raw = try await item.loadTransferable(type: Data.self), let image = UIImage(data: raw) else { throw Unreadable() }
        let maxSide: CGFloat = 4096
        let scale = min(1, maxSide / max(image.size.width, image.size.height))
        let size = CGSize(width: (image.size.width * scale).rounded(), height: (image.size.height * scale).rounded())
        let format = UIGraphicsImageRendererFormat.default()
        format.scale = 1
        let drawn = UIGraphicsImageRenderer(size: size, format: format).image { _ in image.draw(in: CGRect(origin: .zero, size: size)) }
        guard let jpeg = drawn.jpegData(compressionQuality: 0.85) else { throw Unreadable() }
        let preview = drawn.preparingThumbnail(of: CGSize(width: 120, height: 120 * size.height / max(size.width, 1)))
        return Prepared(data: jpeg, name: "Photo-\(stamp()).jpg", contentType: "image/jpeg", isVideo: false, w: Int(size.width), h: Int(size.height), secs: 0, preview: preview)
    }
}
