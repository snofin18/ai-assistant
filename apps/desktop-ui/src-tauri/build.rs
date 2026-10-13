//! Tauri build-script entry point.
//!
//! Responsibility: invoke `tauri_build` so generated desktop assets exist before the binary builds.
//! Boundary: no application runtime logic and no direct filesystem or network access.
//! Invariant: this script only delegates to the Tauri-owned build hook.

fn main() {
    tauri_build::build();
}
