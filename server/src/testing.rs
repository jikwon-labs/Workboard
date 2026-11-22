// What the tests need to run things for real: the server on a local port
// with its data in a temporary folder, a local livekit-server for calls,
// temporary folders and a wait loop. Used by this crate's tests, the app's
// tests and wb-voice's tests; built only with the `testing` feature, which
// their dev-dependencies turn on.

use crate::ai::{AiSetup, Claude, Usage, AI_STOPPED};
use crate::{serve_with, Options, MAIL_FILE, OUTBOX_DIR};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A new temporary folder path, wb-<tag>-<random>; not made yet.
pub fn temp_home(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("wb-{}-{:x}", tag, wb_sync::random_u64()))
}

/// A new temporary folder path like `temp_home`, removed with what is in it
/// when this is dropped, so a test that fails doesn't leave its folder
/// behind. Declare it before a server that uses it: the server stops first.
pub struct TempHome(PathBuf);

impl TempHome {
    pub fn new(tag: &str) -> TempHome {
        TempHome(temp_home(tag))
    }
}

impl std::ops::Deref for TempHome {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl AsRef<Path> for TempHome {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<std::ffi::OsStr> for TempHome {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_os_str()
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Calls `f` every `step` until it returns true, for at most `timeout`;
/// whether it did. Callers fail the test with what they were waiting for
/// and the state they have then.
pub fn wait_until(timeout: Duration, step: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(step);
    }
    false
}

/// A workboard-server running on its own thread, stopped when dropped.
pub struct TestServer {
    pub port: u16,
    pub home: PathBuf,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Started on a folder only this server uses: the folder is removed
    /// when it stops, and a panic on its thread is left alone. A server
    /// from `start_with` keeps its folder and passes such a panic on.
    own_home: bool,
}

impl TestServer {
    /// A server on a free port with its data in a new temporary folder.
    pub fn start() -> TestServer {
        Self::start_in(&temp_home("server"))
    }

    /// `start`, with the mails written to outbox/ in its folder instead of
    /// sent (read them with `mails_to`).
    pub fn with_mail_folder() -> TestServer {
        let home = temp_home("accounts");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(MAIL_FILE), r#"{"transport": "folder"}"#).unwrap();
        Self::start_in(&home)
    }

    /// `start`, with `ai` answering the apps' AI requests.
    pub fn with_ai(ai: Arc<FakeClaude>) -> TestServer {
        Self::spawn(
            &temp_home("ai"),
            0,
            Options {
                ai: Some(ai_setup(ai, 100)),
                ..Options::default()
            },
            true,
        )
    }

    /// A server on a free port with its data in `home`, a folder the test
    /// made for it (it is removed when the server stops).
    pub fn start_in(home: &Path) -> TestServer {
        Self::spawn(home, 0, Options::default(), true)
    }

    /// `start_with` with the default options.
    pub fn start_at(home: &Path, port: u16) -> TestServer {
        Self::start_with(home, port, Options::default())
    }

    /// A server with its data in `home` (kept when it stops, so a test can
    /// start another on it) on `port`, 0 for a free one. The same port again
    /// after a restart is waited for while it frees up.
    pub fn start_with(home: &Path, port: u16, options: Options) -> TestServer {
        Self::spawn(home, port, options, false)
    }

