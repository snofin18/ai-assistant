//! 真库集成测试（一）：迁移 / 两档 `durability` / flush 失败语义 / 重开续链 / 摊销实测。
//!
//! 篡改检出在 `audit_tamper.rs`（同为真库用例，单独一份便于定位失败）。
//!
//! 为什么必须真库：本卡的核心承诺（"只追加"、"摊销 < 1 ms/条"）**没有一条**是纯逻辑能证明的
//! —— 触发器是 SQLite 行为，摊销是真实事务成本。
//!
//! 测试内允许 `expect` / `unwrap` / `panic`（AGENTS.md §5.3）。
//! 额外允许 `print_stdout`：`DoD` 要求"batched 摊销**实测并打印**"，否则通过的运行里看不到数字
//! （AGENTS.md §5.3 明确 crate 顶层那批 deny 在 `tests/` 内可 allow）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#![allow(clippy::print_stdout)]

mod common;

use std::sync::Arc;

use assistant_audit::{AppendOutcome, AuditError, AuditLog, AuditSubject, Durability};
use assistant_storage::SCHEMA_VERSION;
use common::{
    FixedClock, TestDir, audit_row_count, batched_log, immediate_log, object_exists, open_database,
    sample_event,
};

/// 迁移 0002 必须建出表 + 索引 + 两个 append-only 触发器，且 `SCHEMA_VERSION` 抬到 2。
#[test]
fn test_migration_0002_creates_table_index_and_append_only_triggers() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("migration");
    let database = open_database(&dir, &clock);
    let connection = database.connection();

    assert_eq!(SCHEMA_VERSION, 2, "本卡把 schema 版本抬到 2");
    assert_eq!(database.schema_version().expect("schema 版本"), 2);
    assert!(object_exists(connection, "table", "audit_logs"));
    assert!(object_exists(connection, "index", "idx_audit_ts"));
    assert!(object_exists(connection, "trigger", "audit_logs_no_update"));
    assert!(object_exists(connection, "trigger", "audit_logs_no_delete"));
    assert_eq!(audit_row_count(connection), 0);
}

/// 列名与顺序必须与架构 v2 §15.1 的 `audit_logs(...)` 逐字一致（多一列 / 少一列都算改契约）。
#[test]
fn test_audit_logs_columns_match_architecture_section_15_1() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("columns");
    let database = open_database(&dir, &clock);

    let mut statement = database
        .connection()
        .prepare("SELECT name FROM pragma_table_info('audit_logs') ORDER BY cid")
        .expect("prepare");
    let columns: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .expect("query")
        .map(|row| row.expect("列名"))
        .collect();

    assert_eq!(
        columns,
        vec![
            "id",
            "prev_hash",
            "ts",
            "actor",
            "task_id",
            "step_id",
            "event_type",
            "detail_json",
            "hash",
        ]
    );
}

/// 已建的 v1 库（没有 `audit_logs`）必须能自动升级到 v2，且再打开一次是幂等的。
#[test]
fn test_migration_0002_upgrades_v1_library_and_is_idempotent() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("upgrade");
    {
        let database = open_database(&dir, &clock);
        // 退回"只应用了迁移 1"的状态：删表 + 删迁移 2 的记账
        database
            .connection()
            .execute_batch(
                "DROP TABLE audit_logs; DELETE FROM schema_migrations WHERE version = 2;",
            )
            .expect("退回 v1");
        assert_eq!(database.schema_version().expect("版本"), 1);
        database.close().expect("关闭");
    }

    let upgraded = open_database(&dir, &clock);
    assert_eq!(upgraded.schema_version().expect("版本"), SCHEMA_VERSION);
    assert!(object_exists(upgraded.connection(), "table", "audit_logs"));
    assert!(object_exists(
        upgraded.connection(),
        "trigger",
        "audit_logs_no_update"
    ));
    upgraded.close().expect("关闭");

    // 幂等：再开一次不会重复建表 / 报错
    let again = open_database(&dir, &clock);
    assert_eq!(again.schema_version().expect("版本"), SCHEMA_VERSION);
    assert_eq!(audit_row_count(again.connection()), 0);
}

