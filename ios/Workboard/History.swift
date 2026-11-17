import Foundation

// Task history lines are written in English (everyone's devices share them);
// a phone in Korean shows them in Korean, as the Mac does (history_line and
// particles in src/i18n.rs).

enum History {
    static var korean: Bool { Locale.preferredLanguages.first?.hasPrefix("ko") ?? false }

    /// (English pattern, Korean): {} values in order, {0} {1} in Korean.
    static let lines: [(String, String)] = [
        ("changed status from {} to {}", "상태를 {0}에서 {1}(으)로 변경했습니다"),
        ("set priority to {}", "우선순위를 {0}(으)로 설정했습니다"),
        ("changed due date to {}", "마감일을 {0}(으)로 변경했습니다"),
        ("changed start date to {}", "시작일을 {0}(으)로 변경했습니다"),
        ("assigned to {}", "{0}에게 할당했습니다"),
        ("removed assignee {}", "담당자 {0}을(를) 제거했습니다"),
        ("created this task", "이 작업을 만들었습니다"),
        ("moved this task to {}", "이 작업을 {0}(으)로 옮겼습니다"),
        ("duplicated this task from {}", "{0}에서 이 작업을 복제했습니다"),
        ("archived this task", "이 작업을 보관했습니다"),
        ("unarchived this task", "이 작업의 보관을 해제했습니다"),
        ("completed a {} repeat; next due {}", "{0} 반복을 완료했습니다. 다음 마감일: {1}"),
        ("renamed this task to \"{}\"", "작업 이름을 바꿨습니다: \"{0}\""),
        ("updated the description", "설명을 수정했습니다"),
        ("set the time estimate to {}", "예상 시간을 {0}(으)로 설정했습니다"),
        ("removed the time estimate", "예상 시간을 제거했습니다"),
        ("added tag {}", "태그 {0}을(를) 추가했습니다"),
        ("removed tag {}", "태그 {0}을(를) 제거했습니다"),
        ("made this a subtask of {}", "이 작업을 {0}의 하위 작업으로 만들었습니다"),
        ("made this a top-level task", "이 작업을 최상위 작업으로 만들었습니다"),
        ("is waiting on {}", "이 작업이 {0}을(를) 기다리도록 설정했습니다"),
        ("linked {}", "{0}을(를) 연결했습니다"),
        ("attached {}", "{0}을(를) 첨부했습니다"),
        ("submitted the form {}", "{0} 폼을 제출했습니다"),
    ]

    /// "<who> <text>", or in Korean "<who>님이 <Korean text>".
    static func line(who: String, text: String) -> String {
        guard korean else { return "\(who) \(text)" }
        for (en, ko) in lines {
            if let vals = match(en, text) {
                return "\(who)님이 " + particles(fill(ko, vals.map(value)))
            }
        }
        return "\(who) \(text)"
    }

    /// The {} values of `text` when it has the shape of `pattern`.
    static func match(_ pattern: String, _ text: String) -> [String]? {
        let parts = pattern.components(separatedBy: "{}")
        guard text.hasPrefix(parts[0]) else { return nil }
        var rest = Substring(text.dropFirst(parts[0].count))
        var vals: [String] = []
        for (i, sep) in parts.enumerated().dropFirst() {
            if i == parts.count - 1 {
                if sep.isEmpty {
                    vals.append(String(rest))
                } else {
                    guard rest.hasSuffix(sep) else { return nil }
                    vals.append(String(rest.dropLast(sep.count)))
                }
                rest = ""
            } else {
                guard let r = rest.range(of: sep) else { return nil }
                vals.append(String(rest[..<r.lowerBound]))
                rest = rest[r.upperBound...]
            }
        }
        return rest.isEmpty ? vals : nil
    }

    static func fill(_ template: String, _ vals: [String]) -> String {
        var s = template
        for (i, v) in vals.enumerated() { s = s.replacingOccurrences(of: "{\(i)}", with: v) }
        return s
    }

    /// Default statuses, priorities, repeats and the day words the Mac
    /// writes ("Today", "Tomorrow at 2:30 PM") in Korean.
    static func value(_ v: String) -> String {
        if let r = v.range(of: " at ", options: .backwards) {
            return "\(value(String(v[..<r.lowerBound]))) \(v[r.upperBound...])"
        }
        let low = v.lowercased()
        if ["to do", "in progress", "complete"].contains(low) { return String(localized: String.LocalizationValue(low)) }
        let cap = low.prefix(1).uppercased() + low.dropFirst()
        if ["Daily", "Weekly", "Monthly", "Yearly", "None", "Today", "Tomorrow", "Yesterday"].contains(cap) || Edits_priority.contains(v) {
            return String(localized: String.LocalizationValue(["Daily", "Weekly", "Monthly", "Yearly", "None", "Today", "Tomorrow", "Yesterday"].contains(cap) ? cap : v))
        }
        return v
    }

    static let Edits_priority = ["No Priority", "Urgent", "High", "Normal", "Low"]

    /// Picks the particle after a value by its last sound: "(으)로" becomes
    /// "로" or "으로", "을(를)" becomes "을" or "를".
    static func particles(_ s: String) -> String {
        func coda(_ c: Character, ro: Bool) -> Bool? {
            if let u = c.unicodeScalars.first?.value, (0xAC00...0xD7A3).contains(u) {
                let f = (u - 0xAC00) % 28
                return ro ? (f != 0 && f != 8) : f != 0
            }
            switch c.lowercased() {
            case "a", "i", "o", "u", "y": return false
            case "m", "n": return true
            case "l": return !ro
            default: break
            }
            guard let d = c.wholeNumberValue else { return nil }
            switch d {
            case 0, 3, 6: return true
            case 1, 7, 8: return !ro
            default: return false
            }
        }
        var out = ""
        var rest = Substring(s)
        while true {
            let ro = rest.range(of: "(으)로"), eul = rest.range(of: "을(를)")
            let pick: (Range<Substring.Index>, Bool)
            switch (ro, eul) {
            case let (a?, b?): pick = b.lowerBound < a.lowerBound ? (b, false) : (a, true)
            case let (a?, nil): pick = (a, true)
            case let (nil, b?): pick = (b, false)
            default:
                out += rest
                return out
            }
            out += rest[..<pick.0.lowerBound]
            switch out.last.flatMap({ coda($0, ro: pick.1) }) {
            case true?: out += pick.1 ? "으로" : "을"
            case false?: out += pick.1 ? "로" : "를"
            case nil: out += pick.1 ? "(으)로" : "을(를)"
            }
            rest = rest[pick.0.upperBound...]
        }
    }
}