    fn spawn(home: &Path, port: u16, options: Options, own_home: bool) -> TestServer {
        let h = home.to_path_buf();
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                // "a free port" is one from below the range the system hands
                // out (free_port), not bind(0): tests restart the server on
                // the port it had, and a port from that range could be given
                // to another socket while the server was stopped. The same
                // port again after a restart can take a moment to free up.
                let mut tries = 0;
                let listener = loop {
                    let p = if port == 0 { free_port() } else { port };
                    match tokio::net::TcpListener::bind(("127.0.0.1", p)).await {
                        Ok(l) => break l,
                        // another test took the one just chosen: choose again
                        Err(_) if port == 0 && tries < 50 => tries += 1,
                        Err(_) if tries < 50 => {
                            tries += 1;
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        Err(e) => panic!("bind: {}", e),
                    }
                };
                port_tx.send(listener.local_addr().unwrap().port()).unwrap();
                serve_with(listener, h, options, async {
                    let _ = stop_rx.await;
                })
                .await
                .unwrap();
            });
        });
        let port = port_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("server started");
        TestServer {
            port,
            home: home.to_path_buf(),
            stop: Some(stop_tx),
            thread: Some(thread),
            own_home,
        }
    }

    /// Where the apps connect: 127.0.0.1 and its port.
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// The mails to `to` in outbox/ (see `with_mail_folder`), oldest first.
    pub fn mails_to(&self, to: &str) -> Vec<String> {
        let mut files: Vec<_> = std::fs::read_dir(self.home.join(OUTBOX_DIR))
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.sort();
        files
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .filter(|t| t.starts_with(&format!("To: {}\n", to)))
            .collect()
    }

    pub fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.thread.take() {
            if self.own_home {
                let _ = t.join();
            } else {
                t.join().unwrap();
            }
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.halt();
        if self.own_home {
            let _ = std::fs::remove_dir_all(&self.home);
        }
    }
}

/// A local port that is free for both TCP and UDP right now, picked at
/// random below the range the system hands out by itself (49152 and up on
/// macOS and Windows, 32768 and up on Linux).
///
/// A port from that range (bind to port 0) can be handed to another socket
/// between this check and its use. livekit-server binds its RTC port for
/// UDP on every address, and still starts when a socket on 127.0.0.1
/// already has that port, which then gets the call's packets instead: the
/// call never connected and `two_people_hear_each_other_in_a_huddle`
/// timed out at "Ann joined". Call clients' own UDP sockets, outgoing
/// connections and test servers all take ports from that range, so ports
/// below it can't be taken that way; only another test choosing the same
/// number at the same moment could, and `start_livekit` starts again then.
pub fn free_port() -> u16 {
    loop {
        let port = 20_000 + (wb_sync::random_u64() % 12_000) as u16;
        let free = ["127.0.0.1", "0.0.0.0"].iter().all(|a| {
            std::net::TcpListener::bind((*a, port)).is_ok()
                && std::net::UdpSocket::bind((*a, port)).is_ok()
        });
        if free {
            return port;
        }
    }
}

/// A local livekit-server, stopped when dropped.
pub struct LiveKit {
    child: Child,
    pub port: u16,
}

