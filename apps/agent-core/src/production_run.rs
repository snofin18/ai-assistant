//! Completed production execution result.

use assistant_task_engine::{MemoryCheckpointStore, TaskEngine, TaskSnapshot};

/// Result of one production execution.
pub struct ProductionRun {
    /// Snapshots observed after each committed step.
    pub snapshots: Vec<TaskSnapshot>,
    /// Final task snapshot.
    pub final_snapshot: TaskSnapshot,
    engine: TaskEngine<MemoryCheckpointStore>,
}

impl ProductionRun {
    /// Returns the engine for the UI command handler.
    #[must_use]
    pub fn into_engine(self) -> TaskEngine<MemoryCheckpointStore> {
        self.engine
    }

    /// Creates a run result from its owned engine and observed snapshots.
    #[must_use]
    pub(crate) const fn new(
        snapshots: Vec<TaskSnapshot>,
        final_snapshot: TaskSnapshot,
        engine: TaskEngine<MemoryCheckpointStore>,
    ) -> Self {
        Self {
            snapshots,
            final_snapshot,
            engine,
        }
    }
}
