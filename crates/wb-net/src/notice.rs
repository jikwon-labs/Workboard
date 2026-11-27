// The words of notifications the team server writes itself (server/src/push.rs
// and news.rs: banners for a Mac or iPhone whose app isn't running), in the
// language the device asked for (`PushPrefs::lang`). They are the Mac app's
// own words: the same English templates its src/app/notify.rs passes to
// `tr`, with the Korean of its tables (src/i18n/ko_*.rs). An app test
// (src/i18n.rs) checks every entry here against those tables, and the task
// history lines against `crate::i18n::history_line`, so the two can't drift.
//
// `fill`, `particles` and `match_pattern` are the app's too: src/i18n.rs uses
// these.

/// The language of a banner.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Ko,
}

impl Lang {
    /// From `PushPrefs::lang`: "ko" is Korean, anything else English.
    pub fn of(code: Option<&str>) -> Lang {
        match code {
            Some(c) if c.trim().to_ascii_lowercase().starts_with("ko") => Lang::Ko,
            _ => Lang::En,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ko => "ko",
        }
    }
}

/// English template -> Korean, as the Mac app's tables have them.
pub const KO: &[(&str, &str)] = &[
    ("{} in {}", "{}님 · {}"),
    ("{} is calling you", "{}님이 통화를 걸었습니다"),
    (
        "{} started a huddle in {}",
        "{}님이 {}에서 허들을 시작했습니다",
    ),
    ("Join from the conversation", "대화에서 참여하세요"),
    (
        "{} reacted {} to your message",
        "{}님이 내 메시지에 {} 반응을 남겼습니다",
    ),
    ("{} mentioned you in {}", "{}님이 {}에서 나를 멘션했습니다"),
    ("{} replied in {}", "{}님이 {}에서 답글을 남겼습니다"),
    (
        "{} said \"{}\" in {}",
        "{}님의 메시지에 \"{}\" 가 있습니다 ({})",
    ),
    ("{} assigned you a task", "{}님이 작업을 맡겼습니다"),
    ("{} commented on {}", "{}님이 {}에 댓글을 남겼습니다"),
    ("{} joined {}", "{}님이 {}에 들어왔습니다"),
    ("Say hello in a direct message", "DM으로 인사해 보세요"),
    ("New answer to {}", "{} 새 응답"),
    ("{} answered: {}", "{}님의 응답: {}"),
    ("Untitled form", "제목 없는 양식"),
    ("{} reached {}%", "{} {}% 달성"),
    ("{}% done", "{}% 완료"),
    ("Goal achieved: {}", "목표 달성: {}"),
    ("All targets met", "목표치를 모두 채웠습니다"),
    ("{} new notifications", "새 알림 {}개"),
    ("Sent a picture", "사진을 보냈습니다"),
    ("Sent a video", "영상을 보냈습니다"),
    ("Sent pictures and videos", "사진과 영상을 보냈습니다"),
    ("Sent a file", "파일을 보냈습니다"),
    ("Sent a voice message", "음성 메시지를 보냈습니다"),
    ("Started a call", "통화를 시작했습니다"),
    ("Started a huddle", "허들을 시작했습니다"),
    ("Joined the workspace.", "워크스페이스에 들어왔습니다."),
    (
        "Hi! I just joined the workspace.",
        "안녕하세요! 워크스페이스에 새로 가입했습니다.",
    ),
    ("Someone", "누군가"),
    ("Just you", "나만"),
    // values inside task history lines
    ("to do", "할 일"),
    ("in progress", "진행 중"),
    ("complete", "완료"),
    ("None", "없음"),
    ("Urgent", "긴급"),
    ("High", "높음"),
    ("Normal", "보통"),
    ("Low", "낮음"),
    ("Today", "오늘"),
    ("Tomorrow", "내일"),
    ("Yesterday", "어제"),
    ("Daily", "매일"),
    ("Weekly", "매주"),
    ("Monthly", "매월"),
    ("Yearly", "매년"),
];

/// `en` in `lang` (English, or the Korean entry when there is one).
pub fn tr(lang: Lang, en: &str) -> &str {
    if lang == Lang::En {
        return en;
    }
    KO.iter()
        .find(|(e, _)| *e == en)
        .map(|(_, k)| *k)
        .unwrap_or(en)
}

/// A template in `lang` with `args` put in (`fill`).
pub fn trf(lang: Lang, en: &str, args: &[&str]) -> String {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    fill(tr(lang, en), &args)
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
    const RO: &str = "(으)로";
    const EUL: &str = "을(를)";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let ro = rest.find(RO);
        let eul = rest.find(EUL);
        let (at, is_ro) = match (ro, eul) {
            (Some(a), Some(b)) if b < a => (b, false),
            (Some(a), _) => (a, true),
            (None, Some(b)) => (b, false),
            (None, None) => break,
        };
        let before = &rest[..at];
        out.push_str(before);
        let last = before.chars().last().or_else(|| out.chars().last());
        let token = if is_ro { RO } else { EUL };
        match last.and_then(|c| coda(c, is_ro)) {
            Some(true) => out.push_str(if is_ro { "으로" } else { "을" }),
            Some(false) => out.push_str(if is_ro { "로" } else { "를" }),
            None => out.push_str(token),
        }
        rest = &rest[at + token.len()..];
    }
    out.push_str(rest);
    out
}

