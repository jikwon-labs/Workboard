// Sleeping until there is something to do on a connection.
//
// A connection thread has two things to wait for: the server sending
// something, and the app asking for something (a push, a request, stop).
// std has no way to wait on a socket and a channel at once, so a small
// helper thread (the bell) waits on the socket and puts a "readable" note on
// the same channel the app's requests come through. The connection thread
// then sleeps on that one channel and wakes only when there is something to
// read or send, or when a ping is due, instead of looking every 100 ms.
//
// The two threads take turns on the socket's read timeout: the bell waits
// (peeks, so nothing is taken from the socket) only while the connection
// thread isn't reading, and the connection thread reads with a very short
// timeout only after the bell rang, until nothing more is there. Then it
// hands the socket back to the bell (`idle`).

use crate::protocol::PING_SECS;
use crate::Conn;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::time::Duration;

/// How long a read waits for more once the bell rang: a message that
/// arrives in pieces is read on as the pieces come (the bell rings again).
const READ_SLICE: Duration = Duration::from_millis(2);

/// Every bell has its own number, so a note from the bell of a connection
/// that is over is told apart from the current one's.
static NEXT: AtomicU64 = AtomicU64::new(1);

pub(crate) struct Bell {
    pub(crate) id: u64,
    tcp: TcpStream,
    /// one token per turn the bell gets; dropping it ends the bell's thread
    go: Sender<()>,
}

impl Bell {
    /// Start the bell for `conn`; `rung(id)` is what it puts on `tx` when the
    /// server sent something. The connection thread has the first turn: it
    /// reads (`reading` is set) until nothing more is there, then calls `idle`.
    pub(crate) fn start<T: Send + 'static>(
        conn: &Conn,
        tx: Sender<T>,
        rung: impl Fn(u64) -> T + Send + 'static,
    ) -> std::io::Result<Bell> {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let tcp = conn.tcp().try_clone()?;
        let watch = tcp.try_clone()?;
        let (go, turns) = mpsc::channel::<()>();
        std::thread::Builder::new()
            .name("workboard-bell".into())
            .spawn(move || {
                let mut byte = [0u8; 1];
                // wait for a turn; the connection is over when there are no more
                while turns.recv().is_ok() {
                    loop {
                        // a long wait (the connection pings well within it);
                        // only to notice a connection that ended without a word
                        let _ = watch.set_read_timeout(Some(Duration::from_secs(PING_SECS)));
                        match watch.peek(&mut byte) {
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    ErrorKind::WouldBlock
                                        | ErrorKind::TimedOut
                                        | ErrorKind::Interrupted
                                ) =>
                            {
                                if matches!(turns.try_recv(), Err(TryRecvError::Disconnected)) {
                                    return;
                                }
                            }
                            // something to read, the end, or an error: the
                            // connection thread reads and finds out which
                            _ => break,
                        }
                    }
                    if tx.send(rung(id)).is_err() {
                        return;
                    }
                }
            })?;
        let bell = Bell { id, tcp, go };
        bell.reading();
        Ok(bell)
    }

    /// The bell rang: read with a short timeout until nothing more is there.
    pub(crate) fn reading(&self) {
        let _ = self.tcp.set_read_timeout(Some(READ_SLICE));
    }

    /// Nothing more to read: the bell waits on the socket again.
    pub(crate) fn idle(&self) {
        let _ = self.go.send(());
    }
}
