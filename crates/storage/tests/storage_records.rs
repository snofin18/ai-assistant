//! 核心表 CRUD 集成测试（TASK-012）：W1（任务 / 步骤 / 检查点）与 W6（用量）的往返与约束。
//!
//! 为什么单独一个文件：blob 池的用例在 `storage_integration.rs`，两者关注点不同；
//! 分开也让每个测试文件留在 ADR-0033 的 600 行软上限内。
//!
//! 测试内允许 `unwrap` / `expect` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;

use assistant_storage::{
    CheckpointRecord, TaskRecord, TaskStepRecord, UsageRecord, count_usage_records,
    insert_checkpoint, insert_task, insert_task_step, insert_usage_record, load_latest_checkpoint,
    load_task, load_task_steps,
};
use common::{FixedClock, TestDir, count_rows, open_database};
// ───────────────────────── 核心表 CRUD ─────────────────────────

fn sample_task() -> TaskRecord {
    TaskRecord {
        id: "t_1".to_owned(),
        conversation_id: None,
        goal: "打开记事本并写入一段文本".to_owned(),
        plan_json: Some("[]".to_owned()),
        status: "running".to_owned(),
        reversibility_worst: Some("reversible".to_owned()),
        budget_json: None,
        started_at: 5_000,
        ended_at: None,
        cost_usd: None,
        tokens_in: 0,
        tokens_out: 0,
        error_code: None,
    }
}

fn sample_step() -> TaskStepRecord {
    TaskStepRecord {
        id: "s_1".to_owned(),
        task_id: "t_1".to_owned(),
        seq: 0,
        tool: "notepad.write".to_owned(),
        args_json: Some("{}".to_owned()),
        status: "done".to_owned(),
        attempts: 1,
        pre_fingerprint: None,
        post_fingerprint: Some("fp".to_owned()),
        verify_result_json: None,
        anchor_id: None,
        started_at: 5_000,
        ended_at: Some(5_010),
        duration_ms: Some(10),
        error_code: None,
    }
}

/// W1 任务 / 步骤的往返与约束：读回逐字段一致；重复主键、重复 `(task_id, seq)`、悬空外键都必须报错。
#[test]
fn test_task_and_step_roundtrip_and_constraints() {
    let dir = TestDir::new("records-task");
    let clock = Arc::new(FixedClock::new(5_000));
    let database = open_database(&dir, &clock);
    let connection = database.connection();

    let task = sample_task();
    insert_task(connection, &task).expect("写任务");
    assert_eq!(
        load_task(connection, "t_1").expect("读任务"),
        Some(task.clone())
    );
    assert_eq!(load_task(connection, "t_missing").expect("读任务"), None);
    insert_task(connection, &task).expect_err("同 id 二次插入必须报错（禁止静默覆盖）");

    let step = sample_step();
    insert_task_step(connection, &step).expect("写步骤");
    insert_task_step(
        connection,
        &TaskStepRecord {
            id: "s_2".to_owned(),
            seq: 0,
            ..step.clone()
        },
    )
    .expect_err("(task_id, seq) 必须唯一");

    let steps = load_task_steps(connection, "t_1").expect("读步骤");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps.first(), Some(&step));

    insert_task_step(
        connection,
        &TaskStepRecord {
            id: "s_3".to_owned(),
            task_id: "t_missing".to_owned(),
            ..step
        },
    )
    .expect_err("悬空 task_id 必须被外键拦住");
}

/// W1 检查点 / W6 用量的往返 + 外键 + `ON DELETE CASCADE`。
///
/// 级联这一段同时验证了 `foreign_keys=ON` 真的设上了：若没设，删任务会留下孤儿步骤行。
#[test]
fn test_checkpoint_usage_roundtrip_and_cascade() {
    let dir = TestDir::new("records-checkpoint");
    let clock = Arc::new(FixedClock::new(5_000));
    let database = open_database(&dir, &clock);
    let connection = database.connection();

    // 检查点必须挂在存在的任务上（先插一个悬空的，确认外键真的在拦）
    insert_checkpoint(
        connection,
        &CheckpointRecord {
            id: "c_orphan".to_owned(),
            task_id: "t_missing".to_owned(),
            last_step_seq: 0,
            state_json: "{}".to_owned(),
            created_at: 5_050,
        },
    )
    .expect_err("悬空 task_id 必须被外键拦住");

    insert_task(connection, &sample_task()).expect("写任务");
    insert_task_step(connection, &sample_step()).expect("写步骤");
    for (id, seq, created_at) in [("c_1", 0, 5_100), ("c_2", 1, 5_200)] {
        insert_checkpoint(
            connection,
            &CheckpointRecord {
                id: id.to_owned(),
                task_id: "t_1".to_owned(),
                last_step_seq: seq,
                state_json: format!("{{\"last_step_seq\":{seq}}}"),
                created_at,
            },
        )
        .expect("写检查点");
    }

    let latest = load_latest_checkpoint(connection, "t_1")
        .expect("读检查点")
        .expect("存在");
    assert_eq!(latest.id, "c_2", "恢复路径只读最近一条");
    assert_eq!(latest.last_step_seq, 1);

    insert_usage_record(
        connection,
        &UsageRecord {
            id: "u_1".to_owned(),
            ts: 5_400,
            task_id: Some("t_1".to_owned()),
            step_id: Some("s_1".to_owned()),
            model_config_id: None,
            tokens_in: 10,
            tokens_out: 20,
            cached_tokens: 0,
            cost_usd: Some(0.001),
            latency_ms: Some(120),
            cache_hit: false,
        },
    )
    .expect("写用量");
    assert_eq!(count_usage_records(connection).expect("计数"), 1);

    // ON DELETE CASCADE 生效 = `foreign_keys=ON` 真的设上了（否则这里会留孤儿行）
    connection
        .execute("DELETE FROM tasks WHERE id = ?1", rusqlite::params!["t_1"])
        .expect("删任务");
    assert!(
        load_task_steps(connection, "t_1")
            .expect("读步骤")
            .is_empty()
    );
    assert!(
        load_latest_checkpoint(connection, "t_1")
            .expect("读检查点")
            .is_none()
    );
    assert_eq!(count_rows(connection, "task_steps"), 0);
    assert_eq!(count_rows(connection, "checkpoints"), 0);
}
