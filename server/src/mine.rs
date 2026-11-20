// Each person's own reading state and notification choices, kept in step
// between their devices (`wb_sync::mine`): the server keeps the merged state
// of every member in server.json and sends it only to that member's
// connections.

use super::*;

/// The most a member's state may grow to (bytes of JSON); a bigger one is
/// not taken. Thousands of Inbox rows fit well within it.
pub(crate) const MAX_MINE: usize = 4 * 1024 * 1024;

impl Hub {
    /// What the server keeps for `member` (`{}` when nothing yet), as the
    /// `mine` message sent after welcome.
    pub(crate) fn mine_for(&self, member: u64) -> String {
        let g = self.state();
        let data = g.saved.mine.get(&member).cloned().unwrap_or(json!({}));
        Msg::Mine { data }.to_json()
    }

    /// `member`'s app sent its state: merge it into what the server keeps
    /// and, when that changed, send the result to all their connections
    /// (this one too). When it didn't change but the app lacks some of it,
    /// the answer to this connection alone.
    pub(crate) fn take_mine(&self, member: u64, data: &Value) -> Option<String> {
        if !data.is_object() || serde_json::to_string(data).map_or(true, |s| s.len() > MAX_MINE) {
            return None;
        }
        let merged = {
            let mut g = self.state();
            let kept = g.saved.mine.entry(member).or_insert_with(|| json!({}));
            if !wb_sync::mine::merge(kept, data) {
                return (kept != data).then(|| Msg::Mine { data: kept.clone() }.to_json());
            }
            let merged = Arc::new(kept.clone());
            g.dirty = true;
            merged
        };
        // nobody listening is fine
        let _ = self.mines.send((member, merged));
        None
    }
}
