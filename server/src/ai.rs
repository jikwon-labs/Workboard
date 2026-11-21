// Asking an AI (Anthropic's Claude) for the apps' AI tools: summing up a
// conversation or a thread, rewriting a piece of writing, suggesting
// subtasks, and answering questions about the workspace.
//
// Set up by an ai.json in the server folder (like mail.json):
//
//   {
//     "api_key": "sk-ant-...",        an Anthropic API key
//     "model": "claude-opus-5-5",      optional; this is the default
//     "per_hour": 60,                  optional: requests one person may
//                                      make in an hour (default PER_HOUR)
//     "base_url": "https://..."        optional: where the API is (default
//                                      https://api.anthropic.com), for a
//                                      proxy; http:// only to this computer
//   }
//
// (only the file: an ANTHROPIC_API_KEY left in a shell's environment doesn't
// turn it on, so a test server never reaches the real service). Without a
// key the AI is off: the server says so once when it starts and
// never sends ai_ready, so apps show no AI tools. It reads the setup when it
// starts, so restart it after changing either. The workspaces of one server
// share the setup (and each person's hourly allowance is per workspace).
//
// Requests: an app sends `ai {id, ask, lang}`. The server checks that the
// person may see what they ask about (a conversation hidden from them is
// "not found", as if it didn't exist), builds the request from the team's
// copy, and streams the answer back as `ai_text` pieces (a few a second at
// most), then one `ai_done`. A question gets what the asker may see of the
// workspace (`wb_sync::hidden_from`, the same as their copy): an overview of
// all of it, and the tasks, docs and messages that best match the question,
// within CONTEXT_CHARS. The answer cites them as [task:ID], [doc:ID] and
// [msg:CHANNEL:ID], which the app turns into links. Nothing is kept: neither
// the questions nor the answers. The server log gets one line per request
// with its kind, who asked and the tokens it used.
//
// Sending: one HTTPS POST to api.anthropic.com/v1/messages per request,
// streamed (server-sent events), through the system's curl like push.rs and
// hooks.rs. The key travels on curl's stdin (a curl config), not its
// arguments, so it doesn't show in the process list. When the connection
// that asked closes, curl is stopped.

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wb_net::{AiAsk, AiTurn, AI_MAX_TEXT, AI_WRITE};
use wb_sync::json::{array_at, text_field, u64_field};

use crate::Locked;

pub const AI_FILE: &str = "ai.json";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
/// Requests one person may make in an hour (ai.json "per_hour" changes it).
pub const PER_HOUR: u32 = 60;
/// Requests one person may have going at once.
pub const AT_ONCE: usize = 3;
/// About how much of the workspace a question takes along (bytes of text:
/// Korean takes three a character).
pub const CONTEXT_CHARS: usize = 120_000;
/// The newest messages a conversation summary reads.
pub const SUMMARY_MESSAGES: usize = 400;
/// About how much text a summary reads at most (bytes of text).
pub const SUMMARY_CHARS: usize = 80_000;
/// Earlier questions and answers a question takes along (shared with the apps).
pub use wb_net::AI_MAX_HISTORY as MAX_HISTORY;
/// Pieces of an answer are sent at most this often.
pub const PIECE_GAP: Duration = Duration::from_millis(80);
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

// What the apps are told when a request doesn't go through (the app shows
// them in its language).
pub const AI_OFF: &str = "AI isn't set up on this server. The server's owner can turn it on with an Anthropic API key in ai.json.";
pub const AI_TOO_MANY: &str = "You've asked the AI a lot this hour. Try again in a little while.";
pub const AI_BUSY: &str = "Wait for your other AI requests to finish, then try again.";
pub const AI_NOT_FOUND: &str = "That conversation isn't there any more.";
pub const AI_EMPTY: &str = "There's nothing here for the AI to work with yet.";
pub const AI_TOO_LONG: &str = "That is too long for the AI. Try a shorter part.";
pub const AI_DECLINED: &str = "The AI declined to answer this.";
pub const AI_FAILED: &str = "The AI service didn't answer. Try again in a moment.";
pub const AI_UNKNOWN: &str =
    "This app asked for something the server doesn't know. Update the server.";
/// Not shown: the request was stopped because nobody wants the answer.
pub const AI_STOPPED: &str = "stopped";

/// ai.json as written.
#[derive(Deserialize, Debug, Clone)]
pub struct AiCfg {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub per_hour: Option<u32>,
    #[serde(default)]
    pub base_url: String,
}

/// The setup from the server folder's ai.json.
pub fn load_ai(home: &Path) -> Result<AiCfg, String> {
    let path = home.join(AI_FILE);
    let mut cfg = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<AiCfg>(&text)
            .map_err(|e| format!("{} can't be read: {}", path.display(), e))?,
        Err(_) => AiCfg {
            api_key: String::new(),
            model: String::new(),
            per_hour: None,
            base_url: String::new(),
        },
    };
    cfg.api_key = cfg.api_key.trim().to_string();
    cfg.model = cfg.model.trim().to_string();
    if cfg.model.is_empty() {
        cfg.model = DEFAULT_MODEL.into();
    }
    if cfg.api_key.is_empty() {
        return Err(format!("no Anthropic API key in {}", AI_FILE));
    }
    api_place(&cfg.base_url)?;
    Ok(cfg)
}

/// Whether the part of an http:// address after the scheme names this
/// computer: 127.0.0.1, localhost or [::1], maybe with a port, and no
/// user part (in "localhost:1@elsewhere" the host is "elsewhere").
fn this_computer(rest: &str) -> bool {
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    let (host, port) = match authority.strip_prefix("[::1]") {
        Some(port) => ("[::1]", port),
        None => match authority.find(':') {
            Some(i) => (&authority[..i], &authority[i..]),
            None => (authority, ""),
        },
    };
    let port_ok = port.is_empty()
        || port
            .strip_prefix(':')
            .is_some_and(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
    matches!(host, "127.0.0.1" | "localhost" | "[::1]") && port_ok
}

/// The Messages API address for ai.json's base_url, and the protocols curl
/// may use for it: https, or plain http to this computer only (a key sent
/// in the clear anywhere else could be read on the way).
pub fn api_place(base_url: &str) -> Result<(String, &'static str), String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Ok((API_URL.to_string(), "=https"));
    }
    let lower = base.to_ascii_lowercase();
    let protos = if lower.starts_with("https://") {
        "=https"
    } else if lower.strip_prefix("http://").is_some_and(this_computer) {
        "=http"
    } else {
        return Err(format!(
            "base_url in {} must start with https:// (or http:// to this computer)",
            AI_FILE
        ));
    };
    let url = if lower.ends_with("/v1/messages") {
        base.to_string()
    } else {
        format!("{}/v1/messages", base)
    };
    Ok((url, protos))
}

/// What the AI tools run on: who answers, and how much one person may ask.
#[derive(Clone, Debug)]
pub struct AiSetup {
    pub claude: Arc<dyn Claude>,
    pub model: String,
    pub per_hour: u32,
}

/// The AI from the server folder's setup; None (said once) when it isn't set up.
pub fn from_setup(home: &Path) -> Option<AiSetup> {
    match load_ai(home) {
        Ok(cfg) => {
            // load_ai checked it
            let (url, protos) = api_place(&cfg.base_url).ok()?;
            println!("AI tools on ({})", cfg.model);
            Some(AiSetup {
                claude: Arc::new(CurlClaude {
                    key: cfg.api_key,
                    url,
                    protos,
                }),
                model: cfg.model,
                per_hour: cfg.per_hour.unwrap_or(PER_HOUR).max(1),
            })
        }
        Err(why) => {
            println!("AI tools off: {}", why);
            None
        }
    }
}

/// What one request used, in tokens (for the server log).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub input: u64,
    /// read from the prompt cache (cheaper than `input`)
    pub cached: u64,
    pub output: u64,
}

