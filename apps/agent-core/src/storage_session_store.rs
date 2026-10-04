//! Storage-backed `SessionStore` adapter for the production assembly (PL-108).
//!
//! Responsibilities:
//! - translate Core session snapshots into the public `assistant-storage`
//!   conversation / message records, and back;
//! - keep every SQLite call behind the assembly-owned [`DatabaseHandle`].
//!
//! Boundaries:
//! - no cache, no policy, no orchestration; `crates/core` never sees a connection;
//! - no schema changes and no SQL here: it only calls the record APIs TASK-230
//!   shipped (`insert_conversation_snapshot` / `load_conversation_snapshot` /
//!   `replace_conversation_snapshot`).
//!
//! Invariants:
//! 1. one snapshot is written atomically together with its whole message tree;
//! 2. an update must be exactly the next revision (the storage layer re-checks);
//! 3. a snapshot that cannot be represented, or a row that cannot be mapped back
//!    (unknown role / retention / status, overflow, mismatched conversation id),
//!    fails closed with a stable reason code instead of guessing.
//!
//! Related: `crates/core/src/store.rs`, `crates/storage/src/conversations.rs`,
//! `crates/storage/src/conversation_messages.rs`, `docs/PARKING_LOT.md` PL-108.

use assistant_core::{
    ContextRetention, MessageContent, MessageId, MessageNode, MessageNodeParts, MessageRole,
    SessionId, SessionSnapshot, SessionSnapshotParts, SessionStatus, SessionStore,
    SessionStoreError, TokenCount,
};
use assistant_storage::{
    ConversationMessageRecord, ConversationRecord, StorageError, insert_conversation_snapshot,
    load_conversation, load_conversation_snapshot, replace_conversation_snapshot,
};

use crate::adapters::DatabaseHandle;

/// Persisted status string for an active session (shared with `assistant-storage`).
const STATUS_ACTIVE: &str = "active";
/// Persisted status string for an ended session.
const STATUS_ENDED: &str = "ended";
/// Persisted role strings.
const ROLE_SYSTEM: &str = "system";
const ROLE_USER: &str = "user";
const ROLE_ASSISTANT: &str = "assistant";
const ROLE_TOOL: &str = "tool";
/// Persisted retention strings.
const RETENTION_REQUIRED: &str = "required";
const RETENTION_SUMMARIZABLE: &str = "summarizable";
const RETENTION_DROPPABLE: &str = "droppable";

/// `SessionStore` backed by the assembly-owned SQLite database.
///
/// The host injects this instead of `MemorySessionStore`, so a session really
/// survives a process restart (`PL-108`). Every method takes the shared database
/// mutex for the duration of one record call and holds no state of its own.
pub struct StorageSessionStore {
    database: DatabaseHandle,
}

impl StorageSessionStore {
    /// Wraps the assembly-owned database handle.
    #[must_use]
    pub const fn new(database: DatabaseHandle) -> Self {
        Self { database }
    }

    /// Runs one action against the assembly-owned database, holding its mutex only
    /// for the duration of that action (nothing is cached across calls).
    fn with_database<T>(
        &self,
        action: impl FnOnce(&assistant_storage::Database) -> Result<T, SessionStoreError>,
    ) -> Result<T, SessionStoreError> {
        let database = self.database.lock().map_err(|_| poisoned())?;
        action(&database)
    }
}

impl SessionStore for StorageSessionStore {
    fn insert_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError> {
        let conversation = conversation_from_snapshot(snapshot)?;
        let messages = messages_from_snapshot(snapshot)?;
        self.with_database(|database| {
            let connection = database.connection();
            let existing = load_conversation(connection, &conversation.id)
                .map_err(|error| storage_failed(&error))?;
            if existing.is_some() {
                return Err(SessionStoreError::new(
                    "session_already_exists",
                    format!("session {} already exists", conversation.id),
                ));
            }
            insert_conversation_snapshot(connection, &conversation, &messages)
                .map_err(|error| storage_failed(&error))
        })
    }

