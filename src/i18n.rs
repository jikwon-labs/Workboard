// Interface language. Every piece of text the app shows is written in English
// in the code and passed through `tr` (or `trf!` when it has values in it)
// where it is drawn; `tr` looks the English up in the Korean table when Korean
// is on and gives the English back otherwise, or when there is no entry.
//
// Only display goes through here: names used as keys (settings sections,
// view names, menu ids, accessibility names the tests click by) stay English.
// Text people typed (task names, status names, messages) is never translated.
//
// The language is per thread so tests can switch it without touching tests
// running beside them; the app draws on one thread.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

mod ko_account;
mod ko_app;
mod ko_chat;
mod ko_common;
mod ko_hubs;
mod ko_menus;
mod ko_more;
mod ko_notify;
mod ko_parity;
mod ko_shell;
mod ko_small;
mod ko_sweep;
mod ko_templates;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Ko,
}

/// The language chosen in Settings, kept on this computer (not in the workspace).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    System,
    En,
    Ko,
}

thread_local! {
    static LANG: Cell<Lang> = const { Cell::new(Lang::En) };
}

pub fn lang() -> Lang {
    LANG.with(|l| l.get())
}

pub fn set_lang(l: Lang) {
    LANG.with(|c| c.set(l));
}

/// Runs `f` with English on: for text saved into the workspace (task history,
/// activity), which teammates read whatever language they use.
pub fn in_english<R>(f: impl FnOnce() -> R) -> R {
    let was = lang();
    set_lang(Lang::En);
    let r = f();
    set_lang(was);
    r
}

pub fn is_ko() -> bool {
    lang() == Lang::Ko
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m = HashMap::new();
        for part in ALL {
            for (en, ko) in part.iter() {
                m.entry(*en).or_insert(*ko);
            }
        }
        m
    })
}

const ALL: [&[(&str, &str)]; 13] = [
    ko_common::T,
    ko_account::T,
    ko_app::T,
    ko_chat::T,
    ko_hubs::T,
    ko_menus::T,
    ko_more::T,
    ko_notify::T,
    ko_parity::T,
    ko_shell::T,
    ko_small::T,
    ko_sweep::T,
    ko_templates::T,
];

/// The text in the interface language.
pub fn tr(s: &str) -> &str {
    if lang() == Lang::En {
        return s;
    }
    table().get(s).copied().unwrap_or(s)
}

/// Text whose English is shared by things that read differently in Korean
/// ("Unmute" a channel and the microphone, "Admin" the role and the settings
/// group): `en` in English, the Korean entry of `key` in Korean.
pub fn tr_ctx<'a>(key: &str, en: &'a str) -> &'a str {
    if lang() == Lang::En {
        return en;
    }
    table().get(key).copied().unwrap_or(en)
}

/// A status name for display: the default statuses ("to do", "in progress",
/// "complete") are translated, names people chose are shown as they are.
pub fn status_text(name: &str) -> &str {
    match name {
        "to do" | "in progress" | "complete" => tr(name),
        _ => name,
    }
}

/// A task history line ("Alex changed status from TO DO to COMPLETE") for
/// display. History is saved in English (teammates share it); in Korean the
/// known lines are rewritten here, the rest are shown as saved.
pub fn history_line(who: &str, text: &str) -> String {
    if lang() == Lang::Ko {
        if let Some(ko) = history_ko(text) {
            return format!("{}\u{b2d8}\u{c774} {}", who, ko);
        }
    }
    format!("{} {}", who, text)
}

