//! Structured model-gateway failures and their stable protocol error categories.

use std::fmt;

use assistant_protocol::ErrorCode;

use crate::identity::{DurationMs, ModelId};

/// Result alias for model-gateway operations.
pub type ModelResult<T> = Result<T, ModelGatewayError>;

/// A deterministic gateway, provider, or streaming failure.
///
/// Every variant maps to an existing protocol [`ErrorCode`]. The gateway deliberately does not
/// invent vendor-specific error codes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelGatewayError {
    /// Router, retry, or provider configuration is internally inconsistent.
    InvalidConfiguration {
        /// Human-readable rejection reason.
        reason: String,
    },

    /// A caller supplied an invalid request value.
    InvalidRequest {
        /// Request field that failed validation.
        field: &'static str,
        /// Human-readable rejection reason.
        reason: String,
    },

    /// A selected model has no registered provider.
    ProviderNotFound {
        /// Missing model identifier.
        model_id: ModelId,
    },

    /// A provider does not satisfy a request capability.
    CapabilityMissing {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Missing capability name.
        capability: &'static str,
    },

    /// The provider reported rate limiting.
    RateLimited {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Optional provider-provided retry delay.
        retry_after: Option<DurationMs>,
    },

    /// A provider call or event poll exceeded its deadline.
    Timeout {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Deadline that elapsed.
        timeout: DurationMs,
    },

    /// The provider returned a server error.
    ServerError {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Optional HTTP status.
        status: Option<u16>,
    },

    /// The provider could not reach the network endpoint.
    NetworkFailure {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Human-readable failure reason.
        reason: String,
    },

    /// The provider emitted an event that failed gateway validation.
    InvalidOutput {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Human-readable rejection reason.
        reason: String,
    },

    /// The caller cancelled the request.
    Cancelled {
        /// Active model, if selection had already completed.
        model_id: Option<ModelId>,
        /// Time from gateway start to cancellation.
        elapsed: DurationMs,
    },

    /// The stream ended without the required usage and finish events.
    IncompleteResponse {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Missing or invalid terminal condition.
        reason: &'static str,
    },

    /// A stream failed after emitting text or tool-call output.
    ///
    /// The gateway does not retry or fall back after this error because doing so could duplicate
    /// already-visible output.
    PartialOutput {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Human-readable underlying failure.
        reason: String,
    },

    /// A request-level cost or duration budget was exceeded.
    BudgetExceeded {
        /// Provider/model identifier.
        model_id: ModelId,
        /// Budget resource (`cost` or `duration`).
        resource: &'static str,
        /// Configured limit in native units.
        limit: u64,
        /// Observed value in native units.
        actual: u64,
    },

    /// A token, cost, or delay calculation overflowed.
    NumericOverflow {
        /// Field that overflowed.
        field: &'static str,
    },

    /// An internal invariant failed.
    InternalInvariant {
        /// Invariant that was violated.
        reason: &'static str,
    },
}

impl ModelGatewayError {
    /// Maps the failure to the stable protocol error category.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidConfiguration { .. }
            | Self::InvalidRequest { .. }
            | Self::ProviderNotFound { .. }
            | Self::CapabilityMissing { .. } => ErrorCode::ToolInvalidArgs,
            Self::RateLimited { .. }
            | Self::Timeout { .. }
            | Self::ServerError { .. }
            | Self::NetworkFailure { .. }
            | Self::PartialOutput { .. } => ErrorCode::ModelNetworkFailure,
            Self::InvalidOutput { .. } | Self::IncompleteResponse { .. } => {
                ErrorCode::ModelInvalidOutput
            }
            Self::Cancelled { .. } => ErrorCode::UserInteraction,
            Self::BudgetExceeded { .. } => ErrorCode::PolicyDenied,
            Self::NumericOverflow { .. } | Self::InternalInvariant { .. } => ErrorCode::Fatal,
        }
    }

    /// Returns whether another attempt against the same model can be safe and useful.
    #[must_use]
    pub const fn is_retryable_for_model(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. }
                | Self::Timeout { .. }
                | Self::ServerError { .. }
                | Self::NetworkFailure { .. }
                | Self::IncompleteResponse { .. }
        )
    }

    /// Returns whether the next configured model may be tried.
    #[must_use]
    pub const fn should_fallback(&self) -> bool {
        matches!(
            self,
            Self::ProviderNotFound { .. }
                | Self::CapabilityMissing { .. }
                | Self::RateLimited { .. }
                | Self::Timeout { .. }
                | Self::ServerError { .. }
                | Self::NetworkFailure { .. }
                | Self::IncompleteResponse { .. }
        )
    }

    /// Returns whether this error means the caller or a user stopped the request.
    #[must_use]
    pub const fn is_cancellation(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }
}

