//! Provider-neutral request, stream, capability, pricing, and usage types.

use assistant_protocol::ErrorCode;

use crate::error::{ModelGatewayError, ModelResult};
use crate::identity::{CacheKey, CostMicroUsd, DurationMs, ModelId, TokenCount};

/// Role of one message in a completion request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessageRole {
    /// System instructions.
    System,
    /// User-authored content.
    User,
    /// Prior assistant output.
    Assistant,
    /// Tool result supplied back to the model.
    Tool,
}

/// One ordered completion message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Message role.
    pub role: MessageRole,
    /// Text content.
    pub content: String,
}

impl Message {
    /// Creates a message. Call [`CompletionRequest::validate`] before execution.
    #[must_use]
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

/// One normalized tool definition visible to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    /// Stable tool name.
    pub name: String,
    /// Human/model-readable description.
    pub description: String,
}

impl ToolSpec {
    /// Creates a tool definition.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

/// Tool selection strategy normalized across providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolChoice {
    /// The model may answer directly or call a tool.
    Auto,
    /// The model must call at least one tool.
    Required,
    /// The model must call the named tool.
    Specific {
        /// Required tool name.
        name: String,
    },
    /// The model must not call a tool.
    None,
}

/// Requested structured-output form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseFormat {
    /// Ordinary text output.
    Text,
    /// JSON object output without a caller-provided schema.
    JsonObject,
    /// JSON output constrained by a named schema.
    JsonSchema {
        /// Stable schema name.
        name: String,
        /// JSON schema document.
        schema_json: String,
    },
}

/// Stable-prefix hints for provider-side prompt caching.
///
/// `stable_prefix_end_message_index` is inclusive. The prefix must remain byte-for-byte stable
/// across calls that use the same key; the gateway never reorders or rewrites it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheHints {
    /// Caller-supplied stable key, or `None` when caching is disabled.
    pub cache_key: Option<CacheKey>,
    /// Inclusive index of the final stable message.
    pub stable_prefix_end_message_index: Option<u32>,
    /// Provider cache threshold; zero means provider default.
    pub minimum_cacheable_tokens: TokenCount,
}

impl CacheHints {
    /// Creates disabled cache hints.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            cache_key: None,
            stable_prefix_end_message_index: None,
            minimum_cacheable_tokens: TokenCount::new(0),
        }
    }

    /// Creates enabled cache hints for an inclusive stable message prefix.
    #[must_use]
    pub const fn stable_prefix(
        cache_key: CacheKey,
        stable_prefix_end_message_index: u32,
        minimum_cacheable_tokens: TokenCount,
    ) -> Self {
        Self {
            cache_key: Some(cache_key),
            stable_prefix_end_message_index: Some(stable_prefix_end_message_index),
            minimum_cacheable_tokens,
        }
    }

    /// Returns whether these hints request provider-side prompt caching.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.cache_key.is_some()
    }

    fn validate(&self, message_count: usize) -> ModelResult<()> {
        match (
            self.cache_key.is_some(),
            self.stable_prefix_end_message_index,
        ) {
            (false, None) => {
                if self.minimum_cacheable_tokens.get() != 0 {
                    return Err(ModelGatewayError::InvalidRequest {
                        field: "cache_hints.minimum_cacheable_tokens",
                        reason: "must be zero when prompt caching is disabled".to_string(),
                    });
                }
                Ok(())
            }
            (true, Some(end_index)) => {
                if end_index as usize >= message_count {
                    return Err(ModelGatewayError::InvalidRequest {
                        field: "cache_hints.stable_prefix_end_message_index",
                        reason: "must point to an existing message".to_string(),
                    });
                }
                Ok(())
            }
            _ => Err(ModelGatewayError::InvalidRequest {
                field: "cache_hints",
                reason: "cache key and stable prefix index must be set together".to_string(),
            }),
        }
    }
}

/// Request-level cost and duration limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Budget {
    /// Maximum cost accepted for this request.
    pub max_cost: Option<CostMicroUsd>,
    /// Maximum duration accepted for this request.
    pub max_duration: Option<DurationMs>,
}

impl Budget {
    /// Creates an empty budget.
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            max_cost: None,
            max_duration: None,
        }
    }
}

/// A normalized provider-independent completion request.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionRequest {
    /// Ordered conversation and tool-result messages.
    pub messages: Vec<Message>,
    /// Normalized tools exposed to the model.
    pub tools: Vec<ToolSpec>,
    /// Tool selection strategy.
    pub tool_choice: ToolChoice,
    /// Structured-output request.
    pub response_format: ResponseFormat,
    /// Optional sampling temperature in the inclusive range `0.0..=2.0`.
    pub temperature: Option<f64>,
    /// Optional positive output-token cap.
    pub max_output_tokens: Option<TokenCount>,
    /// Request-level limits.
    pub budget: Budget,
    /// Stable prompt-cache prefix hints.
    pub cache_hints: CacheHints,
}

