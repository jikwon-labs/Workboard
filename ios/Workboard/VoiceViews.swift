// Voice messages, as on the Mac (src/app/voice.rs): the microphone button in
// a message box records one (AAC in an .m4a, the Mac's settings), a bar over
// the box shows the time, how loud it is, Stop and a bin; Stop puts it in
// the box, where it can be played before sending or removed. In a message a
// sound shows as a card with play/pause, a bar to tap to a place, and the
// time; its file comes down only when play is pressed, and one sound plays
// at a time. The rules that need no microphone (times, limits, the
// attachment) are WorkboardKit's Voice.
//
// The audio session: playing a sound sets .playback, recording sets
// .playAndRecord and puts back what was there after. During a huddle the
// session is the call's (CallKit and LiveKit set it), so nothing here
// changes it then, and the record button says why it can't record.

import AVFoundation
import SwiftUI
import WorkboardKit

// MARK: playing

/// The one sound playing (or paused) in the app.
@MainActor
@Observable
final class SoundPlayer: NSObject, AVAudioPlayerDelegate {
    static let shared = SoundPlayer()

    /// What is loaded: a file's sha, or a recording's path in a message box.
    private(set) var key: String?
    private(set) var playing = false
    private(set) var position: Double = 0
    private(set) var duration: Double = 0
    /// A huddle holds the audio session: play in it as it is.
    @ObservationIgnored var callActive: () -> Bool = { false }
    @ObservationIgnored private var player: AVAudioPlayer?
    @ObservationIgnored private var ticker: Task<Void, Never>?

    func isPlaying(_ k: String) -> Bool { key == k && playing }

    /// Where `k` is and how long it is, while it's the one loaded.
    func progress(_ k: String) -> (position: Double, duration: Double)? {
        key == k ? (position, duration) : nil
    }

    /// Play `k` from `url`, or pause/resume it when it's the one loaded.
    /// Returns an error to show when the file can't be played.
    @discardableResult
    func toggle(_ k: String, url: URL) -> String? {
        if key == k, let player {
            if playing { pause() } else { resume(player) }
            return nil
        }
        stop()
        do {
            let p = try AVAudioPlayer(contentsOf: url)
            p.delegate = self
            p.prepareToPlay()
            player = p
            key = k
            duration = p.duration
            position = 0
            resume(p)
            return nil
        } catch {
            return String(localized: "This sound can't be played.")
        }
    }

    func pause() {
        player?.pause()
        playing = false
        ticker?.cancel()
        if let player { position = player.currentTime }
    }

    /// Go to `fraction` of `k` (only the one loaded; another card's bar
    /// starts it from there).
    func seek(_ k: String, fraction: Double, url: URL? = nil) {
        if key != k {
            guard let url, toggle(k, url: url) == nil else { return }
        }
        guard let player else { return }
        player.currentTime = Voice.seekTime(fraction: fraction, duration: player.duration)
        position = player.currentTime
    }

    /// Unload whatever is loaded.
    func stop() {
        ticker?.cancel()
        player?.stop()
        player = nil
        key = nil
        playing = false
        position = 0
        duration = 0
        letSessionGo()
    }

    /// Stop `k` if it's the one loaded (a recording taken out of the box).
    func stop(_ k: String) { if key == k { stop() } }

    private func resume(_ p: AVAudioPlayer) {
        if !callActive() {
            let s = AVAudioSession.sharedInstance()
            try? s.setCategory(.playback, mode: .default)
            try? s.setActive(true)
        }
        p.play()
        playing = true
        ticker?.cancel()
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, let p = self.player, self.playing else { return }
                self.position = p.currentTime
                try? await Task.sleep(nanoseconds: 100_000_000)
            }
        }
    }

    private func letSessionGo() {
        guard !callActive() else { return }
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
    }

    nonisolated func audioPlayerDidFinishPlaying(_ p: AVAudioPlayer, successfully flag: Bool) {
        Task { @MainActor in
            guard p === self.player else { return }
            // back to the start, ready to play again; the time shows the length
            self.ticker?.cancel()
            self.playing = false
            self.position = 0
            p.currentTime = 0
            self.letSessionGo()
        }
    }
}

/// The round play/pause button, the bar and the time of a sound.
struct SoundControls: View {
    let key: String
    let name: String
    let duration: Double
    /// Shown instead of the time (loading, an error).
    var status: String?
    var loading = false
    let toggle: () -> Void
    let seek: (Double) -> Void
    private var sound: SoundPlayer { SoundPlayer.shared }

