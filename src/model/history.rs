// A task's history lines: what an edit changed, written in English (the
// history is shared with teammates), and the dates a status change stamps.

use super::*;

impl Task {
    /// Adds a history line by `me`.
    pub fn log(&mut self, me: u64, text: String) {
        self.activity.push(Activity {
            ts: now_ts(),
            author: me,
            text,
            comment: false,
            ..Default::default()
        });
    }

    /// Logs a change, or updates my own line about the same change if I made
    /// it in the last few minutes (`like` is part of that line).
    pub fn log_edit(&mut self, me: u64, like: &str, text: String) {
        let now = now_ts();
        if let Some(last) = self.activity.last_mut() {
            if !last.comment && last.author == me && now - last.ts < 300 && last.text.contains(like)
            {
                last.text = text;
                last.ts = now;
                return;
            }
        }
        self.log(me, text);
    }
}

impl Data {
    /// A history line about one person ("assigned to <name>", "removed
    /// assignee <name>") that keeps their id, so their Inbox finds it by id.
    pub fn about_line(&self, me: u64, what: &str, who: u64) -> Activity {
        let name = self
            .member(who)
            .map(|m| m.shown().to_string())
            .unwrap_or_default();
        Activity {
            ts: now_ts(),
            author: me,
            text: format!("{} {}", what, name),
            comment: false,
            target: Some(who),
            ..Default::default()
        }
    }

