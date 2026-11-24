// Voice messages (src/app/voice.rs and src/media.rs on the Mac): the
// microphone button in a message box records one, it waits in the box like a
// picked file, and goes out as an attachment of kind "Audio". The rules here
// are the ones that need no microphone or speaker, so they can be tested.

import Foundation

public enum Voice {
    /// The longest voice message the record button takes; it stops by
    /// itself then and keeps it (media::VOICE_MAX_SECS).
    public static let maxSecs: Double = 300
    /// Shorter than this is a slip of the button: dropped with "Too short
    /// to send" (stop_recording).
    public static let minSecs: Double = 0.5
    /// File name endings taken as sounds (media::AUDIO_EXTS). A voice
    /// message is an AAC ".m4a".
    public static let audioExts: Set<String> = ["m4a", "mp3", "wav", "aac", "caf"]
    /// What a recording is sent as.
    public static let contentType = "audio/mp4"
    /// Recording settings, as the Mac's AVAudioRecorder: AAC, mono, 44.1 kHz, 64 kbit/s.
    public static let sampleRate: Double = 44_100
    public static let channels = 1
    public static let bitRate = 64_000

    /// A file with this ending plays as a sound.
    public static func isAudio(ext: String) -> Bool { audioExts.contains(ext.lowercased()) }

    /// The name a voice message recorded at `date` goes by ("Voice message
    /// 2026-11-24 15.04.09.m4a", media::voice_name): what apps that can't
    /// play it show and save.
    public static func fileName(at date: Date = Date(), timeZone: TimeZone = .current, ext: String = "m4a") -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.calendar = Calendar(identifier: .gregorian)
        f.timeZone = timeZone
        f.dateFormat = "yyyy-MM-dd HH.mm.ss"
        return "Voice message \(f.string(from: date)).\(ext)"
    }

    /// "0:12", "1:05", "1:02:03" (media::clock): rounded to the second.
    public static func clock(_ secs: Double) -> String {
        let s = secs.isFinite ? Int(max(0, secs).rounded()) : 0
        if s >= 3600 { return String(format: "%d:%02d:%02d", s / 3600, s / 60 % 60, s % 60) }
        return String(format: "%d:%02d", s / 60, s % 60)
    }

    /// The time under a sound's bar: its length, or "0:03 / 0:12" once
    /// it has played some (voice.rs sound card).
    public static func progressLabel(position: Double, duration: Double) -> String {
        position > 0 ? "\(clock(position)) / \(clock(duration))" : clock(duration)
    }

    /// How far along the bar is filled, 0...1.
    public static func fraction(position: Double, duration: Double) -> Double {
        guard duration > 0, position.isFinite else { return 0 }
        return min(1, max(0, position / duration))
    }

    /// Where a tap at `fraction` of the bar goes, in seconds.
    public static func seekTime(fraction: Double, duration: Double) -> Double {
        guard duration > 0, fraction.isFinite else { return 0 }
        return min(1, max(0, fraction)) * duration
    }

    /// "0:04 / 5:00" in the recording bar.
    public static func recordingLabel(_ secs: Double) -> String {
        "\(clock(secs)) / \(clock(maxSecs))"
    }

    /// What happens to a recording stopped after `secs`.
    public enum Stop: Equatable, Sendable {
        /// Into the message box.
        case keep
        /// Dropped: "Too short to send".
        case tooShort
    }

    public static func stop(after secs: Double) -> Stop {
        secs < minSecs ? .tooShort : .keep
    }

    /// It has gone on as long as a voice message may: stop and keep it.
    public static func reachedLimit(_ secs: Double) -> Bool { secs >= maxSecs }

    /// How loud, 0...1, from AVAudioRecorder's average power in dB (the
    /// Mac's Recorder::level).
    public static func level(db: Float) -> Double {
        guard db.isFinite else { return 0 }
        return Double(min(1, max(0, (db + 50) / 50)))
    }
}
