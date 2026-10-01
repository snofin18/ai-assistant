//! Completed production execution result.

use std::sync::{Arc, Mutex};

use assistant_task_engine::{MemoryCheckpointStore, StepId, TaskEngine, TaskId, TaskSnapshot};

use crate::production_policy::ApprovalWindows;
use crate::runtime_binding::RuntimeBindingState;

/// Approval request the runtime paused on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRuntimeApproval {
    /// Stable request id shown to the UI.
    pub request_id: String,
    /// Task that paused.
    pub task_id: TaskId,
    /// Step whose execution is waiting for the decision.
    pub step_id: StepId,
}

/// Result of one production execution.
pub struct ProductionRun {
    /// Snapshots observed after each committed step.
    pub snapshots: Vec<TaskSnapshot>,
    /// Final task snapshot.
    pub final_snapshot: TaskSnapshot,
    /// Pending approval metadata when execution paused before a step.
    pub awaiting_approval: Option<PendingRuntimeApproval>,
    engine: TaskEngine<MemoryCheckpointStore>,
    pub(crate) binding_state: Arc<Mutex<RuntimeBindingState>>,
    pub(crate) approval_windows: ApprovalWindows,
}

type ResumeParts = (
    Vec<TaskSnapshot>,
    TaskSnapshot,
    Option<PendingRuntimeApproval>,
    TaskEngine<MemoryCheckpointStore>,
    Arc<Mutex<RuntimeBindingState>>,
    ApprovalWindows,
);

impl ProductionRun {
    /// Returns the engine for the UI command handler.
    #[must_use]
    pub fn into_engine(self) -> TaskEngine<MemoryCheckpointStore> {
        self.engine
    }

    /// Returns whether execution stopped at a resumable approval boundary.
    #[must_use]
    pub const fn is_awaiting_approval(&self) -> bool {
        self.awaiting_approval.is_some()
    }

    /// Returns the pending approval metadata, when execution paused.
    #[must_use]
    pub const fn pending_approval(&self) -> Option<&PendingRuntimeApproval> {
        self.awaiting_approval.as_ref()
    }

    /// Creates a run result from its owned engine and observed snapshots.
    #[must_use]
    pub(crate) const fn new(
        snapshots: Vec<TaskSnapshot>,
        final_snapshot: TaskSnapshot,
        engine: TaskEngine<MemoryCheckpointStore>,
        awaiting_approval: Option<PendingRuntimeApproval>,
        binding_state: Arc<Mutex<RuntimeBindingState>>,
        approval_windows: ApprovalWindows,
    ) -> Self {
        Self {
            snapshots,
            final_snapshot,
            awaiting_approval,
            engine,
            binding_state,
            approval_windows,
        }
    }

    pub(crate) fn into_resume_parts(self) -> ResumeParts {
        (
            self.snapshots,
            self.final_snapshot,
            self.awaiting_approval,
            self.engine,
            self.binding_state,
            self.approval_windows,
        )
    }
}
