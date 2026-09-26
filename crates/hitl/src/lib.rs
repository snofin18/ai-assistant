//! Human-in-the-loop approval, authorization scope, takeover, pause/resume,
//! and diff-preview domain logic.
//!
//! Responsibilities:
//! - turn a lossless policy confirmation projection into a validated approval
//!   request;
//! - evaluate four-dimensional authorization scopes with TTL and bounded use;
//! - coordinate pause/resume and takeover through `assistant-task-engine`;
//! - prepare display-independent diff data without silently truncating it.
//!
//! Boundaries:
//! - no platform API, tool execution, lease, undo, verification, networking,
//!   persistence, UI rendering, wall-clock reads, or background timers;
//! - `assistant-policy` remains the only policy allow/deny point;
//! - approval data is not a substitute for executing a policy decision.
//!
//! Invariants:
//! - only a confirmation decision with non-empty `scope_options` can create an
//!   approval request;
//! - high-risk actions allow only `once` with exactly one use;
//! - every grant has a positive TTL and bounded remaining uses;
//! - returning control always requires target re-resolution and fingerprint
//!   re-synchronization, even when the two fingerprints are equal.
//!
//! Related architecture sections: v2 sections 8.4 and 10.

#![deny(unsafe_code)]

mod approval;
mod authorization;
mod control;
mod diff;
mod error;
mod identifiers;

pub use approval::{ApprovalDecision, ApprovalOutcome, ApprovalRequest, ApprovalRequestInput};
pub use authorization::{
    ApprovalRisk, ApprovalScope, AuthorizationContext, AuthorizationDecision, AuthorizationDenial,
    AuthorizationDimensions, AuthorizationGrant, AuthorizationOrigin, AuthorizationRequest,
    TargetAuthorization, ToolAuthorization,
};
pub use control::{HitlCoordinator, TakeoverAssessment};
pub use diff::{
    DiffPreview, FieldDiff, FileChangeKind, FileDiff, IrreversibleDiff, TextDiffEntry,
    TextDiffKind, TextDiffPreview, UiStepDiff,
};
pub use error::{HitlError, HitlResult};
pub use identifiers::{AppSessionId, ApprovalRequestId, SubjectId};