    var body: some View {
        let playing = sound.isPlaying(key)
        let (pos, dur) = sound.progress(key) ?? (0, duration)
        HStack(spacing: 10) {
            Button(action: toggle) {
                ZStack {
                    Circle().fill(Color.accentColor)
                    if loading {
                        ProgressView().tint(.white)
                    } else {
                        Image(systemName: playing ? "pause.fill" : "play.fill")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(.white)
                            .offset(x: playing ? 0 : 1)
                    }
                }
                .frame(width: 36, height: 36)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(playing ? Text("Pause \(name)") : Text("Play \(name)"))
            .accessibilityIdentifier(playing ? "pauseSound" : "playSound")
            VStack(alignment: .leading, spacing: 5) {
                GeometryReader { geo in
                    ZStack(alignment: .leading) {
                        Capsule().fill(Color(.tertiarySystemFill))
                        Capsule().fill(Color.accentColor)
                            .frame(width: geo.size.width * Voice.fraction(position: pos, duration: dur))
                    }
                    .frame(height: 4)
                    .frame(maxHeight: .infinity)
                    .contentShape(Rectangle())
                    .onTapGesture(coordinateSpace: .local) { p in
                        seek(geo.size.width > 0 ? p.x / geo.size.width : 0)
                    }
                }
                .frame(height: 16)
                .accessibilityElement()
                .accessibilityLabel(Text("Progress of \(name)"))
                .accessibilityValue(Text(verbatim: Voice.progressLabel(position: pos, duration: dur)))
                .accessibilityIdentifier("soundProgress")
                Text(verbatim: status ?? Voice.progressLabel(position: pos, duration: dur))
                    .font(.caption)
                    .monospacedDigit()
                    .foregroundStyle(status == nil ? Color.secondary : Color.red)
                    .lineLimit(2)
                    .accessibilityIdentifier("soundTime")
            }
        }
    }
}

/// A sound in a message: fetched (once) when play is first pressed.
struct VoiceCard: View {
    @Environment(SyncClient.self) private var client
    @Environment(CallManager.self) private var calls
    let attachment: AttachmentView
    /// Under the time, and the card takes the whole width (the
    /// conversation's Other files: who sent it and when).
    var detail: String?
    @State private var file = FileLoad()
    @State private var loading = false
    @State private var failed: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            SoundControls(key: attachment.file, name: attachment.name, duration: attachment.secs,
                          status: failed ?? file.error, loading: loading,
                          toggle: { Task { await play(at: nil) } },
                          seek: { f in Task { await play(at: f) } })
            if let detail {
                Text(verbatim: detail).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .padding(10)
        .frame(maxWidth: detail == nil ? 260 : .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color(.secondarySystemBackground)))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("voice-\(attachment.file.prefix(12))")
    }

    /// Play or pause; `at` (0...1, a tap on the bar) plays from there.
    func play(at fraction: Double?) async {
        guard !loading else { return }
        let sound = SoundPlayer.shared
        sound.callActive = { [calls] in calls.isActive }
        if sound.key == attachment.file, let url = file.url {
            if let fraction { sound.seek(attachment.file, fraction: fraction) } else { sound.toggle(attachment.file, url: url) }
            return
        }
        failed = nil
        if file.url == nil {
            loading = true
            await file.load(attachment, client: client)
            loading = false
        }
        guard let url = file.url else { return }
        if let fraction {
            sound.seek(attachment.file, fraction: fraction, url: url)
        } else {
            failed = sound.toggle(attachment.file, url: url)
        }
    }
}

// MARK: recording

/// The one voice message being recorded, in whichever message box started it.
@MainActor
@Observable
final class VoiceRecorder {
    static let shared = VoiceRecorder()

    enum Outcome {
        /// Stopped: the file and its length, for the box.
        case recorded(URL, Double)
        /// Stopped by itself at Voice.maxSecs (kept, with a note).
        case limit(URL, Double)
        /// Shorter than Voice.minSecs: dropped.
        case tooShort
        case failed(String)
    }

