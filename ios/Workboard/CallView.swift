import AVKit
import LiveKit
import SwiftUI
import WorkboardKit

/// The huddle screen: who is in it, who is talking and who has a hand up,
/// reactions over the tiles, and the buttons: hand, reactions, the other
/// camera, where the sound plays, mute, video, leave.
struct CallView: View {
    @Environment(SyncClient.self) private var client
    @Environment(CallManager.self) private var calls
    @State private var picking = false

    var body: some View {
        let title = calls.channel.flatMap { id in client.view.channels.first { $0.id == id } }.map { client.view.title(of: $0, me: client.me ?? 0) } ?? "Huddle"
        VStack(spacing: 0) {
            VStack(spacing: 4) {
                Text("Huddle").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                Text(title).font(.title3.bold())
            }
            .padding(.top, 24)
            .padding(.bottom, 12)

            switch calls.phase {
            case .idle:
                Spacer()
            case .joining:
                Spacer()
                ProgressView("Joining…").accessibilityIdentifier("callJoining")
                Spacer()
            case .failed(let why):
                Spacer()
                ContentUnavailableView {
                    Label("Couldn't join", systemImage: "phone.down")
                } description: {
                    Text(why)
                } actions: {
                    Button("Close") { calls.dismissFailure() }
                        .buttonStyle(.borderedProminent)
                        .accessibilityIdentifier("callClose")
                }
                .accessibilityIdentifier("callFailed")
                Spacer()
            case .inCall:
                ParticipantsGrid(room: calls.room, signals: calls.signals, handUp: calls.handUp)
            }

            if calls.phase == .inCall && picking {
                HStack(spacing: 4) {
                    ForEach(HuddleSignal.reactions, id: \.self) { e in
                        Button {
                            picking = false
                            Task { await calls.react(e) }
                        } label: {
                            Text(e).font(.system(size: 28)).frame(width: 40, height: 44)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("React \(e)")
                    }
                }
                .padding(.horizontal, 8)
                .background(Capsule().fill(Color(.secondarySystemBackground)))
                .padding(.top, 8)
                .transition(.scale(scale: 0.9).combined(with: .opacity))
            }
            if calls.phase == .inCall {
                HStack(spacing: 20) {
                    SmallCallButton(icon: "hand.raised.fill", label: calls.handUp ? "Lower hand" : "Raise hand", on: calls.handUp) {
                        Task { await calls.toggleHand() }
                    }
                    .accessibilityIdentifier("callHand")
                    SmallCallButton(icon: "face.smiling", label: "Reactions", on: picking) {
                        withAnimation(.easeOut(duration: 0.15)) { picking.toggle() }
                    }
                    .accessibilityIdentifier("callReactions")
                    if calls.cameraOn {
                        SmallCallButton(icon: "arrow.triangle.2.circlepath.camera", label: "Switch camera", on: false) {
                            Task { await calls.flipCamera() }
                        }
                    }
                    // where the sound plays: the phone, a speaker, headphones (the system's own picker)
                    AudioRoutePicker()
                        .frame(width: 48, height: 48)
                        .background(Circle().fill(Color(.secondarySystemFill)))
                        .accessibilityLabel("Audio output")
                }
                .padding(.top, 16)
            }
            if calls.phase == .inCall || calls.phase == .joining {
                HStack(spacing: 28) {
                    CallButton(icon: calls.micOn ? "mic.fill" : "mic.slash.fill", label: calls.micOn ? "Mute" : "Unmute", on: !calls.micOn) {
                        Task { await calls.toggleMic() }
                    }
                    CallButton(icon: calls.cameraOn ? "video.fill" : "video.slash.fill", label: calls.cameraOn ? "Stop video" : "Video", on: calls.cameraOn) {
                        Task { await calls.toggleCamera() }
                    }
                    CallButton(icon: "phone.down.fill", label: "Leave", on: false, tint: .red) {
                        Task { await calls.leave() }
                    }
                    .accessibilityIdentifier("callLeave")
                }
                .padding(.vertical, 24)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(.systemBackground))
        .overlay(alignment: .topTrailing) {
            Button {
                calls.showCall = false
            } label: {
                Image(systemName: "chevron.down").font(.headline).padding(12)
            }
            .accessibilityLabel("Minimize")
            .padding(8)
        }
    }
}

private struct ParticipantsGrid: View {
    @Environment(CallManager.self) private var calls
    @ObservedObject var room: Room
    let signals: HuddleSignals
    let handUp: Bool

    var body: some View {
        let people: [Participant] = [room.localParticipant] + room.remoteParticipants.values.sorted { ($0.name ?? "") < ($1.name ?? "") }
        ScrollView {
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
                ForEach(people, id: \.self) { p in
                    let isMe = p === room.localParticipant
                    let key = isMe ? HuddleSignals.me : (p.identity?.stringValue ?? "")
                    ParticipantTile(participant: p, isMe: isMe, hand: isMe ? handUp : signals.handUp(key), reactions: signals.reactions(of: key))
                }
            }
            .padding(16)
        }
        // reactions that finished showing go
        .task(id: signals.reactions.count) {
            guard !signals.reactions.isEmpty else { return }
            try? await Task.sleep(for: .seconds(HuddleSignals.reactionSeconds))
            calls.pruneReactions()
        }
    }
}

private struct ParticipantTile: View {
    @Environment(SyncClient.self) private var client
    @ObservedObject var participant: Participant
    let isMe: Bool
    let hand: Bool
    let reactions: [HuddleSignals.Reaction]

