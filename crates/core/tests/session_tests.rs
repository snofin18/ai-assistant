//! Integration tests for Core session-tree behavior.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

mod common;

use std::sync::Arc;

use assistant_core::{
    ContextRetention, CoreError, MemorySessionStore, MessageContent, MessageNode, MessageNodeParts,
    MessageRole, SessionManager, SessionSnapshot, SessionSnapshotParts, SessionStatus,
    SessionStore, SessionStoreError, TokenCount,
};
use common::{
    FixedClock, content, create_linear_session, manager, message_id, new_message, session_id,
};

#[test]
fn identifiers_and_token_counts_validate_inputs() {
    assert!(assistant_core::SessionId::new("").is_err());
    assert!(assistant_core::SessionId::new("contains space").is_err());
    assert!(assistant_core::SessionId::new("x".repeat(129)).is_err());
    assert_eq!(
        assistant_core::SessionId::new("session-1_a.b")
            .unwrap()
            .as_str(),
        "session-1_a.b"
    );
    assert!(assistant_core::MessageId::new("").is_err());
    assert_eq!(
        TokenCount::new(1)
            .checked_add(TokenCount::new(2))
            .unwrap()
            .get(),
        3
    );
    assert_eq!(
        TokenCount::new(u64::MAX)
            .checked_add(TokenCount::new(1))
            .unwrap_err()
            .reason_code(),
        "numeric_overflow"
    );
}

#[test]
fn content_validates_inputs() {
    assert!(MessageContent::new("").is_err());
    assert!(MessageContent::new("bad\0value").is_err());
    assert!(MessageContent::new("x".repeat(1_048_577)).is_err());
    assert_eq!(
        MessageContent::new("ok\nnext").unwrap().as_str(),
        "ok\nnext"
    );
}

#[test]
fn session_lifecycle_append_delete_restore_and_end() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(Arc::clone(&store), Arc::clone(&clock));
    let (session_id, messages) = create_linear_session(&mut sessions);

    let snapshot = sessions.snapshot(&session_id).unwrap();
    assert_eq!(snapshot.messages().len(), 3);
    assert_eq!(snapshot.messages()[0].sequence(), 0);
    assert_eq!(snapshot.messages()[2].sequence(), 2);
    assert_eq!(snapshot.latest_leaf(), Some(&messages[2]));
    assert_eq!(snapshot.message_path(&messages[2]).unwrap().len(), 3);

    sessions.delete_message(&session_id, &messages[2]).unwrap();
    assert_eq!(sessions.snapshot(&session_id).unwrap().messages().len(), 2);
    clock.set(200);
    sessions.end_session(&session_id).unwrap();
    assert_eq!(
        sessions.snapshot(&session_id).unwrap().status(),
        SessionStatus::Ended
    );
    assert!(matches!(
        sessions.append_message(
            &session_id,
            new_message("m_after_end", None, 1, ContextRetention::Required)
        ),
        Err(CoreError::SessionEnded { .. })
    ));

    let mut restored = manager(Arc::clone(&store), Arc::clone(&clock));
    let restored = restored.restore_session(&session_id).unwrap();
    assert_eq!(restored.status(), SessionStatus::Ended);
    assert_eq!(restored.ended_at_unix_ms(), Some(200));
}

#[test]
fn session_missing_and_duplicate_cases_fail_closed() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(Arc::clone(&store), Arc::clone(&clock));
    assert!(matches!(
        sessions.snapshot(&session_id("missing")),
        Err(CoreError::SessionNotFound { .. })
    ));
    assert!(matches!(
        sessions.restore_session(&session_id("missing")),
        Err(CoreError::SessionNotFound { .. })
    ));
    sessions.create_session(session_id("s_1"), "goal").unwrap();
    assert!(matches!(
        sessions.create_session(session_id("s_1"), "again"),
        Err(CoreError::SessionAlreadyExists { .. })
    ));
    assert!(matches!(
        sessions.append_message(
            &session_id("missing"),
            new_message("m_1", None, 1, ContextRetention::Required)
        ),
        Err(CoreError::SessionNotFound { .. })
    ));
    assert!(matches!(
        sessions.delete_message(&session_id("missing"), &message_id("m_1")),
        Err(CoreError::SessionNotFound { .. })
    ));
    assert!(matches!(
        sessions.end_session(&session_id("missing")),
        Err(CoreError::SessionNotFound { .. })
    ));
}