/// The {} values of `text` when it has the shape of `pattern`.
pub fn match_pattern<'a>(pattern: &str, text: &'a str) -> Option<Vec<&'a str>> {
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

/// The task history lines a notification tells (a change to my task), as
/// `crate::i18n::history_line` writes them in Korean.
const HISTORY_KO: [(&str, &str); 4] = [
    (
        "changed status from {} to {}",
        "상태를 {0}에서 {1}(으)로 변경했습니다",
    ),
    ("set priority to {}", "우선순위를 {0}(으)로 설정했습니다"),
    ("changed due date to {}", "마감일을 {0}(으)로 변경했습니다"),
    ("removed assignee {}", "담당자 {0}을(를) 제거했습니다"),
];

/// "Alex changed status from TO DO to COMPLETE" in `lang`: history is saved
/// in English; in Korean the lines above are rewritten, the rest are told
/// as saved.
pub fn history_line(lang: Lang, who: &str, text: &str) -> String {
    if lang == Lang::Ko {
        for (en, ko) in HISTORY_KO {
            if let Some(vals) = match_pattern(en, text) {
                let vals: Vec<String> = vals.iter().map(|v| history_value(v)).collect();
                return format!("{}님이 {}", who, particles(&fill(ko, &vals)));
            }
        }
    }
    format!("{} {}", who, text)
}

/// A value inside a history line in Korean: default statuses, priorities,
/// repeats and the dates the app wrote ("Today", "Nov 16", "11/16/25").
fn history_value(v: &str) -> String {
    let ko = Lang::Ko;
    // a date with a time ("Nov 16 at 2:30 PM")
    if let Some((day, time)) = v.rsplit_once(" at ") {
        if let Ok(t) = chrono::NaiveTime::parse_from_str(time, "%I:%M %p") {
            return format!(
                "{} {}",
                history_value(day),
                names(t.format("%p %-I:%M").to_string())
            );
        }
    }
    let low = v.to_lowercase();
    if matches!(low.as_str(), "to do" | "in progress" | "complete") {
        return tr(ko, &low).to_string();
    }
    if low == "none" {
        return tr(ko, "None").to_string();
    }
    let cap: String = low
        .chars()
        .take(1)
        .flat_map(|c| c.to_uppercase())
        .chain(low.chars().skip(1))
        .collect();
    if matches!(cap.as_str(), "Daily" | "Weekly" | "Monthly" | "Yearly") {
        return tr(ko, &cap).to_string();
    }
    let t = tr(ko, v);
    if t != v {
        return t.to_string();
    }
    let year = chrono::Local::now().format("%Y").to_string();
    if let Ok(d) = chrono::NaiveDate::parse_from_str(&format!("{} {}", v, year), "%b %d %Y") {
        return d.format("%-m월 %-d일").to_string();
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(v, "%m/%d/%y") {
        return d.format("%y. %-m. %-d.").to_string();
    }
    // a day name such as "Mon"
    names(v.to_string())
}

/// Day names and AM / PM in Korean (chrono writes them in English).
fn names(s: String) -> String {
    const DAYS: [(&str, &str, &str); 7] = [
        ("Monday", "Mon", "월"),
        ("Tuesday", "Tue", "화"),
        ("Wednesday", "Wed", "수"),
        ("Thursday", "Thu", "목"),
        ("Friday", "Fri", "금"),
        ("Saturday", "Sat", "토"),
        ("Sunday", "Sun", "일"),
    ];
    let mut s = s;
    for (long, short, ko) in DAYS {
        s = s.replace(long, &format!("{}요일", ko)).replace(short, ko);
    }
    s.replace("AM", "오전").replace("PM", "오후")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_and_history_in_both_languages() {
        assert_eq!(
            trf(Lang::Ko, "{} commented on {}", &["Bo", "Launch"]),
            "Bo님이 Launch에 댓글을 남겼습니다"
        );
        assert_eq!(
            trf(Lang::En, "{} commented on {}", &["Bo", "Launch"]),
            "Bo commented on Launch"
        );
        assert_eq!(
            history_line(Lang::Ko, "Bo", "changed status from to do to complete"),
            "Bo님이 상태를 할 일에서 완료로 변경했습니다"
        );
        assert_eq!(
            history_line(Lang::Ko, "Bo", "set priority to High"),
            "Bo님이 우선순위를 높음으로 설정했습니다"
        );
        assert_eq!(
            history_line(Lang::En, "Bo", "set priority to High"),
            "Bo set priority to High"
        );
        assert_eq!(Lang::of(Some("ko-KR")), Lang::Ko);
        assert_eq!(Lang::of(None), Lang::En);
    }
}
