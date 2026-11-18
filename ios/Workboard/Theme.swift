// Look of the app: black and white like the Mac app, one accent (#8B7BFF).

import SwiftUI
import WorkboardKit

extension Color {
    /// The accent chosen in Settings > Theme (Workboard's purple at first).
    static var wbAccent: Color { Appearance.color(UserDefaults.standard.string(forKey: "accent") ?? "workboard") }

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
    let title: LocalizedStringKey
    let systemImage: String
    var detail: LocalizedStringKey?

    var body: some View {
        ContentUnavailableView {
            Label(title, systemImage: systemImage)
        } description: {
            if let detail { Text(detail) }
        }
    }
}

/// Words kept in a variable (a menu's labels, a card's title) shown in the
/// phone's language, from Localizable.xcstrings.
func L(_ s: String) -> LocalizedStringKey { LocalizedStringKey(s) }

enum Fmt {
    static func time(_ ts: Int64) -> String {
        let d = Date(timeIntervalSince1970: TimeInterval(ts))
        if Calendar.current.isDateInToday(d) { return d.formatted(date: .omitted, time: .shortened) }
        if Calendar.current.isDateInYesterday(d) { return String(localized: "Yesterday") + " " + d.formatted(date: .omitted, time: .shortened) }
        return d.formatted(.dateTime.month(.abbreviated).day().hour().minute())
    }

    /// "14:30" as the phone shows times ("2:30 PM", "오후 2:30").
    static func clock(_ hhmm: String) -> String {
        guard let d = DayFormat.date(fromTime: hhmm) else { return hhmm }
        return d.formatted(date: .omitted, time: .shortened)
    }

    static func relative(_ ts: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(ts)).formatted(.relative(presentation: .named))
    }

    /// A task's due date with its time when it has one: "Today 2:30 PM".
    static func due(_ task: TaskView) -> String? {
        guard let d = task.due else { return nil }
        return [due(d), task.dueTime.map(clock)].compactMap { $0 }.joined(separator: " ")
    }

    /// "YYYY-MM-DD" -> "Today", "Tomorrow", "Mar 3".
    static func due(_ day: String) -> String {
        let today = dayString()
        if day == today { return String(localized: "Today") }
        if day == dayString(Date().addingTimeInterval(86400)) { return String(localized: "Tomorrow") }
        if day == dayString(Date().addingTimeInterval(-86400)) { return String(localized: "Yesterday") }
        guard let d = DayFormat.date(fromDay: day) else { return day }
        return d.formatted(.dateTime.month(.abbreviated).day())
    }
}

extension Array {
    /// The element at `i`, or nil when there is none.
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
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

/// What a password may hold: English letters, digits, symbols and spaces.
/// Typed in Korean mode the same keys give other letters, and the password
/// then won't match the one typed in English on another computer.
func isPasswordText(_ s: String) -> Bool {
    s.unicodeScalars.allSatisfy { (0x20...0x7E).contains($0.value) }
}

/// A password box: dots by default, an eye button shows what was typed, and
/// only English letters, digits and symbols go in (anything else is left out
/// and a note asks for English input). The field keeps `id` as its
/// accessibility identifier either way; the eye is `id + "Eye"`.
struct PasswordField: View {
    let title: LocalizedStringKey
    @Binding var text: String
    let kind: PasswordKind
    let id: String
    @State private var shown = false
    @State private var note = false
    @FocusState private var focused: Bool

    init(_ title: LocalizedStringKey, text: Binding<String>, kind: PasswordKind, id: String) {
        self.title = title
        _text = text
        self.kind = kind
        self.id = id
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Group {
                    if shown {
                        TextField(title, text: $text)
                    } else {
                        SecureField(title, text: $text)
                    }
                }
                .passwordField(kind)
                .keyboardType(.asciiCapable)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .focused($focused)
                .accessibilityIdentifier(id)
                Button {
                    shown.toggle()
                    focused = true
                } label: {
                    Image(systemName: shown ? "eye.slash" : "eye")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.borderless)
                .accessibilityLabel(shown ? "Hide password" : "Show password")
                .accessibilityIdentifier(id + "Eye")
            }
            if note {
                Text("Passwords take English letters, numbers and symbols. Switch to English input.")
                    .font(.footnote)
                    .foregroundStyle(.red)
            }
        }
        .onChange(of: text) { _, new in
            if isPasswordText(new) {
                if !new.isEmpty { note = false }
            } else {
                note = true
                text = String(String.UnicodeScalarView(new.unicodeScalars.filter { (0x20...0x7E).contains($0.value) }))
            }
        }
    }
}
