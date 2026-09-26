//! Deterministic context selection, trimming, compression, and budgeting.

use std::collections::BTreeSet;

use assistant_protocol::ErrorCode;

use crate::error::{CompressionError, CoreError, CoreResult};
use crate::identifiers::{MessageId, TokenCount};
use crate::message::{ContextRetention, MessageNode, MessageRole, SessionSnapshot};

/// Input/output budget for one model context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBudget {
    max_total: TokenCount,
    reserved_output: TokenCount,
    available_input: TokenCount,
}

impl ContextBudget {
    /// Creates a budget with a reserved output allowance.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidBudget`] unless `max_total_tokens` is
    /// positive and greater than `reserved_output_tokens`.
    pub fn new(
        max_total_tokens: TokenCount,
        reserved_output_tokens: TokenCount,
    ) -> CoreResult<Self> {
        if max_total_tokens.get() == 0 {
            return Err(CoreError::InvalidBudget {
                field: "max_total_tokens",
                reason: "must be positive".to_string(),
            });
        }
        if reserved_output_tokens >= max_total_tokens {
            return Err(CoreError::InvalidBudget {
                field: "reserved_output_tokens",
                reason: "must be smaller than max_total_tokens".to_string(),
            });
        }
        let available_input_tokens =
            TokenCount::new(max_total_tokens.get() - reserved_output_tokens.get());
        Ok(Self {
            max_total: max_total_tokens,
            reserved_output: reserved_output_tokens,
            available_input: available_input_tokens,
        })
    }

    /// Returns the total request budget.
    #[must_use]
    pub const fn max_total_tokens(&self) -> TokenCount {
        self.max_total
    }

    /// Returns the output allowance.
    #[must_use]
    pub const fn reserved_output_tokens(&self) -> TokenCount {
        self.reserved_output
    }

    /// Returns tokens available for selected input.
    #[must_use]
    pub const fn available_input_tokens(&self) -> TokenCount {
        self.available_input
    }
}

/// One message selected into a model context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFragment {
    message_id: MessageId,
    role: MessageRole,
    content: String,
    token_estimate: TokenCount,
}

impl ContextFragment {
    fn from_node(node: &MessageNode) -> Self {
        Self {
            message_id: node.id().clone(),
            role: node.role(),
            content: node.content().as_str().to_string(),
            token_estimate: node.token_estimate(),
        }
    }

    /// Returns the source message id.
    #[must_use]
    pub const fn message_id(&self) -> &MessageId {
        &self.message_id
    }

    /// Returns the message role.
    #[must_use]
    pub const fn role(&self) -> MessageRole {
        self.role
    }

    /// Returns the selected text.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the source token estimate.
    #[must_use]
    pub const fn token_estimate(&self) -> TokenCount {
        self.token_estimate
    }
}

/// Structured summary returned by an injected compressor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSummary {
    text: String,
    token_estimate: TokenCount,
    source_message_ids: Vec<MessageId>,
}

impl ContextSummary {
    /// Creates a structured history summary.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidContent`] for empty text, zero tokens, or
    /// duplicate source ids.
    pub fn new(
        text: impl Into<String>,
        token_estimate: TokenCount,
        source_message_ids: Vec<MessageId>,
    ) -> CoreResult<Self> {
        let text = text.into();
        if text.is_empty() {
            return Err(CoreError::InvalidContent {
                field: "context_summary.text",
                reason: "must not be empty".to_string(),
            });
        }
        if token_estimate.get() == 0 {
            return Err(CoreError::InvalidContent {
                field: "context_summary.token_estimate",
                reason: "must be positive".to_string(),
            });
        }
        let text = crate::message::MessageContent::new(text)?
            .as_str()
            .to_string();
        let mut seen = BTreeSet::new();
        for message_id in &source_message_ids {
            if !seen.insert(message_id) {
                return Err(CoreError::InvalidContent {
                    field: "context_summary.source_message_ids",
                    reason: format!("duplicate source message id {message_id}"),
                });
            }
        }
        Ok(Self {
            text,
            token_estimate,
            source_message_ids,
        })
    }

