//! Public-contract coverage for storage behavior that is not exercised by the
//! focused integration suites.
//!
//! The tests use only public APIs and the shared temporary-directory fixture.
//! They deliberately exercise fail-closed validation, idempotent mutation, and
//! missing-row behavior without changing production code.
//!
//! Tests may use `expect` / `unwrap` / `panic` per AGENTS.md section 5.3.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;

use assistant_storage::{
    BlobId, BlobKind, BlobOwner, CONVERSATION_STATUS_ACTIVE, CONVERSATION_STATUS_ENDED,
    CheckpointRecord, Clock, ConversationMessageRecord, ConversationRecord, Database,
    MAX_MEMORY_CONTENT_BYTES, MAX_MEMORY_QUERY_CHARS, MAX_MEMORY_QUERY_TERMS,
    MAX_MEMORY_RECORD_ID_CHARS, MAX_MEMORY_SEARCH_LIMIT, MAX_MEMORY_SOURCE_REFERENCE_CHARS,
    MAX_MESSAGE_CONTENT_BYTES, MemoryQuery, MemoryRecord, MemoryRecordKind, Migration,
    MigrationSet, StorageError, StoragePaths, SystemClock, UsageRecord, count_usage_records,
    delete_memory_record, insert_conversation, insert_conversation_message,
    insert_conversation_snapshot, insert_memory_record, load_conversation,
    load_conversation_messages, load_latest_checkpoint, load_task, load_task_steps,
    replace_conversation_snapshot, search_memory,
};
use common::{FixedClock, TestDir, open_database_with};

fn full_migrations() -> MigrationSet {
    let mut set = MigrationSet::new();
    set.register_all(assistant_storage::MIGRATIONS)
        .expect("register storage migrations");
    set.register(Migration::new(
        2,
        "0002_audit_logs",
        include_str!("../../audit/migrations/0002_audit_logs.sql"),
    ))
    .expect("register audit 0002");
    set.register(Migration::new(
        3,
        "0003_audit_logs_semantics",
        include_str!("../../audit/migrations/0003_audit_logs_semantics.sql"),
    ))
    .expect("register audit 0003");
    set.validate()
        .expect("full migration set must be contiguous");
    set
}

fn conversation(revision: i64) -> ConversationRecord {
    ConversationRecord {
        id: "contract-session".to_owned(),
        goal: "storage contract coverage".to_owned(),
        title: Some("coverage".to_owned()),
        model_config_id: None,
        status: CONVERSATION_STATUS_ACTIVE.to_owned(),
        created_at_unix_ms: 1_700_000_000_000,
        ended_at_unix_ms: None,
        revision,
        archived: false,
    }
}

fn message(id: &str, sequence: i64, role: &str, retention: &str) -> ConversationMessageRecord {
    ConversationMessageRecord {
        id: id.to_owned(),
        conversation_id: "contract-session".to_owned(),
        parent_id: None,
        sequence,
        role: role.to_owned(),
        content: "bounded contract message".to_owned(),
        token_estimate: 4,
        retention: retention.to_owned(),
    }
}

fn open_full(dir: &TestDir, clock: &Arc<FixedClock>) -> (Database, MigrationSet) {
    let migrations = full_migrations();
    let database = open_database_with(dir, clock, &migrations);
    (database, migrations)
}