#[test]
fn session_tree_rejects_missing_parent_duplicate_and_non_leaf_delete() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    let session = session_id("s_1");
    sessions.create_session(session.clone(), "goal").unwrap();

    assert!(matches!(
        sessions.append_message(
            &session,
            new_message(
                "m_child",
                Some("missing_parent"),
                1,
                ContextRetention::Required
            )
        ),
        Err(CoreError::ParentMessageNotFound { .. })
    ));
    let first = sessions
        .append_message(
            &session,
            new_message("m_1", None, 1, ContextRetention::Required),
        )
        .unwrap();
    let second = sessions
        .append_message(
            &session,
            new_message("m_2", Some(first.as_str()), 1, ContextRetention::Required),
        )
        .unwrap();
    assert!(matches!(
        sessions.append_message(
            &session,
            new_message("m_1", None, 1, ContextRetention::Required)
        ),
        Err(CoreError::InvalidSessionSnapshot { .. })
    ));
    assert!(matches!(
        sessions.delete_message(&session, &first),
        Err(CoreError::MessageHasChildren { .. })
    ));
    assert!(matches!(
        sessions.delete_message(&session, &message_id("missing")),
        Err(CoreError::MessageNotFound { .. })
    ));
    sessions.delete_message(&session, &second).unwrap();
    sessions.delete_message(&session, &first).unwrap();
}

#[test]
fn session_branch_path_uses_the_selected_leaf() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    let session = session_id("s_1");
    sessions.create_session(session.clone(), "goal").unwrap();
    let root = sessions
        .append_message(
            &session,
            new_message("root", None, 1, ContextRetention::Required),
        )
        .unwrap();
    let branch_a = sessions
        .append_message(
            &session,
            new_message(
                "branch_a",
                Some(root.as_str()),
                1,
                ContextRetention::Required,
            ),
        )
        .unwrap();
    let branch_b = sessions
        .append_message(
            &session,
            new_message(
                "branch_b",
                Some(root.as_str()),
                1,
                ContextRetention::Required,
            ),
        )
        .unwrap();
    let snapshot = sessions.snapshot(&session).unwrap();
    assert_eq!(snapshot.latest_leaf(), Some(&branch_b));
    assert_eq!(
        snapshot
            .message_path(&branch_a)
            .unwrap()
            .iter()
            .map(|node| node.id().clone())
            .collect::<Vec<_>>(),
        vec![root, branch_a]
    );
    assert!(matches!(
        snapshot.message_path(&message_id("missing")),
        Err(CoreError::MessageNotFound { .. })
    ));
}

#[test]
fn session_input_validation_rejects_bad_goal_clock_and_token_estimate() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(-1));
    let mut sessions = manager(store, clock);
    assert!(matches!(
        sessions.create_session(session_id("s_1"), "goal"),
        Err(CoreError::InvalidContent { .. })
    ));

    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    assert!(matches!(
        sessions.create_session(session_id("s_1"), ""),
        Err(CoreError::InvalidContent { .. })
    ));
    let session = session_id("s_2");
    sessions.create_session(session.clone(), "goal").unwrap();
    assert!(matches!(
        sessions.append_message(
            &session,
            new_message("m_zero", None, 0, ContextRetention::Required)
        ),
        Err(CoreError::InvalidContent { .. })
    ));
}

#[test]
fn session_snapshot_restore_validates_external_input() {
    let base = SessionSnapshotParts {
        id: session_id("s_1"),
        goal: "goal".to_string(),
        status: SessionStatus::Active,
        created_at_unix_ms: 100,
        ended_at_unix_ms: None,
        revision: 0,
        messages: vec![],
    };
    assert!(SessionSnapshot::restore(base.clone()).is_ok());

    let mut bad = base.clone();
    bad.created_at_unix_ms = -1;
    assert!(SessionSnapshot::restore(bad).is_err());

    let mut bad = base.clone();
    bad.status = SessionStatus::Ended;
    assert!(SessionSnapshot::restore(bad).is_err());

    let mut bad = base.clone();
    bad.ended_at_unix_ms = Some(99);
    bad.status = SessionStatus::Ended;
    assert!(SessionSnapshot::restore(bad).is_err());

    let mut bad = base.clone();
    bad.ended_at_unix_ms = Some(101);
    assert!(SessionSnapshot::restore(bad).is_err());

    let node = MessageNode::restore(MessageNodeParts {
        id: message_id("m_1"),
        parent_id: None,
        sequence: 0,
        role: MessageRole::User,
        content: content("hello"),
        token_estimate: TokenCount::new(1),
        retention: ContextRetention::Required,
    })
    .unwrap();
    let mut duplicate = base.clone();
    duplicate.messages = vec![node.clone(), node.clone()];
    assert!(SessionSnapshot::restore(duplicate).is_err());

    let mut missing_parent = base.clone();
    missing_parent.messages = vec![
        MessageNode::restore(MessageNodeParts {
            id: message_id("m_child"),
            parent_id: Some(message_id("missing")),
            sequence: 1,
            role: MessageRole::User,
            content: content("child"),
            token_estimate: TokenCount::new(1),
            retention: ContextRetention::Required,
        })
        .unwrap(),
    ];
    assert!(SessionSnapshot::restore(missing_parent).is_err());

    let mut bad_sequence = base;
    bad_sequence.messages = vec![
        node,
        MessageNode::restore(MessageNodeParts {
            id: message_id("m_2"),
            parent_id: Some(message_id("m_1")),
            sequence: 0,
            role: MessageRole::User,
            content: content("child"),
            token_estimate: TokenCount::new(1),
            retention: ContextRetention::Required,
        })
        .unwrap(),
    ];
    assert!(SessionSnapshot::restore(bad_sequence).is_err());

    assert!(
        MessageNode::restore(MessageNodeParts {
            id: message_id("m_zero"),
            parent_id: None,
            sequence: 0,
            role: MessageRole::User,
            content: content("zero"),
            token_estimate: TokenCount::new(0),
            retention: ContextRetention::Required,
        })
        .is_err()
    );
}