/// `immediate` 档：每条 `append` 之后事件**立刻**可查。
#[test]
fn test_immediate_mode_is_visible_right_away() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("immediate");
    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);

    let event = sample_event("s_imm", "notepad.read_text", "2026-09-24T00:00:00Z");
    let outcome = log
        .append(&event, &AuditSubject::unattached())
        .expect("append");

    assert_eq!(outcome, AppendOutcome::Flushed { written: 1 });
    assert_eq!(log.pending(), 0);
    assert_eq!(audit_row_count(database.connection()), 1);
    assert!(log.verify_chain().expect("verify").is_intact());
}

/// `batched` 档：攒够 `max_events` 才落库，最后一条触发 flush 且**整批**一起写。
#[test]
fn test_batched_mode_flushes_at_capacity() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("capacity");
    let database = open_database(&dir, &clock);
    let mut log = batched_log(database.connection(), &clock, 3);

    let event = sample_event("s_cap", "notepad.read_text", "2026-09-24T00:00:00Z");
    for expected_pending in 1..=2_usize {
        let outcome = log
            .append(&event, &AuditSubject::unattached())
            .expect("append");
        assert_eq!(
            outcome,
            AppendOutcome::Buffered {
                pending: expected_pending
            }
        );
        assert_eq!(
            audit_row_count(database.connection()),
            0,
            "未到容量不应落库"
        );
    }

    let outcome = log
        .append(&event, &AuditSubject::unattached())
        .expect("append");
    assert_eq!(outcome, AppendOutcome::Flushed { written: 3 });
    assert_eq!(log.pending(), 0);
    assert_eq!(audit_row_count(database.connection()), 3);
}

/// `batched` 档：到点（200 ms）也触发 flush —— 用**注入时钟**推进，不靠 sleep。
#[test]
fn test_batched_mode_flushes_when_interval_elapses() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("interval");
    let database = open_database(&dir, &clock);
    let mut log = AuditLog::new(
        database.connection(),
        database.clock(),
        Durability::Batched {
            max_events: 100,
            max_interval_ms: 200,
        },
    )
    .expect("打开审计写入器");

    let event = sample_event("s_int", "notepad.read_text", "2026-09-24T00:00:00Z");
    assert_eq!(
        log.append(&event, &AuditSubject::unattached())
            .expect("append"),
        AppendOutcome::Buffered { pending: 1 }
    );
    assert_eq!(audit_row_count(database.connection()), 0);

    clock.advance_by(200);
    assert_eq!(
        log.append(&event, &AuditSubject::unattached())
            .expect("append"),
        AppendOutcome::Flushed { written: 2 },
        "到点后本次 append 应把缓冲整批写出"
    );
    assert_eq!(audit_row_count(database.connection()), 2);
}

/// flush 失败必须**不丢事件、不前进链尾**（铁律 1：不许假装写过）。
#[test]
fn test_flush_failure_keeps_buffer_and_does_not_advance_chain() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("flush-failure");
    let database = open_database(&dir, &clock);
    let mut log = batched_log(database.connection(), &clock, 10);

    let event = sample_event("s_fail", "notepad.read_text", "2026-09-24T00:00:00Z");
    log.append(&event, &AuditSubject::unattached())
        .expect("append");
    assert_eq!(log.pending(), 1);
    let tail_before = log.persisted_hash().to_owned();

    // 故障注入：把表删掉，制造"INSERT 一定失败"的环境
    database
        .connection()
        .execute_batch("DROP TABLE audit_logs;")
        .expect("注入故障");

    let failure = log.flush();
    assert!(failure.is_err(), "表不存在时 flush 必须报错");
    assert!(
        matches!(failure, Err(AuditError::Sqlite(_))),
        "应为 Sqlite 错误"
    );
    assert_eq!(log.pending(), 1, "失败后缓冲不得清空（否则丢事件）");
    assert_eq!(log.persisted_hash(), tail_before, "失败后链尾不得前进");
}

