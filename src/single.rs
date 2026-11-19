// One app per data folder on Windows. macOS starts an app only once by
// itself and hands it the links opened later (src/links.rs); Windows starts
// a new copy each time, and two copies would write the same data.json and
// sign-in tokens. So the first copy keeps a named pipe open for its data
// folder. A copy started later hands its arguments (a workboard:// link)
// through the pipe and ends; the first one opens the link and comes forward.
//
// A named pipe rather than a named mutex or a localhost port: it carries the
// arguments (a mutex can't), only the same person can write to it (a pipe's
// default security), there is no port another program could hold or a
// firewall could ask about, and creating it with FILE_FLAG_FIRST_PIPE_INSTANCE
// tells in one step whether a copy is already running.

/// What this copy of the app does.
#[derive(Debug, PartialEq)]
pub enum Start {
    /// Start the app: it is the only copy for its data folder.
    Run,
    /// Another copy took the arguments: end.
    Handed,
}

/// Call before the window opens (not for --screenshot runs).
pub fn start() -> Start {
    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        pipe::start(&pipe_name(&crate::model::Data::dir()), &args, received)
    }
    #[cfg(not(windows))]
    Start::Run
}

/// A later copy's arguments arrived: open its link and come forward.
#[cfg_attr(not(windows), allow(dead_code))]
fn received(args: Vec<String>) {
    for a in args {
        if a.to_ascii_lowercase().starts_with("workboard://") {
            crate::links::arrived(a);
        }
    }
    crate::notifier::bring_forward();
}

/// The pipe for a data folder: one per folder (and so per person, whose
/// %APPDATA% differs), so a WORKBOARD_HOME run beside the real app is fine.
#[cfg_attr(not(windows), allow(dead_code))]
fn pipe_name(dir: &std::path::Path) -> String {
    // FNV-1a: the same number in every copy and every build
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in dir.to_string_lossy().to_lowercase().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    format!(r"\\.\pipe\workboard-{:016x}", h)
}

/// The arguments as sent: their length in bytes, then each one ended by a 0.
#[cfg_attr(not(windows), allow(dead_code))]
fn encode(args: &[String]) -> Vec<u8> {
    let mut body = vec![];
    for a in args {
        body.extend_from_slice(a.replace('\0', "").as_bytes());
        body.push(0);
    }
    let mut out = (body.len() as u32).to_le_bytes().to_vec();
    out.extend(body);
    out
}

/// The arguments from a message body (without its length).
#[cfg_attr(not(windows), allow(dead_code))]
fn decode(body: &[u8]) -> Vec<String> {
    body.split(|&b| b == 0)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect()
}

#[cfg(windows)]
mod pipe {
    use super::Start;
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeServerProcessId,
        WaitNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
        PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

    type Handler = Arc<dyn Fn(Vec<String>) + Send + Sync>;

