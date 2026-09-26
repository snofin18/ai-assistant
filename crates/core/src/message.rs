//! Session tree records and untrusted-input validation.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{CoreError, CoreResult};
use crate::identifiers::{MessageId, SessionId, TokenCount};

const MAX_TEXT_BYTES: usize = 1_048_576;
const MAX_GOAL_BYTES: usize = 4_096;

fn validate_text(field: &'static str, value: &str, maximum: usize) -> CoreResult<()> {
    if value.is_empty() {
        return Err(CoreError::InvalidContent {
            field,
            reason: "must not be empty".to_string(),
        });
    }
    if value.len() > maximum {
        return Err(CoreError::InvalidContent {
            field,
            reason: format!("must not exceed {maximum} bytes"),
        });
    }
    if value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(CoreError::InvalidContent {
            field,
            reason: "contains unsupported control characters".to_string(),
        });
    }
    Ok(())
}

/// Role of one message in a session tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessageRole {
    /// System instruction.
    System,
    /// User-authored input.
    User,
    /// Prior assistant output.
    Assistant,
    /// Tool result or observation.
    Tool,
}

/// Whether context construction may discard a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContextRetention {
    /// Anchor or evidence required by the task; never trimmed.
    Required,
    /// Older history that may be replaced by a structured summary.
    Summarizable,
    /// Low-value history that may be omitted without compression.
    Droppable,
}

/// Validated text content for a session message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageContent(String);

impl MessageContent {
    /// Creates validated message content.
    ///
    /// Content must be non-empty, at most 1 MiB, and may contain only `\n`,
    /// `\r`, and `\t` as control characters.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidContent`] for invalid text.
    pub fn new(value: impl Into<String>) -> CoreResult<Self> {
        let value = value.into();
        validate_text("message.content", &value, MAX_TEXT_BYTES)?;
        Ok(Self(value))
    }

    /// Returns the text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One validated node in a session message tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageNode {
    id: MessageId,
    parent_id: Option<MessageId>,
    sequence: u32,
    role: MessageRole,
    content: MessageContent,
    token_estimate: TokenCount,
    retention: ContextRetention,
}

impl MessageNode {
    pub(crate) fn new(parts: MessageNodeParts) -> CoreResult<Self> {
        if parts.token_estimate.get() == 0 {
            return Err(CoreError::InvalidContent {
                field: "message.token_estimate",
                reason: "must be positive".to_string(),
            });
        }
        Ok(Self {
            id: parts.id,
            parent_id: parts.parent_id,
            sequence: parts.sequence,
            role: parts.role,
            content: parts.content,
            token_estimate: parts.token_estimate,
            retention: parts.retention,
        })
    }

    /// Restores a node from an external store after validating its token count.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidContent`] when `token_estimate` is zero.
    pub fn restore(parts: MessageNodeParts) -> CoreResult<Self> {
        Self::new(parts)
    }

    /// Returns the stable message id.
    #[must_use]
    pub const fn id(&self) -> &MessageId {
        &self.id
    }

    /// Returns the parent id for a branch node.
    #[must_use]
    pub const fn parent_id(&self) -> Option<&MessageId> {
        self.parent_id.as_ref()
    }

    /// Returns the session-local order key.
    #[must_use]
    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Returns the message role.
    #[must_use]
    pub const fn role(&self) -> MessageRole {
        self.role
    }

    /// Returns validated content.
    #[must_use]
    pub const fn content(&self) -> &MessageContent {
        &self.content
    }

    /// Returns the caller-supplied token estimate.
    #[must_use]
    pub const fn token_estimate(&self) -> TokenCount {
        self.token_estimate
    }

    /// Returns context-retention policy.
    #[must_use]
    pub const fn retention(&self) -> ContextRetention {
        self.retention
    }
}

/// Fields required to restore one [`MessageNode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageNodeParts {
    /// Stable message id.
    pub id: MessageId,
    /// Parent id for a branch node.
    pub parent_id: Option<MessageId>,
    /// Session-local order key.
    pub sequence: u32,
    /// Message role.
    pub role: MessageRole,
    /// Validated text.
    pub content: MessageContent,
    /// Positive token estimate.
    pub token_estimate: TokenCount,
    /// Trimming policy.
    pub retention: ContextRetention,
}

