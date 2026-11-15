// Look of the app: black and white like the Mac app, one accent (#8B7BFF).

import SwiftUI
import WorkboardKit

extension Color {
    static let wbAccent = Color(red: 0x8B / 255, green: 0x7B / 255, blue: 0xFF / 255)

    init(rgb: [Int]) {
        guard rgb.count == 3 else { self = .gray; return }
        self = Color(red: Double(rgb[0]) / 255, green: Double(rgb[1]) / 255, blue: Double(rgb[2]) / 255)
    }
}

struct Avatar: View {
    let member: MemberView?
    var size: CGFloat = 32

    var body: some View {
        Circle()
            .fill(member.map { Color(rgb: $0.color) } ?? .gray)
            .frame(width: size, height: size)
            .overlay(
                Text(member?.initials ?? "?")
                    .font(.system(size: size * 0.38, weight: .semibold))
                    .foregroundStyle(.white)
            )
            .accessibilityHidden(true)
    }
}

struct StatusDot: View {
    let status: StatusView
    var size: CGFloat = 10

    var body: some View {
        Group {
            if status.isClosed {
                Image(systemName: "checkmark.circle.fill").resizable()
            } else {
                Circle().strokeBorder(lineWidth: size * 0.22).background(Circle().fill(Color(rgb: status.color).opacity(status.kind == .open ? 0 : 0.35)))
            }
        }
        .foregroundStyle(Color(rgb: status.color))
        .frame(width: size, height: size)
    }
}

/// An empty-state message in the middle of a screen.
struct EmptyNote: View {
    let title: String
    let systemImage: String
    var detail: String?

    var body: some View {
        ContentUnavailableView {
            Label(title, systemImage: systemImage)
        } description: {
            if let detail { Text(detail) }
        }
    }
}

enum Fmt {
    static func time(_ ts: Int64) -> String {
        let d = Date(timeIntervalSince1970: TimeInterval(ts))
        if Calendar.current.isDateInToday(d) { return d.formatted(date: .omitted, time: .shortened) }
        if Calendar.current.isDateInYesterday(d) { return "Yesterday " + d.formatted(date: .omitted, time: .shortened) }
        return d.formatted(.dateTime.month(.abbreviated).day().hour().minute())
    }

    static func relative(_ ts: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(ts)).formatted(.relative(presentation: .named))
    }

    /// A task's due date with its time when it has one: "Today 2:30 PM".
    static func due(_ task: TaskView) -> String? {
        guard let d = task.due else { return nil }
        return [due(d), task.dueTimeLabel].compactMap { $0 }.joined(separator: " ")
    }

    /// "YYYY-MM-DD" -> "Today", "Tomorrow", "Mar 3".
    static func due(_ day: String) -> String {
        let today = dayString()
        if day == today { return "Today" }
        if day == dayString(Date().addingTimeInterval(86400)) { return "Tomorrow" }
        if day == dayString(Date().addingTimeInterval(-86400)) { return "Yesterday" }
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd"
        f.locale = Locale(identifier: "en_US_POSIX")
        guard let d = f.date(from: day) else { return day }
        return d.formatted(.dateTime.month(.abbreviated).day())
    }
}

/// The app icon's mark ("W into a check") on its dark tile, for the setup screen.
struct WMark: View {
    var body: some View {
        GeometryReader { g in
            let s = min(g.size.width, g.size.height)
            let k = s / 824
            let p = { (x: CGFloat, y: CGFloat) in CGPoint(x: (x - 100) * k, y: (y - 100) * k) }
            ZStack(alignment: .topLeading) {
                RoundedRectangle(cornerRadius: s * 0.225, style: .continuous)
                    .fill(LinearGradient(colors: [Color(rgb: [0x26, 0x25, 0x2B]), Color(rgb: [0x0C, 0x0C, 0x0E])], startPoint: .top, endPoint: .bottom))
                Path { path in
                    path.addLines([p(250, 450), p(352, 680), p(470, 486), p(588, 680), p(790, 330)])
                }
                .stroke(Color(rgb: [0xF4, 0xF3, 0xF0]), style: StrokeStyle(lineWidth: 70 * k, lineCap: .round, lineJoin: .round))
                Circle().fill(Color.wbAccent)
                    .frame(width: 70 * k, height: 70 * k)
                    .position(p(790, 330))
            }
            .frame(width: s, height: s)
        }
        .accessibilityHidden(true)
    }
}

/// Content types for sign-in fields, so iOS offers saved and strong
/// passwords. UI tests launch with `-wbNoPasswordAutoFill YES`: iOS's "Use
/// Strong Password" sheet then never takes the keyboard over halfway through
/// typing (it comes up at its own pace, which made those tests flaky).
enum PasswordKind { case current, new }

private let passwordAutoFillOff = UserDefaults.standard.bool(forKey: "wbNoPasswordAutoFill")

extension View {
    func passwordField(_ kind: PasswordKind) -> some View {
        textContentType(passwordAutoFillOff ? .oneTimeCode : kind == .new ? .newPassword : .password)
    }

    func usernameField() -> some View {
        textContentType(passwordAutoFillOff ? .emailAddress : .username)
    }
}
