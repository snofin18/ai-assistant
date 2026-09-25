//! Completed execution result and provider capability preflight.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::error::{ModelGatewayError, ModelResult};
use crate::identity::{DurationMs, ModelId};
use crate::model::{AttemptRecord, CompletionEvent, CompletionRequest, UsageRecord};
use crate::provider::ModelProvider;

/// Default timeout for one provider event poll.
pub const DEFAULT_EVENT_POLL_TIMEOUT: DurationMs = DurationMs::new(250);
/// Maximum event poll timeout that keeps cancellation observable within one second.
pub const MAX_EVENT_POLL_TIMEOUT: DurationMs = DurationMs::new(500);

/// Shared provider registry used by one gateway session.
pub type ProviderMap = Arc<BTreeMap<ModelId, Arc<dyn ModelProvider>>>;

/// Completed response returned by [`ModelGateway::collect`].
///
/// [`ModelGateway::collect`]: crate::ModelGateway::collect
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedCompletion {
    /// Validated stream events in order.
    pub events: Vec<CompletionEvent>,
    /// Final usage, cost, latency, and cache record.
    pub usage_record: UsageRecord,
    /// Provider attempts, including retries and fallbacks.
    pub attempts: Vec<AttemptRecord>,
}

/// Checks request-derived capabilities before starting a provider call.
pub fn validate_capabilities(
    model_id: &ModelId,
    provider: &dyn ModelProvider,
    request: &CompletionRequest,
) -> ModelResult<()> {
    let capabilities = provider.capabilities();
    if !capabilities.supports_streaming() {
        return Err(ModelGatewayError::CapabilityMissing {
            model_id: model_id.clone(),
            capability: "streaming",
        });
    }
    if !request.tools.is_empty() && !capabilities.supports_tools() {
        return Err(ModelGatewayError::CapabilityMissing {
            model_id: model_id.clone(),
            capability: "tools",
        });
    }
    Ok(())
}

/// Validates the gateway's provider event-poll timeout.
///
/// # Errors
///
/// Returns `InvalidConfiguration` when the timeout is zero or exceeds 500 ms.
pub fn validate_event_poll_timeout(timeout: DurationMs) -> ModelResult<()> {
    if timeout.is_zero() || timeout > MAX_EVENT_POLL_TIMEOUT {
        return Err(ModelGatewayError::InvalidConfiguration {
            reason: "event poll timeout must be within 1..=500 ms".to_string(),
        });
    }
    Ok(())
}