    /// Returns summary text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the summary token estimate.
    #[must_use]
    pub const fn token_estimate(&self) -> TokenCount {
        self.token_estimate
    }

    /// Returns source ids represented by this summary.
    #[must_use]
    pub fn source_message_ids(&self) -> &[MessageId] {
        &self.source_message_ids
    }
}

/// Strategy used to summarize older history when it does not fit.
pub trait HistoryCompressor: Send + Sync {
    /// Summarizes all supplied fragments.
    ///
    /// Returning an error is mandatory when summarization is unavailable; the
    /// context manager will not silently drop the supplied messages instead.
    ///
    /// # Errors
    ///
    /// Returns a structured [`CompressionError`].
    fn summarize(&self, fragments: &[ContextFragment]) -> Result<ContextSummary, CompressionError>;
}

/// Why one source message is absent from the selected context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmissionReason {
    /// The node was explicitly marked `Droppable`.
    DroppedByPolicy,
    /// The node was replaced by a structured history summary.
    ReplacedBySummary,
}

/// Auditable record of omitted source content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOmission {
    message_id: MessageId,
    reason: OmissionReason,
    recoverable: bool,
}

impl ContextOmission {
    /// Returns the omitted message id.
    #[must_use]
    pub const fn message_id(&self) -> &MessageId {
        &self.message_id
    }

    /// Returns the explicit omission reason.
    #[must_use]
    pub const fn reason(&self) -> OmissionReason {
        self.reason
    }

    /// Returns whether the source remains recoverable from the session tree.
    #[must_use]
    pub const fn recoverable(&self) -> bool {
        self.recoverable
    }
}

/// One ordered context entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextEntry {
    /// A selected message.
    Message(ContextFragment),
    /// A structured replacement for omitted older messages.
    Summary(ContextSummary),
}

/// Final context projection returned to the model loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextProjection {
    entries: Vec<ContextEntry>,
    omissions: Vec<ContextOmission>,
    used_tokens: TokenCount,
    available_tokens: TokenCount,
}

impl ContextProjection {
    /// Returns selected messages and summaries in chronological order.
    #[must_use]
    pub fn entries(&self) -> &[ContextEntry] {
        &self.entries
    }

    /// Returns every omitted source with an explicit reason.
    #[must_use]
    pub fn omissions(&self) -> &[ContextOmission] {
        &self.omissions
    }

    /// Returns tokens consumed by entries.
    #[must_use]
    pub const fn used_tokens(&self) -> TokenCount {
        self.used_tokens
    }

    /// Returns the input budget.
    #[must_use]
    pub const fn available_tokens(&self) -> TokenCount {
        self.available_tokens
    }
}

/// Stateless context builder.
#[derive(Debug, Default)]
pub struct ContextManager;