    /// Become the copy that serves `name`, or hand `args` to the one that does.
    pub fn start(
        name: &str,
        args: &[String],
        on: impl Fn(Vec<String>) + Send + Sync + 'static,
    ) -> Start {
        let on: Handler = Arc::new(on);
        for _ in 0..30 {
            match claim(name) {
                Ok(Some(first)) => {
                    serve(first, name.to_string(), on);
                    return Start::Run;
                }
                Ok(None) => {
                    if send(name, args).is_ok() {
                        return Start::Handed;
                    }
                }
                // pipes don't work here: start as before
                Err(_) => return Start::Run,
            }
            // the other copy is just starting or ending
            std::thread::sleep(Duration::from_millis(100));
        }
        // it doesn't answer: better a second copy than no app at all
        Start::Run
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn handle(f: &File) -> HANDLE {
        HANDLE(f.as_raw_handle() as _)
    }

    /// One end of the pipe for a copy to connect to. The first one fails
    /// with "access denied" when another copy already has the pipe.
    fn create(name: &str, first: bool) -> io::Result<File> {
        let mut mode = PIPE_ACCESS_DUPLEX;
        if first {
            mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        let n = wide(name);
        let h = unsafe {
            CreateNamedPipeW(
                PCWSTR(n.as_ptr()),
                mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64,
                64 * 1024,
                0,
                None,
            )
        };
        if h == INVALID_HANDLE_VALUE || h.is_invalid() {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_handle(h.0 as _) })
    }

    /// Some(pipe) when this is the first copy, None when another one has it.
    fn claim(name: &str) -> io::Result<Option<File>> {
        match create(name, true) {
            Ok(f) => Ok(Some(f)),
            Err(e)
                if e.raw_os_error() == Some(ERROR_ACCESS_DENIED.0 as i32)
                    || e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// Answer later copies for the life of the app.
    fn serve(first: File, name: String, on: Handler) {
        let _ = std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                let mut cur = first;
                loop {
                    let connected = match unsafe { ConnectNamedPipe(handle(&cur), None) } {
                        Ok(()) => true,
                        Err(e) => e.code() == ERROR_PIPE_CONNECTED.to_hresult(),
                    };
                    // the next end before this one is answered, so a copy
                    // started meanwhile still finds the pipe
                    let next = create(&name, false);
                    if connected {
                        let on = on.clone();
                        std::thread::spawn(move || answer(cur, &*on));
                    }
                    match next {
                        Ok(n) => cur = n,
                        // later copies will find no pipe and start on their own
                        Err(_) => return,
                    }
                }
            });
    }

    /// Read one copy's arguments, pass them on, and say they arrived.
    fn answer(mut f: File, on: &(dyn Fn(Vec<String>) + Send + Sync)) {
        let mut len = [0u8; 4];
        if f.read_exact(&mut len).is_err() {
            return;
        }
        let n = u32::from_le_bytes(len) as usize;
        if n > 1 << 20 {
            return;
        }
        let mut body = vec![0u8; n];
        if f.read_exact(&mut body).is_err() {
            return;
        }
        on(super::decode(&body));
        let _ = f.write_all(b"k");
        // FlushFileBuffers: wait until the other copy has read the answer
        let _ = f.sync_all();
        unsafe {
            let _ = DisconnectNamedPipe(handle(&f));
        }
    }

    /// Hand `args` to the copy serving `name` and wait until it has them.
    fn send(name: &str, args: &[String]) -> io::Result<()> {
        let until = Instant::now() + Duration::from_secs(3);
        let mut f = loop {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(name)
            {
                Ok(f) => break f,
                Err(e)
                    if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32)
                        && Instant::now() < until =>
                {
                    let n = wide(name);
                    unsafe {
                        let _ = WaitNamedPipeW(PCWSTR(n.as_ptr()), 500);
                    }
                }
                Err(e) => return Err(e),
            }
        };
        // this copy may take the front (the person just opened it): let the
        // running one bring its window forward instead
        let mut pid = 0u32;
        unsafe {
            if GetNamedPipeServerProcessId(handle(&f), &mut pid).is_ok() {
                let _ = AllowSetForegroundWindow(pid);
            }
        }
        f.write_all(&super::encode(args))?;
        let mut ok = [0u8; 1];
        f.read_exact(&mut ok)?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::{mpsc, Mutex};

        #[test]
        fn a_later_copy_hands_its_link_to_the_first_and_ends() {
            let name = format!(
                r"\\.\pipe\workboard-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let (tx, rx) = mpsc::channel();
            let tx = Mutex::new(tx);
            let first = start(&name, &[], move |a| {
                let _ = tx.lock().unwrap().send(a);
            });
            assert_eq!(first, Start::Run, "the first copy starts the app");

            let link = "workboard://join?server=https%3A%2F%2Fwb.example.com&code=AB12".to_string();
            let second = start(&name, &[link.clone()], |_| {
                panic!("a later copy doesn't serve the pipe")
            });
            assert_eq!(second, Start::Handed, "the second copy ends");
            let got = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(got, vec![link], "the first copy has the link");

            // the pipe stays open for the next copy, and one without a link
            // still ends (the first copy comes forward)
            let third = start(&name, &["workboard://task/7".into()], |_| panic!());
            assert_eq!(third, Start::Handed);
            assert_eq!(
                rx.recv_timeout(Duration::from_secs(5)).unwrap(),
                vec!["workboard://task/7".to_string()]
            );
            assert_eq!(start(&name, &[], |_| panic!()), Start::Handed);
            assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap().is_empty());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_arrive_as_they_were_sent() {
        let args = vec![
            "workboard://join?server=https%3A%2F%2Fwb.example.com&code=AB12".to_string(),
            "C:\\Users\\Kim Jiwon\\메모.txt".to_string(),
        ];
        let msg = encode(&args);
        let len = u32::from_le_bytes(msg[..4].try_into().unwrap()) as usize;
        assert_eq!(len, msg.len() - 4);
        assert_eq!(decode(&msg[4..]), args);
        assert!(decode(&encode(&[])[4..]).is_empty());
    }

    #[test]
    fn each_data_folder_has_its_own_pipe() {
        let real = pipe_name(std::path::Path::new(
            r"C:\Users\kim\AppData\Roaming\Workboard",
        ));
        assert!(real.starts_with(r"\\.\pipe\workboard-"));
        // Windows paths ignore case
        assert_eq!(
            real,
            pipe_name(std::path::Path::new(
                r"c:\users\KIM\appdata\roaming\workboard"
            ))
        );
        assert_ne!(real, pipe_name(std::path::Path::new(r"C:\scratch\wbhome")));
    }

    #[test]
    fn a_handed_link_waits_for_the_app() {
        while crate::links::take().is_some() {}
        received(vec![
            "--lang".into(),
            "ko".into(),
            "WORKBOARD://task/12".into(),
        ]);
        assert_eq!(crate::links::take().as_deref(), Some("WORKBOARD://task/12"));
        received(vec![]);
        assert_eq!(crate::links::take(), None);
    }
}
