//! Injected session persistence boundary and deterministic in-memory adapter.

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::error::SessionStoreError;
use crate::identifiers::SessionId;
use crate::message::SessionSnapshot;

/// Persistence boundary consumed by [`crate::SessionManager`].
///
/// Production assembly implements this trait with the public
/// `assistant-storage` record APIs. Core never holds a SQLite connection.
pub trait SessionStore: Send + Sync {
    /// Inserts a new session and fails if the id already exists.
    ///
    /// # Errors
    ///
    /// Returns a store-specific [`SessionStoreError`].
    fn insert_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError>;

    /// Replaces an existing session for exactly the next revision.
    ///
    /// # Errors
    ///
    /// Returns a revision-conflict or store-specific [`SessionStoreError`].
    fn update_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError>;

    /// Loads one session snapshot.
    ///
    /// # Errors
    ///
    /// Returns a store-specific [`SessionStoreError`].
    fn load_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionSnapshot>, SessionStoreError>;
}

/// Deterministic session store for tests, replay, and early assembly.
#[derive(Debug, Default)]
pub struct MemorySessionStore {
    sessions: Mutex<BTreeMap<SessionId, SessionSnapshot>>,
}

impl MemorySessionStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for MemorySessionStore {
    fn insert_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError> {
        let mut sessions = self.sessions.lock().map_err(|_| {
            SessionStoreError::new(
                "session_store_poisoned",
                "in-memory session lock is poisoned",
            )
        })?;
        if sessions.contains_key(snapshot.id()) {
            return Err(SessionStoreError::new(
                "session_already_exists",
                format!("session {} already exists", snapshot.id()),
            ));
        }
        sessions.insert(snapshot.id().clone(), snapshot.clone());
        drop(sessions);
        Ok(())
    }

    fn update_session(&self, snapshot: &SessionSnapshot) -> Result<(), SessionStoreError> {
        let mut sessions = self.sessions.lock().map_err(|_| {
            SessionStoreError::new(
                "session_store_poisoned",
                "in-memory session lock is poisoned",
            )
        })?;
        let existing = sessions.get(snapshot.id()).ok_or_else(|| {
            SessionStoreError::new(
                "session_not_found",
                format!("session {} does not exist", snapshot.id()),
            )
        })?;
        let expected_revision = existing
            .revision()
            .checked_add(1)
            .ok_or_else(|| SessionStoreError::new("revision_overflow", "revision overflow"))?;
        if snapshot.revision() != expected_revision {
            return Err(SessionStoreError::new(
                "revision_conflict",
                format!(
                    "session {} expected revision {expected_revision}, got {}",
                    snapshot.id(),
                    snapshot.revision()
                ),
            ));
        }
        sessions.insert(snapshot.id().clone(), snapshot.clone());
        drop(sessions);
        Ok(())
    }

    fn load_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionSnapshot>, SessionStoreError> {
        let sessions = self.sessions.lock().map_err(|_| {
            SessionStoreError::new(
                "session_store_poisoned",
                "in-memory session lock is poisoned",
            )
        })?;
        Ok(sessions.get(session_id).cloned())
    }
}
