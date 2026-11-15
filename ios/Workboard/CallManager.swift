// Huddles (voice, then video) through LiveKit. The app asks workboard-server
// for a join token over the open WebSocket (call_join -> call_token, the same
// as the Mac app) and joins the conversation's room. A server without LiveKit
// turns that down; then the app makes the Mac app's link call instead: a
// "Started a call" message with a Jitsi link, opened in Safari.

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

    /// Start or join the huddle in a channel. `announce` posts the Mac
    /// app's "Started a huddle" card once in (not when joining from one).
    /// Returns a link to open when the server has no huddles and a link call
    /// was posted instead.
    func start(channel ch: UInt64, client: SyncClient, video: Bool = false, announce: Bool = true) async -> URL? {
        if isActive {
            showCall = true
            return nil
        }
        channel = ch
        phase = .joining
        showCall = true
        cameraOn = video
        let grant: CallGrant
        do {
            grant = try await client.requestCall(channel: ch)
        } catch CallError.notAvailable {
            showCall = false
            phase = .idle
            channel = nil
            let link = CallAPI.linkCall()
            client.edit { d, me in Edits.postCall(&d, channel: ch, link: link, me: me, id: client.newId(), ts: nowTs()) }
            return URL(string: link)
        } catch CallError.denied(let why) {
            phase = .failed(why)
            return nil
        } catch CallError.offline {
            phase = .failed("Huddles need the connection to the server. Try again when you're back online.")
            return nil
        } catch {
            phase = .failed("The server didn't answer, so the huddle didn't start. Try again in a moment.")
            return nil
        }
        do {
            let r = Room()
            room = r
            try await r.connect(url: grant.url, token: grant.token)
            phase = .inCall
            if announce {
                client.edit { d, me in Edits.postHuddle(&d, channel: ch, me: me, id: client.newId(), ts: nowTs()) }
            }
            try? await r.localParticipant.setMicrophone(enabled: micOn)
            if video { try? await r.localParticipant.setCamera(enabled: true) }
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