fn history_ko(text: &str) -> Option<String> {
    // (English pattern, Korean): {} values in order, {0} {1} in Korean
    const LINES: [(&str, &str); 28] = [
        ("changed status from {} to {}", "\u{c0c1}\u{d0dc}\u{b97c} {0}\u{c5d0}\u{c11c} {1}(\u{c73c})\u{b85c} \u{bcc0}\u{acbd}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("set priority to {}", "\u{c6b0}\u{c120}\u{c21c}\u{c704}\u{b97c} {0}(\u{c73c})\u{b85c} \u{c124}\u{c815}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("changed due date to {}", "\u{b9c8}\u{ac10}\u{c77c}\u{c744} {0}(\u{c73c})\u{b85c} \u{bcc0}\u{acbd}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("changed start date to {}", "\u{c2dc}\u{c791}\u{c77c}\u{c744} {0}(\u{c73c})\u{b85c} \u{bcc0}\u{acbd}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("assigned to {}", "{0}\u{c5d0}\u{ac8c} \u{d560}\u{b2f9}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("removed assignee {}", "\u{b2f4}\u{b2f9}\u{c790} {0}\u{c744}(\u{b97c}) \u{c81c}\u{ac70}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("created this task", "\u{c774} \u{c791}\u{c5c5}\u{c744} \u{b9cc}\u{b4e4}\u{c5c8}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("moved this task to {}", "\u{c774} \u{c791}\u{c5c5}\u{c744} {0}(\u{c73c})\u{b85c} \u{c62e}\u{acbc}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("duplicated this task from {}", "{0}\u{c5d0}\u{c11c} \u{c774} \u{c791}\u{c5c5}\u{c744} \u{bcf5}\u{c81c}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("archived this task", "\u{c774} \u{c791}\u{c5c5}\u{c744} \u{bcf4}\u{ad00}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("unarchived this task", "\u{c774} \u{c791}\u{c5c5}\u{c758} \u{bcf4}\u{ad00}\u{c744} \u{d574}\u{c81c}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}"),
        ("completed a {} repeat; next due {}", "{0} \u{bc18}\u{bcf5}\u{c744} \u{c644}\u{b8cc}\u{d588}\u{c2b5}\u{b2c8}\u{b2e4}. \u{b2e4}\u{c74c} \u{b9c8}\u{ac10}\u{c77c}: {1}"),
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
        // what an Automation did (`describe_action`)
        ("Automation ran: assign to {}", "자동화로 {0}에게 할당했습니다"),
        ("Automation ran: set priority to {}", "자동화로 우선순위를 {0}(으)로 설정했습니다"),
        ("Automation ran: move to {}", "자동화로 {0}(으)로 옮겼습니다"),
        ("Automation ran: comment \"{}\"", "자동화로 \"{0}\" 댓글을 달았습니다"),
    ];
    for (en, ko) in LINES {
        if let Some(vals) = match_pattern(en, text) {
            let vals: Vec<String> = vals.iter().map(|v| history_value(v)).collect();
            return Some(particles(&fill(ko, &vals)));
        }
    }
    None
}

/// Picks the particle after a value by its last sound: "(으)로" becomes
/// "로" or "으로", "을(를)" becomes "을" or "를". After a Latin letter only
/// the endings whose sound is plain are settled ("me@team.io로", "Kim으로");
/// the rest are left as written.
pub fn particles(s: &str) -> String {
    // Some(true): ends in a consonant other than ㄹ, Some(false): a vowel or ㄹ
    fn coda(c: char, for_ro: bool) -> Option<bool> {
        let code = c as u32;
        if (0xac00..=0xd7a3).contains(&code) {
            let f = (code - 0xac00) % 28;
            return Some(if for_ro { f != 0 && f != 8 } else { f != 0 });
        }
        match c.to_ascii_lowercase() {
            'a' | 'i' | 'o' | 'u' | 'y' => return Some(false),
            'm' | 'n' => return Some(true),
            'l' => return Some(!for_ro),
            _ => {}
        }
        // digits read in Korean: 0 영 1 일 3 삼 6 육 7 칠 8 팔 have a final consonant
        let d = c.to_digit(10)?;
        Some(match d {
            0 | 3 | 6 => true,
            1 | 7 | 8 => !for_ro,
            _ => false,
        })
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let ro = rest.find("(\u{c73c})\u{b85c}");
        let eul = rest.find("\u{c744}(\u{b97c})");
        let (at, is_ro) = match (ro, eul) {
            (Some(a), Some(b)) if b < a => (b, false),
            (Some(a), _) => (a, true),
            (None, Some(b)) => (b, false),
            (None, None) => break,
        };
        let before = &rest[..at];
        out.push_str(before);
        let last = before.chars().last().or_else(|| out.chars().last());
        let token = if is_ro {
            "(\u{c73c})\u{b85c}"
        } else {
            "\u{c744}(\u{b97c})"
        };
        match last.and_then(|c| coda(c, is_ro)) {
            Some(true) => out.push_str(if is_ro {
                "\u{c73c}\u{b85c}"
            } else {
                "\u{c744}"
            }),
            Some(false) => out.push_str(if is_ro { "\u{b85c}" } else { "\u{b97c}" }),
            None => out.push_str(token),
        }
        rest = &rest[at + token.len()..];
    }
    out.push_str(rest);
    out
}

/// The {} values of `text` when it has the shape of `pattern`.
fn match_pattern<'a>(pattern: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let parts: Vec<&str> = pattern.split("{}").collect();
    let mut rest = text.strip_prefix(parts[0])?;
    let mut vals = vec![];
    for (i, sep) in parts.iter().enumerate().skip(1) {
        if i == parts.len() - 1 {
            let v = if sep.is_empty() {
                rest
            } else {
                rest.strip_suffix(sep)?
            };
            vals.push(v);
            rest = "";
        } else {
            let at = rest.find(sep)?;
            vals.push(&rest[..at]);
            rest = &rest[at + sep.len()..];
        }
    }
    rest.is_empty().then_some(vals)
}

/// A value inside a history line: default statuses, priorities, repeats and
/// the dates `fmt_date` wrote ("Today", "Nov 16", "11/16/25") in Korean.
fn history_value(v: &str) -> String {
    // a date with a time ("Nov 16 at 2:30 PM")
    if let Some((day, time)) = v.rsplit_once(" at ") {
        if let Ok(t) = chrono::NaiveTime::parse_from_str(time, "%I:%M %p") {
            return format!("{} {}", history_value(day), t.text("%-I:%M %p"));
        }
    }
    let low = v.to_lowercase();
    if matches!(low.as_str(), "to do" | "in progress" | "complete") {
        return tr(&low).to_string();
    }
    if low == "none" {
        return tr("None").to_string();
    }
    let cap: String = low
        .chars()
        .take(1)
        .flat_map(|c| c.to_uppercase())
        .chain(low.chars().skip(1))
        .collect();
    if matches!(cap.as_str(), "Daily" | "Weekly" | "Monthly" | "Yearly") {
        return tr(&cap).to_string();
    }
    let t = tr(v);
    if t != v {
        return t.to_string();
    }
    let year = chrono::Local::now().format("%Y").to_string();
    if let Ok(d) = chrono::NaiveDate::parse_from_str(&format!("{} {}", v, year), "%b %d %Y") {
        return d.text("%b %-d");
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%m/%d/%y") {
        return d.text("%-m/%-d/%y");
    }
    // a day name such as "Mon"
    names(v.to_string())
}

/// Whether the Korean table has `s` (for the coverage test).
#[cfg(test)]
pub fn has_ko(s: &str) -> bool {
    table().contains_key(s)
}

/// Puts `args` into a translated template: `{}` takes the next value, `{0}`,
/// `{1}`... a numbered one (Korean often needs the values in another order),
/// and `{{` / `}}` are literal braces.
pub fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut it = template.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                out.push('}');
            }
            '{' => {
                let mut num = String::new();
                for d in it.by_ref() {
                    if d == '}' {
                        break;
                    }
                    num.push(d);
                }
                let i = if num.is_empty() {
                    next += 1;
                    next - 1
                } else {
                    num.parse().unwrap_or(usize::MAX)
                };
                if let Some(a) = args.get(i) {
                    out.push_str(a);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// `format!` for interface text: the template is translated first. Only plain
/// `{}` (and `{0}`...) placeholders: format numbers into strings beforehand.
#[macro_export]
macro_rules! trf {
    ($t:literal $(, $a:expr)* $(,)?) => {
        $crate::i18n::fill($crate::i18n::tr($t), &[$(($a).to_string()),*])
    };
}

/// Dates and times in the interface language: `d.text("%b %-d")` gives
/// "Nov 16" in English and "11월 16일" in Korean.
pub trait DateText {
    fn text(&self, pattern: &str) -> String;
}

impl DateText for chrono::NaiveDate {
    fn text(&self, pattern: &str) -> String {
        names(self.format(date_pattern(pattern)).to_string())
    }
}

impl<Tz: chrono::TimeZone> DateText for chrono::DateTime<Tz>
where
    Tz::Offset: std::fmt::Display,
{
    fn text(&self, pattern: &str) -> String {
        names(self.format(date_pattern(pattern)).to_string())
    }
}

impl DateText for chrono::NaiveTime {
    fn text(&self, pattern: &str) -> String {
        names(self.format(date_pattern(pattern)).to_string())
    }
}

/// The Korean order of the date patterns the app uses.
fn date_pattern(p: &str) -> &str {
    if !is_ko() {
        return p;
    }
    match p {
        "%b %-d" => "%-m\u{c6d4} %-d\u{c77c}",
        "%b %-d, %Y" => "%Y\u{b144} %-m\u{c6d4} %-d\u{c77c}",
        "%B %Y" | "%b %Y" => "%Y\u{b144} %-m\u{c6d4}",
        "%A, %B %-d" => "%-m\u{c6d4} %-d\u{c77c} %A",
        "%a %-d" => "%-d\u{c77c} (%a)",
        "%b %-d, %-I:%M %p" | "%b %-d at %-I:%M %p" => "%-m\u{c6d4} %-d\u{c77c} %p %-I:%M",
        "%a %b %-d, %-I:%M %p" => "%-m\u{c6d4} %-d\u{c77c} (%a) %p %-I:%M",
        "%-I:%M %p" => "%p %-I:%M",
        "%-m/%-d/%y" => "%y. %-m. %-d.",
        _ => p,
    }
}

/// Day names and AM / PM in Korean (chrono writes them in English).
fn names(s: String) -> String {
    if !is_ko() {
        return s;
    }
    const DAYS: [(&str, &str, &str); 7] = [
        ("Monday", "Mon", "\u{c6d4}"),
        ("Tuesday", "Tue", "\u{d654}"),
        ("Wednesday", "Wed", "\u{c218}"),
        ("Thursday", "Thu", "\u{baa9}"),
        ("Friday", "Fri", "\u{ae08}"),
        ("Saturday", "Sat", "\u{d1a0}"),
        ("Sunday", "Sun", "\u{c77c}"),
    ];
    let mut s = s;
    for (long, short, ko) in DAYS {
        s = s
            .replace(long, &format!("{}\u{c694}\u{c77c}", ko))
            .replace(short, ko);
    }
    s.replace("AM", "\u{c624}\u{c804}")
        .replace("PM", "\u{c624}\u{d6c4}")
}

/// The language the choice comes to on this computer.
pub fn resolve(c: Choice) -> Lang {
    match c {
        Choice::En => Lang::En,
        Choice::Ko => Lang::Ko,
        Choice::System => {
            if system_language().starts_with("ko") {
                Lang::Ko
            } else {
                Lang::En
            }
        }
    }
}

pub fn choice_key(c: Choice) -> &'static str {
    match c {
        Choice::System => "system",
        Choice::En => "en",
        Choice::Ko => "ko",
    }
}

pub fn choice_from_key(s: &str) -> Choice {
    match s.trim() {
        "en" => Choice::En,
        "ko" => Choice::Ko,
        _ => Choice::System,
    }
}

/// The first preferred language of the system, such as "ko-KR" or "en-US".
fn system_language() -> String {
    #[cfg(target_os = "macos")]
    {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        // SAFETY: NSLocale.preferredLanguages returns an autoreleased NSArray of NSString
        unsafe {
            let langs: *mut Object = msg_send![class!(NSLocale), preferredLanguages];
            if !langs.is_null() {
                let count: usize = msg_send![langs, count];
                if count > 0 {
                    let first: *mut Object = msg_send![langs, objectAtIndex: 0usize];
                    let utf8: *const std::os::raw::c_char = msg_send![first, UTF8String];
                    if !utf8.is_null() {
                        return std::ffi::CStr::from_ptr(utf8)
                            .to_string_lossy()
                            .into_owned();
                    }
                }
            }
        }
    }
    #[cfg(windows)]
    {
        // the display languages chosen in Windows Settings, as names like
        // "ko-KR", one after another, each ending in a 0
        #[link(name = "kernel32")]
        extern "system" {
            fn GetUserPreferredUILanguages(
                flags: u32,
                count: *mut u32,
                buffer: *mut u16,
                len: *mut u32,
            ) -> i32;
        }
        const MUI_LANGUAGE_NAME: u32 = 0x8;
        let mut count = 0u32;
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        // SAFETY: buf holds len u16s; Windows writes at most that many
        let ok = unsafe {
            GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, buf.as_mut_ptr(), &mut len)
        };
        if ok != 0 {
            let first: Vec<u16> = buf.iter().take_while(|c| **c != 0).copied().collect();
            if !first.is_empty() {
                return String::from_utf16_lossy(&first);
            }
        }
    }
    for k in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                return v.replace('_', "-");
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_takes_values_in_order_or_by_number() {
        assert_eq!(fill("{} of {}", &["1".into(), "3".into()]), "1 of 3");
        assert_eq!(fill("{1}개 중 {0}", &["1".into(), "3".into()]), "3개 중 1");
        assert_eq!(fill("{{x}} {}", &["a".into()]), "{x} a");
    }

    #[test]
    fn tr_follows_the_language() {
        set_lang(Lang::En);
        assert_eq!(tr("Save"), "Save");
        set_lang(Lang::Ko);
        assert_eq!(tr("Save"), "저장");
        assert_eq!(tr("no such text 123"), "no such text 123");
        assert_eq!(crate::trf!("{} tasks", 3), "작업 3개");
        // "Never" is only the repeat choice; a reply reads as 답장 like "Replies"
        assert_eq!(tr("Never"), "반복 안 함");
        assert_eq!(
            crate::trf!("{} replied: {}", "Sam", "ok"),
            "Sam님의 답장: ok"
        );
        let d = chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        assert_eq!(d.text("%b %-d"), "11월 16일");
        assert_eq!(d.text("%A, %B %-d"), "11월 16일 월요일");
        assert_eq!(d.text("%a %-d"), "16일 (월)");
        assert_eq!(
            history_line("Alex", "changed status from TO DO to COMPLETE"),
            "Alex님이 상태를 할 일에서 완료로 변경했습니다"
        );
        assert_eq!(
            history_line("Alex", "changed due date to Nov 16"),
            "Alex님이 마감일을 11월 16일로 변경했습니다"
        );
        assert_eq!(
            history_line("Alex", "set priority to Urgent"),
            "Alex님이 우선순위를 긴급으로 설정했습니다"
        );
        assert_eq!(
            history_line("Alex", "changed status from TO DO to REVIEW"),
            "Alex님이 상태를 할 일에서 REVIEW(으)로 변경했습니다"
        );
        assert_eq!(
            history_line("Alex", "removed assignee 민준"),
            "Alex님이 담당자 민준을 제거했습니다"
        );
        assert_eq!(
            history_line("Alex", "removed assignee 지우"),
            "Alex님이 담당자 지우를 제거했습니다"
        );
        assert_eq!(
            history_line("Alex", "commented something odd"),
            "Alex commented something odd"
        );
        assert_eq!(
            history_line("Alex", "added tag 디자인"),
            "Alex님이 태그 디자인을 추가했습니다"
        );
        assert_eq!(
            history_line("Alex", "Automation ran: set priority to Urgent"),
            "Alex님이 자동화로 우선순위를 긴급으로 설정했습니다"
        );
        assert_eq!(
            history_line("Alex", "Automation ran: move to Sprint 2"),
            "Alex님이 자동화로 Sprint 2로 옮겼습니다"
        );
        assert_eq!(
            history_line("Alex", "made this a top-level task"),
            "Alex님이 이 작업을 최상위 작업으로 만들었습니다"
        );
        set_lang(Lang::En);
        assert_eq!(
            history_line("Alex", "created this task"),
            "Alex created this task"
        );
        set_lang(Lang::Ko);
        set_lang(Lang::En);
    }

    /// Each Korean entry keeps the English one's placeholders, so no value is lost.
    #[test]
    fn translations_keep_the_placeholders() {
        let count = |s: &str| {
            fill(
                s,
                &(0..9)
                    .map(|i| format!("\u{1}{}\u{1}", i))
                    .collect::<Vec<_>>(),
            )
            .matches('\u{1}')
            .count()
        };
        for part in ALL {
            for (en, ko) in part.iter() {
                assert_eq!(
                    count(en),
                    count(ko),
                    "placeholders differ: {:?} -> {:?}",
                    en,
                    ko
                );
                assert!(!ko.is_empty(), "empty translation for {:?}", en);
            }
        }
    }

    /// The `.rs` files below `dir` (the Korean tables left out), with their names.
    fn sources(dir: &std::path::Path) -> Vec<(String, String)> {
        let mut out = vec![];
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.file_name().is_some_and(|n| n != "i18n") {
                        stack.push(p);
                    }
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let name = p.file_name().unwrap().to_string_lossy().to_string();
                    out.push((name, std::fs::read_to_string(&p).unwrap()));
                }
            }
        }
        out
    }

    /// The app's own source files that draw the interface: no tests, no tables.
    fn interface_sources() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        sources(&root)
            .into_iter()
            .filter(|(name, _)| !name.contains("tests") && name != "i18n.rs")
            .collect()
    }

    /// Every literal passed to `tr(` / `trf!(` in the source has a Korean entry.
    #[test]
    fn every_interface_text_has_korean() {
        let mut missing = vec![];
        for (name, src) in interface_sources() {
            for lit in literals_after(&src, &["tr(\"", "trf!(\"", "tr_ctx(\""]) {
                if !has_ko(&lit) {
                    missing.push(format!("{}: {:?}", name, lit));
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "{} texts without Korean:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }

    /// Text handed straight to egui is wrapped in `tr` (or `trf!`): a literal
    /// right after `ui.label(`, `RichText::new(` and the like is English on a
    /// Korean screen, and the screen check only sees the screens it opens (and
    /// no tooltips). Text that reads the same in every language is let through.
    #[test]
    fn interface_text_is_wrapped_in_tr() {
        const CALLS: [&str; 10] = [
            ".label(\"",
            ".button(\"",
            ".heading(\"",
            ".small(\"",
            "RichText::new(\"",
            "Button::new(\"",
            ".hint_text(\"",
            ".on_hover_text(\"",
            ".on_disabled_hover_text(\"",
            ".selectable_label(\"",
        ];
        let mut bare = vec![];
        for (name, src) in interface_sources() {
            // the code under a file's test module is not drawn
            let src = src.split("\n#[cfg(test)]\nmod ").next().unwrap_or("");
            let fmt: Vec<String> = CALLS.iter().map(|c| c.replace('"', "format!(\"")).collect();
            let fmt: Vec<&str> = fmt.iter().map(|s| s.as_str()).collect();
            for lit in literals_after(src, &CALLS)
                .into_iter()
                .chain(literals_after(src, &fmt))
            {
                if reads_as_english(&lit) {
                    bare.push(format!("{}: {:?}", name, lit));
                }
            }
        }
        bare.sort();
        bare.dedup();
        assert!(
            bare.is_empty(),
            "{} interface texts not wrapped in tr (wrap them and add the Korean):\n{}",
            bare.len(),
            bare.join("\n")
        );
    }

    /// A literal that would show as English on a Korean screen: a word of two
    /// or more Latin letters, once the text that reads the same in Korean
    /// (the app's name, key names, units, the example server address) is out.
    fn reads_as_english(lit: &str) -> bool {
        const SAME_IN_KOREAN: [&str; 5] =
            ["Workboard", "ID", "Esc", "KB", "https://wb.example.com"];
        let rest = SAME_IN_KOREAN
            .iter()
            .fold(lit.to_string(), |s, k| s.replace(k, ""));
        let mut run = 0;
        rest.chars().any(|c| {
            run = if c.is_ascii_alphabetic() { run + 1 } else { 0 };
            run >= 2
        })
    }

    /// The arguments of the call whose `(` is just before `src`, split at its
    /// own commas (not those inside strings, brackets or a closure's `|a, b|`).
    fn call_args(src: &str) -> Vec<&str> {
        let (mut out, mut depth, mut start, mut pipe) = (vec![], 0, 0, false);
        let b = src.as_bytes();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'"' => {
                    i += 1;
                    while i < b.len() && b[i] != b'"' {
                        i += if b[i] == b'\\' { 2 } else { 1 };
                    }
                }
                // a char literal such as '(' or ','
                b'\'' if i + 2 < b.len() && b[i + 2] == b'\'' => i += 2,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    if depth == 0 {
                        out.push(&src[start..i]);
                        return out;
                    }
                    depth -= 1;
                }
                b'|' if depth == 0 => pipe = !pipe,
                b',' if depth == 0 && !pipe => {
                    out.push(&src[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        out
    }

    /// The string literals in `arg` that are not inside a `tr(`, `trf!(` or
    /// `tr_ctx(` (so `tr(if on { "A" } else { "B" })` passes).
    fn literals_outside_tr(arg: &str) -> Vec<String> {
        let mut outside = String::new();
        let mut rest = arg;
        while let Some(i) = ["tr(", "trf!(", "tr_ctx("]
            .iter()
            .filter_map(|m| {
                rest.match_indices(m)
                    .find(|(i, _)| {
                        !rest[..*i]
                            .chars()
                            .last()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_')
                    })
                    .map(|(i, _)| (i, m.len()))
            })
            .min()
            .map(|(i, len)| (i, i + len))
        {
            outside.push_str(&rest[..i.0]);
            let inner = &rest[i.1..];
            let used: usize = call_args(inner).iter().map(|a| a.len() + 1).sum();
            rest = &inner[used.min(inner.len())..];
        }
        outside.push_str(rest);
        literals_after(&outside, &["\""])
            .into_iter()
            .step_by(2)
            .collect()
    }

    /// The app's own drawing helpers take their text as it is shown, so it is
    /// wrapped in `tr` where they are called: `menu_item(ui, t, "list", "New
    /// list", "")` would show English on a Korean screen. Each helper is listed
    /// with the arguments it draws (counted from 0); arguments that only name
    /// a control for screen readers and tests stay English and are not listed.
    #[test]
    fn helper_text_is_wrapped_in_tr() {
        const HELPERS: [(&str, &[usize]); 21] = [
            ("icon_button", &[3]),
            ("icon_button_ex", &[3]),
            ("menu_item", &[3, 4]),
            ("menu_item2", &[3, 4]),
            ("title", &[2, 3]),
            ("page_title", &[2, 3]),
            ("bar_row", &[2]),
            ("pill", &[3]),
            ("template_row", &[5, 6]),
            ("menu_row", &[2]),
            ("menu_row_named", &[3]),
            ("menu_row_plain", &[2]),
            ("add_line", &[2]),
            ("primary_button", &[2]),
            ("primary_btn", &[1]),
            ("secondary_button", &[1]),
            ("row_button", &[1]),
            ("dialog_header", &[2]),
            ("check", &[2]),
            ("radio", &[2]),
            ("gallery", &[1, 2]),
        ];
        let mut bare = vec![];
        for (name, src) in interface_sources() {
            let src = src.split("\n#[cfg(test)]\nmod ").next().unwrap_or("");
            for (helper, drawn) in HELPERS {
                let call = format!("{}(", helper);
                for (at, _) in src.match_indices(&call) {
                    let before = &src[..at];
                    if before.ends_with("fn ")
                        || before
                            .chars()
                            .last()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_')
                    {
                        continue;
                    }
                    let args = call_args(&src[at + call.len()..]);
                    for &k in drawn {
                        let Some(arg) = args.get(k) else { continue };
                        for lit in literals_outside_tr(arg) {
                            if reads_as_english(&lit) {
                                let line = before.matches('\n').count() + 1;
                                bare.push(format!("{}:{} {}: {:?}", name, line, helper, lit));
                            }
                        }
                    }
                }
            }
        }
        bare.sort();
        bare.dedup();
        assert!(
            bare.is_empty(),
            "{} helper texts not wrapped in tr (wrap them and add the Korean):\n{}",
            bare.len(),
            bare.join("\n")
        );
    }

    /// The helper check sees bare text, text in a format and text in a
    /// branch, and lets translated text and screen-reader names through.
    #[test]
    fn helper_check_reads_arguments_as_written() {
        let args = call_args("ui, &t, \"list\", tr(\"New list\"), \"Name, here\", |p, c| {}) rest");
        assert_eq!(args.len(), 6);
        assert_eq!(args[4].trim(), "\"Name, here\"");
        assert!(literals_outside_tr("tr(if on { \"Watching\" } else { \"Watch\" })").is_empty());
        assert!(literals_outside_tr("trf!(\"{} tasks\", n)").is_empty());
        assert_eq!(
            literals_outside_tr("if on { tr(\"Stop\") } else { \"Start\" }"),
            ["Start"]
        );
        assert_eq!(literals_outside_tr("&format!(\"Open {}\", x)"), ["Open {}"]);
        assert!(!reads_as_english("\u{2318}K"));
        assert!(!reads_as_english("Workboard {}"));
    }

    /// Each English text has one Korean entry: `tr` takes the first one in
    /// `ALL`, so a second entry somewhere else would never show, and changing
    /// it would change nothing.
    #[test]
    fn each_text_has_one_korean_entry() {
        let mut first: HashMap<&str, &str> = HashMap::new();
        let mut twice = vec![];
        for part in ALL {
            for (en, ko) in part.iter() {
                if let Some(was) = first.insert(en, ko) {
                    twice.push(format!("{:?}: {:?} and {:?}", en, was, ko));
                }
            }
        }
        assert!(
            twice.is_empty(),
            "{} texts with more than one Korean entry (keep one):\n{}",
            twice.len(),
            twice.join("\n")
        );
    }

    /// Every Korean entry is for text the code still has: the English is a
    /// string literal somewhere in the app, the server or the crates (whose
    /// messages the app shows). Entries for text the code builds are listed.
    #[test]
    fn no_korean_entry_is_left_unused() {
        // accent colour names, made from the saved lower-case name (menus.rs)
        const BUILT: [&str; 10] = [
            "Black", "Purple", "Blue", "Pink", "Violet", "Indigo", "Orange", "Teal", "Bronze",
            "Mint",
        ];
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut code = String::new();
        for dir in ["src", "server/src", "crates"] {
            for (name, src) in sources(&base.join(dir)) {
                if name != "i18n.rs" {
                    code.push_str(&src);
                }
            }
        }
        // the literals as the code has them, escapes undone
        let lits: std::collections::HashSet<String> =
            literals_after(&code, &["\""]).into_iter().collect();
        let mut unused = vec![];
        for part in ALL {
            for (en, _) in part.iter() {
                if !lits.contains(*en) && !BUILT.contains(en) {
                    unused.push(format!("{:?}", en));
                }
            }
        }
        assert!(
            unused.is_empty(),
            "{} Korean entries no code uses (remove them):\n{}",
            unused.len(),
            unused.join("\n")
        );
    }

    /// The string literals that follow each marker, with Rust escapes undone.
    fn literals_after(src: &str, markers: &[&str]) -> Vec<String> {
        let mut out = vec![];
        for m in markers {
            let mut rest = src;
            while let Some(i) = rest.find(m) {
                // skip identifiers that merely end in "tr", such as "attr("
                let before = rest[..i].chars().last();
                rest = &rest[i + m.len()..];
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let mut s = String::new();
                let mut chars = rest.char_indices();
                while let Some((_, c)) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next().map(|x| x.1) {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some('"') => s.push('"'),
                            Some('\\') => s.push('\\'),
                            Some('\'') => s.push('\''),
                            Some('u') => {
                                let mut hex = String::new();
                                for (_, h) in chars.by_ref() {
                                    if h == '}' {
                                        break;
                                    }
                                    if h != '{' {
                                        hex.push(h);
                                    }
                                }
                                if let Some(ch) =
                                    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                                {
                                    s.push(ch);
                                }
                            }
                            Some(o) => s.push(o),
                            None => {}
                        },
                        _ => s.push(c),
                    }
                }
                out.push(s);
            }
        }
        out
    }
}
