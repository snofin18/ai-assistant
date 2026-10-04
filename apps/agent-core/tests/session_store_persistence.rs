//! Storage-backed `SessionStore` persistence tests (PL-108 / TASK-233).
//!
//! These tests open a real SQLite database in a temporary directory, drive the
//! adapter, and reopen the same data root to prove the session survives a
//! "restart". Every assertion checks persisted evidence, not in-memory state.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use assistant_agent_core::StorageSessionStore;
use assistant_core::{
    ContextRetention, MessageContent, MessageId, MessageNode, MessageNodeParts, MessageRole,
    SessionId, SessionSnapshot, SessionSnapshotParts, SessionStatus, SessionStore, TokenCount,
};
use assistant_storage::{Clock, Database, MIGRATIONS, MigrationSet, StoragePaths};

/// Deterministic clock so persisted timestamps are reproducible.
struct FixedClock;

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        1_700_000_000_000
    }
}

/// Temporary data root removed on drop.
struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "assistant-session-store-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp data root");
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Assembly-shaped database handle used by the adapter.
type Handle = Arc<Mutex<Database>>;

/// Opens (or reopens) the main database of one data root.
fn open_handle(directory: &TestDirectory) -> Handle {
    let mut migrations = MigrationSet::new();
    migrations
        .register_all(MIGRATIONS)
        .expect("storage migrations register");
    migrations
        .register_all(assistant_audit::MIGRATIONS)
        .expect("audit migrations register");
    let paths = StoragePaths::new(&directory.path);
    let database =
        Database::open(&paths, Arc::new(FixedClock), &migrations).expect("open database");
    Arc::new(Mutex::new(database))
}

/// Builds one validated message node.
fn node(
    id: &str,
    parent: Option<&str>,
    sequence: u32,
    role: MessageRole,
    content: &str,
) -> MessageNode {
    MessageNode::restore(MessageNodeParts {
        id: MessageId::new(id).expect("message id"),
        parent_id: parent.map(|parent| MessageId::new(parent).expect("parent id")),
        sequence,
        role,
        content: MessageContent::new(content).expect("message content"),
        token_estimate: TokenCount::new(7),
        retention: ContextRetention::Required,
    })
    .expect("message node")
}

/// Builds one validated session snapshot.
fn snapshot(revision: u64, messages: Vec<MessageNode>) -> SessionSnapshot {
    SessionSnapshot::restore(SessionSnapshotParts {
        id: SessionId::new("session-persist-alpha").expect("session id"),
        goal: "persist me across a restart".to_owned(),
        status: SessionStatus::Active,
        created_at_unix_ms: 1_700_000_000_000,
        ended_at_unix_ms: None,
        revision,
        messages,
    })
    .expect("session snapshot")
}

#[test]
fn test_storage_session_store_round_trips_through_reopen() {
    let directory = TestDirectory::new("round-trip");
    let first = snapshot(
        0,
        vec![
            node("m1", None, 0, MessageRole::User, "open notepad"),
            node("m2", Some("m1"), 1, MessageRole::Assistant, "opened"),
        ],
    );
    {
        let store = StorageSessionStore::new(open_handle(&directory));
        store.insert_session(&first).expect("insert session");
    }

    // Reopen the same data root: a fresh handle + adapter must see the session.
    let reopened = StorageSessionStore::new(open_handle(&directory));
    let loaded = reopened
        .load_session(first.id())
        .expect("load session")
        .expect("session must survive the reopen");
    assert_eq!(loaded, first, "reopened snapshot must match逐字段");

    // A revision+1 update must also survive a second reopen.
    let mut next_messages = first.messages().to_vec();
    next_messages.push(node(
        "m3",
        Some("m2"),
        2,
        MessageRole::Tool,
        "saved to disk",
    ));
    let updated = snapshot(1, next_messages);
    reopened
        .update_session(&updated)
        .expect("update to the next revision");
    drop(reopened);

    let third = StorageSessionStore::new(open_handle(&directory));
    let loaded = third
        .load_session(updated.id())
        .expect("load updated session")
        .expect("updated session must survive the reopen");
    assert_eq!(loaded.revision(), 1);
    assert_eq!(loaded.messages().len(), 3);
    assert_eq!(loaded, updated);
}

