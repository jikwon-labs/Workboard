// Huddles in the system call UI. A huddle started or joined here is reported
// to CallKit as an outgoing call, so it shows on the lock screen, in the
// status bar and in Control Center like a phone call, and mute and end work
// from there as well as from the huddle screen:
//
//   app -> CallKit   start: CXStartCallAction, then connecting / connected
//                    mute button: CXSetMutedCallAction; Leave: CXEndCallAction
//   CallKit -> app   the same actions from the system UI (perform...), and
//                    "the call is over" when the room drops (reportCall ended)
//
// CallKit owns the audio session while a call is reported: LiveKit's own
// session handling is turned off and its audio engine is held off until
// CallKit activates the session (didActivate), as LiveKit's docs describe.
//
// There is no ringing for incoming huddles (that needs VoIP push). Where
// CallKit is not available (the simulator answers every request with an
// error) the huddle runs without it, with LiveKit handling audio as before.

import AVFoundation
import CallKit
import LiveKit
import UIKit

@MainActor
protocol CallKitActions: AnyObject {
    /// The person ended the call from the system UI (or Leave went through CallKit).
    func callKitEnded() async
    /// Mute changed from the system UI (or the mute button went through CallKit).
    func callKitSetMic(on: Bool) async
}

@MainActor
final class CallKitController: NSObject {
    private let provider: CXProvider
    private let controller = CXCallController()
    weak var actions: CallKitActions?
    /// The call reported to CallKit now, if any.
    private(set) var current: UUID?
    /// False once CallKit turned a request down as unsupported (simulator);
    /// huddles then run without it.
    private(set) var available = true

    override init() {
        let cfg = CXProviderConfiguration()
        cfg.supportsVideo = true
        cfg.maximumCallGroups = 1
        cfg.maximumCallsPerCallGroup = 1
        cfg.supportedHandleTypes = [.generic]
        // huddles are team conversations, not phone calls
        cfg.includesCallsInRecents = false
        if let icon = UIImage(systemName: "headphones")?.pngData() { cfg.iconTemplateImageData = icon }
        provider = CXProvider(configuration: cfg)
        super.init()
        // nil: the delegate is called on the main queue
        provider.setDelegate(self, queue: nil)
        #if targetEnvironment(simulator)
        available = false
        #endif
    }

    var isActive: Bool { current != nil }

    /// Report a huddle about to start. True when CallKit took it (it then
    /// owns the audio session); false to go on without CallKit.
    func start(title: String, channel: UInt64, video: Bool) async -> Bool {
        guard available, current == nil else { return false }
        let id = UUID()
        let handle = CXHandle(type: .generic, value: "huddle-\(channel)")
        let action = CXStartCallAction(call: id, handle: handle)
        action.isVideo = video
        action.contactIdentifier = title
        current = id
        // LiveKit waits for CallKit's audio session
        AudioManager.shared.audioSession.isAutomaticConfigurationEnabled = false
        try? AudioManager.shared.setEngineAvailability(.none)
        do {
            try await controller.request(CXTransaction(action: action))
        } catch {
            // not available here (or turned down): go on without CallKit
            current = nil
            useLiveKitAudio()
            return false
        }
        let update = CXCallUpdate()
        update.remoteHandle = handle
        update.localizedCallerName = title
        update.hasVideo = video
        update.supportsHolding = false
        update.supportsGrouping = false
        update.supportsUngrouping = false
        update.supportsDTMF = false
        provider.reportCall(with: id, updated: update)
        return true
    }

    /// The room is up.
    func reportConnected() {
        guard let id = current else { return }
        provider.reportOutgoingCall(with: id, connectedAt: Date())
    }

    /// The video state changed (the system UI shows a video call).
    func reportVideo(_ on: Bool) {
        guard let id = current else { return }
        let update = CXCallUpdate()
        update.hasVideo = on
        provider.reportCall(with: id, updated: update)
    }

    /// Ask CallKit to end the call (Leave). False when there is no
    /// CallKit call or it refused; then leave directly.
    func requestEnd() async -> Bool {
        guard let id = current else { return false }
        do {
            try await controller.request(CXTransaction(action: CXEndCallAction(call: id)))
            return true
        } catch {
            finish()
            return false
        }
    }

    /// Ask CallKit to mute or unmute (the mute button), so the system UI
    /// shows the same. False when that didn't go through CallKit.
    func requestMic(on: Bool) async -> Bool {
        guard let id = current else { return false }
        do {
            try await controller.request(CXTransaction(action: CXSetMutedCallAction(call: id, muted: !on)))
            return true
        } catch {
            return false
        }
    }

    /// The call ended without the person ending it (the room dropped, the
    /// join failed).
    func reportEnded(failed: Bool) {
        guard let id = current else { return }
        provider.reportCall(with: id, endedAt: Date(), reason: failed ? .failed : .remoteEnded)
        finish()
    }

    private func finish() {
        current = nil
        useLiveKitAudio()
    }

    /// Back to LiveKit handling the audio session itself.
    private func useLiveKitAudio() {
        AudioManager.shared.audioSession.isAutomaticConfigurationEnabled = true
        try? AudioManager.shared.setEngineAvailability(.default)
    }

    /// The session CallKit activates for a huddle.
    nonisolated static func configureSession(video: Bool) {
        let s = AVAudioSession.sharedInstance()
        try? s.setCategory(.playAndRecord, mode: video ? .videoChat : .voiceChat, options: [.allowBluetoothHFP, .allowBluetoothA2DP, .defaultToSpeaker])
    }
}

extension CallKitController: CXProviderDelegate {
    nonisolated func providerDidReset(_ provider: CXProvider) {
        MainActor.assumeIsolated {
            let had = current != nil
            finish()
            if had { Task { await actions?.callKitEnded() } }
        }
    }

    nonisolated func provider(_ provider: CXProvider, perform action: CXStartCallAction) {
        Self.configureSession(video: action.isVideo)
        provider.reportOutgoingCall(with: action.callUUID, startedConnectingAt: Date())
        action.fulfill()
    }

    nonisolated func provider(_ provider: CXProvider, perform action: CXEndCallAction) {
        MainActor.assumeIsolated {
            guard action.callUUID == current else { return action.fail() }
            finish()
            Task {
                await actions?.callKitEnded()
                action.fulfill()
            }
        }
    }

    nonisolated func provider(_ provider: CXProvider, perform action: CXSetMutedCallAction) {
        MainActor.assumeIsolated {
            guard action.callUUID == current else { return action.fail() }
            Task {
                await actions?.callKitSetMic(on: !action.isMuted)
                action.fulfill()
            }
        }
    }

    nonisolated func provider(_ provider: CXProvider, perform action: CXSetHeldCallAction) {
        // a huddle can't be held; another call interrupting ends it in CallKit's eyes
        action.fail()
    }

    nonisolated func provider(_ provider: CXProvider, didActivate audioSession: AVAudioSession) {
        // CallKit has activated the session: LiveKit may start its audio engine
        try? AudioManager.shared.setEngineAvailability(.default)
    }

    nonisolated func provider(_ provider: CXProvider, didDeactivate audioSession: AVAudioSession) {
        MainActor.assumeIsolated {
            // only while a call is still reported (after it ends LiveKit has the session back)
            if current != nil { try? AudioManager.shared.setEngineAvailability(.none) }
        }
    }
}
