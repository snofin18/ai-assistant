//! `memory_fts` 迁移、检索与一致性自检（TASK-206）。
//!
//! 这里测的是**真实 SQLite + FTS5**：触发器同步、BM25 检索、非法输入 fail-closed、
//! 以及源表与索引被人为篡改时的一致性报告。
//!
//! 测试内允许 `unwrap` / `expect` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;

use assistant_protocol::ErrorCategory;
use assistant_storage::{
    Database, MIGRATIONS, MemoryIndexIssueKind, MemoryQuery, MemoryRecord, MemoryRecordKind,
    Migration, MigrationSet, StorageError, delete_memory_record, insert_memory_record,
    search_memory, verify_memory_index,
};
use common::{FixedClock, TestDir, count_rows, open_database_with};
use rusqlite::params;

fn sample_record(kind: MemoryRecordKind, id: &str, reference: &str, content: &str) -> MemoryRecord {
    MemoryRecord {
        record_kind: kind,
        record_id: id.to_owned(),
        source_reference: reference.to_owned(),
        content: content.to_owned(),
        updated_at: 1_700_000_000_000,
    }
}

fn object_kind(connection: &rusqlite::Connection, name: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT type FROM sqlite_master WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )
        .ok()
}

fn memory_migrations() -> MigrationSet {
    let mut set = MigrationSet::new();
    set.register_all(MIGRATIONS)
        .expect("注册 storage 0001 + 0004");
    set.register(Migration::new(
        2,
        "0002_audit_logs",
        include_str!("../../audit/migrations/0002_audit_logs.sql"),
    ))
    .expect("注册 audit 0002");
    set.register(Migration::new(
        3,
        "0003_audit_logs_semantics",
        include_str!("../../audit/migrations/0003_audit_logs_semantics.sql"),
    ))
    .expect("注册 audit 0003");
    set.validate().expect("测试用全局迁移集必须从 1 连续");
    set
}

fn baseline_migrations() -> MigrationSet {
    let mut set = MigrationSet::new();
    let base = MIGRATIONS
        .iter()
        .find(|migration| migration.version() == 1)
        .copied()
        .expect("storage 必须声明 0001");
    set.register(base).expect("注册 storage 0001");
    set.validate().expect("v1 集合必须从 1 连续");
    set
}

fn open_memory_database(dir: &TestDir, clock: &Arc<FixedClock>) -> Database {
    open_database_with(dir, clock, &memory_migrations())
}

/// 迁移 0004 同时建立源表、FTS5 虚表与三个同步触发器。
#[test]
fn test_memory_fts_migration_creates_source_index_and_triggers() {
    let dir = TestDir::new("memory-schema");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_memory_database(&dir, &clock);
    let connection = database.connection();

    assert_eq!(
        object_kind(connection, "memory_records").as_deref(),
        Some("table")
    );
    assert_eq!(
        object_kind(connection, "memory_fts").as_deref(),
        Some("table")
    );
    for trigger in [
        "memory_records_after_insert",
        "memory_records_after_delete",
        "memory_records_after_update",
    ] {
        assert_eq!(
            object_kind(connection, trigger).as_deref(),
            Some("trigger"),
            "缺少同步触发器 {trigger}"
        );
    }
}

/// 旧 v1 库升级到 0004 后，与新建库到达同一个 schema 版本。
#[test]
fn test_memory_fts_migration_upgrade_and_fresh_paths_converge() {
    let upgraded_dir = TestDir::new("memory-upgrade");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));

    {
        let old = open_database_with(&upgraded_dir, &clock, &baseline_migrations());
        assert_eq!(old.schema_version().expect("读 v1 版本"), 1);
        old.close().expect("关闭 v1 库");
    }

    let upgraded = open_database_with(&upgraded_dir, &clock, &memory_migrations());
    let upgraded_version = upgraded.schema_version().expect("读升级后版本");
    assert_eq!(upgraded_version, 4);
    assert_eq!(count_rows(upgraded.connection(), "schema_migrations"), 4);

    let fresh_dir = TestDir::new("memory-fresh");
    let fresh = open_memory_database(&fresh_dir, &clock);
    assert_eq!(
        fresh.schema_version().expect("读新库版本"),
        upgraded_version,
        "新库与升级库必须到达同一 schema 版本"
    );
}

