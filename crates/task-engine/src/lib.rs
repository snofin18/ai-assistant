//! Deterministic task and step orchestration for the local AI assistant.
//!
//! Responsibilities:
//! - enforce the 12 task states and the step-level execution state machine;
//! - validate Plan / Step DAGs and compute a deterministic ready frontier;
//! - persist a recoverable checkpoint before exposing every successful transition;
//! - classify crash recovery as completed, not completed, or unknown;
//! - enforce task budgets and phase-specific watchdogs.
//!
//! Boundaries:
//! - does not resolve targets, call platform APIs, execute tools, or verify postconditions;
//! - does not acquire leases, manage undo anchors, request approval, or run a model loop;
//! - does not own storage schema or write SQL directly;
//! - does not use wall-clock time or background timers; callers inject timestamps.
//!
//! Invariants:
//! - an unknown outcome after a crash always produces `NeedsHuman`;
//! - illegal `(state, event)` pairs are rejected rather than coerced;
//! - a checkpoint is persisted before a new in-memory snapshot is returned;
//! - an `L3Irreversible` step is always marked `point_of_no_return`;
//! - every write step declares at least one postcondition.
//!
//! Typical use:
//! ```
//! use assistant_protocol::serde_json::json;
//! use assistant_task_engine::{
//!     Budget, CheckpointPolicy, MemoryCheckpointStore, Plan, PlanId, PlanStep, Reversibility,
//!     StepEffect, StepId, StepTimeouts, TaskEngine, TaskId,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let task_id = TaskId::new("t_1")?;
//! let step_id = StepId::new("s_1")?;
//! let plan = Plan {
//!     plan_id: PlanId::new("p_1")?,
//!     task_id: task_id.clone(),
//!     goal: "write a greeting".to_owned(),
//!     steps: vec![PlanStep {
//!         id: step_id,
//!         sequence: 1,
//!         tool: "notepad.text.write".to_owned(),
//!         args: json!({"text": "hello"}),
//!         depends_on: Vec::new(),
//!         postconditions: vec![json!({"kind": "element_exists"})],
//!         effect: StepEffect::Write,
//!         reversibility: Reversibility::L0UndoStack,
//!         point_of_no_return: false,
//!         timeouts: StepTimeouts::new(500, 2_000, 1_000)?,
//!     }],
//!     budget: Budget::new(10, 60_000, 1_000, 0.5)?,
//!     checkpoint_policy: CheckpointPolicy::AfterEachTransition,
//! };
//!
//! let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
//! let snapshot = engine.create_task(plan, 1_000)?;
//! assert_eq!(snapshot.status.as_str(), "draft");
//! # Ok(())
//! # }
//! ```
//!
//! Related architecture sections: v2 sections 8.1 through 8.5 and 8.9;
//! `docs/storage-design.md` sections 3.1, 4, 6, and 8.

#![deny(unsafe_code)]

mod budget;
mod checkpoint;
mod engine;
mod error;
mod identifiers;
mod plan;
mod recovery;
mod scheduler;
mod snapshot;
mod status;
mod watchdog;

pub use budget::{Budget, BudgetCheck, BudgetLimit, BudgetUsage, UsageDelta};
pub use checkpoint::{
    CheckpointStore, CheckpointStoreError, MemoryCheckpointStore, SqliteCheckpointStore,
    TaskCheckpoint,
};
pub use engine::TaskEngine;
pub use error::{TaskEngineError, TaskEngineResult};
pub use identifiers::{PlanId, StepId, TaskId};
pub use plan::{CheckpointPolicy, Plan, PlanStep, Reversibility, StepEffect, StepTimeouts};
pub use recovery::{
    RecoveryAction, RecoveryAssessment, RecoveryEvidence, StepRecoveryAssessment, assess_recovery,
};
pub use scheduler::ready_step_ids;
pub use snapshot::{StepSnapshot, TaskHoldReason, TaskSnapshot};
pub use status::{
    StepEvent, StepPhase, StepStatus, TaskEvent, TaskStatus, transition_step, transition_task,
};
pub use watchdog::{WatchdogDecision, check_watchdog};
