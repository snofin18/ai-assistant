//! Window screenshot pipeline for architecture v2 section 13 (ADR-0071).
//!
//! Responsibilities:
//! - orchestrate one window screenshot end to end: platform capture, redaction,
//!   scroll cleanup, and privacy-mode retention;
//! - own the capture-side retention policy and its explicit failure modes.
//!
//! Boundaries:
//! - no platform API calls; the only screenshot primitive is the
//!   `WindowProvider::capture` trait from `crates/platform/api` (iron rule 7);
//! - no policy or egress decision (those live in `crates/dlp` / `crates/policy`);
//! - no third-party dependency yet: this crate is a boundary skeleton.
//!
//! Invariants:
//! 1. a capture is always scoped to one resolved window, never the whole screen;
//! 2. privacy mode never persists image bytes;
//! 3. an unsupported or failed capture is an explicit error, never a blank image.
//!
//! This is a boundary skeleton created by TASK-238: ADR-0071 freezes the edge
//! before any implementation (iron rule 10). TASK-041 split A now implements
//! the zero-dependency orchestration, redaction decisions, and bounded scroll
//! plan; real pixels and platform calls remain outside this crate.

mod error;
mod pipeline;
mod privacy;
mod scroll;

pub use error::CaptureError;
pub use pipeline::{CaptureOutcome, CapturePipeline, RedactionPolicy};
pub use privacy::PrivacyMode;
pub use scroll::{MAX_SCROLL_STEPS, ScrollCleanupError, ScrollCleanupPlan, ScrollMergeDirection};