/// Lifecycle state of one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// New messages may be appended.
    Active,
    /// The session is immutable.
    Ended,
}

/// Immutable snapshot of one session tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    id: SessionId,
    goal: String,
    status: SessionStatus,
    created_at_unix_ms: i64,
    ended_at_unix_ms: Option<i64>,
    revision: u64,
    messages: Vec<MessageNode>,
}

impl SessionSnapshot {
    pub(crate) fn new(id: SessionId, goal: String, created_at_unix_ms: i64) -> CoreResult<Self> {
        validate_text("session.goal", &goal, MAX_GOAL_BYTES)?;
        if created_at_unix_ms < 0 {
            return Err(CoreError::InvalidContent {
                field: "session.created_at_unix_ms",
                reason: "must be non-negative".to_string(),
            });
        }
        Ok(Self {
            id,
            goal,
            status: SessionStatus::Active,
            created_at_unix_ms,
            ended_at_unix_ms: None,
            revision: 0,
            messages: Vec::new(),
        })
    }

    /// Restores and validates a snapshot supplied by a session store.
    ///
    /// Validation covers timestamps, unique ids, parent references, sequence
    /// ordering, and branch acyclicity.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidContent`] or
    /// [`CoreError::InvalidSessionSnapshot`] when the snapshot is malformed.
    pub fn restore(parts: SessionSnapshotParts) -> CoreResult<Self> {
        validate_text("session.goal", &parts.goal, MAX_GOAL_BYTES)?;
        if parts.created_at_unix_ms < 0 {
            return Err(CoreError::InvalidContent {
                field: "session.created_at_unix_ms",
                reason: "must be non-negative".to_string(),
            });
        }
        if let Some(ended_at) = parts.ended_at_unix_ms {
            if ended_at < parts.created_at_unix_ms {
                return Err(CoreError::InvalidSessionSnapshot {
                    reason: "ended_at precedes created_at".to_string(),
                });
            }
            if parts.status != SessionStatus::Ended {
                return Err(CoreError::InvalidSessionSnapshot {
                    reason: "ended_at is set while the session is active".to_string(),
                });
            }
        }
        if parts.status == SessionStatus::Ended && parts.ended_at_unix_ms.is_none() {
            return Err(CoreError::InvalidSessionSnapshot {
                reason: "ended session has no ended_at".to_string(),
            });
        }
        validate_message_tree(&parts.messages)?;
        Ok(Self {
            id: parts.id,
            goal: parts.goal,
            status: parts.status,
            created_at_unix_ms: parts.created_at_unix_ms,
            ended_at_unix_ms: parts.ended_at_unix_ms,
            revision: parts.revision,
            messages: parts.messages,
        })
    }

    /// Returns the session id.
    #[must_use]
    pub const fn id(&self) -> &SessionId {
        &self.id
    }

    /// Returns the original user goal.
    #[must_use]
    pub fn goal(&self) -> &str {
        &self.goal
    }

    /// Returns the lifecycle state.
    #[must_use]
    pub const fn status(&self) -> SessionStatus {
        self.status
    }

    /// Returns creation time in Unix milliseconds.
    #[must_use]
    pub const fn created_at_unix_ms(&self) -> i64 {
        self.created_at_unix_ms
    }

    /// Returns end time when the session is ended.
    #[must_use]
    pub const fn ended_at_unix_ms(&self) -> Option<i64> {
        self.ended_at_unix_ms
    }

    /// Returns the monotonic snapshot revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns all message nodes in session-local sequence order.
    #[must_use]
    pub fn messages(&self) -> &[MessageNode] {
        &self.messages
    }

    /// Returns the highest-sequence leaf, or `None` for an empty tree.
    #[must_use]
    pub fn latest_leaf(&self) -> Option<&MessageId> {
        let parents: BTreeSet<&MessageId> = self
            .messages
            .iter()
            .filter_map(MessageNode::parent_id)
            .collect();
        self.messages
            .iter()
            .filter(|node| !parents.contains(node.id()))
            .max_by_key(|node| node.sequence())
            .map(MessageNode::id)
    }