fn assert_invalid(error: StorageError, field: &'static str) {
    assert_eq!(error.reason_code(), "invalid_argument");
    match error {
        StorageError::InvalidArgument { field: actual, .. } => assert_eq!(actual, field),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[test]
fn test_system_clock_and_database_view_contracts() {
    let clock = Arc::new(FixedClock::new(42));
    let dir = TestDir::new("storage-view");
    let database = open_database_with(&dir, &clock, &full_migrations());

    assert_eq!(database.paths().root(), dir.paths().root());
    assert_eq!(database.clock().now_unix_ms(), 42);
    assert!(format!("{database:?}").contains("paths"));
    assert!(SystemClock.now_unix_ms() >= 1_700_000_000_000);

    let storage_paths = StoragePaths::new(dir.paths().root());
    assert_eq!(storage_paths.database_file(), dir.paths().database_file());
    assert_eq!(storage_paths.blob_root(), dir.paths().blob_root());
    assert_eq!(storage_paths.shadow_root(), dir.paths().shadow_root());
}

#[test]
fn test_migration_set_rejects_invalid_duplicate_and_non_contiguous_sets() {
    let mut set = MigrationSet::new();
    assert!(set.is_empty());
    assert_eq!(set.expected_version(), 0);
    assert_eq!(set.len(), 0);

    let invalid = set
        .register(Migration::new(0, "zero", "SELECT 1;"))
        .expect_err("zero version must fail");
    assert_eq!(invalid.reason_code(), "migration_invalid_version");
    assert!(!invalid.to_string().is_empty());

    set.register(Migration::new(2, "two", "SELECT 2;"))
        .expect("version 2");
    let duplicate = set
        .register(Migration::new(2, "two-again", "SELECT 3;"))
        .expect_err("duplicate version must fail");
    assert_eq!(duplicate.reason_code(), "migration_duplicate_version");
    assert!(duplicate.to_string().contains("two-again"));

    set.register(Migration::new(1, "one", "SELECT 1;"))
        .expect("version 1");
    set.register(Migration::new(3, "three", "SELECT 3;"))
        .expect("version 3");
    assert_eq!(set.expected_version(), 3);
    assert_eq!(set.len(), 3);
    assert_eq!(
        set.iter()
            .map(|migration| migration.version())
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    set.validate().expect("contiguous set");

    let mut gap = MigrationSet::new();
    gap.register(Migration::new(1, "one", "SELECT 1;"))
        .expect("version 1");
    gap.register(Migration::new(3, "three", "SELECT 3;"))
        .expect("version 3");
    let non_contiguous = gap.validate().expect_err("gap must fail");
    assert_eq!(
        non_contiguous.reason_code(),
        "migration_non_contiguous_versions"
    );
    assert!(!non_contiguous.to_string().is_empty());
}

#[test]
fn test_conversation_field_validation_is_fail_closed() {
    let clock = Arc::new(FixedClock::new(10));
    let dir = TestDir::new("conversation-fields");
    let (database, _) = open_full(&dir, &clock);
    let connection = database.connection();

    let mut invalid = conversation(0);
    invalid.id.clear();
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("empty id"),
        "conversation.id",
    );

    let mut invalid = conversation(0);
    invalid.goal.clear();
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("empty goal"),
        "goal",
    );

    let mut invalid = conversation(0);
    invalid.goal = "a".repeat(4_097);
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("oversized goal"),
        "goal",
    );

    let mut invalid = conversation(0);
    invalid.goal = "bad\u{7}goal".to_owned();
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("control goal"),
        "goal",
    );

    let mut invalid = conversation(0);
    invalid.status = "paused".to_owned();
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("unknown status"),
        "status",
    );

    let mut invalid = conversation(0);
    invalid.created_at_unix_ms = -1;
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("negative created"),
        "created_at_unix_ms",
    );

    let invalid = conversation(-1);
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("negative revision"),
        "revision",
    );

    let mut invalid = conversation(0);
    invalid.ended_at_unix_ms = Some(1);
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("active ended"),
        "ended_at_unix_ms",
    );

    let mut invalid = conversation(0);
    invalid.status = CONVERSATION_STATUS_ENDED.to_owned();
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("ended without timestamp"),
        "ended_at_unix_ms",
    );

    let mut invalid = conversation(0);
    invalid.status = CONVERSATION_STATUS_ENDED.to_owned();
    invalid.ended_at_unix_ms = Some(invalid.created_at_unix_ms - 1);
    assert_invalid(
        insert_conversation(connection, &invalid).expect_err("ended before created"),
        "ended_at_unix_ms",
    );

    assert_eq!(
        load_conversation(connection, "contract-session").expect("missing load"),
        None
    );
}

