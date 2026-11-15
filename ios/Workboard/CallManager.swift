// Huddles (voice, then video) through LiveKit, as in docs/server-design.md
// phases 3 and 4: the app asks workboard-server for a join token
// (POST /call-token) and joins the channel's room. A server without calls
// gets the Mac app's link call instead: a "Started a call" message with a
// Jitsi link, opened in Safari.

import LiveKit
import SwiftUI
import WorkboardKit

@MainActor
@Observable
final class CallManager {
    enum Phase: Equatable {
        case idle
        case joining
        case inCall
        case failed(String)
    }

    private(set) var phase: Phase = .idle
    private(set) var channel: UInt64?
    private(set) var micOn = true
    private(set) var cameraOn = false
    /// The LiveKit room; replaced for every call.
    private(set) var room = Room()
    var showCall = false

    var isActive: Bool { phase == .joining || phase == .inCall }

    /// Start or join the huddle in a channel. Returns a link to open when the
    /// server has no calls and a link call was posted instead.
    func start(channel ch: UInt64, client: SyncClient, video: Bool = false) async -> URL? {
        if isActive {
            showCall = true
            return nil
        }
        channel = ch
        phase = .joining
        showCall = true
        cameraOn = video
        do {
            let grant = try await CallAPI.requestToken(server: client.settings.server, deviceToken: client.deviceToken, channel: ch)
            let r = Room()
            room = r
            try await r.connect(url: grant.url, token: grant.token)
            phase = .inCall
            try? await r.localParticipant.setMicrophone(enabled: micOn)
            if video { try? await r.localParticipant.setCamera(enabled: true) }
            return nil
        } catch CallError.notAvailable {
            showCall = false
            phase = .idle
            channel = nil
            let link = CallAPI.linkCall()
            client.edit { d, me in Edits.postCall(&d, channel: ch, link: link, me: me, id: client.newId(), ts: nowTs()) }
            return URL(string: link)
        } catch CallError.unauthorized {
            phase = .failed("Sign in again to join huddles.")
        } catch CallError.failed(let why) {
            phase = .failed("Couldn't reach the server: \(why)")
        } catch {
            phase = .failed("Couldn't join the huddle. Is the call server running? (\(error.localizedDescription))")
        }
        return nil
    }

    func toggleMic() async {
        micOn.toggle()
        if phase == .inCall { try? await room.localParticipant.setMicrophone(enabled: micOn) }
    }

    func toggleCamera() async {
        cameraOn.toggle()
        if phase == .inCall { try? await room.localParticipant.setCamera(enabled: cameraOn) }
    }

    func leave() async {
        let r = room
        phase = .idle
        channel = nil
        showCall = false
        cameraOn = false
        micOn = true
        await r.disconnect()
    }

    func dismissFailure() {
        if case .failed = phase {
            phase = .idle
            channel = nil
            showCall = false
        }
    }
}