impl fmt::Display for ModelGatewayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration { reason } => {
                write!(formatter, "invalid model gateway configuration: {reason}")
            }
            Self::InvalidRequest { field, reason } => {
                write!(formatter, "invalid model request field {field}: {reason}")
            }
            Self::ProviderNotFound { model_id } => {
                write!(formatter, "no model provider is registered for {model_id}")
            }
            Self::CapabilityMissing {
                model_id,
                capability,
            } => write!(
                formatter,
                "model {model_id} does not provide required capability {capability}"
            ),
            Self::RateLimited {
                model_id,
                retry_after,
            } => Self::fmt_rate_limit(formatter, model_id, *retry_after),
            Self::Timeout { model_id, timeout } => write!(
                formatter,
                "model {model_id} timed out after {} ms",
                timeout.get()
            ),
            Self::ServerError { model_id, status } => {
                Self::fmt_server_error(formatter, model_id, *status)
            }
            Self::NetworkFailure { model_id, reason } => {
                write!(formatter, "model {model_id} network failure: {reason}")
            }
            Self::InvalidOutput { .. }
            | Self::Cancelled { .. }
            | Self::IncompleteResponse { .. }
            | Self::PartialOutput { .. } => self.fmt_stream_failure(formatter),
            Self::BudgetExceeded {
                model_id,
                resource,
                limit,
                actual,
            } => write!(
                formatter,
                "model {model_id} exceeded the request {resource} budget: limit {limit}, actual {actual}"
            ),
            Self::NumericOverflow { field } => {
                write!(formatter, "numeric overflow while calculating {field}")
            }
            Self::InternalInvariant { reason } => {
                write!(
                    formatter,
                    "model gateway internal invariant failed: {reason}"
                )
            }
        }
    }
}

impl ModelGatewayError {
    fn fmt_rate_limit(
        formatter: &mut fmt::Formatter<'_>,
        model_id: &ModelId,
        retry_after: Option<DurationMs>,
    ) -> fmt::Result {
        write!(formatter, "model {model_id} is rate limited")?;
        if let Some(delay) = retry_after {
            write!(formatter, "; retry after {} ms", delay.get())?;
        }
        Ok(())
    }

    fn fmt_server_error(
        formatter: &mut fmt::Formatter<'_>,
        model_id: &ModelId,
        status: Option<u16>,
    ) -> fmt::Result {
        write!(formatter, "model {model_id} returned a server error")?;
        if let Some(status) = status {
            write!(formatter, " with status {status}")?;
        }
        Ok(())
    }

    fn fmt_stream_failure(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOutput { model_id, reason } => {
                write!(
                    formatter,
                    "model {model_id} emitted invalid output: {reason}"
                )
            }
            Self::Cancelled { model_id, elapsed } => match model_id {
                Some(model_id) => write!(
                    formatter,
                    "model request to {model_id} was cancelled after {} ms",
                    elapsed.get()
                ),
                None => write!(
                    formatter,
                    "model request was cancelled after {} ms",
                    elapsed.get()
                ),
            },
            Self::IncompleteResponse { model_id, reason } => write!(
                formatter,
                "model {model_id} ended with an incomplete response: {reason}"
            ),
            Self::PartialOutput { model_id, reason } => write!(
                formatter,
                "model {model_id} failed after partial output; retry is forbidden: {reason}"
            ),
            _ => Err(fmt::Error),
        }
    }
}

impl std::error::Error for ModelGatewayError {}