/// Sends one Messages API request (`body`, with "stream": true) and calls
/// `text` with each piece of the answer as it comes. `stop` says the answer
/// isn't wanted any more (the asker stopped it or left): the request ends
/// soon after, with `Err(AI_STOPPED)`. What the request used, or why it
/// failed: one of the AI_* sentences (the details go to the server log).
pub trait Claude: Send + Sync + std::fmt::Debug {
    fn stream(
        &self,
        body: &Value,
        text: &mut dyn FnMut(&str),
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Usage, String>;
}

/// Reads the answer's server-sent events, a line at a time.
#[derive(Debug, Default)]
pub struct Sse {
    pub usage: Usage,
    /// why the answer ended ("end_turn", "max_tokens", "refusal"...)
    pub stop: Option<String>,
    /// an error the service sent, in the stream or as the whole answer
    pub error: Option<String>,
}

impl Sse {
    /// One line of the answer; the text it adds, if any.
    pub fn line(&mut self, line: &str) -> Option<String> {
        let line = line.trim_end_matches(['\r', '\n']);
        // "event: ..." lines name what the data line says again in "type"
        let data = match line.strip_prefix("data:") {
            Some(d) => d.trim_start(),
            // an error answer is plain JSON, not events
            None if line.starts_with('{') => line,
            None => return None,
        };
        let v: Value = serde_json::from_str(data).ok()?;
        match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "message_start" => {
                let u = &v["message"]["usage"];
                let n = |k: &str| u.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                self.usage.input += n("input_tokens") + n("cache_creation_input_tokens");
                self.usage.cached += n("cache_read_input_tokens");
                None
            }
            "content_block_delta" if v["delta"]["type"] == "text_delta" => v["delta"]["text"]
                .as_str()
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string()),
            "message_delta" => {
                if let Some(s) = v["delta"]["stop_reason"].as_str() {
                    self.stop = Some(s.to_string());
                }
                if let Some(o) = v["usage"]["output_tokens"].as_u64() {
                    self.usage.output = o;
                }
                None
            }
            "error" => {
                let e = &v["error"];
                self.error = Some(format!(
                    "{}: {}",
                    e["type"].as_str().unwrap_or("error"),
                    e["message"].as_str().unwrap_or("")
                ));
                None
            }
            _ => None,
        }
    }
}

/// Anthropic's API through the system's curl.
#[derive(Debug)]
pub struct CurlClaude {
    key: String,
    url: String,
    /// the protocols curl may use: https only, but plain http for a test
    protos: &'static str,
}

/// Marks the line curl adds after the answer with its HTTP status.
const STATUS_MARK: &str = "wb-http-status ";

