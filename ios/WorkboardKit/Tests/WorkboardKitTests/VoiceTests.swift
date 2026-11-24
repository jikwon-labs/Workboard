import XCTest
@testable import WorkboardKit
import WorkboardFakeServer

/// Voice messages: the attachment the Mac writes, how a sound is told
/// apart, and the times and limits of the record button and the player.
final class VoiceTests: XCTestCase {
    let sha = String(repeating: "e", count: 64)

    func testVoiceAttachmentIsTheMacsShape() {
        let a = FileAPI.audioAttachment(sha: sha, name: "Voice message 2026-11-24 15.04.09.m4a", bytes: 48_213, secs: 12.4)
        XCTAssertEqual(a.object?.keys.sorted(), ["bytes", "file", "kind", "name", "secs"], "no w, h or thumb")
        XCTAssertEqual(a["file"]?.string, sha)
        XCTAssertEqual(a["name"]?.string, "Voice message 2026-11-24 15.04.09.m4a")
        XCTAssertEqual(a["kind"]?.string, "Audio")
        XCTAssertEqual(a["bytes"]?.int, 48_213)
        XCTAssertEqual(a["secs"]?.double ?? 0, 12.4, accuracy: 0.0001)
        XCTAssertEqual(FileAPI.audioAttachment(sha: sha, name: "x.m4a", bytes: 1, secs: .nan)["secs"]?.double, 0)
        XCTAssertEqual(FileAPI.audioAttachment(sha: sha, name: "x.m4a", bytes: 1, secs: -3)["secs"]?.double, 0)
    }

    func testVoiceMessageSentAndRead() {
        var d = SampleWorkspace.make()
        let a = FileAPI.audioAttachment(sha: sha, name: "Voice message 2026-11-24 15.04.09.m4a", bytes: 2048, secs: 3.5)
        XCTAssertTrue(Edits.sendMessage(&d, channel: 301, text: "", attachments: [a], me: 1, id: 9500, ts: 9))
        let m = WorkspaceView(d).channels.first { $0.id == 301 }!.messages.last!
        let v = m.attachments.first!
        XCTAssertTrue(v.isAudio)
        XCTAssertFalse(v.isFile)
        XCTAssertFalse(v.isVideo)
        XCTAssertEqual(v.secs, 3.5, accuracy: 0.0001)
        XCTAssertEqual(v.bytes, 2048)
        XCTAssertEqual(v.w, 0)
        XCTAssertNil(v.thumb)
        // a Trash item or a restore puts the file back on the server
        XCTAssertEqual(FileAPI.files(inPayload: #"{"attachments":[{"file":"\#(sha)","kind":"Audio"}]}"#), [sha])
        // a message with only a sound, in a banner
        XCTAssertEqual(Notify.sentWhat([a]), "Sent a voice message")
    }

    func testAudioEndingsAndContentTypes() {
        for e in ["m4a", "MP3", "wav", "aac", "caf"] { XCTAssertTrue(Voice.isAudio(ext: e), e) }
        for e in ["mp4", "mov", "pdf", ""] { XCTAssertFalse(Voice.isAudio(ext: e), e) }
        XCTAssertEqual(FileAPI.contentType(ext: "m4a"), "audio/mp4")
        XCTAssertEqual(FileAPI.contentType(ext: "mp3"), "audio/mpeg")
        XCTAssertEqual(FileAPI.contentType(ext: "wav"), "audio/wav")
        XCTAssertEqual(FileAPI.contentType(ext: "aac"), "audio/aac")
        XCTAssertEqual(FileAPI.contentType(ext: "caf"), "audio/x-caf")
        XCTAssertEqual(Voice.contentType, "audio/mp4")
    }

    func testFileNameAsTheMacNamesIt() {
        var c = DateComponents()
        (c.year, c.month, c.day, c.hour, c.minute, c.second) = (2026, 11, 24, 15, 4, 9)
        let utc = TimeZone(identifier: "UTC")!
        var cal = Calendar(identifier: .gregorian)
        cal.timeZone = utc
        let date = cal.date(from: c)!
        XCTAssertEqual(Voice.fileName(at: date, timeZone: utc), "Voice message 2026-11-24 15.04.09.m4a")
    }

    func testClockAndProgress() {
        XCTAssertEqual(Voice.clock(0), "0:00")
        XCTAssertEqual(Voice.clock(4.4), "0:04")
        XCTAssertEqual(Voice.clock(4.6), "0:05")
        XCTAssertEqual(Voice.clock(65), "1:05")
        XCTAssertEqual(Voice.clock(300), "5:00")
        XCTAssertEqual(Voice.clock(3723), "1:02:03")
        XCTAssertEqual(Voice.clock(-2), "0:00")
        XCTAssertEqual(Voice.clock(.infinity), "0:00")
        XCTAssertEqual(Voice.progressLabel(position: 0, duration: 12), "0:12", "its length before it plays")
        XCTAssertEqual(Voice.progressLabel(position: 3.2, duration: 12), "0:03 / 0:12")
        XCTAssertEqual(Voice.recordingLabel(4), "0:04 / 5:00")
        XCTAssertEqual(Voice.fraction(position: 3, duration: 12), 0.25)
        XCTAssertEqual(Voice.fraction(position: 30, duration: 12), 1)
        XCTAssertEqual(Voice.fraction(position: 3, duration: 0), 0)
        XCTAssertEqual(Voice.seekTime(fraction: 0.5, duration: 12), 6)
        XCTAssertEqual(Voice.seekTime(fraction: 1.4, duration: 12), 12)
        XCTAssertEqual(Voice.seekTime(fraction: -1, duration: 12), 0)
    }

    func testRecordingLimits() {
        XCTAssertEqual(Voice.stop(after: 0.2), .tooShort)
        XCTAssertEqual(Voice.stop(after: 0.49), .tooShort)
        XCTAssertEqual(Voice.stop(after: 0.5), .keep)
        XCTAssertEqual(Voice.stop(after: 42), .keep)
        XCTAssertFalse(Voice.reachedLimit(299.9))
        XCTAssertTrue(Voice.reachedLimit(300))
        XCTAssertEqual(Voice.maxSecs, 300)
        XCTAssertEqual(Voice.level(db: -50), 0)
        XCTAssertEqual(Voice.level(db: -80), 0)
        XCTAssertEqual(Voice.level(db: -25), 0.5)
        XCTAssertEqual(Voice.level(db: 0), 1)
        XCTAssertEqual(Voice.level(db: -.infinity), 0)
    }
}
