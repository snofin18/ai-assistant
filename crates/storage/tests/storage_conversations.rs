//! 会话与消息树持久化测试（TASK-230）：真实 SQLite、真实关闭 / 重开、真实外键与顺序校验。
//!
//! 这里不把 `MemorySessionStore` 当持久化证据：每条正向用例都必须经过 `Database::close()`，
//! 再从同一路径重新打开并读回相同内容；负向用例必须证明失败后没有半个树落库。
//!
//! 测试内允许 `unwrap` / `expect` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;

use assistant_storage::{
    CONVERSATION_STATUS_ACTIVE, ConversationMessageRecord, ConversationRecord, Migration,
    MigrationSet, StorageError, insert_conversation_message, insert_conversation_snapshot,
    load_conversation, load_conversation_messages, load_conversation_snapshot,
    replace_conversation_snapshot,
};
use common::{FixedClock, TestDir, count_rows, open_database_with};

fn conversation_migrations() -> MigrationSet {
    let mut set = MigrationSet::new();
    set.register_all(assistant_storage::MIGRATIONS)
        .expect("注册 storage 全部迁移");
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
    set.validate().expect("装配后的测试迁移集必须从 1 连续");
    set
}

fn sample_conversation(revision: i64) -> ConversationRecord {
    ConversationRecord {
        id: "session-1".to_owned(),
        goal: "整理并保存一份报告".to_owned(),
        title: Some("报告整理".to_owned()),
        model_config_id: None,
        status: CONVERSATION_STATUS_ACTIVE.to_owned(),
        created_at_unix_ms: 1_700_000_000_000,
        ended_at_unix_ms: None,
        revision,
        archived: false,
    }
}

fn sample_message(
    id: &str,
    parent_id: Option<&str>,
    sequence: i64,
    role: &str,
    content: &str,
) -> ConversationMessageRecord {
    ConversationMessageRecord {
        id: id.to_owned(),
        conversation_id: "session-1".to_owned(),
        parent_id: parent_id.map(str::to_owned),
        sequence,
        role: role.to_owned(),
        content: content.to_owned(),
        token_estimate: 10,
        retention: "required".to_owned(),
    }
}

/// 正向路径：写入、显式关闭、重新打开同一数据库后，会话字段与消息树逐字段一致。
#[test]
fn test_conversation_snapshot_survives_close_and_reopen() {
    let dir = TestDir::new("conversations-reopen");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let migrations = conversation_migrations();
    let conversation = sample_conversation(0);
    let messages = vec![
        sample_message("m-1", None, 0, "user", "请整理报告"),
        sample_message("m-2", Some("m-1"), 1, "assistant", "开始整理"),
        sample_message("m-3", Some("m-2"), 2, "tool", "已生成摘要"),
    ];

    {
        let database = open_database_with(&dir, &clock, &migrations);
        assert_eq!(database.schema_version().expect("版本"), 5);
        insert_conversation_snapshot(database.connection(), &conversation, &messages)
            .expect("写入快照");
        let loaded = load_conversation_snapshot(database.connection(), "session-1")
            .expect("首次读取")
            .expect("会话存在");
        assert_eq!(loaded.0, conversation);
        assert_eq!(loaded.1, messages);
        database.close().expect("显式关闭，模拟进程退出");
    }

    let reopened = open_database_with(&dir, &clock, &migrations);
    let loaded = load_conversation_snapshot(reopened.connection(), "session-1")
        .expect("重开读取")
        .expect("会话仍在");
    assert_eq!(loaded.0, conversation);
    assert_eq!(loaded.1, messages);
    assert_eq!(
        load_conversation(reopened.connection(), "session-1")
            .expect("读取会话")
            .expect("会话存在"),
        conversation
    );
    assert_eq!(
        load_conversation_messages(reopened.connection(), "session-1").expect("读取消息"),
        messages
    );

    // 第二个 revision 也经过一次真实重开，证明替换不是只改内存副本。
    let updated = sample_conversation(1);
    let mut updated_messages = messages;
    updated_messages.push(sample_message(
        "m-4",
        Some("m-3"),
        3,
        "assistant",
        "报告已保存",
    ));
    replace_conversation_snapshot(reopened.connection(), &updated, &updated_messages)
        .expect("替换到 revision 1");
    reopened.close().expect("关闭更新后的数据库");

    let reopened_again = open_database_with(&dir, &clock, &migrations);
    let loaded = load_conversation_snapshot(reopened_again.connection(), "session-1")
        .expect("第二次重开读取")
        .expect("会话仍在");
    assert_eq!(loaded.0, updated);
    assert_eq!(loaded.1, updated_messages);
}