    fn update_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError> {
        let conversation = conversation_from_snapshot(snapshot)?;
        let messages = messages_from_snapshot(snapshot)?;
        self.with_database(|database| {
            let connection = database.connection();
            let existing = load_conversation(connection, &conversation.id)
                .map_err(|error| storage_failed(&error))?;
            let Some(existing) = existing else {
                return Err(SessionStoreError::new(
                    "session_not_found",
                    format!("session {} does not exist", conversation.id),
                ));
            };
            let expected_revision = existing
                .revision
                .checked_add(1)
                .ok_or_else(|| revision_conflict(&conversation, existing.revision))?;
            if conversation.revision != expected_revision {
                return Err(revision_conflict(&conversation, existing.revision));
            }
            replace_conversation_snapshot(connection, &conversation, &messages)
                .map_err(|error| storage_failed(&error))
        })
    }

    fn load_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionSnapshot>, SessionStoreError> {
        self.with_database(|database| {
            let Some((conversation, messages)) =
                load_conversation_snapshot(database.connection(), session_id.as_str())
                    .map_err(|error| storage_failed(&error))?
            else {
                return Ok(None);
            };
            snapshot_from_records(conversation, messages).map(Some)
        })
    }
}

/// Maps one Core snapshot onto a conversation row.
fn conversation_from_snapshot(
    snapshot: &SessionSnapshot,
) -> Result<ConversationRecord, SessionStoreError> {
    let revision = i64::try_from(snapshot.revision()).map_err(|_| {
        not_persistable("session.revision does not fit the persisted i64 revision column")
    })?;
    let status = match snapshot.status() {
        SessionStatus::Active => STATUS_ACTIVE,
        SessionStatus::Ended => STATUS_ENDED,
    };
    Ok(ConversationRecord {
        id: snapshot.id().as_str().to_owned(),
        goal: snapshot.goal().to_owned(),
        title: None,
        model_config_id: None,
        status: status.to_owned(),
        created_at_unix_ms: snapshot.created_at_unix_ms(),
        ended_at_unix_ms: snapshot.ended_at_unix_ms(),
        revision,
        archived: false,
    })
}

/// Maps the whole message tree onto message rows in sequence order.
fn messages_from_snapshot(
    snapshot: &SessionSnapshot,
) -> Result<Vec<ConversationMessageRecord>, SessionStoreError> {
    let mut records = Vec::with_capacity(snapshot.messages().len());
    for node in snapshot.messages() {
        let sequence = i64::from(node.sequence());
        let token_estimate = i64::try_from(node.token_estimate().get()).map_err(|_| {
            not_persistable("message.token_estimate does not fit the persisted i64 column")
        })?;
        records.push(ConversationMessageRecord {
            id: node.id().as_str().to_owned(),
            conversation_id: snapshot.id().as_str().to_owned(),
            parent_id: node.parent_id().map(|parent| parent.as_str().to_owned()),
            sequence,
            role: role_name(node.role()).to_owned(),
            content: node.content().as_str().to_owned(),
            token_estimate,
            retention: retention_name(node.retention()).to_owned(),
        });
    }
    Ok(records)
}

/// Rebuilds a Core snapshot from persisted rows, re-validating the whole tree.
fn snapshot_from_records(
    conversation: ConversationRecord,
    messages: Vec<ConversationMessageRecord>,
) -> Result<SessionSnapshot, SessionStoreError> {
    let status = match conversation.status.as_str() {
        STATUS_ACTIVE => SessionStatus::Active,
        STATUS_ENDED => SessionStatus::Ended,
        other => {
            return Err(row_invalid(format!(
                "session {} has unknown persisted status `{other}`",
                conversation.id
            )));
        }
    };
    let revision = u64::try_from(conversation.revision).map_err(|_| {
        row_invalid(format!(
            "session {} has a negative persisted revision {}",
            conversation.id, conversation.revision
        ))
    })?;
    let mut nodes = Vec::with_capacity(messages.len());
    for message in messages {
        nodes.push(node_from_record(&conversation.id, message)?);
    }
    let id = SessionId::new(conversation.id.clone())
        .map_err(|error| row_invalid(format!("session id `{}`: {error}", conversation.id)))?;
    SessionSnapshot::restore(SessionSnapshotParts {
        id,
        goal: conversation.goal,
        status,
        created_at_unix_ms: conversation.created_at_unix_ms,
        ended_at_unix_ms: conversation.ended_at_unix_ms,
        revision,
        messages: nodes,
    })
    .map_err(|error| row_invalid(format!("session {}: {error}", conversation.id)))
}

