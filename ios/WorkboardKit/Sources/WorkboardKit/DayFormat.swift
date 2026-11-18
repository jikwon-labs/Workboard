// Dates as the workspace stores them: days as "YYYY-MM-DD" and times of day
// as "HH:mm", in the device's time zone (the Mac app's dates are local).
// The formatters are made once: DateFormatter is slow to build and safe to
// share as long as nobody changes it.

import Foundation

public enum DayFormat {
    /// A fixed-format formatter: en_US_POSIX, Gregorian, device time zone.
    static func posix(_ format: String) -> DateFormatter {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.calendar = Calendar(identifier: .gregorian)
        f.dateFormat = format
        return f
    }

    static let dayFormatter = posix("yyyy-MM-dd")
    static let timeFormatter = posix("HH:mm")
    static let dayTimeFormatter = posix("yyyy-MM-dd HH:mm")

    /// The time zone the stored days are read in.
    static var timeZone: TimeZone { dayFormatter.timeZone }

    /// "YYYY-MM-DD" for this moment.
    public static func day(_ date: Date = Date()) -> String { dayFormatter.string(from: date) }

    /// "HH:mm" for this moment.
    public static func time(_ date: Date = Date()) -> String { timeFormatter.string(from: date) }

    /// The start of a "YYYY-MM-DD" day, or nil when it isn't one.
    public static func date(fromDay s: String) -> Date? { dayFormatter.date(from: s) }

    /// An "HH:mm" time on the formatter's reference day, or nil.
    public static func date(fromTime s: String) -> Date? { timeFormatter.date(from: s) }

    /// A day and a time of day together ("2026-11-18", "14:30"), as meetings keep them.
    public static func date(day: String, time: String) -> Date? { dayTimeFormatter.date(from: "\(day) \(time)") }
}

/// "YYYY-MM-DD" in the device's time zone, as the Mac app's dates are local.
public func dayString(_ date: Date = Date()) -> String { DayFormat.day(date) }

/// The time of day as "HH:mm" (what `TaskView.isOverdue` compares).
public func timeString(_ date: Date = Date()) -> String { DayFormat.time(date) }