/// 源表写入 / 删除必须由触发器同步到 FTS5；不能出现“源表删了、索引还在”。
#[test]
fn test_memory_record_write_and_delete_synchronize_fts() {
    let dir = TestDir::new("memory-sync");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_memory_database(&dir, &clock);
    let connection = database.connection();

    insert_memory_record(
        connection,
        &sample_record(
            MemoryRecordKind::Preference,
            "pref-1",
            "settings#L3",
            "prefers concise reports",
        ),
    )
    .expect("写偏好");
    assert_eq!(count_rows(connection, "memory_records"), 1);
    assert_eq!(count_rows(connection, "memory_fts"), 1);

    assert!(
        delete_memory_record(connection, MemoryRecordKind::Preference, "pref-1").expect("删除偏好")
    );
    assert_eq!(count_rows(connection, "memory_records"), 0);
    assert_eq!(count_rows(connection, "memory_fts"), 0);
    assert!(
        !delete_memory_record(connection, MemoryRecordKind::Preference, "pref-1")
            .expect("重复删除必须幂等"),
        "不存在的记录不得谎报删除成功"
    );
}

/// 检索命中必须返回类型、主键、原文引用和高亮片段。
#[test]
fn test_search_memory_returns_traceable_match() {
    let dir = TestDir::new("memory-search");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_memory_database(&dir, &clock);
    let connection = database.connection();

    insert_memory_record(
        connection,
        &sample_record(
            MemoryRecordKind::TaskHistory,
            "task-42",
            "audit:2026-09-27#L88",
            "notepad save report summary",
        ),
    )
    .expect("写任务历史");

    let results = search_memory(connection, &MemoryQuery::new("report")).expect("检索");
    assert_eq!(results.len(), 1);
    let hit = results.first().expect("命中");
    assert_eq!(hit.record_kind, MemoryRecordKind::TaskHistory);
    assert_eq!(hit.record_id, "task-42");
    assert_eq!(hit.source_reference, "audit:2026-09-27#L88");
    assert!(hit.snippet.contains("[report]"), "{}", hit.snippet);
    assert!(hit.score.is_finite());
}

/// 未命中返回空结果，而不是伪造命中或报错。
#[test]
fn test_search_memory_returns_empty_when_no_match() {
    let dir = TestDir::new("memory-no-match");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_memory_database(&dir, &clock);
    let connection = database.connection();

    insert_memory_record(
        connection,
        &sample_record(MemoryRecordKind::Note, "note-1", "notes#L1", "alpha beta"),
    )
    .expect("写笔记");

    assert!(
        search_memory(connection, &MemoryQuery::new("gamma"))
            .expect("检索")
            .is_empty()
    );
}

/// 非法查询必须返回稳定 `reason_code()`，不能退化成“空结果成功”。
#[test]
fn test_invalid_memory_queries_fail_closed() {
    let dir = TestDir::new("memory-invalid-query");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_memory_database(&dir, &clock);
    let connection = database.connection();

    let queries = [
        MemoryQuery::new(""),
        MemoryQuery::new("   "),
        MemoryQuery::new("!!! ???"),
        MemoryQuery::new("x".repeat(513)),
        MemoryQuery::new("too many").with_limit(0),
        MemoryQuery::new("too many").with_limit(101),
    ];
    for query in queries {
        let error = search_memory(connection, &query).expect_err("非法查询必须失败");
        assert_eq!(error.reason_code(), "invalid_memory_query");
        assert_eq!(error.error_category(), ErrorCategory::Fatal);
    }

    let error = search_memory(
        connection,
        &MemoryQuery::new(
            (1..=33)
                .map(|n| format!("term{n}"))
                .collect::<Vec<_>>()
                .join(" "),
        ),
    )
    .expect_err("词项数越界必须失败");
    assert_eq!(error.reason_code(), "invalid_memory_query");
}

