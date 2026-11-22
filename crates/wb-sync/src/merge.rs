// Merging copies item by item and field by field, newest change winning.

use super::*;

pub(crate) type Cands<'a> = HashMap<u64, Vec<(usize, Option<u64>, &'a Value)>>;

pub(crate) struct Ctx<'a> {
    stamps: Vec<&'a BTreeMap<u64, i64>>,
    tombs: BTreeMap<u64, i64>,
    revived: BTreeMap<u64, i64>,
    out: BTreeMap<u64, i64>,
    /// id -> every copy of the item, in every source and every place.
    cands: HashMap<u64, Vec<(usize, &'a Value)>>,
    /// id -> the item it sits in, in the newest copy (None at the top).
    home: HashMap<u64, Option<u64>>,
    /// id -> when it moved there (0: not known)
    home_moved: HashMap<u64, i64>,
    out_moved: BTreeMap<u64, i64>,
    /// source -> whether that copy knows the `DEFAULT_WHEN_MISSING` fields
    /// (`SyncMeta.v` 2 and later).
    omits_defaults: Vec<bool>,
    /// source -> its field stamps
    fields: Vec<&'a BTreeMap<u64, BTreeMap<String, i64>>>,
    out_fields: BTreeMap<u64, BTreeMap<String, i64>>,
    /// Items that came out exactly as this copy had them, with everything
    /// inside them (see `untouched`).
    kept: std::collections::HashSet<u64>,
    /// While merging a top-level collection: its place in `SHARED`, and
    /// where each of this copy's items is in it. Such an item that comes
    /// out untouched is moved over afterwards instead of copied: a
    /// `KEEP` stand-in holds its place until then (see `put_back`).
    top: Option<(usize, HashMap<u64, usize>)>,
    /// (collection, place in this copy's list) of each stand-in
    slots: Vec<(usize, usize)>,
}

#[cfg(test)]
thread_local! {
    /// Tests: merge every item the long way (no `Ctx::untouched`).
    pub(crate) static LONG_WAY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Tests: how many items came out untouched the quick way.
    pub(crate) static QUICK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The field of a stand-in for an untouched top-level item (`Ctx::top`).
const KEEP: &str = "\u{0}wb-keep";

/// Every item with an id in one source, with the item it sits in.
pub(crate) fn collect<'a>(src: usize, arr: &'a [Value], parent: Option<u64>, out: &mut Cands<'a>) {
    for item in arr {
        if let (Some(id), Some(obj)) = (id_of(item), item.as_object()) {
            out.entry(id).or_default().push((src, parent, item));
            for (_, child) in obj {
                if is_id_array(child) {
                    collect(src, child.as_array().unwrap(), Some(id), out);
                }
            }
        }
    }
}

impl<'a> Ctx<'a> {
    fn new(
        stamps: Vec<&'a BTreeMap<u64, i64>>,
        fields: Vec<&'a BTreeMap<u64, BTreeMap<String, i64>>>,
        moved: Vec<Option<&'a BTreeMap<u64, i64>>>,
        tombs: BTreeMap<u64, i64>,
        revived: BTreeMap<u64, i64>,
        all: Cands<'a>,
    ) -> Self {
        let omits_defaults: Vec<bool> = moved.iter().map(|m| m.is_some()).collect();
        let mut cands = HashMap::new();
        let mut home = HashMap::new();
        let mut home_moved = HashMap::new();
        // the copy that moved it last decides where the item is (copies that
        // don't keep move times count their item's last change as one), then
        // the newest copy; on a tie this computer's
        let key = |src: usize, id: u64| {
            let s = stamps[src].get(&id).copied().unwrap_or(0);
            let m = match moved[src] {
                Some(m) => m.get(&id).copied().unwrap_or(0),
                None => s,
            };
            (m, s)
        };
        for (id, list) in all {
            let mut best = list[0];
            let mut best_key = key(best.0, id);
            for c in &list[1..] {
                let k = key(c.0, id);
                if k > best_key {
                    best = *c;
                    best_key = k;
                }
            }
            home.insert(id, best.1);
            // a move time worth keeping: one the deciding copy keeps, or a
            // real move (the copies disagree where it is). Copies that don't
            // keep move times otherwise record nothing, or every item they
            // win would get one.
            let recorded = if moved[best.0].is_some() || list.iter().any(|c| c.1 != best.1) {
                best_key.0
            } else {
                list.iter()
                    .filter_map(|c| moved[c.0].and_then(|m| m.get(&id)).copied())
                    .max()
                    .unwrap_or(0)
            };
            home_moved.insert(id, recorded);
            cands.insert(id, list.into_iter().map(|(s, _, v)| (s, v)).collect());
        }
        Ctx {
            stamps,
            tombs,
            revived,
            out: BTreeMap::new(),
            cands,
            home,
            home_moved,
            out_moved: BTreeMap::new(),
            omits_defaults,
            fields,
            out_fields: BTreeMap::new(),
            kept: Default::default(),
            top: None,
            slots: vec![],
        }
    }

