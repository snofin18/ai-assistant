//! Checkpoint records plus memory and SQLite-backed stores.

use std::collections::BTreeMap;

use assistant_protocol::serde_json;
use assistant_storage::{
    CheckpointRecord, Database, StorageError, TaskRecord, insert_checkpoint, insert_task,
    load_latest_checkpoint,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::identifiers::TaskId;
use crate::snapshot::TaskSnapshot;

/// Complete recoverable checkpoint payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskCheckpoint {
    /// Complete task snapshot at one revision.
    pub snapshot: TaskSnapshot,
}

impl TaskCheckpoint {
    /// Wraps a validated snapshot.
    #[must_use]
    pub const fn new(snapshot: TaskSnapshot) -> Self {
        Self { snapshot }
    }
}

/// Persistence failure exposed by a checkpoint store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CheckpointStoreError {
    /// A task with the same id already exists.
    #[error("task already exists: {task_id}")]
    TaskAlreadyExists {
        /// Conflicting task.
        task_id: String,
    },
    /// A save targeted a task that was never created.
    #[error("task does not exist: {task_id}")]
    TaskNotFound {
        /// Missing task.
        task_id: String,
    },
    /// Checkpoint serialization or storage failed.
    #[error("checkpoint backend failure: {reason}")]
    Backend {
        /// Stable protocol category for the backend failure.
        code: assistant_protocol::ErrorCode,
        /// Human-readable failure.
        reason: String,
    },
    /// Stored checkpoint JSON failed validation.
    #[error("checkpoint serialization failure: {reason}")]
    Serialization {
        /// Serialization or parse failure.
        reason: String,
    },
}

impl CheckpointStoreError {
    /// Returns the stable protocol error category.
    #[must_use]
    pub const fn error_code(&self) -> assistant_protocol::ErrorCode {
        match self {
            Self::TaskAlreadyExists { .. } | Self::TaskNotFound { .. } => {
                assistant_protocol::ErrorCode::ToolInvalidArgs
            }
            Self::Backend { code, .. } => *code,
            Self::Serialization { .. } => assistant_protocol::ErrorCode::Fatal,
        }
    }
}

/// Persistence interface used by [`crate::TaskEngine`].
pub trait CheckpointStore {
    /// Creates a task and its initial checkpoint atomically.
    ///
    /// # Errors
    ///
    /// Returns [`CheckpointStoreError`] when the task already exists or the
    /// backend cannot commit the initial record.
    fn create_task(&mut self, checkpoint: &TaskCheckpoint) -> Result<(), CheckpointStoreError>;

    /// Appends a new checkpoint revision.
    ///
    /// # Errors
    ///
    /// Returns [`CheckpointStoreError`] when the task does not exist or the
    /// backend cannot append the checkpoint.
    fn save_checkpoint(&mut self, checkpoint: &TaskCheckpoint) -> Result<(), CheckpointStoreError>;

    /// Loads the newest checkpoint for a task.
    ///
    /// # Errors
    ///
    /// Returns [`CheckpointStoreError`] when the backend cannot read or parse
    /// the stored checkpoint. A missing task is `Ok(None)`.
    fn load_latest(&self, task_id: &TaskId)
    -> Result<Option<TaskCheckpoint>, CheckpointStoreError>;
}

/// Deterministic in-memory checkpoint store for unit and replay tests.
#[derive(Debug, Default)]
pub struct MemoryCheckpointStore {
    tasks: BTreeMap<TaskId, Vec<TaskCheckpoint>>,
}

impl MemoryCheckpointStore {
    /// Creates an empty store.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
        }
    }
}

impl CheckpointStore for MemoryCheckpointStore {
    fn create_task(&mut self, checkpoint: &TaskCheckpoint) -> Result<(), CheckpointStoreError> {
        let task_id = checkpoint.snapshot.task_id.clone();
        if self.tasks.contains_key(&task_id) {
            return Err(CheckpointStoreError::TaskAlreadyExists {
                task_id: task_id.to_string(),
            });
        }
        self.tasks.insert(task_id, vec![checkpoint.clone()]);
        Ok(())
    }

    fn save_checkpoint(&mut self, checkpoint: &TaskCheckpoint) -> Result<(), CheckpointStoreError> {
        let task_id = checkpoint.snapshot.task_id.clone();
        let revisions =
            self.tasks
                .get_mut(&task_id)
                .ok_or_else(|| CheckpointStoreError::TaskNotFound {
                    task_id: task_id.to_string(),
                })?;
        if revisions
            .last()
            .is_some_and(|previous| checkpoint.snapshot.revision <= previous.snapshot.revision)
        {
            return Err(CheckpointStoreError::Backend {
                code: assistant_protocol::ErrorCode::Fatal,
                reason: "checkpoint revision must increase monotonically".to_owned(),
            });
        }
        revisions.push(checkpoint.clone());
        Ok(())
    }

