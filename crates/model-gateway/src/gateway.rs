//! Gateway execution state machine with retry, fallback, cancellation, and cost records.

use std::{collections::BTreeMap, sync::Arc};

use crate::cancellation::CancellationToken;
use crate::error::{ModelGatewayError, ModelResult};
use crate::execution::{
    CollectedCompletion, DEFAULT_EVENT_POLL_TIMEOUT, ProviderMap, validate_capabilities,
    validate_event_poll_timeout,
};
use crate::identity::{DurationMs, ModelId, TokenCount};
use crate::model::{
    AttemptOutcome, AttemptRecord, CompletionEvent, CompletionRequest, Message, Usage, UsageRecord,
};
use crate::provider::{CompletionStream, ModelProvider};
use crate::retry::{
    GatewayRuntime, JitterSource, MonotonicClock, NoJitter, RetryPolicy, Sleeper,
    SystemMonotonicClock, ThreadSleeper,
};
use crate::router::{ModelRouter, RouteContext, RoutePlan};

/// Provider-neutral gateway.
///
/// The gateway owns routing, retry, fallback, cancellation checks, event validation, and cost
/// accounting. Concrete providers own network I/O and vendor normalization.
#[derive(Clone)]
pub struct ModelGateway {
    providers: ProviderMap,
    router: ModelRouter,
    retry_policy: RetryPolicy,
    runtime: GatewayRuntime,
    event_poll_timeout: DurationMs,
}

impl ModelGateway {
    /// Creates a gateway with process-local clock and sleeper and no jitter.
    ///
    /// Returns `InvalidConfiguration` when the retry policy is invalid, provider identifiers
    /// collide, or the router references an unregistered model.
    ///
    /// # Errors
    ///
    /// Returns `InvalidConfiguration` or `ProviderNotFound` when construction cannot satisfy the
    /// router and provider registry contracts.
    pub fn new(
        router: ModelRouter,
        retry_policy: RetryPolicy,
        providers: Vec<Arc<dyn ModelProvider>>,
    ) -> ModelResult<Self> {
        Self::with_runtime(
            router,
            retry_policy,
            providers,
            Arc::new(SystemMonotonicClock::default()),
            Arc::new(ThreadSleeper),
            Arc::new(NoJitter),
        )
    }

    /// Creates a gateway with injected clock, sleeper, and jitter.
    ///
    /// This constructor is the deterministic test seam. It has no network side effects.
    ///
    /// # Errors
    ///
    /// Returns `InvalidConfiguration` or `ProviderNotFound` when construction cannot satisfy the
    /// router and provider registry contracts.
    pub fn with_runtime(
        router: ModelRouter,
        retry_policy: RetryPolicy,
        providers: Vec<Arc<dyn ModelProvider>>,
        clock: Arc<dyn MonotonicClock>,
        sleeper: Arc<dyn Sleeper>,
        jitter: Arc<dyn JitterSource>,
    ) -> ModelResult<Self> {
        retry_policy.validate()?;
        let mut provider_map = BTreeMap::new();
        for provider in providers {
            let model_id = provider.model_id().clone();
            if provider_map.insert(model_id.clone(), provider).is_some() {
                return Err(ModelGatewayError::InvalidConfiguration {
                    reason: format!("duplicate provider for model {model_id}"),
                });
            }
        }
        for model_id in router.referenced_models() {
            if !provider_map.contains_key(&model_id) {
                return Err(ModelGatewayError::ProviderNotFound { model_id });
            }
        }
        Ok(Self {
            providers: Arc::new(provider_map),
            router,
            retry_policy,
            runtime: GatewayRuntime {
                clock,
                sleeper,
                jitter,
            },
            event_poll_timeout: DEFAULT_EVENT_POLL_TIMEOUT,
        })
    }

    /// Sets the maximum timeout passed to one provider event poll.
    ///
    /// Returns `InvalidConfiguration` for zero or values above 500 ms. The bound keeps cooperative
    /// cancellation observable within one second.
    ///
    /// # Errors
    ///
    /// Returns `InvalidConfiguration` when the timeout is zero or exceeds 500 ms.
    pub fn with_event_poll_timeout(mut self, timeout: DurationMs) -> ModelResult<Self> {
        validate_event_poll_timeout(timeout)?;
        self.event_poll_timeout = timeout;
        Ok(self)
    }

    /// Counts tokens with a registered provider without executing a completion.
    ///
    /// # Errors
    ///
    /// Returns `ProviderNotFound` when the model is unregistered, or propagates the provider's
    /// counting failure.
    pub fn count_tokens(
        &self,
        model_id: &ModelId,
        messages: &[Message],
    ) -> ModelResult<TokenCount> {
        let provider =
            self.providers
                .get(model_id)
                .ok_or_else(|| ModelGatewayError::ProviderNotFound {
                    model_id: model_id.clone(),
                })?;
        provider.count_tokens(messages)
    }