impl Claude for CurlClaude {
    fn stream(
        &self,
        body: &Value,
        text: &mut dyn FnMut(&str),
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Usage, String> {
        use std::io::BufRead;
        use std::sync::atomic::{AtomicBool, Ordering};
        let mut headers = vec![
            ("content-type", "application/json".to_string()),
            ("x-api-key", self.key.clone()),
            ("anthropic-version", API_VERSION.to_string()),
        ];
        if body.get("fallbacks").is_some() {
            headers.push(("anthropic-beta", FALLBACK_BETA.to_string()));
        }
        let config = crate::push::curl_config(&self.url, &headers, &body.to_string());
        let args = [
            "--silent",
            "--show-error",
            "--no-buffer",
            "--max-time",
            "600",
            "--proto",
            self.protos,
            "--config",
            "-",
            // (curl itself turns each \n here into a new line)
            "--write-out",
            "\\nwb-http-status %{http_code}\\n",
        ];
        let mut child = crate::curl::spawn("curl", &args, config.as_bytes()).map_err(|e| {
            eprintln!("AI: couldn't run curl: {}", e);
            AI_FAILED.to_string()
        })?;
        let out = child.stdout.take();
        let child = Mutex::new(child);
        let mut sse = Sse::default();
        let mut status = 0u16;
        let stopped = AtomicBool::new(false);
        let finished_reading = AtomicBool::new(false);
        std::thread::scope(|s| {
            // stops curl when the answer isn't wanted, even while the model
            // thinks and sends nothing
            s.spawn(|| {
                while !finished_reading.load(Ordering::Relaxed) {
                    if stop() {
                        stopped.store(true, Ordering::Relaxed);
                        let _ = child.locked().kill();
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
            if let Some(out) = out {
                for line in std::io::BufReader::new(out).lines() {
                    let Ok(line) = line else { break };
                    if let Some(code) = line.strip_prefix(STATUS_MARK) {
                        status = code.trim().parse().unwrap_or(0);
                        continue;
                    }
                    if let Some(t) = sse.line(&line) {
                        text(&t);
                    }
                }
            }
            finished_reading.store(true, Ordering::Relaxed);
        });
        let out = child
            .into_inner()
            .unwrap_or_else(|e| e.into_inner())
            .wait_with_output()
            .map_err(|e| e.to_string())?;
        if stopped.load(Ordering::Relaxed) {
            return Err(AI_STOPPED.into());
        }
        if status != 200 || sse.error.is_some() || !out.status.success() || sse.stop.is_none() {
            eprintln!(
                "AI: the request failed (HTTP {}, curl {:?}, ended {:?}): {} {}",
                status,
                out.status.code(),
                sse.stop,
                sse.error.as_deref().unwrap_or(""),
                String::from_utf8_lossy(&out.stderr).trim()
            );
            return Err(AI_FAILED.into());
        }
        finished(&sse)
    }
}

/// How an answer that came through ended: what it used, or why it doesn't count.
pub fn finished(sse: &Sse) -> Result<Usage, String> {
    match sse.stop.as_deref() {
        Some("refusal") => Err(AI_DECLINED.into()),
        _ => Ok(sse.usage),
    }
}

// ------------------------------------------------------------ requests

/// Models that take server-side fallbacks (a declined request is answered
/// by another model in the same call).
pub fn takes_fallbacks(model: &str) -> bool {
    matches!(
        model,
        "claude-opus-5-5" | "claude-opus-5" | "claude-sonnet-5-5"
    ) || model.starts_with("claude-fable-")
}

/// Models that take `output_config.effort` (all current ones).
fn takes_effort(model: &str) -> bool {
    !(model.contains("-4-5") || model.contains("-3-") || model.ends_with("-3"))
}

/// One streamed Messages API request.
pub fn request(
    model: &str,
    system: Vec<Value>,
    messages: Vec<Value>,
    effort: &str,
    format: Option<Value>,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": 64000,
        "stream": true,
        "system": system,
        "messages": messages,
    });
    let mut out = serde_json::Map::new();
    if takes_effort(model) {
        out.insert("effort".into(), json!(effort));
    }
    if let Some(f) = format {
        out.insert("format".into(), f);
    }
    if !out.is_empty() {
        body["output_config"] = Value::Object(out);
    }
    if takes_fallbacks(model) {
        body["fallbacks"] = json!("default");
    }
    body
}

fn text_block(s: impl Into<String>) -> Value {
    json!({"type": "text", "text": s.into()})
}

fn user(s: impl Into<String>) -> Value {
    json!({"role": "user", "content": s.into()})
}

/// The sentence that says which language to answer in.
fn answer_in(lang: &str) -> &'static str {
    match lang {
        "ko" => "Write your answer in Korean.",
        "en" => "Write your answer in English.",
        _ => "Write your answer in the language most of the material is in.",
    }
}

const ROLE: &str =
    "You are the AI assistant built into Workboard, a team app for tasks, chat and docs.";

/// At most `n` bytes of `s`, cut where a character ends (with "…" when cut).
fn cut_bytes(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut end = n;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// Characters of `s`, at most `n` of them (with "…" when cut).
fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

fn when(ts: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(ts, 0) {
        chrono::LocalResult::Single(t) => t.format("%Y-%m-%d %H:%M").to_string(),
        _ => String::new(),
    }
}

fn day(ts: i64) -> String {
    when(ts).split(' ').next().unwrap_or("").to_string()
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Who people are, by member id: the names the apps show.
struct Names(HashMap<u64, String>);

impl Names {
    fn of(copy: &Value) -> Names {
        Names(
            array_at(copy, "members")
                .iter()
                // a blank name reads as "person N" (see `get`)
                .filter_map(|m| {
                    let name = wb_sync::json::shown_name(m);
                    (!name.is_empty()).then_some((u64_field(m, "id")?, name))
                })
                .collect(),
        )
    }

    fn get(&self, id: u64) -> String {
        self.0
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("person {}", id))
    }

    fn list(&self, ids: &[Value]) -> String {
        ids.iter()
            .filter_map(|v| v.as_u64())
            .map(|i| self.get(i))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// What a message says, with its pictures and posts in words.
fn message_words(m: &Value) -> String {
    let mut s = text_field(m, "text");
    if let Some(t) = m.get("post_title").and_then(|t| t.as_str()) {
        s = format!("{} — {}", t, s);
    }
    if let Some(b) = m.get("forwarded") {
        s.push_str(&format!(" (forwarded: {})", text_field(b, "text")));
    }
    let n = array_at(m, "attachments").len();
    if n > 0 {
        s.push_str(&format!(" [{} attachment(s)]", n));
    }
    one_line(&s)
}

fn author(m: &Value, names: &Names) -> String {
    match m.get("bot").and_then(|b| b.as_str()) {
        Some(b) => b.to_string(),
        None => names.get(u64_field(m, "author").unwrap_or(0)),
    }
}

fn channel_title(c: &Value, names: &Names) -> String {
    if c["kind"] == "Direct" {
        format!(
            "Direct message between {}",
            names.list(array_at(c, "members"))
        )
    } else {
        format!("#{}", text_field(c, "name"))
    }
}

fn sorted_messages(c: &Value) -> Vec<&Value> {
    let mut m: Vec<&Value> = array_at(c, "messages").iter().collect();
    m.sort_by_key(|m| m.get("ts").and_then(|t| t.as_i64()).unwrap_or(0));
    m
}

/// A conversation's newest messages, oldest first, with their thread
/// replies under them, within SUMMARY_MESSAGES and SUMMARY_CHARS.
fn conversation_text(c: &Value, names: &Names) -> (String, usize, usize) {
    let all = sorted_messages(c);
    let mut lines: Vec<String> = vec![];
    let mut used = 0;
    let mut taken = 0;
    for m in all.iter().rev() {
        if taken >= SUMMARY_MESSAGES || used >= SUMMARY_CHARS {
            break;
        }
        // the channel's copy of a thread reply shows in its thread already
        if m.get("thread_of").is_some() {
            continue;
        }
        let mut block = format!(
            "[{}] {}: {}",
            when(m["ts"].as_i64().unwrap_or(0)),
            author(m, names),
            cut(&message_words(m), 4_000)
        );
        for r in array_at(m, "replies") {
            block.push_str(&format!(
                "\n    reply [{}] {}: {}",
                when(r["ts"].as_i64().unwrap_or(0)),
                author(r, names),
                cut(&message_words(r), 2_000)
            ));
        }
        used += block.len();
        lines.push(block);
        taken += 1;
    }
    lines.reverse();
    let total = all.iter().filter(|m| m.get("thread_of").is_none()).count();
    (lines.join("\n"), taken, total)
}

/// The request for `ask`, built from what `member` may see of the team's
/// copy (`copy`, already without what is hidden from them).
pub fn build(
    model: &str,
    copy: &Value,
    member: u64,
    ask: &AiAsk,
    lang: &str,
    now: i64,
) -> Result<(Value, &'static str), String> {
    let names = Names::of(copy);
    let lang_line = answer_in(lang);
    match ask {
        AiAsk::SummarizeChannel { channel } => {
            summarize_channel(model, copy, &names, lang_line, *channel)
        }
        AiAsk::SummarizeThread { channel, message } => {
            summarize_thread(model, copy, &names, lang_line, *channel, *message)
        }
        AiAsk::Write { how, text, title } => write(model, lang_line, how, text, title),
        AiAsk::Subtasks { name, description } => subtasks(model, lang_line, name, description),
        AiAsk::Question { question, history } => ask_question(
            model, copy, &names, member, lang_line, question, history, now,
        ),
    }
}

/// The conversation `id` in the copy.
fn find_channel(copy: &Value, id: u64) -> Result<&Value, String> {
    array_at(copy, "channels")
        .iter()
        .find(|c| u64_field(c, "id") == Some(id))
        .ok_or_else(|| AI_NOT_FOUND.to_string())
}

/// A summary of a conversation's newest messages.
fn summarize_channel(
    model: &str,
    copy: &Value,
    names: &Names,
    lang_line: &str,
    channel: u64,
) -> Result<(Value, &'static str), String> {
    let c = find_channel(copy, channel)?;
    let (text, taken, total) = conversation_text(c, names);
    if taken == 0 {
        return Err(AI_EMPTY.into());
    }
    let note = if taken < total {
        format!(
            "\n(Only the newest {} of its {} messages are here.)",
            taken, total
        )
    } else {
        String::new()
    };
    let system = format!(
        "{} You sum up a team chat conversation for someone catching up on it. Lead with what matters: decisions made, open questions, requests and who they are for, and dates or deadlines mentioned. Use short bullet points (\"- \"), under a few bold headings only when the conversation covers several topics. Mention people by name. Say nothing that isn't in the messages. {}",
        ROLE, lang_line
    );
    let prompt = format!(
        "Conversation: {}{}\nMessages, oldest first (replies in a thread are indented under the message they answer):\n\n{}",
        channel_title(c, names),
        note,
        text
    );
    Ok((
        request(
            model,
            vec![text_block(system)],
            vec![user(prompt)],
            "low",
            None,
        ),
        "conversation summary",
    ))
}

/// A summary of one thread.
fn summarize_thread(
    model: &str,
    copy: &Value,
    names: &Names,
    lang_line: &str,
    channel: u64,
    message: u64,
) -> Result<(Value, &'static str), String> {
    let c = find_channel(copy, channel)?;
    let m = array_at(c, "messages")
        .iter()
        .find(|m| u64_field(m, "id") == Some(message))
        .ok_or_else(|| AI_NOT_FOUND.to_string())?;
    let mut text = format!(
        "[{}] {}: {}",
        when(m["ts"].as_i64().unwrap_or(0)),
        author(m, names),
        cut(&message_words(m), 8_000)
    );
    let mut used = text.len();
    for r in array_at(m, "replies") {
        if used > SUMMARY_CHARS {
            text.push_str("\n(The rest of the replies are left out.)");
            break;
        }
        let line = format!(
            "\n[{}] {}: {}",
            when(r["ts"].as_i64().unwrap_or(0)),
            author(r, names),
            cut(&message_words(r), 4_000)
        );
        used += line.len();
        text.push_str(&line);
    }
    let system = format!(
        "{} You sum up one thread of a team chat: what it is about, where it landed, and what is still open or waiting on someone. Use a few short bullet points (\"- \"). Mention people by name. Say nothing that isn't in the messages. {}",
        ROLE, lang_line
    );
    let prompt = format!(
        "A thread in {}. The first message, then the replies in order:\n\n{}",
        channel_title(c, names),
        text
    );
    Ok((
        request(
            model,
            vec![text_block(system)],
            vec![user(prompt)],
            "low",
            None,
        ),
        "thread summary",
    ))
}

/// Rewriting, translating or drafting a piece of writing (`how`, one of AI_WRITE).
fn write(
    model: &str,
    lang_line: &str,
    how: &str,
    text: &str,
    title: &str,
) -> Result<(Value, &'static str), String> {
    if !AI_WRITE.contains(&how) {
        return Err(AI_UNKNOWN.into());
    }
    if text.chars().count() + title.chars().count() > AI_MAX_TEXT {
        return Err(AI_TOO_LONG.into());
    }
    if text.trim().is_empty() && !(how == "draft" && !title.trim().is_empty()) {
        return Err(AI_EMPTY.into());
    }
    let task = match how {
        "improve" => "Improve the writing: clearer and easier to read, with the same meaning, tone and language.",
        "fix" => "Fix the spelling, grammar and punctuation. Change nothing else.",
        "shorter" => "Make it shorter, keeping the key points, in the same language.",
        "longer" => "Make it longer and more detailed, in the same language, without making up facts.",
        "summarize" => "Sum it up in a few short bullet points, in the same language.",
        "korean" => "Translate it into Korean.",
        "english" => "Translate it into English.",
        "continue" => "Continue writing from where it ends, in the same language and style. Give only the new text.",
        _ => "Write a first draft of it from its title (and any notes): what it is for, what to do, and when it is done.",
    };
    let lang_note = if how == "draft" { lang_line } else { "" };
    let system = format!(
        "{} You work on a piece of writing in a team workspace. It may use Markdown (# headings, - lists, - [ ] checklists, **bold**); keep that formatting. Reply with only the resulting text: no introduction, no quotation marks around it, no explanation. {}",
        ROLE, lang_note
    );
    let mut prompt = format!("{}\n\n", task);
    if !title.trim().is_empty() {
        prompt.push_str(&format!("Title: {}\n\n", one_line(title)));
    }
    if !text.trim().is_empty() {
        prompt.push_str(&format!("The text:\n<text>\n{}\n</text>", text));
    }
    let effort = if matches!(how, "fix" | "korean" | "english") {
        "low"
    } else {
        "medium"
    };
    Ok((
        request(
            model,
            vec![text_block(system)],
            vec![user(prompt)],
            effort,
            None,
        ),
        "writing",
    ))
}

/// Subtasks suggested for a task.
fn subtasks(
    model: &str,
    lang_line: &str,
    name: &str,
    description: &str,
) -> Result<(Value, &'static str), String> {
    if name.trim().is_empty() {
        return Err(AI_EMPTY.into());
    }
    if name.chars().count() + description.chars().count() > AI_MAX_TEXT {
        return Err(AI_TOO_LONG.into());
    }
    let system = format!(
        "{} You break a task into subtasks for a team's task list. Suggest 3 to 8 concrete subtasks in a sensible order, each a short name that starts with what to do. Don't repeat the task itself. {}",
        ROLE, lang_line
    );
    let mut prompt = format!("Task: {}", one_line(name));
    if !description.trim().is_empty() {
        prompt.push_str(&format!("\nDescription:\n{}", description));
    }
    let format = json!({
        "type": "json_schema",
        "schema": {
            "type": "object",
            "properties": {
                "subtasks": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["subtasks"],
            "additionalProperties": false
        }
    });
    Ok((
        request(
            model,
            vec![text_block(system)],
            vec![user(prompt)],
            "low",
            Some(format),
        ),
        "subtasks",
    ))
}

/// A question about the workspace, with the conversation so far.
#[allow(clippy::too_many_arguments)]
fn ask_question(
    model: &str,
    copy: &Value,
    names: &Names,
    member: u64,
    lang_line: &str,
    question: &str,
    history: &[AiTurn],
    now: i64,
) -> Result<(Value, &'static str), String> {
    let q = question.trim();
    if q.is_empty() {
        return Err(AI_EMPTY.into());
    }
    if q.chars().count() > AI_MAX_TEXT {
        return Err(AI_TOO_LONG.into());
    }
    // the newest turns, as many as fit: the oldest go first
    let mut history: Vec<&AiTurn> = history.iter().rev().take(MAX_HISTORY).rev().collect();
    let size = |t: &AiTurn| t.question.chars().count() + t.answer.chars().count();
    while q.chars().count() + history.iter().map(|t| size(t)).sum::<usize>() > AI_MAX_TEXT {
        history.remove(0);
    }
    let ws = Workspace::read(copy, names);
    let overview = ws.overview(CONTEXT_CHARS / 2);
    // what the question (and the one before, for "and what about...")
    // is about
    let mut about = q.to_string();
    if let Some(t) = history.last() {
        about.push(' ');
        about.push_str(&t.question);
    }
    let details = ws.details(
        &about,
        CONTEXT_CHARS - overview.len().min(CONTEXT_CHARS / 2),
        now,
    );
    let system = format!(
        "{} You answer questions about a team's workspace (its tasks, docs and chat) for {}, from the workspace information below; they can see all of it. If it doesn't hold the answer, say so plainly rather than guessing. Be brief: a direct answer first, then details as short bullet points (\"- \") when they help. When you mention a task, doc or message from the workspace, put its marker right after it exactly as written below, like [task:123], [doc:45] or [msg:6:789]; the app turns markers into links. Today is {}. {}",
        ROLE,
        names.get(member),
        chrono::Local::now().format("%Y-%m-%d (%A)"),
        lang_line
    );
    let mut messages = vec![];
    for t in history {
        messages.push(user(t.question.clone()));
        messages.push(json!({"role": "assistant", "content": t.answer.clone()}));
    }
    messages.push(user(format!(
        "Workspace items that match the question:\n\n{}\n\nQuestion: {}",
        if details.is_empty() {
            "(none)".to_string()
        } else {
            details
        },
        q
    )));
    let mut ov = text_block(format!("Overview of the workspace:\n\n{}", overview));
    ov["cache_control"] = json!({"type": "ephemeral"});
    Ok((
        request(
            model,
            vec![text_block(system), ov],
            messages,
            "medium",
            None,
        ),
        "question",
    ))
}

// --------------------------------------------- the workspace for questions

/// One task, doc or message a question may need, with the text it is
/// matched against.
struct Item {
    text: String,
    /// the words it is matched by, lower case
    search: String,
    /// when it was written or last changed
    at: i64,
    message: bool,
}

/// What a question reads of the workspace.
struct Workspace<'a> {
    copy: &'a Value,
    names: &'a Names,
    /// list id -> "Space / Folder / List"
    lists: HashMap<u64, String>,
    items: Vec<Item>,
}

impl<'a> Workspace<'a> {
    fn read(copy: &'a Value, names: &'a Names) -> Workspace<'a> {
        let mut lists = HashMap::new();
        for s in array_at(copy, "spaces") {
            let sn = text_field(s, "name");
            for l in array_at(s, "lists") {
                if let Some(id) = u64_field(l, "id") {
                    lists.insert(id, format!("{} / {}", sn, text_field(l, "name")));
                }
            }
            for f in array_at(s, "folders") {
                let fname = text_field(f, "name");
                for l in array_at(f, "lists") {
                    if let Some(id) = u64_field(l, "id") {
                        lists.insert(
                            id,
                            format!("{} / {} / {}", sn, fname, text_field(l, "name")),
                        );
                    }
                }
            }
        }
        let mut ws = Workspace {
            copy,
            names,
            lists,
            items: vec![],
        };
        ws.items = ws.collect();
        ws
    }

    fn tasks(&self) -> impl Iterator<Item = &'a Value> {
        array_at(self.copy, "tasks")
            .iter()
            .filter(|t| t["archived"] != true)
    }

    fn task_line(&self, t: &Value) -> String {
        let mut parts = vec![
            format!(
                "[task:{}] {}",
                u64_field(t, "id").unwrap_or(0),
                one_line(&text_field(t, "name"))
            ),
            format!("status {}", text_field(t, "status")),
        ];
        if let Some(l) = u64_field(t, "list_id").and_then(|l| self.lists.get(&l)) {
            parts.push(format!("in {}", l));
        }
        let who = self.names.list(array_at(t, "assignees"));
        if !who.is_empty() {
            parts.push(format!("assigned to {}", who));
        }
        if let Some(d) = t.get("due").and_then(|d| d.as_str()) {
            parts.push(format!("due {}", d));
        }
        match u64_field(t, "priority") {
            Some(1) => parts.push("urgent".into()),
            Some(2) => parts.push("high priority".into()),
            _ => {}
        }
        if let Some(p) = u64_field(t, "parent") {
            parts.push(format!("subtask of [task:{}]", p));
        }
        if t["milestone"] == true {
            parts.push("milestone".into());
        }
        parts.join(" | ")
    }

    /// Everyone, every place, and every task, doc and conversation in a
    /// line each, within `budget` characters (closed tasks go first when it
    /// runs out).
    fn overview(&self, budget: usize) -> String {
        let mut out = format!(
            "Workspace: {}\n\nPeople:\n",
            text_field(self.copy, "workspace")
        );
        for m in array_at(self.copy, "members") {
            let id = u64_field(m, "id").unwrap_or(0);
            let mut line = format!("- {}", self.names.get(id));
            let full = text_field(m, "name");
            if full != self.names.get(id) && !full.is_empty() {
                line.push_str(&format!(" ({})", full));
            }
            for k in ["title", "role", "email", "status_text"] {
                let v = text_field(m, k);
                if !v.is_empty() {
                    line.push_str(&format!(", {}", one_line(&v)));
                }
            }
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("\nLists (Space / Folder / List):\n");
        let mut lists: Vec<&String> = self.lists.values().collect();
        lists.sort();
        for l in lists {
            out.push_str(&format!("- {}\n", l));
        }
        // open tasks before closed ones, each newest first
        let mut tasks: Vec<&Value> = self.tasks().collect();
        let closed = |t: &Value| t.get("done_at").is_some_and(|d| !d.is_null());
        tasks.sort_by_key(|t| {
            (
                closed(t),
                std::cmp::Reverse(t["updated"].as_i64().unwrap_or(0)),
            )
        });
        let mut docs = String::from("\nDocs:\n");
        for d in array_at(self.copy, "docs")
            .iter()
            .filter(|d| d["archived"] != true)
        {
            docs.push_str(&format!(
                "- [doc:{}] {} (updated {})\n",
                u64_field(d, "id").unwrap_or(0),
                one_line(&text_field(d, "title")),
                day(d["updated"].as_i64().unwrap_or(0))
            ));
        }
        let mut chats = String::from("\nConversations:\n");
        for c in array_at(self.copy, "channels") {
            let msgs = sorted_messages(c);
            chats.push_str(&format!(
                "- {} ({} messages{})\n",
                channel_title(c, self.names),
                msgs.len(),
                msgs.last()
                    .map(|m| format!(", last on {}", day(m["ts"].as_i64().unwrap_or(0))))
                    .unwrap_or_default()
            ));
        }
        // docs and conversations take at most a sixth each; tasks the rest
        let docs = cut_bytes(&docs, budget / 6);
        let chats = cut_bytes(&chats, budget / 6);
        let room = budget.saturating_sub(out.len() + docs.len() + chats.len());
        let mut lines = String::from("\nTasks (one a line; open ones first):\n");
        let mut left = 0;
        for t in &tasks {
            let line = format!("- {}\n", self.task_line(t));
            if lines.len() + line.len() > room {
                left += 1;
                continue;
            }
            lines.push_str(&line);
        }
        if left > 0 {
            lines.push_str(&format!("({} more tasks left out)\n", left));
        }
        out.push_str(&lines);
        out.push_str(&docs);
        out.push_str(&chats);
        out
    }

    /// Every task, doc and message, written out for the details.
    fn collect(&self) -> Vec<Item> {
        let mut items = vec![];
        for t in self.tasks() {
            let mut text = self.task_line(t);
            let desc = text_field(t, "description");
            if !desc.trim().is_empty() {
                text.push_str(&format!("\nDescription:\n{}", cut(desc.trim(), 4_000)));
            }
            let checks: Vec<String> = array_at(t, "checklist")
                .iter()
                .map(|c| {
                    format!(
                        "[{}] {}",
                        if c["done"] == true { "x" } else { " " },
                        one_line(&text_field(c, "text"))
                    )
                })
                .collect();
            if !checks.is_empty() {
                text.push_str(&format!("\nChecklist: {}", checks.join("; ")));
            }
            let comments: Vec<&Value> = array_at(t, "activity")
                .iter()
                .filter(|a| a["comment"] == true)
                .collect();
            if !comments.is_empty() {
                text.push_str("\nComments:");
                for a in comments.iter().rev().take(10).rev() {
                    text.push_str(&format!(
                        "\n- {} ({}): {}",
                        self.names.get(u64_field(a, "author").unwrap_or(0)),
                        day(a["ts"].as_i64().unwrap_or(0)),
                        cut(&one_line(&text_field(a, "text")), 1_000)
                    ));
                }
            }
            let tags: Vec<&str> = array_at(t, "tags")
                .iter()
                .filter_map(|x| x.as_str())
                .collect();
            if !tags.is_empty() {
                text.push_str(&format!("\nTags: {}", tags.join(", ")));
            }
            items.push(Item {
                search: text.to_lowercase(),
                text,
                at: t["updated"].as_i64().unwrap_or(0),
                message: false,
            });
        }
        for d in array_at(self.copy, "docs")
            .iter()
            .filter(|d| d["archived"] != true)
        {
            let text = format!(
                "[doc:{}] {} (updated {})\n{}",
                u64_field(d, "id").unwrap_or(0),
                one_line(&text_field(d, "title")),
                day(d["updated"].as_i64().unwrap_or(0)),
                cut(text_field(d, "body").trim(), 6_000)
            );
            items.push(Item {
                search: text.to_lowercase(),
                text,
                at: d["updated"].as_i64().unwrap_or(0),
                message: false,
            });
        }
        for c in array_at(self.copy, "channels") {
            let ch = u64_field(c, "id").unwrap_or(0);
            let title = channel_title(c, self.names);
            for m in array_at(c, "messages") {
                if m.get("thread_of").is_some() {
                    continue;
                }
                for (msg, reply) in std::iter::once((m, false))
                    .chain(array_at(m, "replies").iter().map(|r| (r, true)))
                {
                    let ts = msg["ts"].as_i64().unwrap_or(0);
                    let text = format!(
                        "[msg:{}:{}] {}{} · {} · {}: {}",
                        ch,
                        u64_field(msg, "id").unwrap_or(0),
                        title,
                        if reply { " (thread reply)" } else { "" },
                        when(ts),
                        author(msg, self.names),
                        cut(&message_words(msg), 2_000)
                    );
                    items.push(Item {
                        search: text.to_lowercase(),
                        text,
                        at: ts,
                        message: true,
                    });
                }
            }
        }
        items
    }

    /// The items that match `about` best, then the newest messages, within
    /// `budget` characters.
    fn details(&self, about: &str, budget: usize, now: i64) -> String {
        let words = terms(about);
        let n = self.items.len().max(1) as f64;
        // rarer words count for more
        let weight: Vec<f64> = words
            .iter()
            .map(|w| {
                let df = self
                    .items
                    .iter()
                    .filter(|i| i.search.contains(w.as_str()))
                    .count();
                (1.0 + n / (1.0 + df as f64)).ln()
            })
            .collect();
        let mut scored: Vec<(f64, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                let s: f64 = words
                    .iter()
                    .zip(&weight)
                    .filter(|(w, _)| it.search.contains(w.as_str()))
                    .map(|(_, k)| k)
                    .sum();
                (s > 0.0).then_some((s, i))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then(self.items[b.1].at.cmp(&self.items[a.1].at))
        });
        let mut out = String::new();
        let mut taken = HashSet::new();
        for (_, i) in scored {
            let t = &self.items[i].text;
            if out.len() + t.len() > budget {
                continue;
            }
            out.push_str(t);
            out.push_str("\n\n");
            taken.insert(i);
        }
        // and what was said lately, for "what's new" and the like
        let week = now - 7 * 86_400;
        let mut recent: Vec<usize> = (0..self.items.len())
            .filter(|i| {
                let it = &self.items[*i];
                it.message && it.at >= week && !taken.contains(i)
            })
            .collect();
        recent.sort_by_key(|i| std::cmp::Reverse(self.items[*i].at));
        let mut lately = String::new();
        for i in recent {
            let t = &self.items[i].text;
            if out.len() + lately.len() + t.len() > budget {
                break;
            }
            lately.push_str(t);
            lately.push('\n');
        }
        if !lately.is_empty() {
            out.push_str("Messages of the last week (newest first):\n");
            out.push_str(&lately);
        }
        out.trim_end().to_string()
    }
}

/// Endings Korean adds to a word ("프로젝트는" is "프로젝트"), longest first.
const PARTICLES: [&str; 22] = [
    "에서는",
    "으로는",
    "이랑",
    "에서",
    "으로",
    "에게",
    "한테",
    "까지",
    "부터",
    "이나",
    "은",
    "는",
    "이",
    "가",
    "을",
    "를",
    "에",
    "의",
    "로",
    "와",
    "과",
    "도",
];

/// Words that say nothing about what a question is about.
const STOP: &[&str] = &[
    "the",
    "and",
    "for",
    "are",
    "was",
    "were",
    "what",
    "who",
    "when",
    "where",
    "how",
    "why",
    "did",
    "does",
    "about",
    "with",
    "this",
    "that",
    "any",
    "our",
    "has",
    "have",
    "from",
    "which",
    "there",
    "can",
    "you",
    "tell",
    "me",
    "is",
    "of",
    "to",
    "in",
    "on",
    "an",
    "it",
    "do",
    "we",
    "my",
    "be",
    "at",
    "or",
    "뭐",
    "무엇",
    "어떤",
    "누가",
    "누구",
    "언제",
    "어디",
    "어떻게",
    "알려줘",
    "알려주세요",
    "있어",
    "있나요",
    "해줘",
    "관련",
];

/// What a question is about: its words, lower case, without Korean endings
/// and words that say nothing.
pub fn terms(q: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for w in q
        .split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
    {
        let mut w = w;
        for p in PARTICLES {
            if let Some(stem) = w.strip_suffix(p) {
                if stem.chars().count() >= 2 {
                    w = stem.to_string();
                    break;
                }
            }
        }
        if w.chars().count() < 2 || STOP.contains(&w.as_str()) || out.contains(&w) {
            continue;
        }
        out.push(w);
    }
    out
}

// --------------------------------------------------- who may ask how much

/// Requests per person: when each started in the last hour, and how many
/// are going now.
#[derive(Debug, Default)]
pub struct Allowance {
    started: Mutex<HashMap<u64, Vec<Instant>>>,
    going: Mutex<HashMap<u64, usize>>,
}

/// One request going; dropping it frees its place.
pub struct Ticket {
    allowance: Arc<Allowance>,
    member: u64,
    /// when it started (what the hour counts)
    at: Instant,
}

impl Ticket {
    /// A request turned down before it went: it doesn't count for the hour.
    pub fn refund(self) {
        let mut started = self.allowance.started.locked();
        if let Some(list) = started.get_mut(&self.member) {
            if let Some(i) = list.iter().rposition(|t| *t == self.at) {
                list.remove(i);
            }
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut g = self.allowance.going.locked();
        if let Some(n) = g.get_mut(&self.member) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                g.remove(&self.member);
            }
        }
    }
}

impl Allowance {
    /// A place for a new request of `member`, or why there is none.
    pub fn take(self: &Arc<Self>, member: u64, per_hour: u32) -> Result<Ticket, String> {
        let mut going = self.going.locked();
        if going.get(&member).copied().unwrap_or(0) >= AT_ONCE {
            return Err(AI_BUSY.into());
        }
        let mut started = self.started.locked();
        let list = started.entry(member).or_default();
        list.retain(|t| t.elapsed() < Duration::from_secs(3600));
        if list.len() >= per_hour as usize {
            return Err(AI_TOO_MANY.into());
        }
        let at = Instant::now();
        list.push(at);
        *going.entry(member).or_default() += 1;
        Ok(Ticket {
            allowance: self.clone(),
            member,
            at,
        })
    }
}

// ---------------------------------------------------- one workspace's AI

/// A request ready to go: who answers, what to send, and its place in the
/// asker's allowance (held until it is answered).
pub struct Started {
    pub claude: Arc<dyn Claude>,
    pub body: Value,
    /// what kind of request it is, for the server log
    pub kind: &'static str,
    pub ticket: Ticket,
}

impl crate::Hub {
    /// The model this server's AI tools use; None when they are off.
    pub(crate) fn ai_model(&self) -> Option<String> {
        self.ai.as_ref().map(|a| a.model.clone())
    }

    /// Gets AI request `ask` of `member` ready, from what they may see of
    /// the team's copy (the same as their own copy), or says why not.
    pub(crate) fn ai_start(&self, member: u64, ask: &AiAsk, lang: &str) -> Result<Started, String> {
        let Some(setup) = self.ai.clone() else {
            return Err(AI_OFF.into());
        };
        // the allowance first: a flood of requests is turned down before
        // any of them copies the workspace
        let ticket = self.ai_allowance.take(member, setup.per_hour)?;
        // copied out of the lock: building a question's context takes a moment
        let copy = {
            let g = self.state();
            match g.canon.as_ref() {
                Some(canon) => {
                    let manages = g.saved.role_of(member).manages();
                    Some(match crate::canon::copy_for(canon, member, manages) {
                        Some((c, _)) => c,
                        None => canon.clone(),
                    })
                }
                None => None,
            }
        };
        let now = chrono::Utc::now().timestamp();
        let built = match &copy {
            Some(copy) => build(&setup.model, copy, member, ask, lang, now),
            None => Err(AI_EMPTY.to_string()),
        };
        let (body, kind) = match built {
            Ok(b) => b,
            Err(why) => {
                // turned down for what it asks: it doesn't count
                ticket.refund();
                return Err(why);
            }
        };
        Ok(Started {
            claude: setup.claude,
            body,
            kind,
            ticket,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_go_by_the_shown_name_and_a_blank_one_reads_as_person_n() {
        let names = Names::of(&serde_json::json!({"members": [
            {"id": 3, "name": "  ", "display_name": " "},
            {"id": 4, "name": " Kim Sam ", "display_name": ""},
            {"id": 5, "name": "Lee Bora", "display_name": " Bora "}
        ]}));
        assert_eq!(names.get(3), "person 3");
        assert_eq!(names.get(4), "Kim Sam");
        assert_eq!(names.get(5), "Bora");
    }

    #[test]
    fn the_answer_streams_in_pieces_and_says_how_it_ended() {
        let mut s = Sse::default();
        let lines = [
            "event: message_start",
            r#"data: {"type":"message_start","message":{"id":"m","usage":{"input_tokens":120,"cache_read_input_tokens":30,"cache_creation_input_tokens":5,"output_tokens":1}}}"#,
            "",
            "event: content_block_start",
            r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":""}}"#,
            r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Hello"}}"#,
            "event: ping",
            r#"data: {"type": "ping"}"#,
            r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":" there"}}"#,
            r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":12}}"#,
            r#"data: {"type":"message_stop"}"#,
        ];
        let text: String = lines.iter().filter_map(|l| s.line(l)).collect();
        assert_eq!(text, "Hello there", "thinking is left out");
        assert_eq!(
            s.usage,
            Usage {
                input: 125,
                cached: 30,
                output: 12
            }
        );
        assert_eq!(finished(&s), Ok(s.usage));

        // declined: the answer doesn't count
        let mut s = Sse::default();
        s.line(r#"data: {"type":"message_delta","delta":{"stop_reason":"refusal"},"usage":{"output_tokens":0}}"#);
        assert_eq!(finished(&s), Err(AI_DECLINED.to_string()));

        // an error in the stream, and an error that is the whole answer
        let mut s = Sse::default();
        s.line(
            r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        );
        assert_eq!(s.error.as_deref(), Some("overloaded_error: Overloaded"));
        let mut s = Sse::default();
        s.line(r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#);
        assert!(s.error.unwrap().contains("authentication_error"));
    }

    #[test]
    fn requests_stream_and_fall_back_on_models_that_can() {
        let b = request(
            "claude-opus-5-5",
            vec![text_block("s")],
            vec![user("q")],
            "low",
            None,
        );
        assert_eq!(b["stream"], true);
        assert_eq!(b["fallbacks"], "default");
        assert_eq!(b["output_config"], json!({"effort": "low"}));
        assert!(b.get("thinking").is_none() && b.get("temperature").is_none());
        let b = request("claude-haiku-5-5", vec![], vec![], "low", None);
        assert!(b.get("fallbacks").is_none());
        let b = request("claude-haiku-4-5", vec![], vec![], "low", None);
        assert!(
            b.get("output_config").is_none(),
            "no effort on older models"
        );
    }

    #[test]
    fn a_question_is_about_its_words_without_korean_endings() {
        assert_eq!(
            terms("지난주 디자인 프로젝트는 누가 맡았어?"),
            ["지난주", "디자인", "프로젝트", "맡았어"]
        );
        assert_eq!(
            terms("What is the status of the Launch plan?"),
            ["status", "launch", "plan"]
        );
        // a short word keeps its ending (이 is the whole word)
        assert_eq!(terms("이 일"), Vec::<String>::new());
    }

    fn workspace() -> Value {
        json!({
            "workspace": "Team",
            "members": [
                {"id": 1, "name": "Alex Kim", "display_name": "", "role": "Owner"},
                {"id": 2, "name": "Bora Lee", "display_name": "Bora", "title": "Designer"}
            ],
            "spaces": [{"id": 10, "name": "Team Space", "lists": [{"id": 11, "name": "Launch"}],
                        "folders": [{"id": 12, "name": "Q4", "lists": [{"id": 13, "name": "Ads"}]}]}],
            "tasks": [
                {"id": 100, "list_id": 11, "name": "Design the landing page", "status": "in progress",
                 "assignees": [2], "due": "2026-11-30", "priority": 2, "description": "Hero, pricing and FAQ",
                 "activity": [{"ts": 1, "author": 1, "text": "Use the new logo", "comment": true}],
                 "updated": 5},
                {"id": 101, "list_id": 13, "name": "Buy ads", "status": "to do", "assignees": [], "updated": 4},
                {"id": 102, "list_id": 13, "name": "Old thing", "status": "to do", "archived": true, "updated": 3}
            ],
            "docs": [{"id": 200, "title": "Launch plan", "body": "We launch on December 3.", "updated": 6}],
            "channels": [{"id": 300, "name": "general", "kind": "Channel", "members": [1, 2], "messages": [
                {"id": 301, "author": 1, "text": "Is the landing page ready?", "ts": 1000,
                 "replies": [{"id": 302, "author": 2, "text": "Almost, pricing left", "ts": 1100}]},
                {"id": 303, "author": 2, "text": "Lunch?", "ts": 2000}
            ]}]
        })
    }

    #[test]
    fn a_question_takes_an_overview_and_what_matches_it() {
        let copy = workspace();
        let (body, kind) = build(
            "claude-opus-5-5",
            &copy,
            1,
            &AiAsk::Question {
                question: "When is the launch?".into(),
                history: vec![AiTurn {
                    question: "Who does the landing page?".into(),
                    answer: "Bora [task:100]".into(),
                }],
            },
            "ko",
            3000,
        )
        .unwrap();
        assert_eq!(kind, "question");
        let system = body["system"][0]["text"].as_str().unwrap();
        assert!(system.contains("for Alex Kim") && system.contains("Korean"));
        let overview = body["system"][1]["text"].as_str().unwrap();
        assert_eq!(body["system"][1]["cache_control"]["type"], "ephemeral");
        assert!(overview.contains("- Bora (Bora Lee), Designer"));
        assert!(overview.contains("Team Space / Q4 / Ads"));
        assert!(overview.contains(
            "[task:100] Design the landing page | status in progress | in Team Space / Launch | assigned to Bora | due 2026-11-30 | high priority"
        ));
        assert!(!overview.contains("Old thing"), "archived tasks stay out");
        assert!(overview.contains("[doc:200] Launch plan"));
        assert!(overview.contains("#general (2 messages"));
        // the earlier turn, then the details and the question
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1]["role"], "assistant");
        let last = msgs[2]["content"].as_str().unwrap();
        assert!(last.ends_with("Question: When is the launch?"));
        assert!(last.contains("We launch on December 3."), "the doc matches");
        assert!(
            last.contains("Hero, pricing and FAQ"),
            "the earlier question's task too"
        );
        assert!(
            last.contains("[msg:300:302]"),
            "replies are items of their own"
        );
    }

    /// A long conversation keeps its newest turns that fit; only a question
    /// too long by itself is turned down.
    #[test]
    fn a_long_conversation_keeps_its_newest_turns() {
        let copy = workspace();
        let turn = |i: usize, n: usize| AiTurn {
            question: format!("question {}", i),
            answer: "a".repeat(n),
        };
        let ask = |history: Vec<AiTurn>, q: String| {
            build(
                "m",
                &copy,
                1,
                &AiAsk::Question {
                    question: q,
                    history,
                },
                "en",
                0,
            )
        };
        let history: Vec<AiTurn> = (0..14).map(|i| turn(i, 20_000)).collect();
        let (b, _) = ask(history, "and now?".into()).unwrap();
        let msgs = b["messages"].as_array().unwrap();
        // two turns of 20,000 fit beside the question, the newest
        assert_eq!(msgs.len(), 2 * 2 + 1);
        assert_eq!(msgs[0]["content"], "question 12");
        assert_eq!(msgs[2]["content"], "question 13");
        let history: Vec<AiTurn> = (0..14).map(|i| turn(i, 10)).collect();
        let (b, _) = ask(history, "and now?".into()).unwrap();
        assert_eq!(b["messages"].as_array().unwrap().len(), MAX_HISTORY * 2 + 1);
        assert_eq!(
            ask(vec![], "x".repeat(AI_MAX_TEXT + 1)).unwrap_err(),
            AI_TOO_LONG
        );
    }

    /// Lots of docs don't push the tasks out of a question's overview.
    #[test]
    fn many_docs_leave_room_for_the_tasks() {
        let mut copy = workspace();
        let docs: Vec<Value> = (0..3000)
            .map(|i| json!({"id": 1000 + i, "title": format!("문서 제목 {}", i), "body": "", "updated": 1}))
            .collect();
        copy["docs"] = json!(docs);
        let names = Names::of(&copy);
        let ws = Workspace::read(&copy, &names);
        let budget = 30_000;
        let o = ws.overview(budget);
        assert!(o.len() <= budget + 100, "{}", o.len());
        assert!(o.contains("[task:100] Design the landing page"));
        assert!(o.contains("[task:101] Buy ads"));
        assert!(o.ends_with('\n') || o.ends_with('…'));
    }

    #[test]
    fn summaries_read_the_conversation_with_its_threads() {
        let copy = workspace();
        let (body, _) = build(
            "m",
            &copy,
            1,
            &AiAsk::SummarizeChannel { channel: 300 },
            "en",
            0,
        )
        .unwrap();
        let p = body["messages"][0]["content"].as_str().unwrap();
        assert!(p.starts_with("Conversation: #general"));
        let a = p.find("Alex Kim: Is the landing page ready?").unwrap();
        let b = p.find("    reply [").unwrap();
        let c = p.find("Bora: Lunch?").unwrap();
        assert!(a < b && b < c, "oldest first, replies under their message");
        let (body, _) = build(
            "m",
            &copy,
            1,
            &AiAsk::SummarizeThread {
                channel: 300,
                message: 301,
            },
            "en",
            0,
        )
        .unwrap();
        let p = body["messages"][0]["content"].as_str().unwrap();
        assert!(p.contains("Almost, pricing left") && !p.contains("Lunch"));
        for ask in [
            AiAsk::SummarizeChannel { channel: 9 },
            AiAsk::SummarizeThread {
                channel: 300,
                message: 9,
            },
        ] {
            assert_eq!(
                build("m", &copy, 1, &ask, "en", 0).unwrap_err(),
                AI_NOT_FOUND
            );
        }
    }

    #[test]
    fn writing_and_subtasks_ask_for_just_the_result() {
        let copy = workspace();
        let w = |how: &str, text: &str, title: &str| {
            build(
                "m",
                &copy,
                1,
                &AiAsk::Write {
                    how: how.into(),
                    text: text.into(),
                    title: title.into(),
                },
                "ko",
                0,
            )
        };
        let (b, _) = w("fix", "teh plan", "").unwrap();
        let p = b["messages"][0]["content"].as_str().unwrap();
        assert!(p.starts_with("Fix the spelling") && p.contains("<text>\nteh plan\n</text>"));
        assert!(
            !b["system"][0]["text"].as_str().unwrap().contains("Korean"),
            "fixing keeps the text's own language"
        );
        let (b, _) = w("draft", "", "Plan the offsite").unwrap();
        assert!(b["system"][0]["text"].as_str().unwrap().contains("Korean"));
        assert_eq!(w("draft", "", "").unwrap_err(), AI_EMPTY);
        assert_eq!(w("improve", "  ", "").unwrap_err(), AI_EMPTY);
        assert_eq!(w("rhyme", "x", "").unwrap_err(), AI_UNKNOWN);
        assert_eq!(
            w("fix", &"a".repeat(AI_MAX_TEXT + 1), "").unwrap_err(),
            AI_TOO_LONG
        );
        let (b, _) = build(
            "m",
            &copy,
            1,
            &AiAsk::Subtasks {
                name: "Plan the offsite".into(),
                description: String::new(),
            },
            "en",
            0,
        )
        .unwrap();
        assert_eq!(b["output_config"]["format"]["type"], "json_schema");
        assert_eq!(
            b["output_config"]["format"]["schema"]["required"],
            json!(["subtasks"])
        );
    }

    #[test]
    fn one_person_may_ask_so_much_an_hour_and_so_much_at_once() {
        let a = Arc::new(Allowance::default());
        let t1 = a.take(1, 10).unwrap();
        let _t2 = a.take(1, 10).unwrap();
        let _t3 = a.take(1, 10).unwrap();
        assert_eq!(a.take(1, 10).err().as_deref(), Some(AI_BUSY));
        assert!(a.take(2, 10).is_ok(), "someone else still may");
        drop(t1);
        let _t4 = a.take(1, 10).unwrap();
        let b = Arc::new(Allowance::default());
        drop(b.take(5, 2).unwrap());
        drop(b.take(5, 2).unwrap());
        assert_eq!(b.take(5, 2).err().as_deref(), Some(AI_TOO_MANY));
        // a refunded one doesn't count for the hour
        let c = Arc::new(Allowance::default());
        c.take(6, 1).unwrap().refund();
        let t = c.take(6, 1).unwrap();
        assert_eq!(c.take(6, 1).err().as_deref(), Some(AI_TOO_MANY));
        drop(t);
    }

    /// A local web server that answers one request with `answer` (status
    /// line, headers and body, written in parts `|` apart, 50 ms between),
    /// and gives back the request it got.
    fn one_answer(answer: &'static str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://127.0.0.1:{}/v1/messages",
            l.local_addr().unwrap().port()
        );
        let t = std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut got = vec![];
            let mut buf = [0u8; 4096];
            // the headers, then as much body as they say
            loop {
                let n = c.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len: usize = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if got.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            for part in answer.split('|') {
                if c.write_all(part.as_bytes()).is_err() {
                    break;
                }
                let _ = c.flush();
                std::thread::sleep(Duration::from_millis(50));
            }
            String::from_utf8_lossy(&got).to_string()
        });
        (url, t)
    }

    fn local(url: String) -> CurlClaude {
        CurlClaude {
            key: "sk-test-key".into(),
            url,
            protos: "=http",
        }
    }

    /// The real sender, against a local stand-in for the API: the key and
    /// version go in headers (from curl's stdin, not its arguments), the
    /// answer streams in as it is written, and a refused key or a broken
    /// answer is a failure.
    #[test]
    fn curl_streams_the_answer_and_reports_failures() {
        let body = request(
            "claude-opus-5-5",
            vec![text_block("s")],
            vec![user("q")],
            "low",
            None,
        );
        let (url, server) = one_answer(concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}\n\n|",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n|",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n|",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":2}}\n\n",
        ));
        let mut pieces = vec![];
        let r = local(url).stream(&body, &mut |t| pieces.push(t.to_string()), &|| false);
        assert_eq!(
            r,
            Ok(Usage {
                input: 7,
                cached: 0,
                output: 2
            })
        );
        assert_eq!(pieces, ["Hel", "lo"]);
        let req = server.join().unwrap();
        let lower = req.to_ascii_lowercase();
        assert!(req.starts_with("POST /v1/messages "), "{req}");
        assert!(lower.contains("x-api-key: sk-test-key"));
        assert!(lower.contains("anthropic-version: 2026-10-21"));
        assert!(lower.contains("anthropic-beta: server-side-fallback-2026-07-01"));
        assert!(lower.contains("content-type: application/json"));
        assert!(req.contains("\"stream\":true"), "the body as JSON");

