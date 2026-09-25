//! Provider-neutral model gateway for architecture v2 section 11.
//!
//! Responsibilities:
//! - define the streaming, cancellation, token-counting, capability, and pricing provider contract;
//! - route each request from typed rules over stage, context size, sensitivity, budget, and tools;
//! - execute retries with injected exponential jitter and move through a configured fallback chain;
//! - forward stable prompt-cache hints without rewriting the request prefix;
//! - record per-step model, token, cache, latency, and integer USD cost data.
//!
//! Boundaries:
//! - no network, credentials, vendor SDK, or concrete provider implementation;
//! - no policy permission decision. This crate routes and executes only after the caller has policy;
//! - no persistence, audit sink, UI, context trimming, or tool-protocol normalization;
//! - no background runtime. Providers expose a pull-based stream and receive a cancellation token.
//!
//! Invariants:
//! 1. Invalid requests and malformed provider events fail with a typed error before success.
//! 2. Retry and fallback happen only before the first non-empty text or tool-call delta.
//! 3. Cancellation is never retried and is checked before every provider poll.
//! 4. A completed response must contain exactly one usage event followed by a finish event.
//! 5. Cost values use integer micro-USD and round upward component-by-component.
//! 6. Provider selection is deterministic: route rules are evaluated in priority/declaration order.
//!
//! Typical use:
//! ```rust
//! use std::sync::Arc;
//! use assistant_model_gateway::{
//!     CacheHints, CompletionEvent, CompletionRequest, CompletionStream, DurationMs,
//!     ModelGateway, ModelGatewayError, ModelId, ModelProvider, ModelResult, ModelRouter,
//!     Message, MessageRole, ProviderCapabilities, ProviderFeatures, Pricing, RetryPolicy,
//!     RouteContext, Sensitivity, TaskStage, TokenCount, ToolChoice, Usage,
//! };
//!
//! struct EchoProvider {
//!     id: ModelId,
//!     pricing: Pricing,
//! }
//!
//! struct EchoStream {
//!     emitted: bool,
//! }
//!
//! impl CompletionStream for EchoStream {
//!     fn next_event(
//!         &mut self,
//!         _cancellation: &assistant_model_gateway::CancellationToken,
//!         _timeout: DurationMs,
//!     ) -> ModelResult<Option<CompletionEvent>> {
//!         if self.emitted {
//!             return Ok(None);
//!         }
//!         self.emitted = true;
//!         Ok(Some(CompletionEvent::Usage(Usage::new(
//!             TokenCount::new(1),
//!             TokenCount::new(0),
//!             TokenCount::new(1),
//!         ))))
//!     }
//! }
//!
//! impl ModelProvider for EchoProvider {
//!     fn model_id(&self) -> &ModelId { &self.id }
//!     fn capabilities(&self) -> ProviderCapabilities {
//!         ProviderCapabilities::new(
//!             ProviderFeatures::STREAMING.union(ProviderFeatures::TOOLS),
//!             TokenCount::new(8_192),
//!         )
//!     }
//!     fn pricing(&self) -> Pricing { self.pricing }
//!     fn count_tokens(&self, messages: &[Message]) -> ModelResult<TokenCount> {
//!         Ok(TokenCount::new(messages.len() as u64))
//!     }
//!     fn complete(
//!         &self,
//!         _request: CompletionRequest,
//!         _cancellation: assistant_model_gateway::CancellationToken,
//!     ) -> ModelResult<Box<dyn CompletionStream>> {
//!         Ok(Box::new(EchoStream { emitted: false }))
//!     }
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let model_id = ModelId::new("local_echo")?;
//! let provider: Arc<dyn ModelProvider> = Arc::new(EchoProvider {
//!     id: model_id.clone(),
//!     pricing: Pricing::new(1, 1, 1),
//! });
//! let router = ModelRouter::new(
//!     model_id.clone(),
//!     Vec::new(),
//!     vec![model_id.clone()],
//! )?;
//! let gateway = ModelGateway::new(router, RetryPolicy::default(), vec![provider])?;
//! let request = CompletionRequest::new(
//!     vec![Message::new(MessageRole::User, "hello")],
//!     Vec::new(),
//!     ToolChoice::Auto,
//!     CacheHints::disabled(),
//! );
//! let context = RouteContext::new(
//!     TaskStage::Chat,
//!     TokenCount::new(1),
//!     Sensitivity::Public,
//!     assistant_model_gateway::CostMicroUsd::new(1_000),
//!     0,
//! );
//! let mut completion = gateway.complete(
//!     request,
//!     context,
//!     assistant_model_gateway::CancellationToken::new(),
//! )?;
//! let _ = completion.next_event()?;
//! # Ok(())
//! # }
//! ```
//!
//! Related documents: architecture v2 sections 11.1 through 11.5,
//! `docs/spec/error-codes.md`, and `docs/spec/naming.md`.

#![deny(unsafe_code)]

mod cancellation;
mod cost;
mod error;
mod execution;
mod gateway;
mod identity;
mod model;
mod pricing;
mod provider;
mod retry;
mod router;

pub use cancellation::CancellationToken;
pub use cost::CostLedger;
pub use error::{ModelGatewayError, ModelResult};
pub use execution::CollectedCompletion;
pub use gateway::{GatewayCompletion, ModelGateway};
pub use identity::{
    CacheKey, CostMicroUsd, DurationMs, ModelId, RuleId, Sensitivity, TaskStage, TokenCount,
};
pub use model::{
    AttemptOutcome, AttemptRecord, Budget, CacheHints, CompletionEvent, CompletionRequest,
    FinishReason, Message, MessageRole, ResponseFormat, ToolCallDelta, ToolChoice, ToolSpec, Usage,
    UsageRecord,
};
pub use pricing::Pricing;
pub use provider::{CompletionStream, ModelProvider, ProviderCapabilities, ProviderFeatures};
pub use retry::{
    JitterSource, MonotonicClock, NoJitter, RetryPolicy, Sleeper, SystemMonotonicClock,
    ThreadSleeper,
};
pub use router::{ModelRouter, RouteCondition, RouteContext, RoutePlan, RoutingRule};