/// 链在重开后继续：新写入器的 `prev_hash` 必须接上已落库的链尾。
#[test]
fn test_chain_survives_reopen_and_continues() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("reopen");
    let first_hash;
    {
        let database = open_database(&dir, &clock);
        let mut log = immediate_log(database.connection(), &clock);
        log.append(
            &sample_event("s_r1", "notepad.read_text", "2026-09-24T00:00:00Z"),
            &AuditSubject::unattached(),
        )
        .expect("append");
        first_hash = log.persisted_hash().to_owned();
        assert_eq!(first_hash.len(), 64);
        database.close().expect("关闭");
    }

    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);
    assert_eq!(log.persisted_hash(), first_hash, "重开后应读回链尾");

    log.append(
        &sample_event("s_r2", "notepad.write_text", "2026-09-24T00:00:01Z"),
        &AuditSubject::step("t_1", "s_1"),
    )
    .expect("append");

    let records = log.records().expect("records");
    assert_eq!(records.len(), 2);
    let second = records.get(1).expect("第二行");
    assert_eq!(second.prev_hash, first_hash, "第二行必须接上第一行");
    assert_eq!(second.task_id.as_deref(), Some("t_1"));
    assert_eq!(second.step_id.as_deref(), Some("s_1"));
    assert!(log.verify_chain().expect("verify").is_intact());
}

/// `separate_db_full` 必须**显式报错**，不许静默降级成 batched（铁律 1）。
#[test]
fn test_separate_db_full_is_rejected_explicitly() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("separate-db");
    let database = open_database(&dir, &clock);

    let failure = AuditLog::new(
        database.connection(),
        database.clock(),
        Durability::SeparateDbFull,
    );
    match failure {
        Err(AuditError::UnsupportedDurability { mode, .. }) => {
            assert_eq!(mode, "separate_db_full");
        }
        Err(other) => panic!("必须返回 UnsupportedDurability，实际：{other:?}"),
        Ok(_) => panic!("必须返回 UnsupportedDurability，实际拿到了可用的 AuditLog"),
    }
}

/// 非法批量参数必须被拒绝（`max_events = 0` 等于"每条都 flush"，那种语义请用 `Immediate`）。
#[test]
fn test_invalid_batched_parameters_are_rejected() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("invalid-params");
    let database = open_database(&dir, &clock);

    let zero_events = AuditLog::new(
        database.connection(),
        database.clock(),
        Durability::Batched {
            max_events: 0,
            max_interval_ms: 200,
        },
    );
    assert!(matches!(
        zero_events,
        Err(AuditError::InvalidArgument {
            field: "max_events",
            ..
        })
    ));

    let zero_interval = AuditLog::new(
        database.connection(),
        database.clock(),
        Durability::Batched {
            max_events: 100,
            max_interval_ms: 0,
        },
    );
    assert!(matches!(
        zero_interval,
        Err(AuditError::InvalidArgument {
            field: "max_interval_ms",
            ..
        })
    ));
}

/// DoD：「batched 摊销 < 1 ms/条」—— **实测并打印**，断言上限。
///
/// 为什么 2000 条：单条 INSERT 的成本约几十 µs，样本太少会被首次分配 / 预热掩盖。
#[test]
fn test_batched_amortized_cost_below_one_ms_per_entry() {
    const ENTRIES: usize = 2_000;
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("amortized");
    let database = open_database(&dir, &clock);
    let mut log = batched_log(database.connection(), &clock, 100);
    let event = sample_event("s_amortized", "notepad.read_text", "2026-09-24T00:00:00Z");

    let started = std::time::Instant::now();
    for _ in 0..ENTRIES {
        log.append(&event, &AuditSubject::unattached())
            .expect("append");
    }
    log.flush().expect("flush 尾批");
    let elapsed = started.elapsed();
    // 用 `u32 -> f64`（精确）而不是 `usize as f64`（可能丢精度）：条目数远小于 u32::MAX。
    let entries_f64 = f64::from(u32::try_from(ENTRIES).expect("条目数在 u32 范围内"));
    let per_entry_ms = elapsed.as_secs_f64() * 1000.0 / entries_f64;

    println!(
        "batched 摊销实测：{ENTRIES} 条 / {elapsed:?} → {per_entry_ms:.4} ms/条（阈值 < 1 ms）"
    );

    assert_eq!(log.records().expect("records").len(), ENTRIES);
    assert!(
        log.verify_chain().expect("verify").is_intact(),
        "2000 条同毫秒事件也必须串成自洽的链（这正是走链校验存在的理由）"
    );
    assert!(
        per_entry_ms < 1.0,
        "batched 摊销 {per_entry_ms:.4} ms/条 超过 1 ms 阈值"
    );
}
