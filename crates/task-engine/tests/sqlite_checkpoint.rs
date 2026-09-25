//! SQLite checkpoint integration through `assistant-storage`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use assistant_storage::{Database, MigrationSet, StoragePaths, SystemClock};
use assistant_task_engine::{SqliteCheckpointStore, TaskEngine, TaskEvent, TaskId, UsageDelta};
use common::single_read_plan;

struct TestDirectory {
    root: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "assistant-task-engine-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("temp dir");
        Self { root }
    }

    fn paths(&self) -> StoragePaths {
        StoragePaths::new(&self.root)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn migrations() -> MigrationSet {
    let mut migrations = MigrationSet::new();
    migrations
        .register_all(assistant_storage::MIGRATIONS)
        .expect("storage migrations");
    migrations.validate().expect("migration continuity");
    migrations
}

#[test]
fn test_sqlite_checkpoint_survives_reopen() {
    let directory = TestDirectory::new();
    let task_id = TaskId::new("t_sqlite").expect("task id");
    let step_id = assistant_task_engine::StepId::new("s_1").expect("step id");

    {
        let database =
            Database::open(&directory.paths(), Arc::new(SystemClock), &migrations()).expect("db");
        let mut engine = TaskEngine::new(SqliteCheckpointStore::new(&database));
        engine
            .create_task(single_read_plan(task_id.as_str(), step_id.as_str()), 1_000)
            .expect("create");
        engine
            .apply_task_event(&task_id, TaskEvent::SubmitForApproval, 1_010)
            .expect("submit");
        engine
            .apply_task_event(&task_id, TaskEvent::ApprovePlan, 1_020)
            .expect("approve");
        engine.begin_step(&task_id, &step_id, 1_030).expect("begin");
        engine
            .approve_step(&task_id, &step_id, 1_040)
            .expect("approve step");
        engine
            .begin_execute(&task_id, &step_id, 1_050)
            .expect("execute");
        engine
            .record_usage(
                &task_id,
                &step_id,
                UsageDelta {
                    tokens_in: 7,
                    tokens_out: 11,
                    cost_usd: 0.01,
                },
                1_060,
            )
            .expect("usage");
        assert_eq!(
            database
                .connection()
                .query_row("SELECT COUNT(*) FROM checkpoints", [], |row| row
                    .get::<_, i64>(0))
                .expect("checkpoint count"),
            7
        );
    }

    let database =
        Database::open(&directory.paths(), Arc::new(SystemClock), &migrations()).expect("reopen");
    let engine = TaskEngine::new(SqliteCheckpointStore::new(&database));
    let recovered = engine.load_snapshot(&task_id).expect("load");
    assert_eq!(recovered.budget_usage.tokens_in, 7);
    assert_eq!(recovered.budget_usage.tokens_out, 11);
    assert_eq!(
        recovered.step(&step_id).map(|step| step.status),
        Some(assistant_task_engine::StepStatus::Executing)
    );
}