struct FailingUpdateStore {
    inner: MemorySessionStore,
}

impl SessionStore for FailingUpdateStore {
    fn insert_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError> {
        self.inner.insert_session(snapshot)
    }

    fn update_session(&self, _snapshot: &SessionSnapshot) -> Result<(), SessionStoreError> {
        Err(SessionStoreError::new(
            "test_store_write_failed",
            "injected update failure",
        ))
    }

    fn load_session(
        &self,
        session_id: &assistant_core::SessionId,
    ) -> Result<Option<SessionSnapshot>, SessionStoreError> {
        self.inner.load_session(session_id)
    }
}

struct WrongSessionStore {
    inner: MemorySessionStore,
}

impl SessionStore for WrongSessionStore {
    fn insert_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError> {
        self.inner.insert_session(snapshot)
    }

    fn update_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError> {
        self.inner.update_session(snapshot)
    }

    fn load_session(
        &self,
        _session_id: &assistant_core::SessionId,
    ) -> Result<Option<SessionSnapshot>, SessionStoreError> {
        self.inner.load_session(&session_id("wrong"))
    }
}

#[test]
fn failed_store_update_leaves_previous_snapshot_authoritative() {
    let store = Arc::new(FailingUpdateStore {
        inner: MemorySessionStore::new(),
    });
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = SessionManager::new(store, clock);
    let session = session_id("s_1");
    sessions.create_session(session.clone(), "goal").unwrap();
    assert!(matches!(
        sessions.append_message(
            &session,
            new_message("m_1", None, 1, ContextRetention::Required)
        ),
        Err(CoreError::SessionStore(_))
    ));
    assert!(sessions.snapshot(&session).unwrap().messages().is_empty());
}

#[test]
fn restore_rejects_a_store_that_returns_the_wrong_session() {
    let inner = MemorySessionStore::new();
    let store = Arc::new(WrongSessionStore { inner });
    let clock = Arc::new(FixedClock::new(100));
    let seed = Arc::new(MemorySessionStore::new());
    let mut setup = manager(seed, Arc::clone(&clock));
    let wrong = setup.create_session(session_id("wrong"), "goal").unwrap();
    store.inner.insert_session(&wrong).unwrap();

    let mut sessions = SessionManager::new(store, clock);
    assert!(matches!(
        sessions.restore_session(&session_id("requested")),
        Err(CoreError::InvalidSessionSnapshot { .. })
    ));
}

#[test]
fn memory_store_enforces_insert_and_revision_contracts() {
    let store = MemorySessionStore::new();
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(Arc::new(MemorySessionStore::new()), clock);
    let snapshot = sessions.create_session(session_id("s_1"), "goal").unwrap();
    store.insert_session(&snapshot).unwrap();
    assert!(store.insert_session(&snapshot).is_err());
    assert!(store.update_session(&snapshot).is_err());
    assert!(
        store
            .load_session(&session_id("missing"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn end_session_rejects_a_clock_that_moves_backwards() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, Arc::clone(&clock));
    let session = session_id("s_1");
    sessions.create_session(session.clone(), "goal").unwrap();
    clock.set(99);
    match sessions.end_session(&session) {
        Err(CoreError::SessionStore(error)) => {
            assert_eq!(error.reason_code(), "clock_went_backwards");
        }
        other => panic!("expected clock error, got {other:?}"),
    }
}