#[test]
fn test_message_shape_validation_is_fail_closed() {
    let clock = Arc::new(FixedClock::new(20));
    let dir = TestDir::new("message-fields");
    let (database, _) = open_full(&dir, &clock);
    let connection = database.connection();
    insert_conversation(connection, &conversation(0)).expect("conversation");

    let mut invalid = message("", 0, "user", "required");
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("empty id"),
        "message.id",
    );

    invalid = message("bad id", 0, "user", "required");
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("bad id"),
        "message.id",
    );

    invalid = message("negative-sequence", -1, "user", "required");
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("negative sequence"),
        "sequence",
    );

    invalid = message("unknown-role", 0, "root", "required");
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("unknown role"),
        "role",
    );

    invalid = message("empty-content", 0, "user", "required");
    invalid.content.clear();
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("empty content"),
        "content",
    );

    invalid = message("control-content", 0, "user", "required");
    invalid.content = "bad\u{7}content".to_owned();
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("control content"),
        "content",
    );

    invalid = message("large-content", 0, "user", "required");
    invalid.content = "a".repeat(MAX_MESSAGE_CONTENT_BYTES + 1);
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("large content"),
        "content",
    );

    invalid = message("zero-token", 0, "user", "required");
    invalid.token_estimate = 0;
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("zero token"),
        "token_estimate",
    );

    invalid = message("unknown-retention", 0, "user", "forever");
    assert_invalid(
        insert_conversation_message(connection, &invalid).expect_err("unknown retention"),
        "retention",
    );
}

#[test]
fn test_message_tree_validation_rejects_structural_conflicts() {
    let clock = Arc::new(FixedClock::new(30));
    let dir = TestDir::new("message-tree");
    let (database, _) = open_full(&dir, &clock);
    let connection = database.connection();

    let duplicate_id = vec![
        message("same", 0, "user", "required"),
        message("same", 1, "assistant", "required"),
    ];
    let error = insert_conversation_snapshot(connection, &conversation(0), &duplicate_id)
        .expect_err("duplicate ids");
    assert_invalid(error, "message.id");

    let duplicate_sequence = vec![
        message("first", 0, "user", "required"),
        message("second", 0, "assistant", "required"),
    ];
    let error = insert_conversation_snapshot(connection, &conversation(0), &duplicate_sequence)
        .expect_err("duplicate sequences");
    assert_invalid(error, "message.sequence");

    let mut wrong_conversation = message("wrong-conversation", 0, "user", "required");
    wrong_conversation.conversation_id = "other-session".to_owned();
    let error = insert_conversation_snapshot(connection, &conversation(0), &[wrong_conversation])
        .expect_err("wrong conversation");
    assert_invalid(error, "conversation_id");

    let mut dangling_parent = message("dangling", 1, "assistant", "required");
    dangling_parent.parent_id = Some("missing-parent".to_owned());
    let error = insert_conversation_snapshot(connection, &conversation(0), &[dangling_parent])
        .expect_err("dangling parent");
    assert_invalid(error, "parent_id");

    let mut wrong_order = message("wrong-order", 2, "assistant", "required");
    wrong_order.parent_id = Some("later-parent".to_owned());
    let parent = message("later-parent", 3, "user", "required");
    let error = insert_conversation_snapshot(connection, &conversation(0), &[parent, wrong_order])
        .expect_err("parent order");
    assert_invalid(error, "sequence");

    assert_eq!(
        load_conversation(connection, "contract-session").expect("no partial tree"),
        None
    );
    assert!(
        load_conversation_messages(connection, "contract-session")
            .expect("no partial messages")
            .is_empty()
    );
}