impl Drop for LiveKit {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The installed livekit-server command; None when there is none (tests
/// that need it say so and skip).
pub fn livekit_bin() -> Option<String> {
    [
        "livekit-server",
        "/opt/homebrew/bin/livekit-server",
        "/usr/local/bin/livekit-server",
    ]
    .into_iter()
    .find(|b| {
        Command::new(b)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
    .map(|s| s.to_string())
}

/// Starts livekit-server `bin` on free local ports with one API key and
/// waits until it takes connections. Err when it can't be started.
pub fn start_livekit(bin: &str, key: &str, secret: &str) -> std::io::Result<LiveKit> {
    // a port can still be taken between choosing it and livekit binding it
    // (another test choosing the same one): livekit then quits, and it is
    // started again on other ports
    let mut last = None;
    for _ in 0..5 {
        match start_livekit_on(bin, key, secret, free_port(), free_port())? {
            Ok(lk) => return Ok(lk),
            Err(why) => last = Some(why),
        }
    }
    Err(std::io::Error::other(format!(
        "livekit-server didn't start: {}",
        last.unwrap_or_default()
    )))
}

/// Starts livekit-server on `port` (signalling) and `rtc` (media, TCP and
/// UDP). Inner Err (with why) when it quit or another program answers on
/// `port` instead.
pub fn start_livekit_on(
    bin: &str,
    key: &str,
    secret: &str,
    port: u16,
    rtc: u16,
) -> std::io::Result<Result<LiveKit, String>> {
    let config = format!(
        "port: {port}\nbind_addresses: [\"127.0.0.1\"]\nrtc:\n  tcp_port: {rtc}\n  udp_port: {rtc}\n  use_external_ip: false\n  node_ip: 127.0.0.1\nkeys:\n  {key}: {secret}\nlogging:\n  level: warn\n"
    );
    let child = Command::new(bin)
        .arg("--config-body")
        .arg(config)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut lk = LiveKit { child, port };
    let mut quit = false;
    // up when livekit itself answers: anything else listening on the port
    // (a port taken after it was chosen) doesn't count
    let up = wait_until(Duration::from_secs(15), Duration::from_millis(50), || {
        if lk.child.try_wait().ok().flatten().is_some() {
            quit = true;
            return true;
        }
        livekit_answers(port)
    });
    if quit {
        return Ok(Err(format!("it quit (ports {} and {} taken?)", port, rtc)));
    }
    if !up {
        return Ok(Err(format!(
            "nothing answered on port {} like livekit",
            port
        )));
    }
    // it answered: still check it did not quit right after (its media port taken)
    if lk.child.try_wait().ok().flatten().is_some() {
        return Ok(Err(format!("it quit (port {} taken?)", rtc)));
    }
    Ok(Ok(lk))
}

/// True when a livekit-server answers on `port`: its "/" says OK.
fn livekit_answers(port: u16) -> bool {
    use std::io::{Read, Write};
    let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    if s.write_all(b"GET / HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut answer = String::new();
    let _ = s.read_to_string(&mut answer);
    answer.starts_with("HTTP/1.") && answer.contains(" 200 ") && answer.ends_with("\r\n\r\nOK")
}

/// An AI for tests: it keeps every request and answers each with `answer`,
/// a piece at a time (the pieces split at '|', a tenth of a second apart).
#[derive(Debug)]
pub struct FakeClaude {
    /// every request body it was sent, in order
    pub asked: Mutex<Vec<Value>>,
    pub answer: Mutex<String>,
    /// fail with this instead of answering
    pub fail: Mutex<Option<String>>,
    /// while set, wait after the first piece (until stopped or cleared)
    pub hold: AtomicBool,
    /// answers stopped because nobody wanted them any more
    pub stopped: AtomicUsize,
}

impl FakeClaude {
    pub fn new(answer: &str) -> Arc<FakeClaude> {
        Arc::new(FakeClaude {
            asked: Mutex::new(vec![]),
            answer: Mutex::new(answer.to_string()),
            fail: Mutex::new(None),
            hold: AtomicBool::new(false),
            stopped: AtomicUsize::new(0),
        })
    }

    /// The requests it got so far.
    pub fn asked(&self) -> Vec<Value> {
        self.asked.lock().unwrap().clone()
    }

    /// Every text the last request carries (system and messages), joined.
    pub fn last_text(&self) -> String {
        let asked = self.asked();
        let Some(b) = asked.last() else {
            return String::new();
        };
        let mut out = String::new();
        for s in b["system"].as_array().into_iter().flatten() {
            out.push_str(s["text"].as_str().unwrap_or(""));
            out.push('\n');
        }
        for m in b["messages"].as_array().into_iter().flatten() {
            out.push_str(m["content"].as_str().unwrap_or(""));
            out.push('\n');
        }
        out
    }
}

impl Claude for FakeClaude {
    fn stream(
        &self,
        body: &Value,
        text: &mut dyn FnMut(&str),
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<Usage, String> {
        self.asked.lock().unwrap().push(body.clone());
        if let Some(e) = self.fail.lock().unwrap().clone() {
            return Err(e);
        }
        let answer = self.answer.lock().unwrap().clone();
        let stopped = || {
            self.stopped.fetch_add(1, Ordering::SeqCst);
            Err(AI_STOPPED.to_string())
        };
        for (i, piece) in answer.split('|').enumerate() {
            if stop() {
                return stopped();
            }
            text(piece);
            std::thread::sleep(Duration::from_millis(100));
            while i == 0 && self.hold.load(Ordering::SeqCst) {
                if stop() {
                    return stopped();
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        Ok(Usage {
            input: 10,
            cached: 0,
            output: 5,
        })
    }
}

/// The AI setup for a server with `ai` answering, `per_hour` requests a
/// person an hour.
pub fn ai_setup(ai: Arc<FakeClaude>, per_hour: u32) -> AiSetup {
    AiSetup {
        claude: ai,
        model: "test-model".into(),
        per_hour,
    }
}