    /// Which message box is recording (its id), nil when none is.
    private(set) var owner: UUID?
    private(set) var secs: Double = 0
    private(set) var level: Double = 0
    @ObservationIgnored private var rec: AVAudioRecorder?
    @ObservationIgnored private var deliver: ((Outcome) -> Void)?
    @ObservationIgnored private var ticker: Task<Void, Never>?
    @ObservationIgnored private var saved: (AVAudioSession.Category, AVAudioSession.Mode, AVAudioSession.CategoryOptions)?

    func isRecording(_ id: UUID) -> Bool { owner == id }

    /// Start recording for the box `id`; what comes of it goes to `done`.
    /// Returns a note to show when it can't start. A recording going on in
    /// another box is stopped and kept there first (as on the Mac).
    func start(for id: UUID, inCall: Bool, done: @escaping (Outcome) -> Void) async -> String? {
        if inCall { return String(localized: "You can't record a voice message during a huddle.") }
        if owner != nil { stop(keep: true) }
        switch AVAudioApplication.shared.recordPermission {
        case .granted: break
        case .undetermined:
            // the system asks; a yes starts it right away
            guard await AVAudioApplication.requestRecordPermission() else { return Self.deniedNote }
        default:
            return Self.deniedNote
        }
        SoundPlayer.shared.stop()
        let s = AVAudioSession.sharedInstance()
        saved = (s.category, s.mode, s.categoryOptions)
        do {
            try s.setCategory(.playAndRecord, mode: .default, options: [.defaultToSpeaker, .allowBluetoothHFP])
            try s.setActive(true)
        } catch {
            restoreSession()
            return String(localized: "The microphone can't be used.")
        }
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("Recordings", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        Self.clearOld(dir)
        let url = dir.appendingPathComponent(Voice.fileName())
        try? FileManager.default.removeItem(at: url)
        let settings: [String: Any] = [
            AVFormatIDKey: kAudioFormatMPEG4AAC,
            AVSampleRateKey: Voice.sampleRate,
            AVNumberOfChannelsKey: Voice.channels,
            AVEncoderBitRateKey: Voice.bitRate,
        ]
        guard let r = try? AVAudioRecorder(url: url, settings: settings) else {
            restoreSession()
            return String(localized: "The microphone can't be used.")
        }
        r.isMeteringEnabled = true
        guard r.record() else {
            restoreSession()
            return String(localized: "The microphone can't be used.")
        }
        rec = r
        owner = id
        deliver = done
        secs = 0
        level = 0
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, let r = self.rec else { return }
                r.updateMeters()
                self.secs = r.currentTime
                self.level = Voice.level(db: r.averagePower(forChannel: 0))
                if Voice.reachedLimit(self.secs) {
                    self.finish(keep: true, limit: true)
                    return
                }
                try? await Task.sleep(nanoseconds: 100_000_000)
            }
        }
        return nil
    }

    /// Recordings from a day ago and before: sent (a copy is kept under
    /// its sha) or let go of long since.
    static func clearOld(_ dir: URL) {
        let fm = FileManager.default
        let cutoff = Date().addingTimeInterval(-86_400)
        for u in (try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.contentModificationDateKey])) ?? [] {
            if let d = try? u.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate, d < cutoff {
                try? fm.removeItem(at: u)
            }
        }
    }

    static var deniedNote: String {
        String(localized: "Workboard can't use the microphone. Turn it on in Settings > Workboard > Microphone.")
    }

    /// Stop: into its box (`keep`) or thrown away.
    func stop(keep: Bool) { finish(keep: keep, limit: false) }

    /// Throw away a recording going on in the box `id` (the box went away).
    func cancel(for id: UUID) { if owner == id { stop(keep: false) } }

    private func finish(keep: Bool, limit: Bool) {
        guard let r = rec else { return }
        ticker?.cancel()
        let length = r.currentTime
        r.stop()
        let url = r.url
        let done = deliver
        rec = nil
        deliver = nil
        owner = nil
        secs = 0
        level = 0
        restoreSession()
        guard keep else {
            r.deleteRecording()
            return
        }
        // a slip of the button: nothing worth sending
        if Voice.stop(after: length) == .tooShort {
            try? FileManager.default.removeItem(at: url)
            done?(.tooShort)
            return
        }
        guard FileManager.default.fileExists(atPath: url.path) else {
            done?(.failed(String(localized: "The recording couldn't be saved.")))
            return
        }
        done?(limit ? .limit(url, length) : .recorded(url, length))
    }

    private func restoreSession() {
        let s = AVAudioSession.sharedInstance()
        try? s.setActive(false, options: .notifyOthersOnDeactivation)
        if let (c, m, o) = saved { try? s.setCategory(c, mode: m, options: o) }
        saved = nil
    }
}

