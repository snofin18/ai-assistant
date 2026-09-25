//! Provider and pull-based stream contracts.

use crate::cancellation::CancellationToken;
use crate::error::ModelResult;
use crate::identity::{DurationMs, ModelId, TokenCount};
use crate::model::{CompletionEvent, CompletionRequest, Message};
use crate::pricing::Pricing;

/// Bit set for provider capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProviderFeatures(u8);

impl ProviderFeatures {
    /// No optional features.
    pub const NONE: Self = Self(0);
    /// Streaming events.
    pub const STREAMING: Self = Self(1 << 0);
    /// Normalized tool calls.
    pub const TOOLS: Self = Self(1 << 1);
    /// Image content.
    pub const VISION: Self = Self(1 << 2);
    /// Provider-side prompt caching.
    pub const PROMPT_CACHE: Self = Self(1 << 3);

    /// Returns the union of two feature sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether every feature in `other` is present.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

/// Capabilities advertised by one provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    features: ProviderFeatures,
    max_context_tokens: TokenCount,
}

impl ProviderCapabilities {
    /// Creates a capability set.
    #[must_use]
    pub const fn new(features: ProviderFeatures, max_context_tokens: TokenCount) -> Self {
        Self {
            features,
            max_context_tokens,
        }
    }

    /// Returns whether streaming events are supported.
    #[must_use]
    pub const fn supports_streaming(self) -> bool {
        self.features.contains(ProviderFeatures::STREAMING)
    }

    /// Returns whether normalized tool calls are supported.
    #[must_use]
    pub const fn supports_tools(self) -> bool {
        self.features.contains(ProviderFeatures::TOOLS)
    }

    /// Returns whether image content is supported.
    #[must_use]
    pub const fn supports_vision(self) -> bool {
        self.features.contains(ProviderFeatures::VISION)
    }

    /// Returns whether provider-side prompt caching is supported.
    #[must_use]
    pub const fn supports_prompt_cache(self) -> bool {
        self.features.contains(ProviderFeatures::PROMPT_CACHE)
    }

    /// Returns the maximum accepted context window.
    #[must_use]
    pub const fn max_context_tokens(self) -> TokenCount {
        self.max_context_tokens
    }
}

/// Provider adapter behind the model gateway.
///
/// Implementations own all vendor-specific I/O and normalization. `complete` must return promptly
/// after starting the request; subsequent work is pulled through [`CompletionStream`]. Every
/// implementation must check cancellation before opening I/O and before each event poll. It must
/// honor `timeout` for each `next_event` call and return [`ModelGatewayError::Timeout`] rather than
/// blocking indefinitely.
///
/// [`ModelGatewayError::Timeout`]: crate::ModelGatewayError::Timeout
pub trait ModelProvider: Send + Sync {
    /// Returns the registered model identifier.
    fn model_id(&self) -> &ModelId;

    /// Returns provider capabilities used by routing and capability checks.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Returns pricing used for integer cost accounting.
    fn pricing(&self) -> Pricing;

    /// Counts input tokens without network I/O.
    ///
    /// This may be an estimate, but the implementation must be deterministic for identical input
    /// and must return an error rather than a fabricated zero for unsupported tokenizers.
    ///
    /// # Errors
    ///
    /// Returns a provider-defined validation or availability error when counting is impossible.
    fn count_tokens(&self, messages: &[Message]) -> ModelResult<TokenCount>;

    /// Starts one streaming completion.
    ///
    /// Returns `Cancelled` when cancellation is already requested. The returned stream owns all
    /// resources needed for subsequent polls and must not require mutable access to the provider.
    ///
    /// # Errors
    ///
    /// Returns `Cancelled` when cancellation is already requested, or a retryable provider failure
    /// when the request cannot be started.
    fn complete(
        &self,
        request: CompletionRequest,
        cancellation: CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>>;
}

/// Pull-based completion stream owned by the gateway.
///
/// `next_event` returns `Ok(None)` only after a successful finish event has already been emitted.
/// Returning `None` earlier is treated as `IncompleteResponse` by the gateway. Once a text or
/// tool-call delta is observed, the gateway will not retry or fall back after an error.
pub trait CompletionStream: Send {
    /// Returns the next provider event.
    ///
    /// `timeout` bounds this poll. Implementations must return `Timeout` when no event arrives in
    /// time and must check `cancellation` before performing work.
    ///
    /// # Errors
    ///
    /// Returns `Cancelled`, `Timeout`, a retryable network/server error, or `InvalidOutput`.
    fn next_event(
        &mut self,
        cancellation: &CancellationToken,
        timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>>;
}