impl CompletionRequest {
    /// Creates a request with text output, no temperature override, and unlimited budget.
    #[must_use]
    pub const fn new(
        messages: Vec<Message>,
        tools: Vec<ToolSpec>,
        tool_choice: ToolChoice,
        cache_hints: CacheHints,
    ) -> Self {
        Self {
            messages,
            tools,
            tool_choice,
            response_format: ResponseFormat::Text,
            temperature: None,
            max_output_tokens: None,
            budget: Budget::unlimited(),
            cache_hints,
        }
    }

    /// Sets the structured-output request.
    #[must_use]
    pub fn with_response_format(mut self, response_format: ResponseFormat) -> Self {
        self.response_format = response_format;
        self
    }

    /// Sets the sampling temperature.
    #[must_use]
    pub const fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Sets the output-token cap.
    #[must_use]
    pub const fn with_max_output_tokens(mut self, max_output_tokens: TokenCount) -> Self {
        self.max_output_tokens = Some(max_output_tokens);
        self
    }

    /// Sets request-level limits.
    #[must_use]
    pub const fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    /// Validates all caller-controlled fields before routing or provider access.
    ///
    /// Returns `InvalidRequest` with the first offending field. Validation has no side effects and
    /// is idempotent.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` with the first offending caller-controlled field.
    pub fn validate(&self) -> ModelResult<()> {
        if self.messages.is_empty() {
            return Err(ModelGatewayError::InvalidRequest {
                field: "messages",
                reason: "must contain at least one message".to_string(),
            });
        }
        for (index, message) in self.messages.iter().enumerate() {
            if message.content.is_empty() {
                return Err(ModelGatewayError::InvalidRequest {
                    field: "messages.content",
                    reason: format!("message {index} must not be empty"),
                });
            }
            if message.content.contains('\0') {
                return Err(ModelGatewayError::InvalidRequest {
                    field: "messages.content",
                    reason: format!("message {index} must not contain NUL"),
                });
            }
        }
        for (index, tool) in self.tools.iter().enumerate() {
            validate_token("tools.name", &tool.name, index)?;
            if tool.description.is_empty() {
                return Err(ModelGatewayError::InvalidRequest {
                    field: "tools.description",
                    reason: format!("tool {index} must have a description"),
                });
            }
        }
        if let ToolChoice::Specific { name } = &self.tool_choice
            && !self.tools.iter().any(|tool| tool.name == *name)
        {
            return Err(ModelGatewayError::InvalidRequest {
                field: "tool_choice.name",
                reason: "must name a supplied tool".to_string(),
            });
        }
        if let Some(temperature) = self.temperature
            && (!temperature.is_finite() || !(0.0..=2.0).contains(&temperature))
        {
            return Err(ModelGatewayError::InvalidRequest {
                field: "temperature",
                reason: "must be finite and within 0.0..=2.0".to_string(),
            });
        }
        if let Some(max_output_tokens) = self.max_output_tokens
            && max_output_tokens.get() == 0
        {
            return Err(ModelGatewayError::InvalidRequest {
                field: "max_output_tokens",
                reason: "must be positive".to_string(),
            });
        }
        if let ResponseFormat::JsonSchema { name, schema_json } = &self.response_format {
            validate_token("response_format.name", name, 0)?;
            if schema_json.is_empty() {
                return Err(ModelGatewayError::InvalidRequest {
                    field: "response_format.schema_json",
                    reason: "must not be empty".to_string(),
                });
            }
        }
        self.cache_hints.validate(self.messages.len())?;
        Ok(())
    }
}

fn validate_token(field: &'static str, value: &str, index: usize) -> ModelResult<()> {
    if value.is_empty() {
        return Err(ModelGatewayError::InvalidRequest {
            field,
            reason: format!("item {index} must not be empty"),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ModelGatewayError::InvalidRequest {
            field,
            reason: format!("item {index} must not contain control characters"),
        });
    }
    Ok(())
}

/// Token usage reported by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    /// Total input tokens, including cached tokens.
    pub input_tokens: TokenCount,
    /// Input tokens served from a provider prompt cache.
    pub cached_input_tokens: TokenCount,
    /// Generated output tokens.
    pub output_tokens: TokenCount,
}

impl Usage {
    /// Creates usage values.
    #[must_use]
    pub const fn new(
        input_tokens: TokenCount,
        cached_input_tokens: TokenCount,
        output_tokens: TokenCount,
    ) -> Self {
        Self {
            input_tokens,
            cached_input_tokens,
            output_tokens,
        }
    }

    /// Validates that cached input does not exceed total input.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when cached input exceeds total input.
    pub fn validate(self) -> ModelResult<()> {
        if self.cached_input_tokens.get() > self.input_tokens.get() {
            return Err(ModelGatewayError::InvalidRequest {
                field: "usage.cached_input_tokens",
                reason: "cached input tokens exceed total input tokens".to_string(),
            });
        }
        Ok(())
    }