    /// Returns the root-to-leaf path for one selected branch.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::MessageNotFound`] when `leaf_id` is absent.
    pub fn message_path(&self, leaf_id: &MessageId) -> CoreResult<Vec<&MessageNode>> {
        let by_id: BTreeMap<&MessageId, &MessageNode> =
            self.messages.iter().map(|node| (node.id(), node)).collect();
        let mut current =
            by_id
                .get(leaf_id)
                .copied()
                .ok_or_else(|| CoreError::MessageNotFound {
                    session_id: self.id.to_string(),
                    message_id: leaf_id.to_string(),
                })?;
        let mut reversed = Vec::new();
        loop {
            reversed.push(current);
            match current.parent_id() {
                Some(parent_id) => {
                    current = by_id.get(parent_id).copied().ok_or_else(|| {
                        CoreError::InvalidSessionSnapshot {
                            reason: format!("message {} has a missing parent", current.id()),
                        }
                    })?;
                }
                None => break,
            }
        }
        reversed.reverse();
        Ok(reversed)
    }

    pub(crate) fn push_message(&mut self, node: MessageNode) {
        self.messages.push(node);
        self.messages.sort_by_key(MessageNode::sequence);
    }

    pub(crate) fn remove_message(&mut self, message_id: &MessageId) {
        self.messages.retain(|node| node.id() != message_id);
    }

    pub(crate) const fn increment_revision(&mut self) -> CoreResult<()> {
        self.revision = match self.revision.checked_add(1) {
            Some(revision) => revision,
            None => {
                return Err(CoreError::NumericOverflow {
                    field: "session.revision",
                });
            }
        };
        Ok(())
    }

    pub(crate) const fn end(&mut self, ended_at_unix_ms: i64) {
        self.status = SessionStatus::Ended;
        self.ended_at_unix_ms = Some(ended_at_unix_ms);
    }
}

/// Fields required to restore one [`SessionSnapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshotParts {
    /// Session id.
    pub id: SessionId,
    /// Original user goal.
    pub goal: String,
    /// Lifecycle state.
    pub status: SessionStatus,
    /// Creation time in Unix milliseconds.
    pub created_at_unix_ms: i64,
    /// End time for ended sessions.
    pub ended_at_unix_ms: Option<i64>,
    /// Monotonic snapshot revision.
    pub revision: u64,
    /// Message nodes.
    pub messages: Vec<MessageNode>,
}

fn validate_message_tree(messages: &[MessageNode]) -> CoreResult<()> {
    let mut ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    let mut parents = BTreeMap::new();
    for node in messages {
        if !ids.insert(node.id()) {
            return Err(CoreError::InvalidSessionSnapshot {
                reason: format!("duplicate message id {}", node.id()),
            });
        }
        if !sequences.insert(node.sequence()) {
            return Err(CoreError::InvalidSessionSnapshot {
                reason: format!("duplicate message sequence {}", node.sequence()),
            });
        }
        parents.insert(node.id(), node.parent_id());
    }
    for node in messages {
        if let Some(parent_id) = node.parent_id() {
            let parent = messages
                .iter()
                .find(|candidate| candidate.id() == parent_id)
                .ok_or_else(|| CoreError::InvalidSessionSnapshot {
                    reason: format!("message {} has a missing parent", node.id()),
                })?;
            if parent.sequence() >= node.sequence() {
                return Err(CoreError::InvalidSessionSnapshot {
                    reason: format!(
                        "message {} must follow parent {} in sequence",
                        node.id(),
                        parent.id()
                    ),
                });
            }
        }
        let mut seen = BTreeSet::new();
        let mut current = node.parent_id();
        while let Some(parent_id) = current {
            if !seen.insert(parent_id) {
                return Err(CoreError::InvalidSessionSnapshot {
                    reason: format!("cycle involving message {}", node.id()),
                });
            }
            current = parents.get(parent_id).copied().flatten();
        }
    }
    Ok(())
}
