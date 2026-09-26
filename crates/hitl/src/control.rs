//! Pause/resume and user-takeover coordination over `assistant-task-engine`.

use std::collections::BTreeMap;

use assistant_task_engine::{TaskEngine, TaskEvent, TaskId, TaskSnapshot};

use crate::error::{HitlError, HitlResult};

#[derive(Debug, Clone, PartialEq, Eq)]
struct TakeoverRecord {
    baseline_fingerprint: String,
    started_at_ms: i64,
}

/// Result of returning control to the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeoverAssessment {
    /// Task returned from takeover.
    pub task_id: TaskId,
    /// Fingerprint captured when the user took control.
    pub baseline_fingerprint: String,
    /// Fingerprint captured when the user returned control.
    pub current_fingerprint: String,
    /// Whether the two fingerprints differ.
    pub has_changed: bool,
    /// Always true: targets must be re-resolved after a takeover.
    pub requires_target_resolution: bool,
    /// Whether the plan may need review because state changed.
    pub requires_plan_review: bool,
}

/// Coordinates pause, resume, and takeover through the task engine.
#[derive(Debug)]
pub struct HitlCoordinator<Store> {
    task_engine: TaskEngine<Store>,
    takeovers: BTreeMap<TaskId, TakeoverRecord>,
}

impl<Store> HitlCoordinator<Store> {
    /// Creates a coordinator over an existing task engine.
    #[must_use]
    pub const fn new(task_engine: TaskEngine<Store>) -> Self {
        Self {
            task_engine,
            takeovers: BTreeMap::new(),
        }
    }

    /// Returns the underlying task engine.
    #[must_use]
    pub const fn task_engine(&self) -> &TaskEngine<Store> {
        &self.task_engine
    }

    /// Returns the underlying task engine mutably for unrelated orchestration.
    #[must_use]
    pub const fn task_engine_mut(&mut self) -> &mut TaskEngine<Store> {
        &mut self.task_engine
    }
}

impl<Store: assistant_task_engine::CheckpointStore> HitlCoordinator<Store> {
    /// Requests pause at the next safe step boundary.
    ///
    /// # Errors
    ///
    /// Propagates task-engine transition and checkpoint errors.
    pub fn request_pause(&mut self, task_id: &TaskId, now_ms: i64) -> HitlResult<TaskSnapshot> {
        Ok(self.task_engine.request_pause(task_id, now_ms)?)
    }

    /// Resumes a paused task.
    ///
    /// # Errors
    ///
    /// Propagates task-engine transition and checkpoint errors.
    pub fn resume_after_pause(
        &mut self,
        task_id: &TaskId,
        now_ms: i64,
    ) -> HitlResult<TaskSnapshot> {
        Ok(self.task_engine.resume(task_id, now_ms)?)
    }

    /// Starts user takeover and records the target fingerprint baseline.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::TakeoverAlreadyActive`] when a record already
    /// exists, [`HitlError::InvalidValue`] for an empty fingerprint, and
    /// propagates task-engine transition errors.
    pub fn begin_takeover(
        &mut self,
        task_id: &TaskId,
        baseline_fingerprint: impl Into<String>,
        now_ms: i64,
    ) -> HitlResult<TaskSnapshot> {
        if self.takeovers.contains_key(task_id) {
            return Err(HitlError::TakeoverAlreadyActive {
                task_id: task_id.to_string(),
            });
        }
        let baseline_fingerprint = baseline_fingerprint.into();
        if baseline_fingerprint.trim().is_empty() {
            return Err(HitlError::InvalidValue {
                reason: "takeover baseline fingerprint must not be empty".to_owned(),
            });
        }
        let snapshot = self.task_engine.take_over(task_id, now_ms)?;
        self.takeovers.insert(
            task_id.clone(),
            TakeoverRecord {
                baseline_fingerprint,
                started_at_ms: now_ms,
            },
        );
        Ok(snapshot)
    }

    /// Returns control and always requires target / fingerprint resynchronization.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::TakeoverNotActive`] when no record exists,
    /// [`HitlError::InvalidValue`] for an empty fingerprint, and propagates
    /// task-engine transition errors.
    pub fn return_control(
        &mut self,
        task_id: &TaskId,
        current_fingerprint: impl Into<String>,
        now_ms: i64,
    ) -> HitlResult<TakeoverAssessment> {
        let record =
            self.takeovers
                .get(task_id)
                .cloned()
                .ok_or_else(|| HitlError::TakeoverNotActive {
                    task_id: task_id.to_string(),
                })?;
        let current_fingerprint = current_fingerprint.into();
        if current_fingerprint.trim().is_empty() {
            return Err(HitlError::InvalidValue {
                reason: "returned-control fingerprint must not be empty".to_owned(),
            });
        }
        if now_ms < record.started_at_ms {
            return Err(HitlError::ClockWentBackwards {
                previous_ms: record.started_at_ms,
                now_ms,
            });
        }
        let _resuming_snapshot = self.task_engine.resume(task_id, now_ms)?;
        self.takeovers.remove(task_id);
        let has_changed = record.baseline_fingerprint != current_fingerprint;
        Ok(TakeoverAssessment {
            task_id: task_id.clone(),
            baseline_fingerprint: record.baseline_fingerprint,
            current_fingerprint,
            has_changed,
            requires_target_resolution: true,
            requires_plan_review: has_changed,
        })
    }

    /// Finishes resynchronization after [`HitlCoordinator::return_control`].
    ///
    /// # Errors
    ///
    /// Propagates task-engine transition and checkpoint errors.
    pub fn finish_resynchronization(
        &mut self,
        task_id: &TaskId,
        now_ms: i64,
    ) -> HitlResult<TaskSnapshot> {
        Ok(self
            .task_engine
            .apply_task_event(task_id, TaskEvent::FinishRecovery, now_ms)?)
    }
}
