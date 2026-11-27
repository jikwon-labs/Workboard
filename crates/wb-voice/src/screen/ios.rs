// Screen sharing on iPad (see screen.rs): nothing to share yet.

use super::*;

pub fn allowed() -> bool {
    true
}
pub fn ask() -> bool {
    true
}
pub fn sources() -> Result<Vec<Source>, ScreenTrouble> {
    Ok(Vec::new())
}
pub fn capture(
    _share: Share,
    _out: &Out,
    _limits: &Limits,
    _stop: &AtomicBool,
) -> Result<(), ScreenTrouble> {
    Err(ScreenTrouble::Failed(
        "Screen sharing isn't available on iPad yet.".into(),
    ))
}