/// Over the message box while it records: a breathing red dot, the time so
/// far out of 5:00, how loud it is, a bin and Stop.
struct RecordingBar: View {
    let id: UUID
    private var rec: VoiceRecorder { VoiceRecorder.shared }
    @State private var breathe = false

    var body: some View {
        HStack(spacing: 10) {
            Circle().fill(Color.red)
                .frame(width: 10, height: 10)
                .opacity(breathe ? 0.35 : 1)
                .animation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true), value: breathe)
                .onAppear { breathe = true }
                .accessibilityHidden(true)
            Text("Recording").font(.subheadline.weight(.medium))
            Text(verbatim: Voice.recordingLabel(rec.secs))
                .font(.subheadline)
                .monospacedDigit()
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("recordingTime")
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color(.tertiarySystemFill))
                    Capsule().fill(Color.red.opacity(0.8))
                        .frame(width: max(4, geo.size.width * rec.level))
                        .animation(.linear(duration: 0.1), value: rec.level)
                }
                .frame(height: 6)
                .frame(maxHeight: .infinity)
            }
            .frame(height: 20)
            .accessibilityHidden(true)
            Button { rec.stop(keep: false) } label: {
                Image(systemName: "trash").font(.system(size: 17))
            }
            .foregroundStyle(.secondary)
            .accessibilityLabel("Discard recording")
            .accessibilityIdentifier("discardRecording")
            Button { rec.stop(keep: true) } label: {
                Image(systemName: "stop.circle.fill").font(.system(size: 26)).foregroundStyle(.red)
            }
            .accessibilityLabel("Stop recording")
            .accessibilityIdentifier("stopRecording")
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color(.secondarySystemBackground)))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("recordingBar")
    }
}

/// A recording waiting in the message box: play it before sending, or
/// take it out with the X like any picked file.
struct PendingVoiceChip: View {
    @Environment(CallManager.self) private var calls
    let file: PendingFile
    let remove: () -> Void

    var body: some View {
        let key = file.voice?.path ?? file.id.uuidString
        let sound = SoundPlayer.shared
        ZStack(alignment: .topTrailing) {
            SoundControls(key: key, name: String(localized: "Voice message"), duration: file.secs,
                          status: file.error, loading: file.attachment == nil && file.error == nil,
                          toggle: {
                              sound.callActive = { [calls] in calls.isActive }
                              if let u = file.voice { sound.toggle(key, url: u) }
                          },
                          seek: { f in
                              sound.callActive = { [calls] in calls.isActive }
                              if let u = file.voice { sound.seek(key, fraction: f, url: u) }
                          })
                .padding(.horizontal, 10)
                .frame(width: 200, height: 56)
                .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Color(.secondarySystemFill)))
            Button {
                SoundPlayer.shared.stop(key)
                remove()
            } label: {
                Image(systemName: "xmark.circle.fill").symbolRenderingMode(.palette).foregroundStyle(.white, .black.opacity(0.6))
            }
            .offset(x: 6, y: -6)
            .accessibilityLabel("Remove")
        }
        .padding(.top, 6)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(file.error ?? (file.attachment == nil ? String(localized: "Uploading") : String(localized: "Ready to send")))
        .accessibilityIdentifier("pendingVoice")
    }
}

/// Puts a recording on the server (as audio/mp4) and keeps a copy under
/// its sha, so playing the sent message never fetches it; the pending file
/// then holds its Attachment. The recording itself stays for playing in
/// the box until the box lets it go.
@MainActor
func uploadVoice(_ url: URL, secs: Double, server: String, auth: String?, into f: PendingFile) async {
    do {
        let data = try Data(contentsOf: url)
        let sha = try await FileAPI.upload(server: server, auth: auth, data: data, contentType: Voice.contentType)
        FileAPI.keep(data, sha: sha, ext: url.pathExtension.lowercased())
        f.attachment = FileAPI.audioAttachment(sha: sha, name: f.name, bytes: data.count, secs: secs)
    } catch let e as FileError {
        f.error = e.explanation
    } catch {
        f.error = error.localizedDescription
    }
}
