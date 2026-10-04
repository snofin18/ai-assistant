//! Data-loss-prevention policies for architecture v2 section 12 (ADR-0071).
//!
//! Responsibilities:
//! - model the three egress tiers `local_only` / `redacted` / `full` (`[ADR:待建 0007]`)
//!   with per-application and per-content-type overrides;
//! - hold redaction rules and expose screenshot occlusion for `crates/capture`.
//!
//! Boundaries:
//! - no platform API calls (iron rule 7);
//! - no capture orchestration; scroll cleanup and retention live in
//!   `crates/capture`;
//! - no third-party dependency yet: this crate is a boundary skeleton.
//!
//! Invariants:
//! 1. `local_only` without a local model is an explicit error, never a silent
//!    failover to a cloud path;
//! 2. redaction is fail-closed: an unclassifiable region is over-redacted;
//! 3. any long-lived rule table has a hard bound and an eviction or rejection
//!    policy (ADR-0063).
//!
//! This is a boundary skeleton created by TASK-238: ADR-0071 freezes the edge
//! before any implementation (iron rule 10). The egress policy is TASK-050 and
//! the redaction rules are TASK-041 / TASK-050, so the crate intentionally
//! exposes no public item yet.
