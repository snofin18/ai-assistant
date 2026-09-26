//! Session lifecycle and message-tree mutations.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::ContextRetention;
use crate::error::{CoreError, CoreResult, SessionStoreError};
use crate::identifiers::{MessageId, SessionId, TokenCount};
use crate::message::{
    MessageContent, MessageNode, MessageNodeParts, MessageRole, SessionSnapshot, SessionStatus,
};
use crate::store::SessionStore;

/// Injected wall-clock source for session timestamps.
pub trait SessionClock: Send + Sync {
    /// Returns Unix time in milliseconds.
    fn now_unix_ms(&self) -> i64;
}

/// One requested message-tree addition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMessage {
    /// Caller-generated stable id.
    pub id: MessageId,
    /// Parent node, or `None` for a new root branch.
    pub parent_id: Option<MessageId>,
    /// Message role.
    pub role: MessageRole,
    /// Validated content.
    pub content: MessageContent,
    /// Positive token estimate produced by the caller's tokenizer.
    pub token_estimate: TokenCount,
    /// Context-retention policy.
    pub retention: ContextRetention,
}

/// Session aggregate with injected persistence and clock.
pub struct SessionManager {
    store: Arc<dyn SessionStore>,
    clock: Arc<dyn SessionClock>,
    sessions: BTreeMap<SessionId, SessionSnapshot>,
}

impl SessionManager {
    /// Creates a session manager.
    ///
    /// Construction has no side effects. No global singleton or system clock is
    /// used; the assembly point supplies both dependencies.
    #[must_use]
    pub fn new(store: Arc<dyn SessionStore>, clock: Arc<dyn SessionClock>) -> Self {
        Self {
            store,
            clock,
            sessions: BTreeMap::new(),
        }
    }

    /// Creates, persists, and caches a new active session.
    ///
    /// # Errors
    ///
    /// - [`CoreError::InvalidContent`] for an invalid goal or negative clock.
    /// - [`CoreError::SessionAlreadyExists`] when the id is already cached.
    /// - [`CoreError::SessionStore`] when persistence fails.
    pub fn create_session(
        &mut self,
        session_id: SessionId,
        goal: impl Into<String>,
    ) -> CoreResult<SessionSnapshot> {
        if self.sessions.contains_key(&session_id) {
            return Err(CoreError::SessionAlreadyExists {
                session_id: session_id.to_string(),
            });
        }
        let snapshot =
            SessionSnapshot::new(session_id.clone(), goal.into(), self.clock.now_unix_ms())?;
        self.store.insert_session(&snapshot)?;
        self.sessions.insert(session_id, snapshot.clone());
        Ok(snapshot)
    }

    /// Loads a session from cache or the injected store.
    ///
    /// Restoration is idempotent and returns an immutable snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::SessionNotFound`] when the store has no session and
    /// [`CoreError::SessionStore`] when the store fails.
    pub fn restore_session(&mut self, session_id: &SessionId) -> CoreResult<SessionSnapshot> {
        if let Some(snapshot) = self.sessions.get(session_id) {
            return Ok(snapshot.clone());
        }
        let snapshot =
            self.store
                .load_session(session_id)?
                .ok_or_else(|| CoreError::SessionNotFound {
                    session_id: session_id.to_string(),
                })?;
        if snapshot.id() != session_id {
            return Err(CoreError::InvalidSessionSnapshot {
                reason: format!(
                    "store returned session {} for requested session {}",
                    snapshot.id(),
                    session_id
                ),
            });
        }
        self.sessions.insert(session_id.clone(), snapshot.clone());
        Ok(snapshot)
    }

