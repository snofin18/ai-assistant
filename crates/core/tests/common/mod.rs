//! Shared deterministic fixtures for Core integration tests.

use std::sync::{Arc, Mutex};

use assistant_core::{
    ContextRetention, MemorySessionStore, MessageContent, MessageId, MessageRole, NewMessage,
    SessionClock, SessionId, SessionManager, TokenCount,
};

pub struct FixedClock(Mutex<i64>);

impl FixedClock {
    pub const fn new(now_unix_ms: i64) -> Self {
        Self(Mutex::new(now_unix_ms))
    }

    pub fn set(&self, now_unix_ms: i64) {
        *self.0.lock().unwrap() = now_unix_ms;
    }
}

impl SessionClock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        *self.0.lock().unwrap()
    }
}

pub fn session_id(value: &str) -> SessionId {
    SessionId::new(value).unwrap()
}

pub fn message_id(value: &str) -> MessageId {
    MessageId::new(value).unwrap()
}

pub fn content(value: &str) -> MessageContent {
    MessageContent::new(value).unwrap()
}

pub fn manager(store: Arc<MemorySessionStore>, clock: Arc<FixedClock>) -> SessionManager {
    SessionManager::new(store, clock)
}

pub fn new_message(
    id: &str,
    parent_id: Option<&str>,
    tokens: u64,
    retention: ContextRetention,
) -> NewMessage {
    NewMessage {
        id: message_id(id),
        parent_id: parent_id.map(message_id),
        role: MessageRole::User,
        content: content(id),
        token_estimate: TokenCount::new(tokens),
        retention,
    }
}

pub fn create_linear_session(manager: &mut SessionManager) -> (SessionId, Vec<MessageId>) {
    let session = session_id("s_1");
    manager
        .create_session(session.clone(), "test goal")
        .unwrap();
    let required = manager
        .append_message(
            &session,
            new_message("m_required", None, 5, ContextRetention::Required),
        )
        .unwrap();
    let old = manager
        .append_message(
            &session,
            new_message(
                "m_old",
                Some(required.as_str()),
                4,
                ContextRetention::Summarizable,
            ),
        )
        .unwrap();
    let recent = manager
        .append_message(
            &session,
            new_message(
                "m_recent",
                Some(old.as_str()),
                6,
                ContextRetention::Summarizable,
            ),
        )
        .unwrap();
    (session, vec![required, old, recent])
}
