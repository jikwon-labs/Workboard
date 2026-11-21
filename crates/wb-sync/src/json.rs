// Reading and writing a workspace copy's JSON: values at dotted paths
// ("extra.apps"), item ids, plain field getters, and the sync metadata a
// copy keeps in extra.sync. The server reads and guards copies with these
// too. None of them panics, whatever shape the copy has.

use super::SyncMeta;
use serde_json::{Map, Value};

/// The value at a dotted path ("extra.workspace_color").
pub fn get_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

/// The value at a dotted path, to change.
pub fn get_path_mut<'a>(v: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get_mut(part)?;
    }
    Some(cur)
}

/// Set the value at a dotted path, making the objects on the way that are
/// missing. Where something other than an object (or null) is on the way,
/// nothing is written.
pub fn set_path(v: &mut Value, path: &str, new: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut cur = v;
    for part in &parts[..parts.len() - 1] {
        if !(cur.is_object() || cur.is_null()) {
            return;
        }
        if cur.get(*part).is_none() {
            cur[*part] = Value::Object(Map::new());
        }
        let Some(next) = cur.get_mut(*part) else {
            return;
        };
        cur = next;
    }
    if cur.is_object() || cur.is_null() {
        cur[parts[parts.len() - 1]] = new;
    }
}

/// Set (Some) or take away (None) the value at a dotted path. Setting makes
/// objects on the way, replacing whatever else is there; taking away makes
/// nothing. A copy that isn't an object is left alone.
pub fn put_path(v: &mut Value, path: &str, new: Option<Value>) {
    let mut parts: Vec<&str> = path.split('.').collect();
    let Some(last) = parts.pop() else { return };
    let mut cur = v;
    for part in parts {
        if !cur.get(part).is_some_and(|x| x.is_object()) {
            if new.is_none() || !cur.is_object() {
                return;
            }
            cur[part] = Value::Object(Map::new());
        }
        cur = &mut cur[part];
    }
    let Some(obj) = cur.as_object_mut() else {
        return;
    };
    match new {
        Some(n) => {
            obj.insert(last.into(), n);
        }
        None => {
            obj.remove(last);
        }
    }
}

/// An item's id. 0 is no id: apps leave an id out of the file until one is
/// given (a stroke just drawn), and one written as 0 means the same.
pub fn id_of(v: &Value) -> Option<u64> {
    v.get("id")?.as_u64().filter(|i| *i != 0)
}

/// The number written in an item's "id" field, 0 included (unlike `id_of`,
/// for which 0 is no id). The server's guards go by this one.
pub fn written_id(v: &Value) -> Option<u64> {
    v.get("id")?.as_u64()
}

/// The array at a dotted path; empty when there is none.
pub fn array_at<'a>(v: &'a Value, path: &str) -> &'a [Value] {
    get_path(v, path)
        .and_then(|a| a.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// The array at a dotted path, to change.
pub fn array_at_mut<'a>(v: &'a mut Value, path: &str) -> Option<&'a mut Vec<Value>> {
    get_path_mut(v, path).and_then(|a| a.as_array_mut())
}

/// A whole-number field of an object ("id", "author").
pub fn u64_field(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|x| x.as_u64())
}

/// A text field of an object ("name", "text"); "" when there is none.
pub fn text_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

/// The name others see a member record by, as the apps show it
/// (`Member::shown`): the display name when there is one, else the full
/// name, both trimmed. "" when neither is set.
pub fn shown_name(member: &Value) -> String {
    let d = text_field(member, "display_name");
    let d = d.trim();
    if d.is_empty() {
        text_field(member, "name").trim().to_string()
    } else {
        d.to_string()
    }
}

/// A copy's sync metadata (extra.sync); the default when it has none or it
/// can't be read.
pub fn meta_of(v: &Value) -> SyncMeta {
    v.get("extra")
        .and_then(|e| e.get("sync"))
        .and_then(|s| serde_json::from_value(s.clone()).ok())
        .unwrap_or_default()
}

/// Write a copy's sync metadata as its extra.sync, making `extra` an object
/// first when it isn't one. A copy that isn't an object is left alone.
pub fn set_meta(v: &mut Value, meta: &SyncMeta) {
    if v.is_object() && !v.get("extra").is_some_and(|e| e.is_object()) {
        v["extra"] = Value::Object(Map::new());
    }
    if let Some(extra) = v.get_mut("extra").and_then(|e| e.as_object_mut()) {
        extra.insert(
            "sync".into(),
            serde_json::to_value(meta).unwrap_or_default(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_read_and_write_and_never_panic() {
        let mut v = json!({"extra": {"apps": [1]}, "workspace": "W"});
        assert_eq!(get_path(&v, "extra.apps"), Some(&json!([1])));
        assert_eq!(array_at(&v, "extra.apps").len(), 1);
        assert!(array_at(&v, "extra.none").is_empty());
        set_path(&mut v, "extra.workspace_color", json!([1, 2, 3]));
        assert_eq!(v["extra"]["workspace_color"], json!([1, 2, 3]));
        set_path(&mut v, "extra.new.deep", json!(1));
        assert_eq!(v["extra"]["new"]["deep"], json!(1));
        // something else in the way: nothing written, no panic
        set_path(&mut v, "workspace.color", json!(1));
        assert_eq!(v["workspace"], json!("W"));
        let mut s = json!("not a copy");
        set_path(&mut s, "a", json!(1));
        set_path(&mut s, "a.b", json!(1));
        assert_eq!(s, json!("not a copy"));
        // a null on the way becomes an object, as before
        let mut n = json!({"extra": null});
        set_path(&mut n, "extra.apps", json!([]));
        assert_eq!(n, json!({"extra": {"apps": []}}));
        put_path(&mut v, "extra.apps", None);
        assert!(get_path(&v, "extra.apps").is_none());
        put_path(&mut v, "workspace.x", Some(json!(1)));
        assert_eq!(v["workspace"], json!({"x": 1}));
    }

    #[test]
    fn shown_name_prefers_a_display_name_and_trims() {
        let m = |name: &str, display: &str| json!({"name": name, "display_name": display});
        assert_eq!(shown_name(&m(" Kim Sam ", "")), "Kim Sam");
        assert_eq!(shown_name(&m("Kim Sam", "  ")), "Kim Sam");
        assert_eq!(shown_name(&m("Kim Sam", " Sam ")), "Sam");
        assert_eq!(shown_name(&m("   ", "")), "");
        assert_eq!(shown_name(&json!({"id": 1})), "");
    }

    #[test]
    fn id_of_takes_zero_as_none_and_written_id_does_not() {
        assert_eq!(id_of(&json!({"id": 0})), None);
        assert_eq!(written_id(&json!({"id": 0})), Some(0));
        assert_eq!(id_of(&json!({"id": 5})), Some(5));
        assert_eq!(written_id(&json!({"id": "5"})), None);
    }

    #[test]
    fn meta_goes_into_extra_whatever_was_there() {
        let mut meta = SyncMeta::default();
        meta.stamps.insert(1, 2);
        for start in [json!({}), json!({"extra": null}), json!({"extra": "x"})] {
            let mut v = start;
            set_meta(&mut v, &meta);
            assert_eq!(meta_of(&v).stamps.get(&1), Some(&2));
        }
        let mut s = json!([1]);
        set_meta(&mut s, &meta);
        assert_eq!(s, json!([1]));
    }
}
