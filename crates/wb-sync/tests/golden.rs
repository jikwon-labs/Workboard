// Fixed fingerprints for a fixed workspace. The iPhone app works these out
// on its own (ios/WorkboardKit, Sync.swift) and compares them with the
// ones the Mac app and the server saved, so a change to any of them, from a
// refactor or a new serde_json feature, would make every copy look edited.
// The iPhone tests (GoldenTests.swift) check the same expected.json.
//
// To change them on purpose (a new META_V): WB_GOLDEN_WRITE=1 cargo test
// -p wb-sync --test golden, then review the diff of expected.json.

use serde_json::{json, Map, Value};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn num_map<V: ToString>(m: impl IntoIterator<Item = (impl ToString, V)>) -> Value {
    Value::Object(
        m.into_iter()
            .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
            .collect::<Map<String, Value>>(),
    )
}

fn computed() -> Value {
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("workspace.json")).unwrap())
            .unwrap();
    let parts: Map<String, Value> = wb_sync::part_hashes(&data)
        .into_iter()
        .map(|(id, p)| (id.to_string(), num_map(p)))
        .collect();
    json!({
        "fingerprints": num_map(wb_sync::fingerprints(&data)),
        "part_hashes": parts,
        "value_hashes": num_map(wb_sync::value_hashes(&data)),
        "fnv": num_map([
            ("", wb_sync::fnv(b"")),
            ("a", wb_sync::fnv(b"a")),
            ("팀 👍", wb_sync::fnv("팀 👍".as_bytes())),
        ]),
        "activity_id": wb_sync::activity_id(1001, 1763000000, 2, true, "made it ✓", 1).to_string(),
        "board_item_id": wb_sync::board_item_id(6001, "strokes", "0,0,0|1.5,2.25", 2).to_string(),
    })
}

#[test]
fn fingerprints_of_a_fixed_workspace_never_change() {
    let got = computed();
    let path = dir().join("expected.json");
    if std::env::var("WB_GOLDEN_WRITE").is_ok_and(|v| v == "1") {
        std::fs::write(&path, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
    }
    let want: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(
        got == want,
        "fingerprints changed; every saved copy would look edited.\n got: {}",
        serde_json::to_string_pretty(&got).unwrap()
    );
}