#[test]
fn test_replace_conversation_rejects_missing_and_revision_overflow() {
    let clock = Arc::new(FixedClock::new(40));
    let dir = TestDir::new("replace-errors");
    let (database, _) = open_full(&dir, &clock);
    let connection = database.connection();

    let error = replace_conversation_snapshot(connection, &conversation(1), &[])
        .expect_err("missing conversation");
    assert_invalid(error, "conversation_id");

    let mut max_revision = conversation(i64::MAX);
    max_revision.id = "max-revision".to_owned();
    insert_conversation(connection, &max_revision).expect("insert max revision");
    max_revision.revision = i64::MAX;
    let error = replace_conversation_snapshot(connection, &max_revision, &[])
        .expect_err("revision overflow");
    assert_invalid(error, "revision");
}

#[test]
fn test_blob_unknown_and_idempotent_reference_contracts() {
    let clock = Arc::new(FixedClock::new(50));
    let dir = TestDir::new("blob-contract");
    let database = open_database_with(&dir, &clock, &full_migrations());
    let blobs = database.blob_store();
    let connection = database.connection();
    let unknown = BlobId::of_content(b"not-registered");
    let owner = BlobOwner::new("contract", "owner-1").expect("owner");

    assert_eq!(blobs.root(), dir.paths().blob_root());
    assert_eq!(
        blobs.path_for(&unknown),
        blobs.root().join(unknown.relative_path())
    );
    assert_eq!(
        blobs
            .get(connection, &unknown)
            .expect_err("unknown get")
            .reason_code(),
        "blob_unknown"
    );
    assert_eq!(
        blobs
            .add_reference(connection, &unknown, &owner)
            .expect_err("unknown reference")
            .reason_code(),
        "blob_unknown"
    );
    assert!(
        !blobs
            .remove_reference(connection, &unknown, &owner)
            .expect("idempotent remove")
    );
    assert_eq!(
        blobs
            .reference_count(connection, &unknown)
            .expect("reference count"),
        0
    );

    let known = blobs
        .put(connection, BlobKind::Other, b"known")
        .expect("put known");
    assert!(
        blobs
            .add_reference(connection, &known, &owner)
            .expect("first reference")
    );
    assert!(
        !blobs
            .add_reference(connection, &known, &owner)
            .expect("duplicate reference")
    );
    assert_eq!(
        blobs
            .reference_count(connection, &known)
            .expect("known reference count"),
        1
    );
    assert!(
        blobs
            .remove_reference(connection, &known, &owner)
            .expect("remove reference")
    );
}

