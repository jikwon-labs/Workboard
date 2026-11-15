// Huddles (voice, then video) through LiveKit. The app asks workboard-server
// for a join token over the open WebSocket (call_join -> call_token, the same
// as the Mac app) and joins the conversation's room. A server without LiveKit
// turns that down; then the app makes the Mac app's link call instead: a
// "Started a call" message with a Jitsi link, opened in Safari.
//
// A huddle is also reported to CallKit (CallKit.swift), so it shows in the
// system call UI and on the lock screen; mute and leave go through CallKit
// then, and work from either side.

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

    @ObservationIgnored private let callKit = CallKitController()
    @ObservationIgnored private var watcher: RoomWatcher?

    init() {
        callKit.actions = self
    }

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
        let title = client.view.channels.first { $0.id == ch }.map { client.view.title(of: $0, me: client.me ?? 0) } ?? "Huddle"
        let viaCallKit = await callKit.start(title: title, channel: ch, video: video)
        do {
            let r = Room()
            room = r
            let w = RoomWatcher { [weak self, weak r] error in
                guard let self, let r, r === self.room else { return }
                self.dropped(error)
            }
            watcher = w
            r.add(delegate: w)
            try await r.connect(url: grant.url, token: grant.token)
            phase = .inCall
            callKit.reportConnected()
            if announce {
                client.edit { d, me in Edits.postHuddle(&d, channel: ch, me: me, id: client.newId(), ts: nowTs()) }
            }
            try? await r.localParticipant.setMicrophone(enabled: micOn)
            if video { try? await r.localParticipant.setCamera(enabled: true) }
        } catch {
            if viaCallKit { callKit.reportEnded(failed: true) }
            phase = .failed("Couldn't join the huddle. Is the call server running? (\(error.localizedDescription))")
        }
        return nil
    }

    /// The mute button: through CallKit when the huddle is reported there,
    /// so the system call UI shows the same.
    func toggleMic() async {
        let on = !micOn
        if callKit.isActive, await callKit.requestMic(on: on) { return }
        await applyMic(on)
    }

    private func applyMic(_ on: Bool) async {
        micOn = on
        if phase == .inCall { try? await room.localParticipant.setMicrophone(enabled: on) }
    }

    func toggleCamera() async {
        cameraOn.toggle()
        callKit.reportVideo(cameraOn)
        if phase == .inCall { try? await room.localParticipant.setCamera(enabled: cameraOn) }
    }

    /// Leave: through CallKit when the huddle is reported there.
    func leave() async {
        if callKit.isActive, await callKit.requestEnd() { return }
        await hangUp()
    }

    /// The room went away without Leave (server gone, network lost).
    private func dropped(_ error: LiveKitError?) {
        guard phase == .inCall else { return }
        callKit.reportEnded(failed: error != nil)
        channel = nil
        cameraOn = false
        micOn = true
        if error != nil {
            phase = .failed("The huddle was cut off. Check the connection and join again.")
        } else {
            phase = .idle
            showCall = false
        }
    }

    private func hangUp() async {
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

extension CallManager: CallKitActions {
    func callKitEnded() async { await hangUp() }
    func callKitSetMic(on: Bool) async { await applyMic(on) }
}

/// Tells the manager when a room disconnects.
final class RoomWatcher: RoomDelegate, @unchecked Sendable {
    let onDisconnect: @MainActor (LiveKitError?) -> Void

    init(onDisconnect: @escaping @MainActor (LiveKitError?) -> Void) {
        self.onDisconnect = onDisconnect
    }

    func room(_ room: Room, didDisconnectWithError error: LiveKitError?) {
        Task { @MainActor in self.onDisconnect(error) }
    }
}
