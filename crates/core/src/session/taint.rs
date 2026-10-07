//! Runtime taint state for one cached session.
//!
//! Responsibility: apply the session-level propagation rule
//! `Tool -> tainted`, `User -> clean`, and derive the initial state from a
//! restored message tree.
//!
//! Boundary: this module stores no messages and performs no persistence. The
//! durable source remains `SessionSnapshot`; a restored manager recomputes the
//! conservative security state from the role sequence.

use crate::{MessageNode, MessageRole};

/// Runtime-only taint flag for one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct TaintState {
    tainted: bool,
}

impl TaintState {
    /// A clean state for a new session.
    #[must_use]
    pub(super) const fn clean() -> Self {
        Self { tainted: false }
    }

    /// Whether untrusted content currently taints the session.
    #[must_use]
    pub(super) const fn is_tainted(self) -> bool {
        self.tainted
    }

    /// Applies one message role to the state.
    #[must_use]
    pub(super) const fn observe(self, role: MessageRole) -> Self {
        match role {
            MessageRole::Tool => Self { tainted: true },
            MessageRole::User => Self { tainted: false },
            MessageRole::System | MessageRole::Assistant => self,
        }
    }

    /// Recomputes state from an ordered message tree.
    #[must_use]
    pub(super) fn from_messages(messages: &[MessageNode]) -> Self {
        messages
            .iter()
            .fold(Self::clean(), |state, node| state.observe(node.role()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::TaintState;
    use crate::{
        ContextRetention, MessageContent, MessageId, MessageNode, MessageNodeParts, MessageRole,
        TokenCount,
    };

    fn node(sequence: u32, role: MessageRole) -> MessageNode {
        MessageNode::restore(MessageNodeParts {
            id: MessageId::new(format!("m_{sequence}")).expect("message id"),
            parent_id: None,
            sequence,
            role,
            content: MessageContent::new(format!("message {sequence}")).expect("content"),
            token_estimate: TokenCount::new(1),
            retention: ContextRetention::Required,
        })
        .expect("message node")
    }

    #[test]
    fn test_tool_taints_and_user_clears() {
        let tainted = TaintState::clean().observe(MessageRole::Tool);
        assert!(tainted.is_tainted());
        assert!(tainted.observe(MessageRole::Assistant).is_tainted());
        assert!(!tainted.observe(MessageRole::User).is_tainted());
    }

    #[test]
    fn test_recompute_uses_last_relevant_role() {
        let messages = vec![
            node(0, MessageRole::User),
            node(1, MessageRole::Tool),
            node(2, MessageRole::Assistant),
        ];
        assert!(TaintState::from_messages(&messages).is_tainted());
        let messages = vec![
            node(0, MessageRole::Tool),
            node(1, MessageRole::User),
            node(2, MessageRole::Assistant),
        ];
        assert!(!TaintState::from_messages(&messages).is_tainted());
    }
}
