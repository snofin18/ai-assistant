//! Rollback anchors, recipes, conflict detection, and incident reporting for architecture v2
//! section 9.
//!
//! Responsibilities:
//! - model the four reversibility levels and compute the worst level in a task;
//! - choose and validate an [`AnchorKind`] for a step;
//! - build a typed [`RollbackRecipe`] and execute it through [`RollbackExecutor`];
//! - detect user changes before rollback ([`detect_conflict`]);
//! - verify the final fingerprint and return a structured [`IncidentReport`] on failure.
//!
//! Boundaries:
//! - no platform API, filesystem, network, clock, or randomness;
//! - no policy decision, persistence, audit write, UI prompt, or retry loop;
//! - no diff-only rollback. The safe choices are restore-to-anchor or stop with an incident.
//!
//! Invariants:
//! 1. A rollback succeeds only when the final observed fingerprint equals the anchor's
//!    pre-step fingerprint.
//! 2. Missing evidence is never treated as "no conflict".
//! 3. User changes block rollback unless the caller explicitly requests `RestoreOverall`.
//! 4. Anchor and recipe variants must agree; empty recipes and invalid counts fail closed.
//! 5. L3 steps have evidence-only anchors and cannot produce an executable rollback recipe.
//! 6. An L0 anchor may carry an L1 snapshot fallback; the engine uses it before declaring an
//!    incident when undo steps fail or end at the wrong fingerprint.
//!
//! Related documents: architecture v2 sections 7.3 and 9, `docs/spec/naming.md` section 7,
//! `docs/spec/error-codes.md`, and `tasks/TASK-024-undo-four-level-rollback-anchor.md`.

#![deny(unsafe_code)]

mod anchor;
mod conflict;
mod error;
mod id;
mod incident;
mod recipe;
mod reversibility;
mod rollback;

pub use anchor::{
    Anchor, AnchorAvailability, AnchorKind, AnchorStrategy, AnchorTargetKind, ContentDigest,
    EvidenceAvailability, ShadowCopy, ShadowCopyPath, UndoBudget, UndoStepCount, UndoValidity,
    plan_anchor_strategy,
};
pub use conflict::{ConflictResolution, RollbackConflict, blocks_rollback, detect_conflict};
pub use error::{UndoError, UndoResult};
pub use id::{AnchorId, IncidentId, StepId, TargetId};
pub use incident::{
    IncidentKind, IncidentReport, IncidentReporter, IncidentSeverity, ReportFailure,
    publish_incident,
};
pub use recipe::{
    RollbackAction, RollbackRecipe, ToolInvocation, ToolName, build_fallback_recipe,
    build_rollback_recipe, validate_rollback_recipe,
};
pub use reversibility::Reversibility;
pub use rollback::{
    ActionOutcome, ExecutorFailure, RollbackExecutor, RollbackOutcome, RollbackRequest,
    execute_rollback,
};