    /// Returns a cached snapshot without I/O.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::SessionNotFound`] when the session is not cached.
    pub fn snapshot(&self, session_id: &SessionId) -> CoreResult<&SessionSnapshot> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| CoreError::SessionNotFound {
                session_id: session_id.to_string(),
            })
    }

    /// Appends one message to an active session.
    ///
    /// Validation and persistence complete before the in-memory snapshot is
    /// replaced, so a failed store write leaves the previous state authoritative.
    ///
    /// # Errors
    ///
    /// Returns session-not-found, ended-session, parent-not-found, content, or
    /// store errors. The operation is not idempotent: a message id may be used
    /// only once.
    pub fn append_message(
        &mut self,
        session_id: &SessionId,
        message: NewMessage,
    ) -> CoreResult<MessageId> {
        let mut next =
            self.sessions
                .get(session_id)
                .cloned()
                .ok_or_else(|| CoreError::SessionNotFound {
                    session_id: session_id.to_string(),
                })?;
        ensure_active(&next)?;
        if next.messages().iter().any(|node| node.id() == &message.id) {
            return Err(CoreError::InvalidSessionSnapshot {
                reason: format!("duplicate message id {}", message.id),
            });
        }
        if let Some(parent_id) = &message.parent_id
            && !next.messages().iter().any(|node| node.id() == parent_id)
        {
            return Err(CoreError::ParentMessageNotFound {
                session_id: session_id.to_string(),
                parent_message_id: parent_id.to_string(),
            });
        }
        let sequence = next
            .messages()
            .iter()
            .map(MessageNode::sequence)
            .max()
            .map_or(Some(0), |sequence| sequence.checked_add(1))
            .ok_or(CoreError::NumericOverflow {
                field: "message.sequence",
            })?;
        let node = MessageNode::new(MessageNodeParts {
            id: message.id.clone(),
            parent_id: message.parent_id,
            sequence,
            role: message.role,
            content: message.content,
            token_estimate: message.token_estimate,
            retention: message.retention,
        })?;
        next.push_message(node);
        next.increment_revision()?;
        self.store.update_session(&next)?;
        self.sessions.insert(session_id.clone(), next);
        Ok(message.id)
    }

    /// Deletes a leaf message from an active session.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::MessageHasChildren`] when deletion would orphan a
    /// branch, plus not-found, ended-session, or store errors.
    pub fn delete_message(
        &mut self,
        session_id: &SessionId,
        message_id: &MessageId,
    ) -> CoreResult<()> {
        let mut next =
            self.sessions
                .get(session_id)
                .cloned()
                .ok_or_else(|| CoreError::SessionNotFound {
                    session_id: session_id.to_string(),
                })?;
        ensure_active(&next)?;
        if !next.messages().iter().any(|node| node.id() == message_id) {
            return Err(CoreError::MessageNotFound {
                session_id: session_id.to_string(),
                message_id: message_id.to_string(),
            });
        }
        if next
            .messages()
            .iter()
            .any(|node| node.parent_id() == Some(message_id))
        {
            return Err(CoreError::MessageHasChildren {
                message_id: message_id.to_string(),
            });
        }
        next.remove_message(message_id);
        next.increment_revision()?;
        self.store.update_session(&next)?;
        self.sessions.insert(session_id.clone(), next);
        Ok(())
    }

    /// Ends an active session and persists the terminal state.
    ///
    /// Ending an already-ended session returns [`CoreError::SessionEnded`].
    ///
    /// # Errors
    ///
    /// Returns session-not-found, ended-session, invalid-clock, or store errors.
    pub fn end_session(&mut self, session_id: &SessionId) -> CoreResult<()> {
        let mut next =
            self.sessions
                .get(session_id)
                .cloned()
                .ok_or_else(|| CoreError::SessionNotFound {
                    session_id: session_id.to_string(),
                })?;
        ensure_active(&next)?;
        let ended_at = self.clock.now_unix_ms();
        if ended_at < next.created_at_unix_ms() {
            return Err(CoreError::SessionStore(SessionStoreError::new(
                "clock_went_backwards",
                "session end time precedes creation time",
            )));
        }
        next.end(ended_at);
        next.increment_revision()?;
        self.store.update_session(&next)?;
        self.sessions.insert(session_id.clone(), next);
        Ok(())
    }
}

fn ensure_active(snapshot: &SessionSnapshot) -> CoreResult<()> {
    if snapshot.status() == SessionStatus::Ended {
        return Err(CoreError::SessionEnded {
            session_id: snapshot.id().to_string(),
        });
    }
    Ok(())
}