#[test]
fn test_memory_delete_and_query_validation_contracts() {
    let clock = Arc::new(FixedClock::new(60));
    let dir = TestDir::new("memory-contract");
    let database = open_database_with(&dir, &clock, &full_migrations());
    let connection = database.connection();

    let record = MemoryRecord {
        record_kind: MemoryRecordKind::Note,
        record_id: "note-1".to_owned(),
        source_reference: "contract#1".to_owned(),
        content: "storage coverage note".to_owned(),
        updated_at: 60,
    };
    insert_memory_record(connection, &record).expect("insert memory");
    assert_eq!(
        search_memory(connection, &MemoryQuery::new("storage"))
            .expect("search")
            .len(),
        1
    );
    assert!(
        delete_memory_record(connection, MemoryRecordKind::Note, "note-1").expect("delete memory")
    );
    assert!(
        !delete_memory_record(connection, MemoryRecordKind::Note, "note-1")
            .expect("idempotent delete")
    );

    let mut invalid = record.clone();
    invalid.record_id.clear();
    assert_invalid(
        insert_memory_record(connection, &invalid).expect_err("empty record id"),
        "record_id",
    );

    invalid = record.clone();
    invalid.record_id = "a".repeat(MAX_MEMORY_RECORD_ID_CHARS + 1);
    assert_invalid(
        insert_memory_record(connection, &invalid).expect_err("long record id"),
        "record_id",
    );

    invalid = record.clone();
    invalid.source_reference.clear();
    assert_invalid(
        insert_memory_record(connection, &invalid).expect_err("empty source"),
        "source_reference",
    );

    invalid = record.clone();
    invalid.source_reference = "a".repeat(MAX_MEMORY_SOURCE_REFERENCE_CHARS + 1);
    assert_invalid(
        insert_memory_record(connection, &invalid).expect_err("long source"),
        "source_reference",
    );

    let mut empty_content = record.clone();
    empty_content.content.clear();
    assert_invalid(
        insert_memory_record(connection, &empty_content).expect_err("empty content"),
        "content",
    );

    let mut large_content = record;
    large_content.content = "a".repeat(MAX_MEMORY_CONTENT_BYTES + 1);
    assert_invalid(
        insert_memory_record(connection, &large_content).expect_err("large content"),
        "content",
    );

    for query in [
        MemoryQuery::new("   "),
        MemoryQuery::new("a".repeat(MAX_MEMORY_QUERY_CHARS + 1)),
        MemoryQuery::new("valid").with_limit(0),
        MemoryQuery::new("valid").with_limit(MAX_MEMORY_SEARCH_LIMIT + 1),
        MemoryQuery::new("bad\u{7}query"),
        MemoryQuery::new("!!!"),
    ] {
        let error = search_memory(connection, &query).expect_err("invalid query");
        assert_eq!(error.reason_code(), "invalid_memory_query");
    }

    let many_terms = (0..=MAX_MEMORY_QUERY_TERMS)
        .map(|index| format!("term{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    let error =
        search_memory(connection, &MemoryQuery::new(many_terms)).expect_err("too many terms");
    assert_eq!(error.reason_code(), "invalid_memory_query");
}

#[test]
fn test_missing_record_reads_and_usage_count_contracts() {
    let clock = Arc::new(FixedClock::new(70));
    let dir = TestDir::new("records-missing");
    let database = open_database_with(&dir, &clock, &full_migrations());
    let connection = database.connection();

    assert_eq!(
        load_task(connection, "missing").expect("missing task"),
        None
    );
    assert!(
        load_task_steps(connection, "missing")
            .expect("missing steps")
            .is_empty()
    );
    assert_eq!(
        load_latest_checkpoint(connection, "missing").expect("missing checkpoint"),
        None
    );
    assert_eq!(count_usage_records(connection).expect("usage count"), 0);

    assistant_storage::insert_task(
        connection,
        &assistant_storage::TaskRecord {
            id: "task-1".to_owned(),
            conversation_id: None,
            goal: "contract".to_owned(),
            plan_json: None,
            status: "running".to_owned(),
            reversibility_worst: None,
            budget_json: None,
            started_at: 70,
            ended_at: None,
            cost_usd: Some(0.0),
            tokens_in: 0,
            tokens_out: 0,
            error_code: None,
        },
    )
    .expect("insert task");
    assistant_storage::insert_checkpoint(
        connection,
        &CheckpointRecord {
            id: "checkpoint-1".to_owned(),
            task_id: "task-1".to_owned(),
            last_step_seq: 0,
            state_json: "{}".to_owned(),
            created_at: 70,
        },
    )
    .expect("insert checkpoint");
    assistant_storage::insert_usage_record(
        connection,
        &UsageRecord {
            id: "usage-1".to_owned(),
            ts: 70,
            task_id: Some("task-1".to_owned()),
            step_id: None,
            model_config_id: None,
            tokens_in: 1,
            tokens_out: 2,
            cached_tokens: 0,
            cost_usd: Some(0.0),
            latency_ms: Some(1),
            cache_hit: true,
        },
    )
    .expect("insert usage");
    assert_eq!(count_usage_records(connection).expect("usage count"), 1);
}