/// 一致性自检必须分别发现“源行少索引”“索引孤儿”“字段快照不一致”。
#[test]
fn test_memory_index_integrity_reports_missing_orphan_and_mismatch() {
    let dir = TestDir::new("memory-integrity");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_memory_database(&dir, &clock);
    let connection = database.connection();

    insert_memory_record(
        connection,
        &sample_record(
            MemoryRecordKind::Note,
            "note-integrity",
            "notes#L9",
            "original searchable content",
        ),
    )
    .expect("写笔记");
    assert!(
        verify_memory_index(connection).expect("自检").is_empty(),
        "健康索引不得误报"
    );

    let row_id: i64 = connection
        .query_row(
            "SELECT id FROM memory_records WHERE record_kind = ?1 AND record_id = ?2",
            params!["note", "note-integrity"],
            |row| row.get(0),
        )
        .expect("读 rowid");

    // 命中路径也必须 fail-closed：索引正文过期时不得把 stale snippet 当结果返回。
    connection
        .execute(
            "UPDATE memory_fts SET content = ?1 WHERE rowid = ?2",
            params!["tampered", row_id],
        )
        .expect("篡改索引正文");
    let error = search_memory(connection, &MemoryQuery::new("tampered"))
        .expect_err("索引与源表不一致时必须失败");
    assert_eq!(error.reason_code(), "memory_index_inconsistent");

    // 模拟外部篡改 1：删掉索引行，源表还在。
    connection
        .execute("DELETE FROM memory_fts WHERE rowid = ?1", params![row_id])
        .expect("删索引行");
    let issues = verify_memory_index(connection).expect("自检");
    assert!(issues.iter().any(|issue| {
        issue.row_id == row_id && issue.kind == MemoryIndexIssueKind::SourceRowMissingFromIndex
    }));

    // 模拟外部篡改 2：索引行被手工塞回，但正文快照与源表不一致。
    connection
        .execute(
            "INSERT INTO memory_fts (rowid, record_kind, record_id, source_reference, content)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![row_id, "note", "note-integrity", "notes#L9", "tampered"],
        )
        .expect("塞回不一致索引行");
    let issues = verify_memory_index(connection).expect("自检");
    assert!(issues.iter().any(|issue| {
        issue.row_id == row_id
            && matches!(
                issue.kind,
                MemoryIndexIssueKind::IndexedFieldMismatch { .. }
            )
    }));

    // 模拟外部篡改 3：凭空插入一个没有源记录的索引行。
    connection
        .execute(
            "INSERT INTO memory_fts (rowid, record_kind, record_id, source_reference, content)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![9_001_i64, "note", "orphan", "notes#L404", "orphan content"],
        )
        .expect("插入孤儿索引行");
    let issues = verify_memory_index(connection).expect("自检");
    assert!(issues.iter().any(|issue| {
        issue.row_id == 9_001 && issue.kind == MemoryIndexIssueKind::IndexRowMissingFromSource
    }));
}

/// 记录形状错误直接拒绝，不把无效数据写进源表或索引。
#[test]
fn test_insert_memory_record_rejects_invalid_shape() {
    let dir = TestDir::new("memory-invalid-record");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_memory_database(&dir, &clock);
    let connection = database.connection();

    let invalid = [
        sample_record(MemoryRecordKind::Note, "", "notes#L1", "content"),
        sample_record(MemoryRecordKind::Note, "note-1", "", "content"),
        sample_record(MemoryRecordKind::Note, "note-1", "notes#L1", "  "),
    ];
    for record in invalid {
        let error = insert_memory_record(connection, &record).expect_err("非法记录必须失败");
        assert_eq!(error.reason_code(), "invalid_argument");
    }
    assert_eq!(count_rows(connection, "memory_records"), 0);
    assert_eq!(count_rows(connection, "memory_fts"), 0);
}

/// 未知记录类型不得被静默当成默认类型。
#[test]
fn test_memory_record_kind_parse_rejects_unknown() {
    assert_eq!(
        MemoryRecordKind::parse("note").expect("合法类型"),
        MemoryRecordKind::Note
    );
    let error = MemoryRecordKind::parse("system").expect_err("未知类型必须失败");
    assert_eq!(error.reason_code(), "invalid_argument");
    assert_eq!(error.error_category(), ErrorCategory::Fatal);
}

/// 两颗新错误码同样是稳定的 Fatal 类，且 Display 可读。
#[test]
fn test_new_memory_error_codes_are_stable() {
    let query_error = StorageError::InvalidMemoryQuery {
        field: "text",
        detail: "empty".to_owned(),
    };
    assert_eq!(query_error.reason_code(), "invalid_memory_query");
    assert_eq!(query_error.error_category(), ErrorCategory::Fatal);
    assert!(!query_error.to_string().is_empty());

    let index_error = StorageError::MemoryIndexInconsistent {
        detail: "row 1 differs".to_owned(),
    };
    assert_eq!(index_error.reason_code(), "memory_index_inconsistent");
    assert_eq!(index_error.error_category(), ErrorCategory::Fatal);
    assert!(!index_error.to_string().is_empty());
}
