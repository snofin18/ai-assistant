//! Coverage-focused contract tests for checkpoint stores, recovery, and snapshots.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use assistant_protocol::ErrorCode;
use assistant_storage::{Database, MigrationSet, StoragePaths, SystemClock};
use assistant_task_engine::{
    CheckpointStore, CheckpointStoreError, MemoryCheckpointStore, RecoveryAction, RecoveryEvidence,
    SqliteCheckpointStore, StepId, StepStatus, TaskCheckpoint, TaskEngine, TaskHoldReason, TaskId,
    TaskSnapshot, TaskStatus, assess_recovery,
};
use common::single_read_plan;

fn snapshot(task_id: &str, step_id: &str) -> TaskSnapshot {
    let task_id = TaskId::new(task_id).expect("task id");
    let step_id = StepId::new(step_id).expect("step id");
    TaskEngine::new(MemoryCheckpointStore::new())
        .create_task(single_read_plan(task_id.as_str(), step_id.as_str()), 1_000)
        .expect("snapshot")
}

fn set_active(snapshot: &mut TaskSnapshot, status: StepStatus) {
    snapshot.status = TaskStatus::Running;
    let step = snapshot.steps.first_mut().expect("step");
    step.status = status;
    step.phase_started_at_ms = Some(1_500);
    snapshot.current_step = Some(step.id.clone());
}

fn migrate_storage() -> MigrationSet {
    let mut migrations = MigrationSet::new();
    let first = assistant_storage::MIGRATIONS
        .iter()
        .find(|migration| migration.version() == 1)
        .copied()
        .expect("storage migration 0001");
    migrations.register(first).expect("register migration");
    migrations.validate().expect("continuous migrations");
    migrations
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "assistant-task-engine-{label}-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("temp directory");
    root
}

