Vendored from crates.io nokhwa-bindings-macos 0.2.4 for one fix: `AVCaptureDevice::lock` now records the lock (`locked = true`) so `unlock` releases it. Without it every lock leaked and other apps could not use the camera while Workboard ran. Used through `[patch.crates-io]` in the root Cargo.toml.

# nokhwa-bindings-macos
This crate is the AVFoundation bindings for the `nokhwa` crate.

It is not meant for general consumption. If you are looking for a MacOS camera capture crate, consider using `nokhwa` with feature `input-native`.

No support or API stability will be given. Subject to change at any time.
