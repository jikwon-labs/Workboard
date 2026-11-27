// The app is away: the iPad's desktop app went to the background, where
// iOS soon stops it and it draws no frames (so it tells nothing itself).
// Every connection hangs up meanwhile, so the server sees the device as not
// connected and pushes its notifications through Apple instead (it pushes
// only to devices that aren't connected); they come back by themselves when
// the app does. The app's iOS code sets it (src/os.rs); nothing else does.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static AWAY: AtomicBool = AtomicBool::new(false);

/// Each connection thread's bell, rung when `AWAY` changes; false once
/// that thread is gone (it is forgotten then).
type Ring = Box<dyn Fn() -> bool + Send>;
static RINGS: Mutex<Vec<Ring>> = Mutex::new(Vec::new());

/// Say the app went away (true) or came back (false).
pub fn set_away(away: bool) {
    if AWAY.swap(away, Ordering::SeqCst) == away {
        return;
    }
    let mut rings = RINGS.lock().unwrap_or_else(|e| e.into_inner());
    rings.retain(|ring| ring());
}

/// Whether the app is away.
pub fn is_away() -> bool {
    AWAY.load(Ordering::SeqCst)
}

/// A connection thread's bell (it wakes the thread to look at `is_away`).
pub(crate) fn ring_me(ring: Ring) {
    RINGS.lock().unwrap_or_else(|e| e.into_inner()).push(ring);
}