    /// Writes into `new`'s history what changed since `old` (status,
    /// priority, dates, assignees, name, description, estimate, tags), and
    /// on a status change sets or clears its Date done / Date closed.
    pub fn log_task_changes(&self, old: &Task, new: &mut Task, me: u64) {
        if old.status != new.status {
            let m = format!(
                "changed status from {} to {}",
                old.status.to_uppercase(),
                new.status.to_uppercase()
            );
            new.log(me, m);
            // Date done / Date closed: when it entered such a status
            let (was, now_k) = (self.status_of(old).kind, self.status_of(new).kind);
            let done = |k: StatusKind| matches!(k, StatusKind::Done | StatusKind::Closed);
            if done(now_k) && !done(was) {
                new.done_at = Some(now_ts());
            } else if !done(now_k) {
                new.done_at = None;
            }
            if now_k == StatusKind::Closed && was != StatusKind::Closed {
                new.closed_at = Some(now_ts());
            } else if now_k != StatusKind::Closed {
                new.closed_at = None;
            }
        }
        if old.priority != new.priority {
            let m = format!(
                "set priority to {}",
                PRIORITIES[new.priority as usize].label
            );
            new.log(me, m);
        }
        // the history is shared with teammates: keep it in English
        // with a time: "Nov 16 at 2:30 PM"
        let when = |d: NaiveDate, time: Option<NaiveTime>| {
            use crate::widgets::{fmt_date, fmt_time};
            crate::i18n::in_english(|| match time {
                Some(time) => format!("{} at {}", fmt_date(d), fmt_time(time)),
                None => fmt_date(d),
            })
        };
        if (old.due, old.due_at()) != (new.due, new.due_at()) {
            let s = new
                .due
                .map(|d| when(d, new.due_at()))
                .unwrap_or_else(|| "none".into());
            new.log(me, format!("changed due date to {}", s));
        }
        if (old.start, old.start_at()) != (new.start, new.start_at()) {
            let s = new
                .start
                .map(|d| when(d, new.start_at()))
                .unwrap_or_else(|| "none".into());
            new.log(me, format!("changed start date to {}", s));
        }
        for aid in new.assignees.clone() {
            if !old.assignees.contains(&aid) {
                new.activity.push(self.about_line(me, "assigned to", aid));
            }
        }
        for &aid in &old.assignees {
            if !new.assignees.contains(&aid) {
                new.activity
                    .push(self.about_line(me, "removed assignee", aid));
            }
        }
        // typed fields change on every key press: while I keep typing,
        // the line I logged a moment ago is updated instead of adding more
        if old.name != new.name && !new.name.trim().is_empty() {
            let m = format!("renamed this task to \"{}\"", new.name.trim());
            new.log_edit(me, "renamed this task to ", m);
        }
        if old.description != new.description {
            new.log_edit(
                me,
                "updated the description",
                "updated the description".into(),
            );
        }
        if old.estimate_min != new.estimate_min {
            let m = match new.estimate_min {
                Some(min) => format!(
                    "set the time estimate to {}",
                    crate::i18n::in_english(|| crate::widgets::fmt_duration(min as i64 * 60))
                ),
                None => "removed the time estimate".into(),
            };
            new.log_edit(me, "time estimate", m);
        }
        for tag in new.tags.clone() {
            if !old.tags.contains(&tag) {
                new.log(me, format!("added tag {}", tag));
            }
        }
        for tag in &old.tags {
            if !new.tags.contains(tag) {
                new.log(me, format!("removed tag {}", tag));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example workspace, a task in it, and the lines a change adds.
    fn changed(edit: impl FnOnce(&mut Task)) -> (Data, Task, Vec<String>) {
        let d = Data::seed();
        let old = d.tasks[0].clone();
        let mut new = old.clone();
        edit(&mut new);
        d.log_task_changes(&old, &mut new, d.me);
        let lines = new.activity[old.activity.len()..]
            .iter()
            .map(|a| a.text.clone())
            .collect();
        (d, new, lines)
    }

    fn status_named(d: &Data, t: &Task, kind: StatusKind) -> String {
        d.statuses_for_list(t.list_id)
            .iter()
            .find(|s| s.kind == kind)
            .expect("a status of that kind")
            .name
            .clone()
    }

    #[test]
    fn nothing_changed_adds_nothing() {
        let (_, _, lines) = changed(|_| {});
        assert!(lines.is_empty(), "{lines:?}");
    }

    #[test]
    fn status_change_is_logged_and_stamps_done_and_closed() {
        let d = Data::seed();
        let mut old = d.tasks[0].clone();
        old.status = status_named(&d, &old, StatusKind::Open);
        old.done_at = None;
        old.closed_at = None;
        let mut new = old.clone();
        new.status = status_named(&d, &old, StatusKind::Closed);
        d.log_task_changes(&old, &mut new, d.me);
        let last = new.activity.last().unwrap();
        assert_eq!(
            last.text,
            format!(
                "changed status from {} to {}",
                old.status.to_uppercase(),
                new.status.to_uppercase()
            )
        );
        assert_eq!(last.author, d.me);
        assert!(new.done_at.is_some() && new.closed_at.is_some());

        // back open: both dates go
        let old = new.clone();
        new.status = status_named(&d, &old, StatusKind::Open);
        d.log_task_changes(&old, &mut new, d.me);
        assert!(new.done_at.is_none() && new.closed_at.is_none());
    }

    #[test]
    fn priority_change_names_the_priority() {
        let (_, new, lines) = changed(|t| t.priority = if t.priority == 1 { 2 } else { 1 });
        let label = PRIORITIES[new.priority as usize].label;
        assert_eq!(lines, vec![format!("set priority to {label}")]);
    }

    #[test]
    fn dates_are_logged_in_english_with_their_time() {
        let day = NaiveDate::from_ymd_opt(2001, 2, 3).unwrap();
        let (_, _, lines) = changed(|t| {
            t.due = Some(day);
            t.set_due_time(NaiveTime::from_hms_opt(14, 30, 0));
            t.start = Some(day);
            t.set_start_time(None);
        });
        assert!(lines.contains(&"changed due date to 2/3/01 at 2:30 PM".to_string()));
        assert!(lines.contains(&"changed start date to 2/3/01".to_string()));
        let (_, _, lines) = changed(|t| {
            t.due = None;
            t.start = None;
        });
        let had = Data::seed().tasks[0].clone();
        if had.due.is_some() {
            assert!(lines.contains(&"changed due date to none".to_string()));
        }
        if had.start.is_some() {
            assert!(lines.contains(&"changed start date to none".to_string()));
        }
    }

    #[test]
    fn assignees_added_and_removed_keep_the_persons_id() {
        let d = Data::seed();
        let (a, b) = (d.members[0].id, d.members[1].id);
        let mut old = d.tasks[0].clone();
        old.assignees = vec![a];
        let mut new = old.clone();
        new.assignees = vec![b];
        d.log_task_changes(&old, &mut new, d.me);
        let added = &new.activity[new.activity.len() - 2];
        let gone = &new.activity[new.activity.len() - 1];
        assert_eq!(
            added.text,
            format!("assigned to {}", d.member(b).unwrap().shown())
        );
        assert_eq!(added.target, Some(b));
        assert_eq!(
            gone.text,
            format!("removed assignee {}", d.member(a).unwrap().shown())
        );
        assert_eq!(gone.target, Some(a));
    }

    #[test]
    fn typing_a_name_updates_one_line() {
        let d = Data::seed();
        let old = d.tasks[0].clone();
        let mut new = old.clone();
        new.name = "Wri".into();
        d.log_task_changes(&old, &mut new, d.me);
        let mid = new.clone();
        new.name = " Write it ".into();
        d.log_task_changes(&mid, &mut new, d.me);
        assert_eq!(new.activity.len(), old.activity.len() + 1);
        assert_eq!(
            new.activity.last().unwrap().text,
            "renamed this task to \"Write it\""
        );
        // a blank name is not logged
        let (_, _, lines) = changed(|t| t.name = "  ".into());
        assert!(lines.is_empty(), "{lines:?}");
    }

    #[test]
    fn description_change_is_one_line() {
        let (_, _, lines) = changed(|t| t.description.push_str(" more"));
        assert_eq!(lines, vec!["updated the description".to_string()]);
    }

    #[test]
    fn estimate_set_and_removed() {
        let (_, _, lines) = changed(|t| t.estimate_min = Some(90));
        assert_eq!(lines, vec!["set the time estimate to 1h 30m".to_string()]);
        let d = Data::seed();
        let mut old = d.tasks[0].clone();
        old.estimate_min = Some(30);
        let mut new = old.clone();
        new.estimate_min = None;
        d.log_task_changes(&old, &mut new, d.me);
        assert_eq!(
            new.activity.last().unwrap().text,
            "removed the time estimate"
        );
    }

    #[test]
    fn tags_added_and_removed() {
        let d = Data::seed();
        let mut old = d.tasks[0].clone();
        old.tags = vec!["red".into()];
        let mut new = old.clone();
        new.tags = vec!["blue".into()];
        d.log_task_changes(&old, &mut new, d.me);
        let n = new.activity.len();
        assert_eq!(new.activity[n - 2].text, "added tag blue");
        assert_eq!(new.activity[n - 1].text, "removed tag red");
    }
}