/// Rebuilds one message node from its persisted row.
fn node_from_record(
    conversation_id: &str,
    message: ConversationMessageRecord,
) -> Result<MessageNode, SessionStoreError> {
    if message.conversation_id != conversation_id {
        return Err(row_invalid(format!(
            "message {} belongs to session {}, not {}",
            message.id, message.conversation_id, conversation_id
        )));
    }
    let role = role_from_name(&message.role).ok_or_else(|| {
        row_invalid(format!(
            "message {} has unknown persisted role `{}`",
            message.id, message.role
        ))
    })?;
    let retention = retention_from_name(&message.retention).ok_or_else(|| {
        row_invalid(format!(
            "message {} has unknown persisted retention `{}`",
            message.id, message.retention
        ))
    })?;
    let sequence = u32::try_from(message.sequence).map_err(|_| {
        row_invalid(format!(
            "message {} has an out-of-range persisted sequence {}",
            message.id, message.sequence
        ))
    })?;
    let token_estimate = u64::try_from(message.token_estimate).map_err(|_| {
        row_invalid(format!(
            "message {} has a non-positive persisted token estimate {}",
            message.id, message.token_estimate
        ))
    })?;
    let id = MessageId::new(message.id.clone())
        .map_err(|error| row_invalid(format!("message id `{}`: {error}", message.id)))?;
    let parent_id = match message.parent_id {
        Some(parent) => Some(
            MessageId::new(parent.clone())
                .map_err(|error| row_invalid(format!("message parent `{parent}`: {error}")))?,
        ),
        None => None,
    };
    let content = MessageContent::new(message.content)
        .map_err(|error| row_invalid(format!("message {} content: {error}", message.id)))?;
    MessageNode::restore(MessageNodeParts {
        id,
        parent_id,
        sequence,
        role,
        content,
        token_estimate: TokenCount::new(token_estimate),
        retention,
    })
    .map_err(|error| row_invalid(format!("message {}: {error}", message.id)))
}

/// Canonical persisted name for one role.
const fn role_name(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => ROLE_SYSTEM,
        MessageRole::User => ROLE_USER,
        MessageRole::Assistant => ROLE_ASSISTANT,
        MessageRole::Tool => ROLE_TOOL,
    }
}

/// Parses one persisted role name; unknown names are rejected by the caller.
fn role_from_name(name: &str) -> Option<MessageRole> {
    match name {
        ROLE_SYSTEM => Some(MessageRole::System),
        ROLE_USER => Some(MessageRole::User),
        ROLE_ASSISTANT => Some(MessageRole::Assistant),
        ROLE_TOOL => Some(MessageRole::Tool),
        _ => None,
    }
}

/// Canonical persisted name for one retention policy.
const fn retention_name(retention: ContextRetention) -> &'static str {
    match retention {
        ContextRetention::Required => RETENTION_REQUIRED,
        ContextRetention::Summarizable => RETENTION_SUMMARIZABLE,
        ContextRetention::Droppable => RETENTION_DROPPABLE,
    }
}

/// Parses one persisted retention name; unknown names are rejected by the caller.
fn retention_from_name(name: &str) -> Option<ContextRetention> {
    match name {
        RETENTION_REQUIRED => Some(ContextRetention::Required),
        RETENTION_SUMMARIZABLE => Some(ContextRetention::Summarizable),
        RETENTION_DROPPABLE => Some(ContextRetention::Droppable),
        _ => None,
    }
}

/// The shared database mutex is poisoned.
fn poisoned() -> SessionStoreError {
    SessionStoreError::new(
        "session_store_unavailable",
        "assembly-owned database mutex is poisoned",
    )
}

/// A Core snapshot cannot be represented in the persisted columns.
fn not_persistable(detail: impl Into<String>) -> SessionStoreError {
    SessionStoreError::new("snapshot_not_persistable", detail)
}

/// A persisted row cannot be mapped back into a Core snapshot.
fn row_invalid(detail: impl Into<String>) -> SessionStoreError {
    SessionStoreError::new("persisted_row_invalid", detail)
}

/// The requested update is not exactly the next revision.
fn revision_conflict(conversation: &ConversationRecord, existing: i64) -> SessionStoreError {
    SessionStoreError::new(
        "revision_conflict",
        format!(
            "session {} expected revision {}, got {}",
            conversation.id,
            existing.saturating_add(1),
            conversation.revision
        ),
    )
}

/// One stored record call failed at the SQLite/validation layer.
fn storage_failed(error: &StorageError) -> SessionStoreError {
    SessionStoreError::new("session_store_failure", error.to_string())
}