    /// Starts one request and returns a streaming completion session.
    ///
    /// Validation and route selection are synchronous. Provider calls begin on the first
    /// [`GatewayCompletion::next_event`]. The session returns `Cancelled` without retry when the
    /// token is observed, and never retries or falls back after output has been emitted.
    ///
    /// # Errors
    ///
    /// Returns `InvalidRequest` when the request fails validation.
    pub fn complete(
        &self,
        request: CompletionRequest,
        context: RouteContext,
        cancellation: CancellationToken,
    ) -> ModelResult<GatewayCompletion> {
        request.validate()?;
        let RoutePlan {
            primary,
            fallback_chain,
        } = self.router.route(&context);
        let mut candidates = Vec::with_capacity(fallback_chain.len() + 1);
        candidates.push(primary);
        candidates.extend(fallback_chain);
        Ok(GatewayCompletion {
            providers: Arc::clone(&self.providers),
            request,
            cancellation,
            retry_policy: self.retry_policy,
            runtime: self.runtime.clone(),
            event_poll_timeout: self.event_poll_timeout,
            candidates,
            candidate_index: 0,
            retry_index: 0,
            active: None,
            last_error: None,
            events_emitted: false,
            usage: None,
            started_at: self.runtime.clock.now_ms(),
            finished: false,
            attempt_records: Vec::new(),
            usage_record: None,
        })
    }

    /// Executes a request to completion and returns all events and the final usage record.
    ///
    /// # Errors
    ///
    /// Returns request, routing, provider, streaming, cancellation, or budget errors from the
    /// underlying streaming session.
    pub fn collect(
        &self,
        request: CompletionRequest,
        context: RouteContext,
        cancellation: CancellationToken,
    ) -> ModelResult<CollectedCompletion> {
        self.complete(request, context, cancellation)?.collect()
    }
}

struct ActiveAttempt {
    model_id: ModelId,
    retry_index: u32,
    started_at: u64,
    stream: Box<dyn CompletionStream>,
}

/// Streaming gateway execution.
pub struct GatewayCompletion {
    providers: ProviderMap,
    request: CompletionRequest,
    cancellation: CancellationToken,
    retry_policy: RetryPolicy,
    runtime: GatewayRuntime,
    event_poll_timeout: DurationMs,
    candidates: Vec<ModelId>,
    candidate_index: usize,
    retry_index: u32,
    active: Option<ActiveAttempt>,
    last_error: Option<ModelGatewayError>,
    events_emitted: bool,
    usage: Option<Usage>,
    started_at: u64,
    finished: bool,
    attempt_records: Vec<AttemptRecord>,
    usage_record: Option<UsageRecord>,
}

impl GatewayCompletion {
    /// Returns the next validated stream event.
    ///
    /// Returns `Ok(None)` only after a finish event has been emitted. Cancellation is checked
    /// before every provider call and poll and is never retried.
    ///
    /// # Errors
    ///
    /// Returns cancellation, provider, fallback, validation, or partial-output errors. No error is
    /// returned after a successful finish event.
    pub fn next_event(&mut self) -> ModelResult<Option<CompletionEvent>> {
        if self.finished {
            return Ok(None);
        }
        self.check_cancellation(None)?;
        if self.active.is_none() {
            self.start_available_attempt()?;
        }
        let (model_id, retry_index) = self.active_identity()?;
        let cancellation = self.cancellation.clone();
        let timeout = self.event_poll_timeout;
        let result = self
            .active
            .as_mut()
            .ok_or(ModelGatewayError::InternalInvariant {
                reason: "active attempt disappeared before polling",
            })?
            .stream
            .next_event(&cancellation, timeout);

        match result {
            Ok(Some(event)) => {
                if let Err(error) = event.validate(&model_id) {
                    self.handle_stream_failure(error, &model_id, retry_index)?;
                    return self.next_event();
                }
                self.handle_event(event, &model_id, retry_index)
            }
            Ok(None) => {
                let error = ModelGatewayError::IncompleteResponse {
                    model_id: model_id.clone(),
                    reason: "stream ended before a finish event",
                };
                self.handle_stream_failure(error, &model_id, retry_index)?;
                self.next_event()
            }
            Err(error) => {
                let error = if cancellation.is_cancelled() && !error.is_cancellation() {
                    self.cancellation_error(Some(model_id.clone()))
                } else {
                    error
                };
                self.handle_stream_failure(error, &model_id, retry_index)?;
                self.next_event()
            }
        }
    }