#[test]
fn test_memory_store_create_load_and_monotonic_revision_contract() {
    let task_id = TaskId::new("t_memory").expect("task id");
    let initial = snapshot("t_memory", "s_memory");
    let mut store = MemoryCheckpointStore::new();

    assert!(store.load_latest(&task_id).expect("missing load").is_none());
    store
        .create_task(&TaskCheckpoint::new(initial.clone()))
        .expect("create");

    let duplicate = store
        .create_task(&TaskCheckpoint::new(initial.clone()))
        .expect_err("duplicate create");
    assert!(matches!(
        duplicate,
        CheckpointStoreError::TaskAlreadyExists { .. }
    ));
    assert_eq!(duplicate.error_code(), ErrorCode::ToolInvalidArgs);

    let mut revision_one = initial.clone();
    revision_one.revision = 1;
    store
        .save_checkpoint(&TaskCheckpoint::new(revision_one.clone()))
        .expect("save revision one");

    let stale = store
        .save_checkpoint(&TaskCheckpoint::new(revision_one.clone()))
        .expect_err("stale revision");
    assert!(matches!(stale, CheckpointStoreError::Backend { .. }));
    assert_eq!(stale.error_code(), ErrorCode::Fatal);
    assert_eq!(
        store
            .load_latest(&task_id)
            .expect("latest")
            .expect("checkpoint")
            .snapshot,
        revision_one
    );

    let missing_id = TaskId::new("t_missing").expect("missing id");
    let mut missing = initial;
    missing.task_id = missing_id.clone();
    missing.plan.task_id = missing_id;
    let missing_error = store
        .save_checkpoint(&TaskCheckpoint::new(missing))
        .expect_err("missing task");
    assert!(matches!(
        missing_error,
        CheckpointStoreError::TaskNotFound { .. }
    ));
    assert_eq!(missing_error.error_code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn test_sqlite_store_persists_reopen_and_rejects_corrupt_payloads() {
    let root = temp_root("coverage-contract");
    let paths = StoragePaths::new(&root);
    let migrations = migrate_storage();
    let task_id = TaskId::new("t_sqlite_contract").expect("task id");
    let mut revision_one = snapshot("t_sqlite_contract", "s_sqlite_contract");
    revision_one.revision = 1;

    {
        let database =
            Database::open(&paths, Arc::new(SystemClock), &migrations).expect("open database");
        let mut store = SqliteCheckpointStore::new(&database);
        let missing_id = TaskId::new("t_sqlite_missing").expect("missing id");
        let mut missing = revision_one.clone();
        missing.task_id = missing_id.clone();
        missing.plan.task_id = missing_id;
        let save_error = store
            .save_checkpoint(&TaskCheckpoint::new(missing))
            .expect_err("missing task save");
        assert!(matches!(save_error, CheckpointStoreError::Backend { .. }));

        store
            .create_task(&TaskCheckpoint::new(revision_one.clone()))
            .expect("create");
        let duplicate = store
            .create_task(&TaskCheckpoint::new(revision_one.clone()))
            .expect_err("duplicate create");
        assert!(matches!(duplicate, CheckpointStoreError::Backend { .. }));
        assert!(store.load_latest(&task_id).expect("load latest").is_some());
    }

    {
        let database =
            Database::open(&paths, Arc::new(SystemClock), &migrations).expect("reopen database");
        let store = SqliteCheckpointStore::new(&database);
        let loaded = store
            .load_latest(&task_id)
            .expect("load after reopen")
            .expect("checkpoint after reopen");
        assert_eq!(loaded.snapshot, revision_one);

        database
            .connection()
            .execute(
                "UPDATE checkpoints SET state_json = replace(state_json, \
                 't_sqlite_contract', 't_other')",
                [],
            )
            .expect("replace task id");
        let mismatch = store.load_latest(&task_id).expect_err("task id mismatch");
        assert!(matches!(
            mismatch,
            CheckpointStoreError::Serialization { .. }
        ));

        database
            .connection()
            .execute("UPDATE checkpoints SET state_json = 'not-json'", [])
            .expect("corrupt json");
        let invalid = store.load_latest(&task_id).expect_err("invalid json");
        assert!(matches!(
            invalid,
            CheckpointStoreError::Serialization { .. }
        ));
        assert_eq!(invalid.error_code(), ErrorCode::Fatal);
    }

    std::fs::remove_dir_all(Path::new(&root)).expect("remove temp directory");
}

#[test]
fn test_recovery_safe_reset_states_return_redo() {
    for status in [
        StepStatus::Prechecking,
        StepStatus::Approved,
        StepStatus::Retrying,
    ] {
        let mut candidate = snapshot("t_recovery_reset", "s_recovery_reset");
        set_active(&mut candidate, status);
        let assessment =
            assess_recovery(candidate, &BTreeMap::new(), 2_000).expect("recovery assessment");

        assert_eq!(assessment.snapshot.status, TaskStatus::Resuming);
        assert_eq!(assessment.snapshot.current_step, None);
        assert_eq!(
            assessment.snapshot.steps.first().map(|step| step.status),
            Some(StepStatus::Pending)
        );
        assert_eq!(
            assessment.steps.first().map(|step| step.action.clone()),
            Some(RecoveryAction::Redo)
        );
    }
}

#[test]
fn test_recovery_completed_evidence_commits_and_not_completed_redoes() {
    let step_id = StepId::new("s_recovery_evidence").expect("step id");
    let mut completed = snapshot("t_recovery_completed", step_id.as_str());
    set_active(&mut completed, StepStatus::Executing);
    let assessment = assess_recovery(
        completed,
        &BTreeMap::from([(step_id.clone(), RecoveryEvidence::Completed)]),
        2_000,
    )
    .expect("completed recovery");
    assert_eq!(assessment.snapshot.status, TaskStatus::Completed);
    assert_eq!(assessment.snapshot.current_step, None);
    let committed = assessment.snapshot.step(&step_id).expect("committed step");
    assert_eq!(committed.status, StepStatus::Committed);
    assert_eq!(committed.phase_started_at_ms, None);
    assert_eq!(committed.ended_at_ms, Some(2_000));
    assert_eq!(
        assessment.steps.first().map(|step| step.action.clone()),
        Some(RecoveryAction::Commit)
    );

    let mut not_completed = snapshot("t_recovery_redo", step_id.as_str());
    set_active(&mut not_completed, StepStatus::Verifying);
    let assessment = assess_recovery(
        not_completed,
        &BTreeMap::from([(step_id.clone(), RecoveryEvidence::NotCompleted)]),
        2_100,
    )
    .expect("not-completed recovery");
    assert_eq!(assessment.snapshot.status, TaskStatus::Resuming);
    assert_eq!(assessment.snapshot.current_step, None);
    let redone = assessment.snapshot.step(&step_id).expect("redone step");
    assert_eq!(redone.status, StepStatus::Pending);
    assert_eq!(redone.phase_started_at_ms, None);
    assert_eq!(redone.ended_at_ms, None);
    assert_eq!(
        assessment.steps.first().map(|step| step.action.clone()),
        Some(RecoveryAction::Redo)
    );
}

#[test]
fn test_recovery_unknown_and_missing_evidence_require_human() {
    let step_id = StepId::new("s_recovery_unknown").expect("step id");
    let mut missing = snapshot("t_recovery_missing", step_id.as_str());
    set_active(&mut missing, StepStatus::Executing);
    let assessment = assess_recovery(missing, &BTreeMap::new(), 2_000).expect("missing recovery");
    assert_eq!(assessment.snapshot.status, TaskStatus::NeedsHuman);
    assert_eq!(assessment.snapshot.current_step, Some(step_id.clone()));
    assert!(matches!(
        assessment.snapshot.hold_reason,
        Some(TaskHoldReason::MissingRecoveryEvidence { .. })
    ));
    assert_eq!(
        assessment.steps.first().map(|step| step.action.clone()),
        Some(RecoveryAction::NeedsHuman)
    );

    let mut unknown = snapshot("t_recovery_unknown", step_id.as_str());
    set_active(&mut unknown, StepStatus::Verifying);
    let assessment = assess_recovery(
        unknown,
        &BTreeMap::from([(step_id, RecoveryEvidence::Unknown)]),
        2_000,
    )
    .expect("unknown recovery");
    assert_eq!(assessment.snapshot.status, TaskStatus::NeedsHuman);
    assert!(matches!(
        assessment.snapshot.hold_reason,
        Some(TaskHoldReason::UnknownStepOutcome { .. })
    ));
}

#[test]
fn test_recovery_handles_cancel_blocked_and_completed_warning_states() {
    let task_id = TaskId::new("t_recovery_terminal").expect("task id");
    let step_id = StepId::new("s_recovery_terminal").expect("step id");

    let mut cancelled = snapshot(task_id.as_str(), step_id.as_str());
    cancelled.cancel_requested = true;
    let cancelled = assess_recovery(cancelled, &BTreeMap::new(), 2_000).expect("cancel recovery");
    assert_eq!(cancelled.snapshot.status, TaskStatus::Cancelled);
    assert_eq!(cancelled.snapshot.hold_reason, None);

    let mut blocked = snapshot(task_id.as_str(), step_id.as_str());
    blocked.steps.first_mut().expect("step").status = StepStatus::Failed;
    let blocked = assess_recovery(blocked, &BTreeMap::new(), 2_000).expect("blocked recovery");
    assert_eq!(blocked.snapshot.status, TaskStatus::NeedsHuman);
    assert!(matches!(
        blocked.snapshot.hold_reason,
        Some(TaskHoldReason::RecoveryBlockedByStep { .. })
    ));

    let mut warned = snapshot(task_id.as_str(), step_id.as_str());
    warned.steps.first_mut().expect("step").status = StepStatus::Committed;
    warned.warning_count = 1;
    let warned = assess_recovery(warned, &BTreeMap::new(), 2_000).expect("warning recovery");
    assert_eq!(warned.snapshot.status, TaskStatus::CompletedWithWarnings);
    assert_eq!(warned.snapshot.hold_reason, None);
}

#[test]
fn test_recovery_rejects_clock_going_backwards() {
    let candidate = snapshot("t_recovery_clock", "s_recovery_clock");
    let error = assess_recovery(candidate, &BTreeMap::new(), 999).expect_err("clock guard");
    assert_eq!(error.error_code(), ErrorCode::Fatal);
}

#[test]
fn test_snapshot_validation_rejects_incompatible_or_misaligned_state() {
    let mut schema = snapshot("t_snapshot_schema", "s_snapshot_schema");
    schema.schema_version = 99;
    assert_eq!(
        schema.validate().expect_err("schema version").error_code(),
        ErrorCode::Fatal
    );

    let mut identity = snapshot("t_snapshot_identity", "s_snapshot_identity");
    identity.task_id = TaskId::new("t_other").expect("other id");
    assert_eq!(
        identity.validate().expect_err("task identity").error_code(),
        ErrorCode::Fatal
    );

    let mut alignment = snapshot("t_snapshot_alignment", "s_snapshot_alignment");
    alignment.steps.first_mut().expect("step").sequence = 7;
    assert_eq!(
        alignment
            .validate()
            .expect_err("step alignment")
            .error_code(),
        ErrorCode::Fatal
    );

    let mut current = snapshot("t_snapshot_current", "s_snapshot_current");
    current.current_step = Some(StepId::new("s_absent").expect("absent id"));
    assert_eq!(
        current.validate().expect_err("current step").error_code(),
        ErrorCode::Fatal
    );
}

#[test]
fn test_snapshot_helpers_report_settlement_and_watched_phase() {
    let step_id = StepId::new("s_snapshot_helpers").expect("step id");
    let mut candidate = snapshot("t_snapshot_helpers", step_id.as_str());
    assert_eq!(candidate.last_step_sequence(), 0);
    assert!(!candidate.all_steps_settled());
    assert_eq!(
        candidate.step(&step_id).expect("step").watched_phase(),
        None
    );

    let step = candidate.steps.first_mut().expect("step");
    step.status = StepStatus::Executing;
    assert_eq!(
        candidate.step(&step_id).expect("step").watched_phase(),
        Some(assistant_task_engine::StepPhase::Execute)
    );
    candidate.steps.first_mut().expect("step").status = StepStatus::Committed;
    assert_eq!(candidate.last_step_sequence(), 1);
    assert!(candidate.all_steps_settled());
}
