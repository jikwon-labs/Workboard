import LiveKit
import SwiftUI
import WorkboardKit

/// The huddle screen: who is in it, who is talking, mute, video, leave.
struct CallView: View {
    @Environment(SyncClient.self) private var client
    @Environment(CallManager.self) private var calls

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
                ParticipantsGrid(room: calls.room)
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
    @ObservedObject var room: Room

    var body: some View {
        let people: [Participant] = [room.localParticipant] + room.remoteParticipants.values.sorted { ($0.name ?? "") < ($1.name ?? "") }
        ScrollView {
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
                ForEach(people, id: \.self) { p in ParticipantTile(participant: p, isMe: p === room.localParticipant) }
            }
            .padding(16)
        }
    }
}

private struct ParticipantTile: View {
    @Environment(SyncClient.self) private var client
    @ObservedObject var participant: Participant
    let isMe: Bool

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
        .aspectRatio(3 / 4, contentMode: .fit)
        .overlay(
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .strokeBorder(Color.wbAccent, lineWidth: participant.isSpeaking ? 3 : 0)
        )
        .animation(.easeOut(duration: 0.15), value: participant.isSpeaking)
    }

    /// The server names people "member-<id>" (see the fake server and docs/ios-design.md).
    var memberId: UInt64? {
        guard let s = participant.identity?.stringValue, s.hasPrefix("member-") else { return nil }
        return UInt64(s.dropFirst(7))
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