    /// Returns the final usage, cost, latency, and cache record.
    #[must_use]
    pub const fn usage_record(&self) -> Option<&UsageRecord> {
        self.usage_record.as_ref()
    }

    /// Returns attempt records in execution order.
    #[must_use]
    pub fn attempt_records(&self) -> &[AttemptRecord] {
        &self.attempt_records
    }

    /// Consumes the session and returns all events plus the final usage record.
    ///
    /// # Errors
    ///
    /// Returns the first streaming failure or `InternalInvariant` if a successful stream has no
    /// usage record.
    pub fn collect(mut self) -> ModelResult<CollectedCompletion> {
        let mut events = Vec::new();
        while let Some(event) = self.next_event()? {
            events.push(event);
        }
        let usage_record = self
            .usage_record
            .ok_or(ModelGatewayError::InternalInvariant {
                reason: "successful completion has no usage record",
            })?;
        Ok(CollectedCompletion {
            events,
            usage_record,
            attempts: self.attempt_records,
        })
    }

    fn start_available_attempt(&mut self) -> ModelResult<()> {
        loop {
            let Some(model_id) = self.candidates.get(self.candidate_index).cloned() else {
                return Err(self.last_error.take().unwrap_or(
                    ModelGatewayError::InternalInvariant {
                        reason: "no route candidate remained",
                    },
                ));
            };
            self.check_cancellation(Some(model_id.clone()))?;
            let provider = self.providers.get(&model_id).cloned().ok_or_else(|| {
                ModelGatewayError::ProviderNotFound {
                    model_id: model_id.clone(),
                }
            });
            let provider = match provider {
                Ok(provider) => provider,
                Err(error) => {
                    if self
                        .handle_stream_failure(error, &model_id, self.retry_index)
                        .is_ok()
                    {
                        continue;
                    }
                    return Err(self.last_error.take().unwrap_or(
                        ModelGatewayError::InternalInvariant {
                            reason: "provider failure was not retained",
                        },
                    ));
                }
            };
            if let Err(error) = validate_capabilities(&model_id, provider.as_ref(), &self.request) {
                if self
                    .handle_stream_failure(error, &model_id, self.retry_index)
                    .is_ok()
                {
                    continue;
                }
                return Err(self.last_error.take().unwrap_or(
                    ModelGatewayError::InternalInvariant {
                        reason: "capability failure was not retained",
                    },
                ));
            }
            let started_at = self.runtime.clock.now_ms();
            match provider.complete(self.request.clone(), self.cancellation.clone()) {
                Ok(stream) => {
                    self.active = Some(ActiveAttempt {
                        model_id,
                        retry_index: self.retry_index,
                        started_at,
                        stream,
                    });
                    return Ok(());
                }
                Err(error) => {
                    if self
                        .handle_stream_failure(error, &model_id, self.retry_index)
                        .is_ok()
                    {
                        continue;
                    }
                    return Err(self.last_error.take().unwrap_or(
                        ModelGatewayError::InternalInvariant {
                            reason: "provider start failure was not retained",
                        },
                    ));
                }
            }
        }
    }

    fn handle_event(
        &mut self,
        event: CompletionEvent,
        model_id: &ModelId,
        retry_index: u32,
    ) -> ModelResult<Option<CompletionEvent>> {
        match &event {
            CompletionEvent::TextDelta(_) | CompletionEvent::ToolCallDelta(_) => {
                self.events_emitted = true;
            }
            CompletionEvent::Usage(usage) => {
                if self.usage.is_some() {
                    let error = ModelGatewayError::InvalidOutput {
                        model_id: model_id.clone(),
                        reason: "provider emitted more than one usage event".to_string(),
                    };
                    return self
                        .handle_stream_failure(error, model_id, retry_index)
                        .map(|()| None);
                }
                self.usage = Some(*usage);
            }
            CompletionEvent::Finished(_) => {
                let Some(usage) = self.usage else {
                    let error = ModelGatewayError::IncompleteResponse {
                        model_id: model_id.clone(),
                        reason: "finish event arrived before usage",
                    };
                    return self
                        .handle_stream_failure(error, model_id, retry_index)
                        .map(|()| None);
                };
                self.finish_success(model_id, usage, retry_index)?;
            }
        }
        Ok(Some(event))
    }

