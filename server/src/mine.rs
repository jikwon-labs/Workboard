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
        let data = g.saved.mine.get(&member).cloned();
        drop(g);
        mine_json(data.as_deref().unwrap_or(&json!({})))
    }

    /// `member`'s app sent its state, whole or only what changed: merge it
    /// into what the server keeps and, when that changed, send the result
    /// to all their connections (this one too; apps that take parts get
    /// only what changed). When it didn't change but the app lacks some of
    /// it, the answer to this connection alone: the whole state, or for an
    /// app that takes parts (`parts`), the entries it sent that the server
    /// has newer.
    pub(crate) fn take_mine(&self, member: u64, data: &Value, parts: bool) -> Option<String> {
        if !data.is_object() || serde_json::to_string(data).map_or(true, |s| s.len() > MAX_MINE) {
            return None;
        }
        let (merged, part) = {
            let mut g = self.state();
            let kept = g
                .saved
                .mine
                .entry(member)
                .or_insert_with(|| Arc::new(json!({})));
            // a merge changes only the entries sent
            let before = entries_of(kept, data);
            // (copied only when a connection still holds the last one)
            if !wb_sync::mine::merge(Arc::make_mut(kept), data) {
                if !parts {
                    return (**kept != *data).then(|| mine_json(kept));
                }
                let stale = wb_sync::mine::changes(&entries_of(kept, data), data);
                return stale
                    .as_object()
                    .is_some_and(|o| !o.is_empty())
                    .then(|| mine_part_json(&stale));
            }
            let part = wb_sync::mine::changes(&entries_of(kept, data), &before);
            let merged = kept.clone();
            g.mine_dirty = true;
            (merged, part)
        };
        // nobody listening is fine
        let _ = self.mines.send((member, merged, Some(Arc::new(part))));
        None
    }
}

/// The parts and entries of `state` that `sent` has (a state, whole or
/// only what changed).
fn entries_of(state: &Value, sent: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for (k, v) in sent.as_object().into_iter().flatten() {
        let Some(have) = state.get(k) else {
            continue;
        };
        match (v.as_object(), have.as_object()) {
            (Some(entries), Some(theirs)) if k != "replies_seen" => {
                let e: serde_json::Map<String, Value> = entries
                    .keys()
                    .filter_map(|e| Some((e.clone(), theirs.get(e)?.clone())))
                    .collect();
                out.insert(k.clone(), Value::Object(e));
            }
            _ => {
                out.insert(k.clone(), have.clone());
            }
        }
    }
    Value::Object(out)
}

/// `Msg::MinePart` for `data`, written from a borrowed copy.
pub(crate) fn mine_part_json(data: &Value) -> String {
    #[derive(Serialize)]
    #[serde(tag = "type", rename = "mine_part")]
    struct MinePart<'a> {
        data: &'a Value,
    }
    serde_json::to_string(&MinePart { data }).unwrap_or_default()
}

/// `Msg::Mine` for `data`, written from a borrowed copy.
pub(crate) fn mine_json(data: &Value) -> String {
    #[derive(Serialize)]
    #[serde(tag = "type", rename = "mine")]
    struct Mine<'a> {
        data: &'a Value,
    }
    serde_json::to_string(&Mine { data }).unwrap_or_default()
}
