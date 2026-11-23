// The window's size and place, kept in window.json beside the data so the
// app opens as it was left. The first start has no file: the usual
// 1440 x 900 in the middle of the screen.

use crate::model::Data;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The size the window opens at when nothing was kept.
pub const DEFAULT_SIZE: [f32; 2] = [1440.0, 900.0];

/// The smallest window the app allows (narrower ones get the drawer and
/// then one column, src/app/shell/narrow.rs).
pub const MIN_SIZE: [f32; 2] = [360.0, 520.0];

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct WinState {
    /// inside size, in points
    pub w: f32,
    pub h: f32,
    /// top left of the window on the screen, in points
    pub x: f32,
    pub y: f32,
}

fn path() -> PathBuf {
    Data::dir().join("window.json")
}

/// What was kept, if it makes a usable window.
pub fn load() -> Option<WinState> {
    let s = std::fs::read_to_string(path()).ok()?;
    let st: WinState = serde_json::from_str(&s).ok()?;
    let ok = [st.w, st.h, st.x, st.y].iter().all(|v| v.is_finite())
        && st.w >= MIN_SIZE[0]
        && st.h >= MIN_SIZE[1];
    ok.then_some(st)
}

pub fn save(st: WinState) {
    if let Ok(s) = serde_json::to_string(&st) {
        let _ = std::fs::write(path(), s);
    }
}
