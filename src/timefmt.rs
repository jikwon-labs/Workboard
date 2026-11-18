// Between Unix timestamps (what tasks, messages and reminders store) and
// this computer's local dates and times (what people see and type).

use crate::model::{now_ts, parse_time_of_day, today};
use chrono::{DateTime, Local, NaiveDate, NaiveTime};

/// A timestamp in local time. None only when it's out of range.
pub fn local(ts: i64) -> Option<DateTime<Local>> {
    DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&Local))
}

/// The local day a timestamp falls on (today when it's out of range).
pub fn local_date(ts: i64) -> NaiveDate {
    local(ts).map(|d| d.date_naive()).unwrap_or_else(today)
}

/// The timestamp of a local date and time. When the clocks go back and the
/// time happens twice, the first one; None when the clocks skip over it.
pub fn local_ts(d: NaiveDate, t: NaiveTime) -> Option<i64> {
    d.and_time(t)
        .and_local_timezone(Local)
        .earliest()
        .map(|x| x.timestamp())
}

/// 9:00, when reminders and typed hours land without a time of their own.
pub fn nine_am() -> NaiveTime {
    NaiveTime::from_hms_opt(9, 0, 0).unwrap_or_default()
}

/// A local date and a typed time ("14:30", "2pm", ...) as a timestamp:
/// 9:00 when the time isn't one, now when the day has no such time.
pub fn reminder_ts(d: NaiveDate, time: &str) -> i64 {
    let t = parse_time_of_day(time).unwrap_or_else(nine_am);
    local_ts(d, t).unwrap_or_else(now_ts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn a_timestamp_and_its_local_day_and_time_go_both_ways() {
        let d = NaiveDate::from_ymd_opt(2026, 3, 14).unwrap();
        let t = NaiveTime::from_hms_opt(15, 9, 0).unwrap();
        let ts = local_ts(d, t).unwrap();
        assert_eq!(local_date(ts), d);
        let back = local(ts).unwrap();
        assert_eq!((back.hour(), back.minute()), (15, 9));
        // out of range: today, not a panic
        assert_eq!(local(i64::MAX), None);
        assert_eq!(local_date(i64::MAX), today());
    }

    #[test]
    fn a_reminder_time_is_read_the_way_people_type_it() {
        let d = NaiveDate::from_ymd_opt(2026, 3, 14).unwrap();
        let at = |h, m| local_ts(d, NaiveTime::from_hms_opt(h, m, 0).unwrap()).unwrap();
        assert_eq!(reminder_ts(d, "14:30"), at(14, 30));
        assert_eq!(reminder_ts(d, " 2:30 pm "), at(14, 30));
        assert_eq!(reminder_ts(d, "7"), at(7, 0));
        // not a time: 9:00 that day
        assert_eq!(reminder_ts(d, ""), at(9, 0));
        assert_eq!(reminder_ts(d, "soon"), at(9, 0));
        assert_eq!(reminder_ts(d, "25:00"), at(9, 0));
    }
}