/// 缺失会话：消息插入必须显式失败，且不留下消息行。
#[test]
fn test_insert_message_rejects_missing_conversation() {
    let dir = TestDir::new("conversations-missing");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database_with(&dir, &clock, &conversation_migrations());

    let error = insert_conversation_message(
        database.connection(),
        &sample_message("m-1", None, 0, "user", "孤立消息"),
    )
    .expect_err("缺失会话必须失败");
    assert_eq!(error.reason_code(), "invalid_argument");
    assert!(matches!(
        error,
        StorageError::InvalidArgument {
            field: "conversation_id",
            ..
        }
    ));
    assert_eq!(
        count_rows(database.connection(), "conversation_messages"),
        0
    );
}

/// 非法父子关系：父不存在或父 sequence 不小于子 sequence 都必须 fail-closed。
#[test]
fn test_insert_message_rejects_invalid_parent_relationships() {
    let dir = TestDir::new("conversations-parent");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database_with(&dir, &clock, &conversation_migrations());
    let connection = database.connection();

    assistant_storage::insert_conversation(connection, &sample_conversation(0)).expect("创建会话");
    insert_conversation_message(
        connection,
        &sample_message("m-root", None, 10, "user", "根消息"),
    )
    .expect("写入根消息");

    let missing_parent = insert_conversation_message(
        connection,
        &sample_message("m-missing-parent", Some("m-nope"), 20, "assistant", "孤儿"),
    )
    .expect_err("缺失父节点必须失败");
    assert_eq!(missing_parent.reason_code(), "invalid_argument");

    let wrong_order = insert_conversation_message(
        connection,
        &sample_message("m-wrong-order", Some("m-root"), 10, "assistant", "顺序错误"),
    )
    .expect_err("父 sequence 不小于子 sequence 必须失败");
    assert_eq!(wrong_order.reason_code(), "invalid_argument");

    assert_eq!(count_rows(connection, "conversation_messages"), 1);
}

/// 整树插入是原子的：树里任何一条非法关系都不得留下会话行或部分消息行。
#[test]
fn test_insert_snapshot_rejects_invalid_tree_without_partial_rows() {
    let dir = TestDir::new("conversations-atomic");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database_with(&dir, &clock, &conversation_migrations());
    let invalid_messages = vec![sample_message(
        "m-1",
        Some("m-missing"),
        0,
        "assistant",
        "缺少父节点",
    )];

    let error = insert_conversation_snapshot(
        database.connection(),
        &sample_conversation(0),
        &invalid_messages,
    )
    .expect_err("非法树必须失败");
    assert_eq!(error.reason_code(), "invalid_argument");
    assert_eq!(count_rows(database.connection(), "conversations"), 0);
    assert_eq!(
        count_rows(database.connection(), "conversation_messages"),
        0
    );
}

/// revision 不能跳号；失败后旧快照必须原样保留。
#[test]
fn test_replace_snapshot_rejects_revision_gap_and_keeps_old_tree() {
    let dir = TestDir::new("conversations-revision");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database_with(&dir, &clock, &conversation_migrations());
    let connection = database.connection();
    let original = sample_conversation(0);
    let original_messages = vec![sample_message("m-1", None, 0, "user", "原消息")];
    insert_conversation_snapshot(connection, &original, &original_messages).expect("初写");

    let error = replace_conversation_snapshot(
        connection,
        &sample_conversation(2),
        &[sample_message(
            "m-2",
            Some("m-1"),
            1,
            "assistant",
            "跳号更新",
        )],
    )
    .expect_err("revision 跳号必须失败");
    assert_eq!(error.reason_code(), "invalid_argument");

    let loaded = load_conversation_snapshot(connection, "session-1")
        .expect("读取原快照")
        .expect("会话存在");
    assert_eq!(loaded.0, original);
    assert_eq!(loaded.1, original_messages);
}