        // a refused key
        let (url, server) = one_answer(concat!(
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 86\r\nConnection: close\r\n\r\n",
            "{\"type\":\"error\",\"error\":{\"type\":\"authentication_error\",\"message\":\"invalid x-api-key\"}}",
        ));
        let r = local(url).stream(&body, &mut |_| {}, &|| false);
        assert_eq!(r, Err(AI_FAILED.to_string()));
        server.join().unwrap();

        // overloaded in the middle of the answer
        let (url, server) = one_answer(concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n|",
            "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n",
        ));
        let r = local(url).stream(&body, &mut |_| {}, &|| false);
        assert_eq!(r, Err(AI_FAILED.to_string()));
        server.join().unwrap();

        // cut off: the stream ends without saying how the answer ended
        let (url, server) = one_answer(concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Half an\"}}\n\n",
        ));
        let r = local(url).stream(&body, &mut |_| {}, &|| false);
        assert_eq!(r, Err(AI_FAILED.to_string()));
        server.join().unwrap();

        // nobody there
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let r = local(format!("http://127.0.0.1:{}/v1/messages", port)).stream(
            &body,
            &mut |_| {},
            &|| false,
        );
        assert_eq!(r, Err(AI_FAILED.to_string()));
    }

    /// Stopping ends the request at once, also while nothing comes (the
    /// model thinking).
    #[test]
    fn curl_stops_when_the_answer_isnt_wanted() {
        let body = request("m", vec![], vec![user("q")], "low", None);
        let (url, server) = one_answer(concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n|",
            "|||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||||",
            "data: {\"type\":\"message_stop\"}\n\n",
        ));
        let started = Instant::now();
        let got = std::sync::atomic::AtomicBool::new(false);
        let mut pieces = 0;
        let r = local(url).stream(
            &body,
            &mut |_| {
                pieces += 1;
                got.store(true, std::sync::atomic::Ordering::SeqCst);
            },
            &|| got.load(std::sync::atomic::Ordering::SeqCst),
        );
        assert_eq!(r, Err(AI_STOPPED.to_string()));
        assert_eq!(pieces, 1);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        let _ = server.join();
    }

    #[test]
    fn the_setup_comes_from_ai_json() {
        let home = std::env::temp_dir().join(format!("wb-ai-{:x}", wb_sync::random_u64()));
        std::fs::create_dir_all(&home).unwrap();
        assert!(load_ai(&home).is_err(), "no key, no AI");
        std::fs::write(home.join(AI_FILE), r#"{"api_key": "  "}"#).unwrap();
        assert!(load_ai(&home).is_err(), "a blank key is none");
        std::fs::write(
            home.join(AI_FILE),
            r#"{"api_key": " sk-ant-x ", "per_hour": 5}"#,
        )
        .unwrap();
        let cfg = load_ai(&home).unwrap();
        assert_eq!(cfg.model, DEFAULT_MODEL);
        assert_eq!(cfg.per_hour, Some(5));
        assert_eq!(cfg.api_key, "sk-ant-x");
        std::fs::write(home.join(AI_FILE), "{").unwrap();
        assert!(load_ai(&home).unwrap_err().contains("can't be read"));
        std::fs::write(
            home.join(AI_FILE),
            r#"{"api_key": "k", "base_url": "http://example.com"}"#,
        )
        .unwrap();
        assert!(load_ai(&home)
            .unwrap_err()
            .contains("must start with https://"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn the_api_can_be_reached_through_another_address() {
        assert_eq!(api_place("").unwrap(), (API_URL.to_string(), "=https"));
        assert_eq!(
            api_place("https://gw.example.com/anthropic/").unwrap(),
            (
                "https://gw.example.com/anthropic/v1/messages".to_string(),
                "=https"
            )
        );
        assert_eq!(
            api_place("http://127.0.0.1:9000").unwrap(),
            ("http://127.0.0.1:9000/v1/messages".to_string(), "=http")
        );
        assert_eq!(
            api_place("http://localhost/v1/messages").unwrap().1,
            "=http"
        );
        assert_eq!(api_place("http://[::1]:8080").unwrap().1, "=http");
        for bad in [
            "http://localhost:1@evil.example/",
            "http://127.0.0.1@evil.example",
            "http://localhost:abc",
            "http://localhost:",
            "http://10.0.0.5:9000",
            "http://localhost.evil.com",
            "ftp://x",
            "api.anthropic.com",
        ] {
            assert!(api_place(bad).is_err(), "{bad}");
        }
    }
}