    var body: some View {
        let member = memberId.flatMap { client.view.member($0) }
        ZStack(alignment: .bottomLeading) {
            RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Color(.secondarySystemBackground))
            if let track = participant.firstCameraVideoTrack, participant.isCameraEnabled() {
                SwiftUIVideoView(track, layoutMode: .fill)
                    .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
            } else {
                Avatar(member: member, size: 64).frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            HStack(spacing: 4) {
                if !participant.isMicrophoneEnabled() { Image(systemName: "mic.slash.fill").font(.caption2) }
                Text(isMe ? "You" : (member?.name ?? participant.name ?? "Guest")).font(.caption.weight(.medium)).lineLimit(1)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(.ultraThinMaterial, in: Capsule())
            .padding(8)
        }
        .overlay(alignment: .topLeading) {
            if hand {
                Text("\u{270B}")
                    .font(.system(size: 20))
                    .frame(width: 34, height: 34)
                    .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(.white))
                    .padding(8)
                    .accessibilityLabel("Hand raised")
            }
        }
        .overlay {
            // each reaction rises from the bottom and fades
            GeometryReader { geo in
                TimelineView(.animation(paused: reactions.isEmpty)) { tl in
                    ForEach(Array(reactions.enumerated()), id: \.element.id) { n, r in
                        let t = min(1, tl.date.timeIntervalSince(r.at) / HuddleSignals.reactionSeconds)
                        Text(r.emoji)
                            .font(.system(size: 36))
                            .position(
                                x: geo.size.width / 2 + [0, -28, 28][n % 3],
                                y: geo.size.height * (0.8 - 0.6 * t)
                            )
                            .opacity(t > 0.7 ? (1 - t) / 0.3 : 1)
                    }
                }
            }
            .allowsHitTesting(false)
        }
        .aspectRatio(3 / 4, contentMode: .fit)
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(Color.wbAccent, lineWidth: participant.isSpeaking ? 3 : 0)
        )
        .animation(.easeOut(duration: 0.15), value: participant.isSpeaking)
    }

    /// The server names people "<member>-<device>" (call_token in server/src/lib.rs).
    var memberId: UInt64? {
        participant.identity.flatMap { CallAPI.member(inIdentity: $0.stringValue) }
    }
}

private struct CallButton: View {
    let icon: String
    let label: String
    let on: Bool
    var tint: Color = .primary
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.system(size: 22, weight: .semibold))
                    .frame(width: 60, height: 60)
                    .foregroundStyle(tint == .red ? .white : (on ? Color(.systemBackground) : .primary))
                    .background(Circle().fill(tint == .red ? Color.red : (on ? Color.primary : Color(.secondarySystemFill))))
                Text(label).font(.caption).foregroundStyle(.secondary)
            }
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }
}

/// A smaller round button (hand, reactions, the other camera).
private struct SmallCallButton: View {
    let icon: String
    let label: String
    let on: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: 18, weight: .semibold))
                .frame(width: 48, height: 48)
                .foregroundStyle(on ? Color(.systemBackground) : .primary)
                .background(Circle().fill(on ? Color.primary : Color(.secondarySystemFill)))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }
}

/// The system's picker of where the call's sound plays.
private struct AudioRoutePicker: UIViewRepresentable {
    func makeUIView(context: Context) -> AVRoutePickerView {
        let v = AVRoutePickerView()
        v.prioritizesVideoDevices = false
        v.tintColor = .label
        v.activeTintColor = .systemBlue
        return v
    }

    func updateUIView(_ v: AVRoutePickerView, context: Context) {}
}

/// A slim bar over the tabs while a huddle is going and its screen is hidden.
struct OngoingCallBar: View {
    @Environment(CallManager.self) private var calls

    var body: some View {
        if calls.phase == .inCall && !calls.showCall {
            Button { calls.showCall = true } label: {
                HStack(spacing: 8) {
                    Image(systemName: "waveform")
                    Text("Huddle in progress · tap to return").lineLimit(1)
                    Spacer()
                }
                .font(.footnote.weight(.semibold))
                .foregroundStyle(.white)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(Color.wbAccent)
            }
            .buttonStyle(.plain)
        }
    }
}