    /// Whether merging the item `id` gives back exactly this copy's version
    /// of it, with everything inside it, and its bookkeeping as it is: it
    /// is only here (no other copy has it or anything inside it), alive, and
    /// already in the shape a merge leaves it in (`merge_fields` and
    /// `merge_arr` would rebuild it field by field into the same thing).
    /// Such an item is copied as it is, which is much quicker. Anything a
    /// merge could reorder or tidy (reactions, running timers, time
    /// entries, items out of time order, entries without an id among the
    /// items, field stamps a merge would leave out) takes the long way.
    fn untouched(&self, id: u64) -> bool {
        #[cfg(test)]
        if LONG_WAY.with(|l| l.get()) {
            return false;
        }
        let Some([(0, x)]) = self.cands.get(&id).map(|l| l.as_slice()) else {
            return false;
        };
        if !self.alive(id) {
            return false;
        }
        let Some(obj) = x.as_object() else {
            return false;
        };
        let nonempty = |k: &str| obj.get(k).is_some_and(|v| v.is_array() && !is_empty_arr(v));
        if nonempty("time_entries") || !timer_list(obj).is_empty() {
            return false;
        }
        let listed = self.fields[0].get(&id);
        let stamp = self.stamps[0].get(&id).copied().unwrap_or(0);
        let base = listed.and_then(|e| e.get("")).copied().unwrap_or(stamp);
        // reactions are put back in the order they were added
        let pairs = match obj.get("reactions") {
            Some(v) if nonempty("reactions") => match reaction_pairs(v) {
                Some(pairs) => {
                    if !reactions_in_order(v, &pairs, listed, base) {
                        return false;
                    }
                    pairs
                }
                // not reactions as the app writes them: an ordinary field
                None => vec![],
            },
            _ => vec![],
        };
        if let Some(e) = listed {
            // a base the same as the stamp is left out
            if e.contains_key("") && base == stamp {
                return false;
            }
            for (k, t) in e {
                if k.is_empty() {
                    continue;
                }
                if SPLIT_LATER.contains(&k.as_str()) {
                    return false;
                }
                let present = match k.split_once('/') {
                    Some(("custom", key)) => obj
                        .get("custom")
                        .and_then(|c| c.as_object())
                        .is_some_and(|c| c.contains_key(key)),
                    Some(("reactions", pair)) => pairs
                        .iter()
                        .any(|(e, u)| pair.rsplit_once('/') == Some((e.as_str(), &u.to_string()))),
                    Some(_) => return false,
                    None => match obj.get(k.as_str()) {
                        None => false,
                        Some(v) => {
                            if k == "reactions"
                                || LOCAL_KEYS.contains(&k.as_str())
                                || APPEND_KEYS.contains(&k.as_str())
                                || TIMER_KEYS.contains(&k.as_str())
                                || is_id_array(v)
                            {
                                return false;
                            }
                            !is_empty_arr(v)
                        }
                    },
                };
                // a field that is there and changed at the base is left out
                if present && *t == base {
                    return false;
                }
            }
        }
        for (k, v) in obj {
            if is_id_array(v) {
                let a = v.as_array().unwrap();
                // entries without an id go after the items
                if let Some(first_loose) = a.iter().position(|x| id_of(x).is_none()) {
                    if a[first_loose..].iter().any(|x| id_of(x).is_some()) {
                        return false;
                    }
                }
                let items = a.iter().filter(|x| id_of(x).is_some());
                // messages and replies read in time order
                let times: Option<Vec<i64>> = items
                    .clone()
                    .map(|x| x.get("ts").and_then(|t| t.as_i64()))
                    .collect();
                if times.is_some_and(|t| t.windows(2).any(|w| w[0] > w[1])) {
                    return false;
                }
                for c in items {
                    if !id_of(c).is_some_and(|c| self.untouched(c)) {
                        return false;
                    }
                }
            } else if APPEND_KEYS.contains(&k.as_str()) {
                // kept once each, in time order
                let Some(a) = v.as_array() else {
                    return false;
                };
                let ts = |x: &Value| x.get("ts").and_then(|t| t.as_i64()).unwrap_or(0);
                if a.windows(2).any(|w| ts(&w[0]) > ts(&w[1])) {
                    return false;
                }
                let mut run = 0;
                for i in 1..a.len() {
                    if ts(&a[i]) != ts(&a[run]) {
                        run = i;
                    } else if a[run..i].contains(&a[i]) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// An item `untouched` said comes out as it is, with everything
    /// inside it: its bookkeeping as merging it would note it.
    fn keep_untouched(&mut self, x: &'a Value) {
        fn go(cx: &mut Ctx, x: &Value) {
            let (Some(id), Some(obj)) = (id_of(x), x.as_object()) else {
                return;
            };
            cx.out
                .insert(id, cx.stamps[0].get(&id).copied().unwrap_or(0));
            if let Some(m) = cx.home_moved.get(&id).filter(|m| **m > 0) {
                cx.out_moved.insert(id, *m);
            }
            if let Some(e) = cx.fields[0].get(&id) {
                cx.out_fields.insert(id, e.clone());
            }
            for (_, child) in obj {
                if is_id_array(child) {
                    for c in child.as_array().unwrap() {
                        go(cx, c);
                    }
                }
            }
        }
        if let Some(id) = id_of(x) {
            self.kept.insert(id);
        }
        go(self, x);
    }

    fn alive(&self, id: u64) -> bool {
        match self.tombs.get(&id) {
            None => true,
            Some(t) => self.revived.get(&id).map_or(false, |r| r > t),
        }
    }
}

/// Whether an item's reactions (`v`, of which `pairs`) are already as
/// `put_reactions` writes them: one entry per emoji with just its emoji and
/// people, in the order they were added (`read_part`: by the field stamps
/// `listed`, else at `base`).
fn reactions_in_order(
    v: &Value,
    pairs: &[(String, u64)],
    listed: Option<&BTreeMap<String, i64>>,
    base: i64,
) -> bool {
    // no field stamps for reactions: every one at the base
    let listed = listed.filter(|l| {
        l.range::<str, _>((
            std::ops::Bound::Included("reactions"),
            std::ops::Bound::Unbounded,
        ))
        .next()
        .is_some_and(|(k, _)| k.starts_with("reactions"))
    });
    let mut timed: Vec<(i64, &str, u64)> = pairs
        .iter()
        .map(|(e, u)| {
            let t = listed
                .and_then(|l| {
                    l.get(&format!("reactions/{}/{}", e, u))
                        .or_else(|| l.get("reactions"))
                })
                .copied()
                .unwrap_or(base);
            (t, e.as_str(), *u)
        })
        .collect();
    timed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)).then(a.2.cmp(&b.2)));
    let mut groups: Vec<(&str, Vec<u64>)> = vec![];
    for (_, e, u) in timed {
        match groups.iter_mut().find(|(x, _)| *x == e) {
            Some((_, users)) => users.push(u),
            None => groups.push((e, vec![u])),
        }
    }
    let Some(have) = v.as_array() else {
        return false;
    };
    have.len() == groups.len()
        && have.iter().zip(&groups).all(|(r, (e, users))| {
            r.as_object().is_some_and(|o| o.len() == 2)
                && r.get("emoji").and_then(|x| x.as_str()) == Some(*e)
                && r.get("users").and_then(|x| x.as_array()).is_some_and(|a| {
                    a.len() == users.len()
                        && a.iter()
                            .zip(users)
                            .all(|(x, u)| x.as_u64() == Some(*u) && x.is_u64())
                })
        })
}

/// One copy of an item, as `merge_fields` sees it.
pub(crate) struct Side<'a> {
    obj: &'a Map<String, Value>,
    parts: Vec<(String, std::borrow::Cow<'a, Value>)>,
    /// `parts` by name
    by_name: HashMap<String, usize>,
    /// when its fields not listed in `listed` changed
    base: i64,
    listed: Option<&'a BTreeMap<String, i64>>,
    omits: bool,
}

/// When a copy's version of `part` was set, or None when the copy can't
/// tell (it doesn't have the part, and may simply not know the field).
/// A part that is there changed at its stamp in the field stamps, or else
/// at the base. A missing part is taken away as of its stamp, or as of the
/// base when the copy knows it would be there: a field the app leaves out
/// at its default (`DEFAULT_WHEN_MISSING`, from apps that do), an element
/// missing from a list the copy has.
pub(crate) fn read_part(
    sd: &Side,
    part: &str,
    present: bool,
    in_vec: &std::collections::HashSet<u64>,
) -> Option<i64> {
    if let Some(e) = sd.listed {
        if let Some(t) = e.get(part) {
            return Some(*t);
        }
        // a stamp for a whole field from before it was split stands for
        // each of its elements
        if let Some((f, _)) = part.split_once('/') {
            if let Some(t) = e.get(f) {
                return Some(*t);
            }
        }
    }
    let counts = present
        || match part.split_once('/') {
            Some(("reactions", _)) => {
                sd.omits || sd.obj.get("reactions").is_some_and(|v| !is_empty_arr(v))
            }
            Some(("time_entries", _)) => {
                sd.omits || sd.obj.get("time_entries").is_some_and(|v| !is_empty_arr(v))
            }
            Some(("custom", _)) => {
                sd.omits
                    || sd
                        .obj
                        .get("custom")
                        .and_then(|v| v.as_object())
                        .is_some_and(|o| !o.is_empty())
            }
            Some(("timers", m)) => {
                let m: u64 = m.parse().unwrap_or(0);
                // an app that doesn't know `timers` can't have stopped one there
                sd.omits
                    || sd.obj.contains_key("timers")
                    || (sd.obj.contains_key("timer_started") && !in_vec.contains(&m))
            }
            _ => DEFAULT_WHEN_MISSING.contains(&part) && sd.omits,
        };
    counts.then_some(sd.base)
}

/// What `merge_fields` decided: the fields, the field names it decided
/// (ones it left out stay out), and for lists of items the entries without
/// an id that go after the merged items.
pub(crate) type Merged = (
    Map<String, Value>,
    std::collections::HashSet<String>,
    HashMap<String, Value>,
);

/// Merge the own fields of one item's copies part by part (see
/// `SyncMeta::fields`): for each part the copy that set it last wins.
/// `stamp` is the item's merged stamp, `newest` the newest copy's fields.
pub(crate) fn merge_fields<'a>(
    cx: &mut Ctx<'a>,
    id: u64,
    list: &[(usize, &'a Value)],
    stamp: i64,
    newest: &Map<String, Value>,
) -> Merged {
    use std::collections::HashSet;
    // fields holding items in any copy merge as items
    let child: HashSet<&str> = list
        .iter()
        .filter_map(|c| c.1.as_object())
        .flat_map(|o| {
            o.iter()
                .filter(|(_, v)| is_id_array(v))
                .map(|(k, _)| k.as_str())
        })
        .collect();
    let sides: Vec<Side> = list
        .iter()
        .filter_map(|c| {
            let obj = c.1.as_object()?;
            let s = cx.stamps[c.0].get(&id).copied().unwrap_or(0);
            let listed = cx.fields[c.0].get(&id);
            // a list of items keeps only its entries without an id here
            let parts: Vec<_> = parts_of(obj)
                .into_iter()
                .filter(|(p, _)| !child.contains(part_field(p)) || child.contains(p.as_str()))
                .collect();
            let mut by_name = HashMap::with_capacity(parts.len());
            for (i, (p, _)) in parts.iter().enumerate() {
                // the first of a name, as a search from the start finds
                by_name.entry(p.clone()).or_insert(i);
            }
            Some(Side {
                obj,
                parts,
                by_name,
                base: listed.and_then(|e| e.get("")).copied().unwrap_or(s),
                listed,
                omits: cx.omits_defaults.get(c.0).copied().unwrap_or(false),
            })
        })
        .collect();
    let any_entry = sides.iter().any(|s| s.listed.is_some());
    // parts in the order the newest copy has them, so every side ends up
    // with the same order
    let mut order: Vec<&str> = vec![];
    let mut seen = HashSet::new();
    let first = sides
        .iter()
        .position(|sd| std::ptr::eq(sd.obj, newest))
        .unwrap_or(0);
    for sd in sides[first..].iter().chain(&sides[..first]) {
        for (p, _) in &sd.parts {
            if seen.insert(p.as_str()) {
                order.push(p);
            }
        }
        for p in sd.listed.into_iter().flat_map(|e| e.keys()) {
            // (a whole split field's stamp is read for each of its elements)
            if !p.is_empty()
                && (!child.contains(part_field(p)) || child.contains(p.as_str()))
                && !SPLIT_LATER.contains(&p.as_str())
                && seen.insert(p.as_str())
            {
                order.push(p);
            }
        }
    }
    let in_vec: HashSet<u64> = sides
        .iter()
        .flat_map(|sd| timer_list(sd.obj).into_iter().filter(|t| t.2).map(|t| t.0))
        .collect();
    let base_out = sides.iter().map(|s| s.base).min().unwrap_or(stamp);
    let mut entry = BTreeMap::new();
    let mut obj = Map::new();
    let mut decided: HashSet<String> = HashSet::new();
    let mut reactions: Vec<(String, u64, i64)> = vec![];
    let mut timers: Vec<(u64, i64)> = vec![];
    let mut entries: Vec<(&str, Value)> = vec![];
    let mut custom = Map::new();
    let mut loose = HashMap::new();
    for p in order {
        let mut win: Option<(i64, &Side, Option<&Value>)> = None;
        for sd in &sides {
            let val = sd.by_name.get(p).map(|i| sd.parts[*i].1.as_ref());
            let Some(t) = read_part(sd, p, val.is_some(), &in_vec) else {
                continue;
            };
            if win.as_ref().map_or(true, |w| t > w.0) {
                win = Some((t, sd, val));
            }
        }
        let Some((t, sd, val)) = win else { continue };
        if any_entry && (val.is_none() || t != base_out) {
            entry.insert(p.to_string(), t);
        }
        if child.contains(p) {
            // the entries without an id of a list of items
            if let Some(v) = val {
                loose.insert(p.to_string(), v.clone());
            }
            continue;
        }
        match p.split_once('/') {
            Some(("reactions", rest)) => {
                decided.insert("reactions".into());
                if let (Some(_), Some((e, u))) = (val, rest.rsplit_once('/')) {
                    if let Ok(u) = u.parse() {
                        reactions.push((e.to_string(), u, t));
                    }
                }
            }
            Some(("time_entries", _)) => {
                decided.insert("time_entries".into());
                if let Some(v) = val {
                    entries.push((p, v.clone()));
                }
            }
            Some(("custom", key)) => {
                decided.insert("custom".into());
                if let Some(v) = val {
                    custom.insert(key.to_string(), v.clone());
                }
            }
            Some(("timers", _)) => {
                for k in TIMER_KEYS {
                    decided.insert(k.to_string());
                }
                if let Some(v) = val {
                    timers.push((
                        v["member"].as_u64().unwrap_or(0),
                        v["start"].as_i64().unwrap_or(0),
                    ));
                }
            }
            _ => {
                decided.insert(p.to_string());
                match val {
                    Some(v) => {
                        obj.insert(p.to_string(), v.clone());
                    }
                    // taken away there; keep how that copy writes it (an empty list)
                    None => {
                        if let Some(v) = sd.obj.get(p) {
                            obj.insert(p.to_string(), v.clone());
                        }
                    }
                }
            }
        }
    }
    if decided.contains("reactions") {
        put_reactions(&mut obj, reactions, newest);
    }
    if decided.contains("timers") {
        put_timers(&mut obj, timers, &sides);
    }
    if decided.contains("time_entries") {
        put_time_entries(&mut obj, entries, &sides);
    }
    if decided.contains("custom") {
        put_custom(&mut obj, custom, &sides);
    }
    if any_entry {
        if base_out != stamp {
            entry.insert(String::new(), base_out);
        }
        cx.out_fields.insert(id, entry);
    }
    (obj, decided, loose)
}

/// The merged reactions, one per (emoji, person), into `obj`, each with
/// the time it was added.
fn put_reactions(
    obj: &mut Map<String, Value>,
    mut reactions: Vec<(String, u64, i64)>,
    newest: &Map<String, Value>,
) {
    // in the order they were added, the same on every device whatever
    // order the copies had them in: emoji by when someone first left it,
    // people by when they did (ties by emoji, then by person)
    reactions.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
    let mut out: Vec<(String, Vec<u64>)> = vec![];
    for (e, u, _) in reactions {
        match out.iter_mut().find(|(x, _)| *x == e) {
            Some((_, users)) => users.push(u),
            None => out.push((e, vec![u])),
        }
    }
    if !out.is_empty() || newest.contains_key("reactions") {
        obj.insert(
            "reactions".into(),
            Value::Array(
                out.into_iter()
                    .map(|(e, users)| serde_json::json!({"emoji": e, "users": users}))
                    .collect(),
            ),
        );
    }
}

/// The merged running timers, one per person, into `obj`: the first in
/// the field older apps read, the rest in `timers`.
fn put_timers(obj: &mut Map<String, Value>, mut timers: Vec<(u64, i64)>, sides: &[Side]) {
    // the same order everywhere: an old timer nobody owns first (only
    // the field older apps read can hold it), then by start
    timers.sort_by_key(|(m, st)| (*m != 0, *st, *m));
    let mut rest = timers.into_iter();
    match rest.next() {
        Some((m, st)) => {
            obj.insert("timer_started".into(), Value::from(st));
            if m != 0 {
                obj.insert("timer_by".into(), Value::from(m));
            }
        }
        None => {
            if sides.iter().any(|sd| sd.obj.contains_key("timer_started")) {
                obj.insert("timer_started".into(), Value::Null);
            }
        }
    }
    let rest: Vec<Value> = rest
        .map(|(m, st)| serde_json::json!({"member": m, "start": st}))
        .collect();
    if !rest.is_empty() {
        obj.insert("timers".into(), Value::Array(rest));
    }
}

/// The merged time entries, one per (person, start), into `obj`.
fn put_time_entries(obj: &mut Map<String, Value>, mut entries: Vec<(&str, Value)>, sides: &[Side]) {
    // the same order everywhere: by start, then whose
    let key = |v: &Value| {
        (
            v.get("start").and_then(|s| s.as_i64()).unwrap_or(0),
            v.get("member").and_then(|m| m.as_u64()).unwrap_or(0),
        )
    };
    entries.sort_by(|(p, a), (q, b)| key(a).cmp(&key(b)).then_with(|| p.cmp(q)));
    if !entries.is_empty() || sides.iter().any(|sd| sd.obj.contains_key("time_entries")) {
        obj.insert(
            "time_entries".into(),
            Value::Array(entries.into_iter().map(|(_, v)| v).collect()),
        );
    }
}

/// The merged custom field values, one per field, into `obj`.
fn put_custom(obj: &mut Map<String, Value>, custom: Map<String, Value>, sides: &[Side]) {
    if !custom.is_empty() || sides.iter().any(|sd| sd.obj.contains_key("custom")) {
        obj.insert("custom".into(), Value::Object(custom));
    }
}

/// Merge one array of items. `parent` is the item it sits in (None at the
/// top). An item that moved sits only where its newest copy puts it, and its
/// copies from every place are merged, so nothing added inside it is lost.
pub(crate) fn merge_arr<'a>(
    cx: &mut Ctx<'a>,
    sources: &[(usize, &'a Vec<Value>)],
    parent: Option<u64>,
) -> Vec<Value> {
    let mut order: Vec<u64> = vec![];
    let mut seen = std::collections::HashSet::new();
    for (_, arr) in sources {
        for v in arr.iter() {
            let Some(id) = id_of(v) else { continue };
            if seen.insert(id) {
                order.push(id);
            }
        }
    }
    let mut out = vec![];
    for id in order {
        if !cx.alive(id) {
            continue;
        }
        if parent.is_some() && cx.home.get(&id).is_some_and(|h| *h != parent) {
            continue;
        }
        if cx.untouched(id) {
            #[cfg(test)]
            QUICK.with(|q| q.set(q.get() + 1));
            let x = cx.cands[&id][0].1;
            cx.keep_untouched(x);
            let at = match (&cx.top, parent) {
                (Some((path, places)), None) => places.get(&id).map(|i| (*path, *i)),
                _ => None,
            };
            match at {
                Some(slot) => {
                    // a stand-in, sorted like the item: it has its time
                    let mut o = Map::new();
                    o.insert(KEEP.into(), Value::from(cx.slots.len()));
                    if let Some(ts) = x.get("ts") {
                        o.insert("ts".into(), ts.clone());
                    }
                    cx.slots.push(slot);
                    out.push(Value::Object(o));
                }
                None => out.push(x.clone()),
            }
            continue;
        }
        let Some(list) = cx.cands.get(&id).cloned() else {
            continue;
        };
        let list = &list;
        // newest own fields win; on a tie this computer's version stays
        let mut best = list[0];
        let mut best_stamp = cx.stamps[best.0].get(&id).copied().unwrap_or(0);
        for c in &list[1..] {
            let s = cx.stamps[c.0].get(&id).copied().unwrap_or(0);
            if s > best_stamp {
                best = *c;
                best_stamp = s;
            }
        }
        cx.out.insert(id, best_stamp);
        if let Some(m) = cx.home_moved.get(&id).filter(|m| **m > 0) {
            cx.out_moved.insert(id, *m);
        }
        let Some(wobj) = best.1.as_object() else {
            continue;
        };
        let local = list.iter().find(|c| c.0 == 0).and_then(|c| c.1.as_object());
        let mut keys: Vec<String> = wobj.keys().cloned().collect();
        for c in list {
            if let Some(o) = c.1.as_object() {
                for k in o.keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }
        }
        // own fields first, field by field (see `merge_fields`)
        let (mut obj, decided, loose) = merge_fields(cx, id, list, best_stamp, wobj);
        for k in keys {
            if decided.contains(k.as_str()) || obj.contains_key(&k) {
                continue;
            }
            let vals: Vec<(usize, &'a Value)> = list
                .iter()
                .filter_map(|c| c.1.get(&k).map(|v| (c.0, v)))
                .collect();
            let value = if vals.iter().any(|(_, v)| is_id_array(v)) {
                let subs: Vec<(usize, &'a Vec<Value>)> = vals
                    .iter()
                    .filter_map(|(s, v)| v.as_array().map(|a| (*s, a)))
                    .collect();
                let mut items = merge_arr(cx, &subs, Some(id));
                // entries without an id stay as they are, after the items
                if let Some(Value::Array(rest)) = loose.get(&k) {
                    items.extend(rest.iter().cloned());
                }
                Value::Array(items)
            } else if APPEND_KEYS.contains(&k.as_str()) {
                let mut all: Vec<&Value> = vec![];
                // entries by fingerprint, to find ones already there
                // without comparing each with every other
                let mut by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
                for (_, v) in &vals {
                    for x in v.as_array().into_iter().flatten() {
                        let at = by_hash.entry(fnv_json(x)).or_default();
                        if !at.iter().any(|i| all[*i] == x) {
                            at.push(all.len());
                            all.push(x);
                        }
                    }
                }
                all.sort_by_key(|x| x.get("ts").and_then(|t| t.as_i64()).unwrap_or(0));
                Value::Array(all.into_iter().cloned().collect())
            } else if LOCAL_KEYS.contains(&k.as_str())
                && local.map_or(false, |o| o.contains_key(&k))
            {
                local.unwrap()[&k].clone()
            } else if let Some(v) = wobj.get(&k) {
                v.clone()
            } else if DEFAULT_WHEN_MISSING.contains(&k.as_str())
                && cx.omits_defaults.get(best.0).copied().unwrap_or(false)
            {
                // the newest copy has it at its default (see `omits_defaults`)
                continue;
            } else {
                vals[0].1.clone()
            };
            obj.insert(k, value);
        }
        out.push(Value::Object(obj));
    }
    // messages and replies read in time order
    if !out.is_empty()
        && out
            .iter()
            .all(|x| x.get("ts").map_or(false, |t| t.is_i64()))
    {
        out.sort_by_key(|x| x["ts"].as_i64().unwrap_or(0));
    }
    out
}

/// Merge other people's copies into this one. Returns true when anything changed.
pub fn merge(local: &mut Value, others: &[Value]) -> bool {
    let mut meta = stamps_of(local);
    let changed = merge_apart(local, &mut meta, others, false);
    set_path(
        local,
        "extra.sync",
        serde_json::to_value(&meta).unwrap_or_default(),
    );
    changed
}

/// `merge` for a copy whose bookkeeping is kept apart from it: `meta` is
/// this copy's (any in `local` is not read or written) and becomes the
/// merged one. With `current`, `meta`'s fingerprints (`hashes`,
/// `field_hashes`, `parents`) are exactly what `local` gives, as `stamp`
/// leaves them: the items that come out of the merge as they went in keep
/// theirs instead of being fingerprinted again. Returns true when
/// anything changed.
pub fn merge_apart(
    local: &mut Value,
    meta: &mut SyncMeta,
    others: &[Value],
    current: bool,
) -> bool {
    // copies from before activity had ids: give their entries the same ids
    // here as everywhere else, or their comments would not be matched up
    let filled_here = fill_activity_ids(local) | fill_board_ids(local) | fill_goal_ids(local);
    // ids given here change the items they're in: fingerprinted again
    let current = current && !filled_here;
    let filled: Vec<Value> = others
        .iter()
        .map(|o| {
            let mut o = o.clone();
            fill_activity_ids(&mut o);
            fill_board_ids(&mut o);
            fill_goal_ids(&mut o);
            o
        })
        .collect();
    let others = &filled[..];
    // this copy's items, taken out while the merged ones are made (and
    // compared with them after)
    let mut before: Vec<Option<Value>> = SHARED
        .iter()
        .map(|p| get_path_mut(local, p).map(std::mem::take))
        .collect();
    let lmeta = meta;
    let metas: Vec<SyncMeta> = others.iter().map(stamps_of).collect();
    let mut tombs = lmeta.tombs.clone();
    let mut revived = lmeta.revived.clone();
    for m in &metas {
        for (k, v) in &m.tombs {
            let e = tombs.entry(*k).or_insert(*v);
            *e = (*e).max(*v);
        }
        for (k, v) in &m.revived {
            let e = revived.entry(*k).or_insert(*v);
            *e = (*e).max(*v);
        }
    }
    let empty = vec![];
    let mut merged: Vec<(String, Value)>;
    let out_stamps;
    let out_moved;
    let out_fields;
    let kept;
    let slots;
    {
        let mut stamps: Vec<&BTreeMap<u64, i64>> = vec![&lmeta.stamps];
        stamps.extend(metas.iter().map(|m| &m.stamps));
        let mut fields: Vec<&BTreeMap<u64, BTreeMap<String, i64>>> = vec![&lmeta.fields];
        fields.extend(metas.iter().map(|m| &m.fields));
        let keeps = |m: &SyncMeta| m.v >= 2;
        let mut moved: Vec<Option<&BTreeMap<u64, i64>>> =
            vec![keeps(lmeta).then_some(&lmeta.moved)];
        moved.extend(metas.iter().map(|m| keeps(m).then_some(&m.moved)));
        let mut all: Vec<Vec<(usize, &Vec<Value>)>> = vec![];
        let mut cands = HashMap::new();
        for (path, mine) in SHARED.iter().zip(&before) {
            let mut sources: Vec<(usize, &Vec<Value>)> = vec![(
                0,
                mine.as_ref().and_then(|v| v.as_array()).unwrap_or(&empty),
            )];
            for (i, o) in others.iter().enumerate() {
                if let Some(a) = get_path(o, path).and_then(|v| v.as_array()) {
                    sources.push((i + 1, a));
                }
            }
            for (src, arr) in &sources {
                collect(*src, arr, None, &mut cands);
            }
            all.push(sources);
        }
        let mut cx = Ctx::new(stamps, fields, moved, tombs.clone(), revived.clone(), cands);
        let mut res = vec![];
        for (n, (path, sources)) in SHARED.iter().zip(&all).enumerate() {
            let places: HashMap<u64, usize> = sources[0]
                .1
                .iter()
                .enumerate()
                .filter_map(|(i, x)| Some((id_of(x)?, i)))
                .collect();
            cx.top = Some((n, places));
            let mut items = merge_arr(&mut cx, sources, None);
            // this computer's entries without an id stay as they are
            items.extend(sources[0].1.iter().filter(|x| id_of(x).is_none()).cloned());
            res.push((path.to_string(), Value::Array(items)));
        }
        merged = res;
        out_stamps = cx.out;
        out_moved = cx.out_moved;
        out_fields = cx.out_fields;
        kept = cx.kept;
        slots = cx.slots;
    }
    // changed: compared without the bookkeeping, which always moves a little
    let mut changed = false;
    for (n, ((path, v), mine)) in merged.iter_mut().zip(&mut before).enumerate() {
        changed |= put_back(v, mine, n, &slots);
        set_path(local, path, std::mem::take(v));
    }
    // shared settings: the newest change wins; on a tie this computer's stays
    for path in SHARED_VALUES {
        let mut best: Option<&Value> = None;
        let mut best_stamp = lmeta.values.get(*path).copied().unwrap_or(0);
        for (o, m) in others.iter().zip(&metas) {
            let s = m.values.get(*path).copied().unwrap_or(0);
            if s > best_stamp {
                if let Some(v) = get_path(o, path) {
                    best = Some(v);
                    best_stamp = s;
                }
            }
        }
        if let Some(v) = best {
            let was = get_path(local, path).cloned();
            set_path(local, path, v.clone());
            changed |= get_path(local, path) != was.as_ref();
        }
        if best_stamp > 0 {
            lmeta.values.insert(path.to_string(), best_stamp);
        }
    }
    let maps_before: Vec<Option<Value>> = SHARED_MAPS
        .iter()
        .map(|p| get_path(local, p).cloned())
        .collect();
    merge_maps(local, lmeta, others, &metas);
    for (p, was) in SHARED_MAPS.iter().zip(&maps_before) {
        changed |= get_path(local, p) != was.as_ref();
    }
    lmeta.tombs = tombs;
    lmeta.revived = revived;
    lmeta.stamps = out_stamps;
    lmeta.moved = out_moved;
    shrink_moved(&mut lmeta.moved, local);
    lmeta.fields = out_fields;
    if current {
        rehash_changed(local, lmeta, &kept);
    } else {
        let all = hash_all(local);
        lmeta.hashes = all.hashes;
        lmeta.field_hashes = all.parts;
        lmeta.parents = all.parents;
    }
    lmeta.value_hashes = value_hashes(local);
    lmeta.v = META_V;
    changed
}

/// A top-level collection as merged (`out`): the untouched items of this
/// copy's list (`mine`, collection `n`) moved into their stand-ins
/// (`Ctx::top`). True when it isn't what this copy had: as comparing it
/// with `mine` before anything was moved would say.
fn put_back(out: &mut Value, mine: &mut Option<Value>, n: usize, slots: &[(usize, usize)]) -> bool {
    let Some(items) = out.as_array_mut() else {
        return true;
    };
    // where each item came from in `mine`, for the moved ones
    let mut from: Vec<Option<usize>> = vec![None; items.len()];
    let mut taken = vec![];
    for (i, x) in items.iter_mut().enumerate() {
        let Some(k) = x.get(KEEP).and_then(|k| k.as_u64()) else {
            continue;
        };
        let (path, at) = slots[k as usize];
        debug_assert_eq!(path, n);
        let Some(arr) = mine.as_mut().and_then(|m| m.as_array_mut()) else {
            continue;
        };
        *x = std::mem::take(&mut arr[at]);
        from[i] = Some(at);
        taken.push(at);
    }
    let Some(arr) = mine.as_ref().and_then(|m| m.as_array()) else {
        return true;
    };
    if arr.len() != items.len() {
        return true;
    }
    let mut gone = vec![false; arr.len()];
    for at in taken {
        gone[at] = true;
    }
    // an untouched item is only ever equal to itself (ids don't repeat)
    items.iter().enumerate().any(|(i, x)| match from[i] {
        Some(at) => at != i,
        None => gone[i] || *x != arr[i],
    })
}

/// The fingerprints of a merged copy (`local`) from those of the copy that
/// went in (`meta`, see `merge_apart`'s `current`): items in `kept` (and
/// everything inside them) keep theirs, the others are fingerprinted
/// again, and items no longer there are left out. The same as `hash_all`.
fn rehash_changed(local: &Value, meta: &mut SyncMeta, kept: &std::collections::HashSet<u64>) {
    use std::collections::HashSet;
    struct Walk<'k> {
        kept: &'k HashSet<u64>,
        seen: HashSet<u64>,
        parents: BTreeMap<u64, u64>,
    }
    fn go(v: &Value, parent: Option<u64>, inside_kept: bool, w: &mut Walk, meta: &mut SyncMeta) {
        for item in v.as_array().into_iter().flatten() {
            let (Some(id), Some(obj)) = (id_of(item), item.as_object()) else {
                continue;
            };
            w.seen.insert(id);
            if let Some(p) = parent {
                w.parents.insert(id, p);
            }
            // fingerprinted in the same place as now
            let keep = (inside_kept || w.kept.contains(&id))
                && meta.hashes.contains_key(&id)
                && meta.field_hashes.contains_key(&id)
                && meta.parents.get(&id) == parent.as_ref();
            if !keep {
                let h = own_hash(obj);
                let h = match parent {
                    Some(p) => fnv(format!("{}/{}", h, p).as_bytes()),
                    None => h,
                };
                meta.hashes.insert(id, h);
                meta.field_hashes.insert(
                    id,
                    parts_of(obj)
                        .into_iter()
                        .map(|(p, v)| (p, part_hash(&v)))
                        .collect(),
                );
            }
            for (_, child) in obj {
                if is_id_array(child) {
                    go(child, Some(id), keep, w, meta);
                }
            }
        }
    }
    let mut w = Walk {
        kept,
        seen: HashSet::new(),
        parents: BTreeMap::new(),
    };
    for path in SHARED {
        if let Some(v) = get_path(local, path) {
            go(v, None, false, &mut w, meta);
        }
    }
    meta.hashes.retain(|id, _| w.seen.contains(id));
    meta.field_hashes.retain(|id, _| w.seen.contains(id));
    meta.parents = w.parents;
}

/// Shared maps (`SHARED_MAPS`): for each key the newest change wins (a
/// stamp of 0 is a copy that never had it changed). On a tie an entry
/// beats none, then the larger value, so every copy ends up the same; an
/// entry left out with a newer stamp than any copy's value was taken away.
pub(crate) fn merge_maps(
    local: &mut Value,
    lmeta: &mut SyncMeta,
    others: &[Value],
    metas: &[SyncMeta],
) {
    for path in SHARED_MAPS {
        let mine = get_path(local, path).and_then(|m| m.as_object()).cloned();
        let mut sides: Vec<(&BTreeMap<String, i64>, Option<&Map<String, Value>>)> =
            vec![(&lmeta.values, mine.as_ref())];
        for (o, m) in others.iter().zip(metas) {
            sides.push((&m.values, get_path(o, path).and_then(|x| x.as_object())));
        }
        let mut keys: std::collections::BTreeSet<String> = Default::default();
        for (values, obj) in &sides {
            keys.extend(obj.into_iter().flat_map(|o| o.keys().cloned()));
            keys.extend(values.keys().filter_map(|p| match map_entry(p) {
                Some((mp, k)) if mp == *path => Some(k.to_string()),
                _ => None,
            }));
        }
        let mut out = mine.clone().unwrap_or_default();
        let mut stamps = vec![];
        for key in keys {
            let part = map_part(path, &key);
            let mut best: Option<(i64, Option<&Value>)> = None;
            for (values, obj) in &sides {
                let s = values.get(&part).copied().unwrap_or(0);
                let v = obj.and_then(|o| o.get(&key));
                let better = match best {
                    None => true,
                    Some((bs, bv)) => match (s, v.is_some()).cmp(&(bs, bv.is_some())) {
                        std::cmp::Ordering::Greater => true,
                        std::cmp::Ordering::Less => false,
                        std::cmp::Ordering::Equal => match (v, bv) {
                            (Some(a), Some(b)) => {
                                serde_json::to_string(a).unwrap_or_default()
                                    > serde_json::to_string(b).unwrap_or_default()
                            }
                            _ => false,
                        },
                    },
                };
                if better {
                    best = Some((s, v));
                }
            }
            let Some((s, v)) = best else {
                continue;
            };
            match v {
                Some(v) => {
                    out.insert(key.clone(), v.clone());
                }
                None => {
                    out.remove(&key);
                }
            }
            if s > 0 {
                stamps.push((part, s));
            }
        }
        lmeta.values.extend(stamps);
        if mine.is_some() || !out.is_empty() {
            set_path(local, path, Value::Object(out));
        }
    }
}