    fn finish_success(
        &mut self,
        model_id: &ModelId,
        usage: Usage,
        retry_index: u32,
    ) -> ModelResult<()> {
        let provider =
            self.providers
                .get(model_id)
                .ok_or_else(|| ModelGatewayError::ProviderNotFound {
                    model_id: model_id.clone(),
                })?;
        let cost = provider.pricing().compute_cost(&usage)?;
        let now_ms = self.runtime.clock.now_ms();
        let total_latency = DurationMs::new(now_ms.saturating_sub(self.started_at));
        if let Some(max_duration) = self.request.budget.max_duration
            && total_latency > max_duration
        {
            return Err(ModelGatewayError::BudgetExceeded {
                model_id: model_id.clone(),
                resource: "duration",
                limit: max_duration.get(),
                actual: total_latency.get(),
            });
        }
        if let Some(max_cost) = self.request.budget.max_cost
            && cost > max_cost
        {
            return Err(ModelGatewayError::BudgetExceeded {
                model_id: model_id.clone(),
                resource: "cost",
                limit: max_cost.get(),
                actual: cost.get(),
            });
        }
        let attempt_latency = DurationMs::new(
            now_ms.saturating_sub(
                self.active
                    .as_ref()
                    .map_or(now_ms, |active| active.started_at),
            ),
        );
        self.attempt_records.push(AttemptRecord {
            model_id: model_id.clone(),
            retry_index,
            outcome: AttemptOutcome::Succeeded,
            latency: attempt_latency,
        });
        self.usage_record = Some(UsageRecord {
            model_id: model_id.clone(),
            usage,
            cost,
            latency: total_latency,
            cache_hit: usage.cache_hit(),
        });
        self.finished = true;
        self.active = None;
        Ok(())
    }

    fn handle_stream_failure(
        &mut self,
        error: ModelGatewayError,
        model_id: &ModelId,
        retry_index: u32,
    ) -> ModelResult<()> {
        if error.is_cancellation() {
            self.record_attempt(
                model_id,
                retry_index,
                AttemptOutcome::Failed {
                    error_code: error.error_code(),
                },
            );
            self.active = None;
            return Err(error);
        }
        if self.events_emitted {
            self.record_attempt(
                model_id,
                retry_index,
                AttemptOutcome::Failed {
                    error_code: error.error_code(),
                },
            );
            self.active = None;
            return Err(ModelGatewayError::PartialOutput {
                model_id: model_id.clone(),
                reason: error.to_string(),
            });
        }

        let can_retry =
            error.is_retryable_for_model() && retry_index < self.retry_policy.max_retries();
        let can_fallback =
            error.should_fallback() && self.candidate_index + 1 < self.candidates.len();
        let outcome = if can_retry {
            AttemptOutcome::Retried {
                error_code: error.error_code(),
            }
        } else if can_fallback {
            AttemptOutcome::FellBack {
                error_code: error.error_code(),
            }
        } else {
            AttemptOutcome::Failed {
                error_code: error.error_code(),
            }
        };
        self.record_attempt(model_id, retry_index, outcome);
        self.active = None;
        self.last_error = Some(error);

        if can_retry {
            let jitter_permille = self.runtime.jitter.jitter_permille(retry_index);
            let delay = self
                .retry_policy
                .delay_for_retry(retry_index, jitter_permille)?;
            self.runtime.sleeper.sleep(delay);
            self.retry_index = retry_index + 1;
            return Ok(());
        }
        if can_fallback {
            self.candidate_index += 1;
            self.retry_index = 0;
            return Ok(());
        }
        Err(self
            .last_error
            .take()
            .unwrap_or(ModelGatewayError::InternalInvariant {
                reason: "failure path lost its error",
            }))
    }

    fn record_attempt(&mut self, model_id: &ModelId, retry_index: u32, outcome: AttemptOutcome) {
        let now_ms = self.runtime.clock.now_ms();
        let started_at = self
            .active
            .as_ref()
            .map_or(now_ms, |active| active.started_at);
        self.attempt_records.push(AttemptRecord {
            model_id: model_id.clone(),
            retry_index,
            outcome,
            latency: DurationMs::new(now_ms.saturating_sub(started_at)),
        });
    }

    fn active_identity(&self) -> ModelResult<(ModelId, u32)> {
        self.active
            .as_ref()
            .map(|active| (active.model_id.clone(), active.retry_index))
            .ok_or(ModelGatewayError::InternalInvariant {
                reason: "active attempt is missing",
            })
    }

    fn check_cancellation(&self, model_id: Option<ModelId>) -> ModelResult<()> {
        if self.cancellation.is_cancelled() {
            return Err(self.cancellation_error(model_id));
        }
        Ok(())
    }

    fn cancellation_error(&self, model_id: Option<ModelId>) -> ModelGatewayError {
        ModelGatewayError::Cancelled {
            model_id,
            elapsed: DurationMs::new(self.runtime.clock.now_ms().saturating_sub(self.started_at)),
        }
    }
}