impl ContextManager {
    /// Creates a context manager.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Builds a bounded context for one selected tree branch.
    ///
    /// Required messages must fit. Droppable messages are omitted explicitly.
    /// Older summarizable messages are compressed only when needed; compressor
    /// failure or an oversized summary fails the whole operation.
    ///
    /// # Errors
    ///
    /// - missing leaf or branch nodes;
    /// - [`CoreError::RequiredContextExceedsBudget`];
    /// - [`CoreError::CompressionFailed`] or summary validation failure.
    pub fn build_context(
        &self,
        snapshot: &SessionSnapshot,
        leaf_id: &MessageId,
        budget: ContextBudget,
        compressor: &dyn HistoryCompressor,
    ) -> CoreResult<ContextProjection> {
        let path = snapshot.message_path(leaf_id)?;
        let available = budget.available_input_tokens();
        let mut required_tokens = TokenCount::default();
        for node in &path {
            if node.retention() == ContextRetention::Required {
                required_tokens = required_tokens.checked_add(node.token_estimate())?;
            }
        }
        if required_tokens > available {
            return Err(CoreError::RequiredContextExceedsBudget {
                required_tokens: required_tokens.get(),
                available_tokens: available.get(),
            });
        }

        let summarizable = summarizable_candidates(&path);
        let mut included = vec![false; summarizable.len()];
        let mut selected_tokens = required_tokens;
        for index in (0..summarizable.len()).rev() {
            let Some(fragment) = summarizable.get(index) else {
                continue;
            };
            let next_total = selected_tokens.checked_add(fragment.token_estimate())?;
            if next_total <= available {
                if let Some(flag) = included.get_mut(index) {
                    *flag = true;
                }
                selected_tokens = next_total;
            } else {
                break;
            }
        }

        let mut omitted_candidates = Vec::new();
        for (index, fragment) in summarizable.iter().enumerate() {
            if !included.get(index).copied().unwrap_or(false) {
                omitted_candidates.push(fragment.clone());
            }
        }
        let summary = if omitted_candidates.is_empty() {
            None
        } else {
            let summary = compressor.summarize(&omitted_candidates)?;
            validate_summary(&summary, &omitted_candidates)?;
            Some(summary)
        };
        let summary_tokens = summary
            .as_ref()
            .map_or_else(TokenCount::default, ContextSummary::token_estimate);
        let used_tokens = selected_tokens.checked_add(summary_tokens)?;
        if used_tokens > available {
            return Err(CoreError::RequiredContextExceedsBudget {
                required_tokens: used_tokens.get(),
                available_tokens: available.get(),
            });
        }

        assemble_projection(
            &path,
            &summarizable,
            &included,
            summary.as_ref(),
            used_tokens,
            available,
        )
    }
}

fn summarizable_candidates(path: &[&MessageNode]) -> Vec<ContextFragment> {
    path.iter()
        .filter(|node| node.retention() == ContextRetention::Summarizable)
        .map(|node| ContextFragment::from_node(node))
        .collect()
}

fn validate_summary(summary: &ContextSummary, candidates: &[ContextFragment]) -> CoreResult<()> {
    let expected: Vec<&MessageId> = candidates.iter().map(ContextFragment::message_id).collect();
    if summary.source_message_ids.len() != expected.len()
        || !summary
            .source_message_ids
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
    {
        return Err(CompressionError::new(
            "summary_source_mismatch",
            ErrorCode::ModelInvalidOutput,
            "compressor summary does not match the supplied source messages",
        )
        .into());
    }
    Ok(())
}

fn assemble_projection(
    path: &[&MessageNode],
    summarizable: &[ContextFragment],
    included: &[bool],
    summary: Option<&ContextSummary>,
    used_tokens: TokenCount,
    available_tokens: TokenCount,
) -> CoreResult<ContextProjection> {
    let mut entries = Vec::new();
    let mut omissions = Vec::new();
    let mut summarizable_index = 0_usize;
    let mut summary_written = false;
    for node in path {
        match node.retention() {
            ContextRetention::Required => {
                entries.push(ContextEntry::Message(ContextFragment::from_node(node)));
            }
            ContextRetention::Droppable => {
                omissions.push(ContextOmission {
                    message_id: node.id().clone(),
                    reason: OmissionReason::DroppedByPolicy,
                    recoverable: true,
                });
            }
            ContextRetention::Summarizable => {
                if included.get(summarizable_index).copied().unwrap_or(false) {
                    let Some(fragment) = summarizable.get(summarizable_index) else {
                        return Err(CoreError::InvalidSessionSnapshot {
                            reason: "summarizable candidate index mismatch".to_string(),
                        });
                    };
                    entries.push(ContextEntry::Message(fragment.clone()));
                } else {
                    omissions.push(ContextOmission {
                        message_id: node.id().clone(),
                        reason: OmissionReason::ReplacedBySummary,
                        recoverable: true,
                    });
                    if !summary_written && let Some(summary) = summary {
                        entries.push(ContextEntry::Summary(summary.clone()));
                        summary_written = true;
                    }
                }
                summarizable_index = summarizable_index.saturating_add(1);
            }
        }
    }
    Ok(ContextProjection {
        entries,
        omissions,
        used_tokens,
        available_tokens,
    })
}