    fn load_latest(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<TaskCheckpoint>, CheckpointStoreError> {
        Ok(self
            .tasks
            .get(task_id)
            .and_then(|revisions| revisions.last())
            .cloned())
    }
}

/// SQLite-backed checkpoints using `assistant-storage`'s public record API.
#[derive(Debug)]
pub struct SqliteCheckpointStore<'database> {
    database: &'database Database,
}

impl<'database> SqliteCheckpointStore<'database> {
    /// Creates an adapter over an already-open database.
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl CheckpointStore for SqliteCheckpointStore<'_> {
    fn create_task(&mut self, checkpoint: &TaskCheckpoint) -> Result<(), CheckpointStoreError> {
        let plan_json = serialize_json(&checkpoint.snapshot.plan)?;
        let budget_json = serialize_json(&checkpoint.snapshot.plan.budget)?;
        let task = TaskRecord {
            id: checkpoint.snapshot.task_id.to_string(),
            conversation_id: None,
            goal: checkpoint.snapshot.plan.goal.clone(),
            plan_json: Some(plan_json),
            status: checkpoint.snapshot.status.as_str().to_owned(),
            reversibility_worst: Some(
                checkpoint
                    .snapshot
                    .plan
                    .worst_reversibility()
                    .as_str()
                    .to_owned(),
            ),
            budget_json: Some(budget_json),
            started_at: checkpoint.snapshot.started_at_ms,
            ended_at: checkpoint
                .snapshot
                .status
                .is_terminal()
                .then_some(checkpoint.snapshot.updated_at_ms),
            cost_usd: Some(checkpoint.snapshot.budget_usage.cost_usd),
            tokens_in: token_count(checkpoint.snapshot.budget_usage.tokens_in)?,
            tokens_out: token_count(checkpoint.snapshot.budget_usage.tokens_out)?,
            error_code: checkpoint
                .snapshot
                .last_error_code
                .map(|code| format!("{code:?}")),
        };
        let checkpoint_record = storage_checkpoint(&checkpoint.snapshot)?;
        let transaction = self
            .database
            .connection()
            .unchecked_transaction()
            .map_err(|error| storage_error(&error.into()))?;
        insert_task(&transaction, &task).map_err(|error| storage_error(&error))?;
        insert_checkpoint(&transaction, &checkpoint_record)
            .map_err(|error| storage_error(&error))?;
        transaction
            .commit()
            .map_err(|error| storage_error(&error.into()))
    }

    fn save_checkpoint(&mut self, checkpoint: &TaskCheckpoint) -> Result<(), CheckpointStoreError> {
        let checkpoint_record = storage_checkpoint(&checkpoint.snapshot)?;
        insert_checkpoint(self.database.connection(), &checkpoint_record)
            .map_err(|error| storage_error(&error))
    }

    fn load_latest(
        &self,
        task_id: &TaskId,
    ) -> Result<Option<TaskCheckpoint>, CheckpointStoreError> {
        let Some(record) = load_latest_checkpoint(self.database.connection(), task_id.as_str())
            .map_err(|error| storage_error(&error))?
        else {
            return Ok(None);
        };
        let snapshot: TaskSnapshot = serde_json::from_str(&record.state_json).map_err(|error| {
            CheckpointStoreError::Serialization {
                reason: error.to_string(),
            }
        })?;
        snapshot
            .validate()
            .map_err(|error| CheckpointStoreError::Serialization {
                reason: error.to_string(),
            })?;
        if &snapshot.task_id != task_id {
            return Err(CheckpointStoreError::Serialization {
                reason: "stored task id does not match requested task id".to_owned(),
            });
        }
        Ok(Some(TaskCheckpoint::new(snapshot)))
    }
}

fn storage_checkpoint(snapshot: &TaskSnapshot) -> Result<CheckpointRecord, CheckpointStoreError> {
    let state_json =
        serde_json::to_string(snapshot).map_err(|error| CheckpointStoreError::Serialization {
            reason: error.to_string(),
        })?;
    let last_step_seq = i64::from(snapshot.last_step_sequence());
    Ok(CheckpointRecord {
        id: format!("c_{}_{:020}", snapshot.task_id.as_str(), snapshot.revision),
        task_id: snapshot.task_id.to_string(),
        last_step_seq,
        state_json,
        created_at: snapshot.updated_at_ms,
    })
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String, CheckpointStoreError> {
    serde_json::to_string(value).map_err(|error| CheckpointStoreError::Serialization {
        reason: error.to_string(),
    })
}

fn token_count(value: u64) -> Result<i64, CheckpointStoreError> {
    i64::try_from(value).map_err(|_| CheckpointStoreError::Serialization {
        reason: "token count exceeds i64".to_owned(),
    })
}

fn storage_error(error: &StorageError) -> CheckpointStoreError {
    let code = error.error_category();
    CheckpointStoreError::Backend {
        code,
        reason: error.to_string(),
    }
}