    /// Returns whether the provider reported a prompt-cache hit.
    #[must_use]
    pub const fn cache_hit(self) -> bool {
        self.cached_input_tokens.get() > 0
    }

    /// Returns total billed token count.
    ///
    /// # Errors
    ///
    /// Returns `NumericOverflow` when input plus output exceeds `u64::MAX`.
    pub fn total_tokens(self) -> ModelResult<TokenCount> {
        self.input_tokens.checked_add(self.output_tokens)
    }
}

/// One streamed completion event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionEvent {
    /// Non-empty text increment.
    TextDelta(String),
    /// Tool-call identity, name, or argument increment.
    ToolCallDelta(ToolCallDelta),
    /// Terminal usage for the active attempt.
    Usage(Usage),
    /// Successful stream termination.
    Finished(FinishReason),
}

impl CompletionEvent {
    /// Validates provider-controlled event data before it reaches callers.
    ///
    /// # Errors
    ///
    /// Returns `InvalidOutput` when an event violates the normalized stream contract.
    pub fn validate(&self, model_id: &ModelId) -> ModelResult<()> {
        match self {
            Self::TextDelta(text) => {
                if text.is_empty() {
                    return Err(ModelGatewayError::InvalidOutput {
                        model_id: model_id.clone(),
                        reason: "text delta must not be empty".to_string(),
                    });
                }
                if text.contains('\0') {
                    return Err(ModelGatewayError::InvalidOutput {
                        model_id: model_id.clone(),
                        reason: "text delta must not contain NUL".to_string(),
                    });
                }
            }
            Self::ToolCallDelta(tool_call) => tool_call.validate(model_id)?,
            Self::Usage(usage) => {
                usage
                    .validate()
                    .map_err(|_| ModelGatewayError::InvalidOutput {
                        model_id: model_id.clone(),
                        reason: "cached input tokens exceed total input tokens".to_string(),
                    })?;
            }
            Self::Finished(_) => {}
        }
        Ok(())
    }
}

/// One streamed tool-call delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallDelta {
    /// Provider-stable call identifier.
    pub call_id: String,
    /// Optional tool-name fragment.
    pub name_delta: Option<String>,
    /// JSON argument fragment.
    pub arguments_delta: String,
}

impl ToolCallDelta {
    /// Creates a tool-call delta.
    #[must_use]
    pub fn new(
        call_id: impl Into<String>,
        name_delta: Option<String>,
        arguments_delta: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            name_delta,
            arguments_delta: arguments_delta.into(),
        }
    }

    fn validate(&self, model_id: &ModelId) -> ModelResult<()> {
        if self.call_id.is_empty() || self.call_id.contains('\0') {
            return Err(ModelGatewayError::InvalidOutput {
                model_id: model_id.clone(),
                reason: "tool call id must be non-empty and contain no NUL".to_string(),
            });
        }
        if self.name_delta.as_ref().is_some_and(String::is_empty) && self.arguments_delta.is_empty()
        {
            return Err(ModelGatewayError::InvalidOutput {
                model_id: model_id.clone(),
                reason: "tool call delta must contain a name or argument fragment".to_string(),
            });
        }
        Ok(())
    }
}

/// Why a provider stream completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    /// Natural stop token or stop sequence.
    Stop,
    /// Output-token limit reached.
    Length,
    /// Model requested one or more tools.
    ToolCalls,
    /// Structured output completed.
    ContentFilter,
}

/// One completed model attempt record for the current task step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRecord {
    /// Selected model.
    pub model_id: ModelId,
    /// Reported token usage.
    pub usage: Usage,
    /// Computed total cost in integer micro-USD.
    pub cost: CostMicroUsd,
    /// Wall-clock latency from gateway start through the finish event.
    pub latency: DurationMs,
    /// Whether the provider reported a prompt-cache hit.
    pub cache_hit: bool,
}

/// Result of one provider attempt, including attempts that failed and retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    /// The provider stream completed successfully.
    Succeeded,
    /// The provider failed and the gateway retried the same model.
    Retried {
        /// Stable protocol error category.
        error_code: ErrorCode,
    },
    /// The provider failed and the gateway moved to a fallback model.
    FellBack {
        /// Stable protocol error category.
        error_code: ErrorCode,
    },
    /// The provider failed without retry or fallback.
    Failed {
        /// Stable protocol error category.
        error_code: ErrorCode,
    },
}

/// One provider attempt made by the gateway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptRecord {
    /// Model used for this attempt.
    pub model_id: ModelId,
    /// Zero-based retry index (first attempt is zero).
    pub retry_index: u32,
    /// Attempt outcome and routing decision.
    pub outcome: AttemptOutcome,
    /// Attempt latency.
    pub latency: DurationMs,
}