#[test]
fn test_storage_session_store_rejects_duplicates_and_bad_revisions() {
    let directory = TestDirectory::new("negatives");
    let store = StorageSessionStore::new(open_handle(&directory));
    let first = snapshot(0, vec![node("m1", None, 0, MessageRole::User, "hi")]);
    store.insert_session(&first).expect("first insert");

    let duplicate = store
        .insert_session(&first)
        .expect_err("duplicate insert must fail");
    assert_eq!(duplicate.reason_code(), "session_already_exists");

    let skipped = snapshot(5, vec![node("m1", None, 0, MessageRole::User, "hi")]);
    let conflict = store
        .update_session(&skipped)
        .expect_err("revision must advance by exactly one");
    assert_eq!(conflict.reason_code(), "revision_conflict");

    let missing = SessionSnapshot::restore(SessionSnapshotParts {
        id: SessionId::new("session-does-not-exist").expect("session id"),
        goal: "nobody wrote me".to_owned(),
        status: SessionStatus::Active,
        created_at_unix_ms: 1_700_000_000_000,
        ended_at_unix_ms: None,
        revision: 1,
        messages: Vec::new(),
    })
    .expect("snapshot");
    let not_found = store
        .update_session(&missing)
        .expect_err("updating an unknown session must fail");
    assert_eq!(not_found.reason_code(), "session_not_found");

    assert!(
        store
            .load_session(missing.id())
            .expect("load unknown session")
            .is_none(),
        "an unknown session is None, not an error"
    );
}

/// Values the schema already constrains (`status` / `role` / `retention` /
/// `revision` / `sequence` / `token_estimate`) are rejected by SQLite itself, so
/// the adapter's own guards are a second layer. To exercise that second layer this
/// test tampers with fields the schema deliberately leaves opaque (`goal`,
/// `content`) and requires the adapter to reject what Core's validators reject.
#[test]
fn test_storage_session_store_rejects_tampered_rows() {
    let directory = TestDirectory::new("tampered");
    let handle = open_handle(&directory);
    let store = StorageSessionStore::new(Arc::clone(&handle));
    let first = snapshot(0, vec![node("m1", None, 0, MessageRole::User, "hi")]);
    store.insert_session(&first).expect("insert session");

    {
        let database = handle.lock().expect("database lock");
        database
            .connection()
            .execute(
                "UPDATE conversations SET goal = '' WHERE id = 'session-persist-alpha'",
                [],
            )
            .expect("tamper goal");
    }
    let goal_error = store
        .load_session(first.id())
        .expect_err("an empty persisted goal must fail closed");
    // The storage record layer validates rows on read too, so this one is caught
    // there first and surfaces as its own failure code — still explicit, never a
    // defaulted snapshot.
    assert_eq!(goal_error.reason_code(), "session_store_failure");

    {
        let database = handle.lock().expect("database lock");
        database
            .connection()
            .execute(
                "UPDATE conversations SET goal = 'restored' WHERE id = 'session-persist-alpha'",
                [],
            )
            .expect("restore goal");
        database
            .connection()
            .execute(
                "UPDATE conversation_messages SET content = ''
                 WHERE conversation_id = 'session-persist-alpha'",
                [],
            )
            .expect("tamper content");
    }
    let content_error = store
        .load_session(first.id())
        .expect_err("an empty persisted content must fail closed");
    assert_eq!(content_error.reason_code(), "session_store_failure");

    {
        let database = handle.lock().expect("database lock");
        database
            .connection()
            .execute(
                "UPDATE conversation_messages SET content = 'ok' WHERE conversation_id = 'session-persist-alpha'",
                [],
            )
            .expect("restore content");
        database
            .connection()
            .execute(
                "UPDATE conversation_messages SET sequence = 4294967296
                 WHERE conversation_id = 'session-persist-alpha'",
                [],
            )
            .expect("tamper sequence beyond Core's u32 bound");
    }
    // The schema only requires `sequence >= 0`, so this value reaches the adapter,
    // which must reject what Core cannot represent instead of truncating it.
    let sequence_error = store
        .load_session(first.id())
        .expect_err("an out-of-range persisted sequence must fail closed");
    assert_eq!(sequence_error.reason_code(), "persisted_row_invalid");
}
