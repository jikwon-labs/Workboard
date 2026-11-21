// Dates, times and durations as text, their colours, and repeating dates.

use super::*;

pub fn fmt_date(d: NaiveDate) -> String {
    let t = today();
    let diff = (d - t).num_days();
    match diff {
        0 => tr("Today").into(),
        1 => tr("Tomorrow").into(),
        -1 => tr("Yesterday").into(),
        2..=6 => d.text("%a"),
        _ => {
            if d.year() == t.year() {
                d.text("%b %-d")
            } else {
                d.text("%-m/%-d/%y")
            }
        }
    }
}

/// A time of day in the date style ("2:30 PM").
pub fn fmt_time(time: NaiveTime) -> String {
    time.text("%-I:%M %p")
}

/// A date with its time of day when there is one ("Today 2:30 PM").
pub fn fmt_when(d: NaiveDate, time: Option<NaiveTime>) -> String {
    match time {
        Some(time) => format!("{} {}", fmt_date(d), fmt_time(time)),
        None => fmt_date(d),
    }
}

/// A task's due date (and time) for display.
pub fn fmt_due(task: &Task) -> Option<String> {
    task.due.map(|d| fmt_when(d, task.due_at()))
}

/// A task's start date (and time) for display.
pub fn fmt_start(task: &Task) -> Option<String> {
    task.start.map(|d| fmt_when(d, task.start_at()))
}

/// The colour of a task's due date: red once it is overdue (with a due time,
/// from that time on), orange later today.
pub fn due_color(task: &Task, closed: bool, t: &Theme) -> Color32 {
    match task.due {
        Some(_) if !closed && task.is_overdue() => t.danger,
        Some(d) => date_color(d, closed, t),
        None => t.text3,
    }
}

/// A week shown as a range: "Nov 15 – Nov 21, 2026" in English, the year
/// first in Korean ("2026년 11월 15일 – 11월 21일"; it used to land between
/// the two dates).
pub fn week_range(start: NaiveDate) -> String {
    let end = start + Duration::days(6);
    crate::trf!(
        "{} \u{2013} {}, {}",
        start.text("%b %-d"),
        end.text("%b %-d"),
        end.year()
    )
}

pub fn date_color(d: NaiveDate, closed: bool, t: &Theme) -> Color32 {
    if closed {
        return t.text3;
    }
    let diff = (d - today()).num_days();
    if diff < 0 {
        t.danger
    } else if diff == 0 {
        WARN_ORANGE
    } else {
        t.text2
    }
}

pub fn fmt_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        crate::trf!("{}h {}m", h, m)
    } else if m > 0 {
        crate::trf!("{}m {}s", m, s)
    } else {
        crate::trf!("{}s", s)
    }
}

/// Tracked time in hours and minutes ("1h 20m", "40m", "2h"), the one way
/// timesheets show it, in their cells and their totals; "-" for none.
pub fn fmt_hm(secs: i64) -> String {
    let min = (secs as f64 / 60.0).round() as i64;
    match (min / 60, min % 60) {
        (0, 0) => "-".into(),
        (0, m) => crate::trf!("{}m", m),
        (h, 0) => crate::trf!("{}h", h),
        (h, m) => crate::trf!("{}h {}m", h, m),
    }
}

pub fn fmt_ago(ts: i64) -> String {
    let d = now_ts() - ts;
    if d < 60 {
        tr("just now").into()
    } else if d < 120 {
        tr("1 min ago").into()
    } else if d < 3600 {
        crate::trf!("{} mins ago", d / 60)
    } else if d < 86400 {
        let h = d / 3600;
        if h == 1 {
            tr("1 hour ago").into()
        } else {
            crate::trf!("{} hours ago", h)
        }
    } else {
        let dt = crate::timefmt::local(ts)
            .map(|x| x.text("%b %-d at %-I:%M %p"))
            .unwrap_or_default();
        dt
    }
}

/// Repeat choices for recurring tasks (stored value, label).
pub const RECUR: [(&str, &str); 5] = [
    ("", "Never"),
    ("daily", "Daily"),
    ("weekly", "Weekly"),
    ("monthly", "Monthly"),
    ("yearly", "Yearly"),
];

pub fn recur_label(r: &Option<String>) -> &'static str {
    RECUR
        .iter()
        .find(|x| Some(x.0) == r.as_deref())
        .map(|x| x.1)
        .unwrap_or("Never")
}

/// The next date of a repeating task.
pub fn next_recur(d: NaiveDate, r: &str) -> NaiveDate {
    match r {
        "daily" => d + Duration::days(1),
        "weekly" => d + Duration::days(7),
        "monthly" => d
            .checked_add_months(chrono::Months::new(1))
            .unwrap_or(d + Duration::days(30)),
        "yearly" => d
            .checked_add_months(chrono::Months::new(12))
            .unwrap_or(d + Duration::days(365)),
        _ => d,
    }
}

/// `s` in at most `n` characters: whole when it fits, else its first n - 1
/// and an ellipsis.
pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut o: String = s.chars().take(n.saturating_sub(1)).collect();
        o.push('\u{2026}');
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_to_the_length_with_its_ellipsis() {
        assert_eq!(truncate("Workboard", 9), "Workboard");
        assert_eq!(truncate("Workboard", 5), "Work\u{2026}");
        assert_eq!(truncate("작업 보드", 3), "작업\u{2026}");
        assert_eq!(truncate("", 0), "");
    }

    #[test]
    fn how_long_ago_reads_right_for_one_minute_and_one_hour() {
        let ago = |s: i64| fmt_ago(now_ts() - s);
        assert_eq!(ago(30), "just now");
        assert_eq!(ago(90), "1 min ago");
        assert_eq!(ago(150), "2 mins ago");
        assert_eq!(ago(3700), "1 hour ago");
        assert_eq!(ago(3 * 3600), "3 hours ago");
    }
}
