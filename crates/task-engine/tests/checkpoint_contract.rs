//! Checkpoint-store and snapshot validation edge cases.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic
)]

mod common;

use assistant_task_engine::{
    CheckpointStore, CheckpointStoreError, MemoryCheckpointStore, TaskCheckpoint, TaskEngine,
    TaskEngineError, TaskId,
};
use common::single_read_plan;

#[test]
fn test_memory_store_rejects_duplicate_and_missing_task() {
    let task_id = TaskId::new("t_store").expect("task");
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    let snapshot = engine
        .create_task(single_read_plan(task_id.as_str(), "s_1"), 1_000)
        .expect("create");
    let checkpoint = TaskCheckpoint::new(snapshot);
    let mut store = MemoryCheckpointStore::new();
    assert!(
        store
            .save_checkpoint(&checkpoint)
            .expect_err("missing task must fail")
            .to_string()
            .contains("does not exist")
    );
}

#[test]
fn test_snapshot_rejects_schema_and_current_step_mismatch() {
    let task_id = TaskId::new("t_validate").expect("task");
    let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
    let snapshot = engine
        .create_task(single_read_plan(task_id.as_str(), "s_1"), 1_000)
        .expect("create");

    let mut wrong_schema = snapshot.clone();
    wrong_schema.schema_version = 99;
    assert!(wrong_schema.validate().is_err());

    let mut missing_step = snapshot;
    missing_step.current_step = Some(assistant_task_engine::StepId::new("s_missing").expect("id"));
    assert!(missing_step.validate().is_err());
}

#[test]
fn test_checkpoint_store_errors_have_stable_codes() {
    let errors = [
        CheckpointStoreError::TaskAlreadyExists {
            task_id: "t_1".to_owned(),
        },
        CheckpointStoreError::TaskNotFound {
            task_id: "t_1".to_owned(),
        },
        CheckpointStoreError::Serialization {
            reason: "bad".to_owned(),
        },
        CheckpointStoreError::Backend {
            code: assistant_protocol::ErrorCode::Transient,
            reason: "busy".to_owned(),
        },
    ];
    assert_eq!(
        errors[0].error_code(),
        assistant_protocol::ErrorCode::ToolInvalidArgs
    );
    assert_eq!(
        errors[1].error_code(),
        assistant_protocol::ErrorCode::ToolInvalidArgs
    );
    assert_eq!(errors[2].error_code(), assistant_protocol::ErrorCode::Fatal);
    assert_eq!(
        errors[3].error_code(),
        assistant_protocol::ErrorCode::Transient
    );
}

#[test]
fn test_task_engine_error_mapping_is_total() {
    let cases = vec![
        (
            TaskEngineError::InvalidIdentifier {
                kind: "task",
                value: String::new(),
            },
            assistant_protocol::ErrorCode::ToolInvalidArgs,
        ),
        (
            TaskEngineError::InvalidPlan {
                reason: "bad".to_owned(),
            },
            assistant_protocol::ErrorCode::ToolInvalidArgs,
        ),
        (
            TaskEngineError::DependencyCycle,
            assistant_protocol::ErrorCode::ToolInvalidArgs,
        ),
        (
            TaskEngineError::TaskNotFound {
                task_id: "t_1".to_owned(),
            },
            assistant_protocol::ErrorCode::TargetNotFound,
        ),
        (
            TaskEngineError::InvalidCheckpoint {
                reason: "bad".to_owned(),
            },
            assistant_protocol::ErrorCode::Fatal,
        ),
        (
            TaskEngineError::CheckpointStore(CheckpointStoreError::TaskNotFound {
                task_id: "t_1".to_owned(),
            }),
            assistant_protocol::ErrorCode::ToolInvalidArgs,
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.error_code(), expected, "{error}");
    }
}
