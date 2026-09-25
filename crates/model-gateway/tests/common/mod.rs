#![allow(
    dead_code,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::missing_const_for_fn
)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

use assistant_model_gateway::{
    CacheHints, CancellationToken, CompletionEvent, CompletionRequest, CompletionStream,
    CostMicroUsd, DurationMs, FinishReason, Message, MessageRole, ModelGatewayError, ModelId,
    ModelProvider, ModelResult, Pricing, ProviderCapabilities, ProviderFeatures, RouteContext,
    Sensitivity, TaskStage, TokenCount, ToolChoice, Usage,
};

pub fn model_id(value: &str) -> ModelId {
    ModelId::new(value).unwrap()
}

pub fn pricing() -> Pricing {
    Pricing::new(1_000_000, 100_000, 2_000_000)
}

pub fn capabilities(tools: bool, prompt_cache: bool) -> ProviderCapabilities {
    let mut features = ProviderFeatures::STREAMING;
    if tools {
        features = features.union(ProviderFeatures::TOOLS);
    }
    if prompt_cache {
        features = features.union(ProviderFeatures::PROMPT_CACHE);
    }
    ProviderCapabilities::new(features, TokenCount::new(128_000))
}

pub fn request() -> CompletionRequest {
    CompletionRequest::new(
        vec![Message::new(MessageRole::User, "hello")],
        Vec::new(),
        ToolChoice::Auto,
        CacheHints::disabled(),
    )
}

pub fn request_with_cache() -> CompletionRequest {
    CompletionRequest::new(
        vec![
            Message::new(MessageRole::System, "stable instructions"),
            Message::new(MessageRole::User, "hello"),
        ],
        Vec::new(),
        ToolChoice::Auto,
        CacheHints::stable_prefix(
            assistant_model_gateway::CacheKey::new("prompt-v1").unwrap(),
            0,
            TokenCount::new(32),
        ),
    )
}

pub fn context(stage: TaskStage) -> RouteContext {
    RouteContext::new(
        stage,
        TokenCount::new(100),
        Sensitivity::Public,
        CostMicroUsd::new(1_000_000),
        0,
    )
}

pub fn successful_events() -> Vec<ModelResult<CompletionEvent>> {
    vec![
        Ok(CompletionEvent::TextDelta("hello".to_string())),
        Ok(CompletionEvent::Usage(Usage::new(
            TokenCount::new(100),
            TokenCount::new(0),
            TokenCount::new(50),
        ))),
        Ok(CompletionEvent::Finished(FinishReason::Stop)),
    ]
}

pub enum ProviderAction {
    Fail(ModelGatewayError),
    Events(Vec<ModelResult<CompletionEvent>>),
}

pub struct ScriptedProvider {
    model_id: ModelId,
    capabilities: ProviderCapabilities,
    pricing: Pricing,
    actions: Mutex<VecDeque<ProviderAction>>,
    requests: Mutex<Vec<CompletionRequest>>,
    call_count: Arc<Mutex<usize>>,
}

impl ScriptedProvider {
    pub fn new(model_id: ModelId, actions: Vec<ProviderAction>) -> Self {
        Self {
            model_id,
            capabilities: capabilities(true, true),
            pricing: pricing(),
            actions: Mutex::new(actions.into()),
            requests: Mutex::new(Vec::new()),
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }

    pub fn requests(&self) -> Vec<CompletionRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl ModelProvider for ScriptedProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn pricing(&self) -> Pricing {
        self.pricing
    }

    fn count_tokens(&self, messages: &[Message]) -> ModelResult<TokenCount> {
        Ok(TokenCount::new(
            messages
                .iter()
                .map(|message| message.content.len() as u64)
                .sum(),
        ))
    }

    fn complete(
        &self,
        request: CompletionRequest,
        cancellation: CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        *self.call_count.lock().unwrap() += 1;
        self.requests.lock().unwrap().push(request);
        let action = self.actions.lock().unwrap().pop_front().ok_or(
            ModelGatewayError::InternalInvariant {
                reason: "test provider script exhausted",
            },
        )?;
        match action {
            ProviderAction::Fail(error) => Err(error),
            ProviderAction::Events(events) => Ok(Box::new(ScriptedStream {
                events: events.into(),
                cancellation,
            })),
        }
    }
}

struct ScriptedStream {
    events: VecDeque<ModelResult<CompletionEvent>>,
    cancellation: CancellationToken,
}

impl CompletionStream for ScriptedStream {
    fn next_event(
        &mut self,
        cancellation: &CancellationToken,
        _timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>> {
        if cancellation.is_cancelled() {
            return Err(ModelGatewayError::Cancelled {
                model_id: None,
                elapsed: DurationMs::new(0),
            });
        }
        self.events
            .pop_front()
            .map_or(Ok(None), |result| result.map(Some))
    }
}

pub struct SlowCancelProvider {
    model_id: ModelId,
    call_count: Arc<Mutex<usize>>,
}

impl SlowCancelProvider {
    pub fn new(model_id: ModelId) -> Self {
        Self {
            model_id,
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }
}

impl ModelProvider for SlowCancelProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        capabilities(false, false)
    }

    fn pricing(&self) -> Pricing {
        pricing()
    }

    fn count_tokens(&self, _messages: &[Message]) -> ModelResult<TokenCount> {
        Ok(TokenCount::new(0))
    }

    fn complete(
        &self,
        _request: CompletionRequest,
        cancellation: CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        *self.call_count.lock().unwrap() += 1;
        Ok(Box::new(SlowCancelStream {
            model_id: self.model_id.clone(),
            cancellation,
        }))
    }
}

struct SlowCancelStream {
    model_id: ModelId,
    cancellation: CancellationToken,
}

impl CompletionStream for SlowCancelStream {
    fn next_event(
        &mut self,
        cancellation: &CancellationToken,
        _timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>> {
        thread::sleep(StdDuration::from_millis(25));
        if cancellation.is_cancelled() || self.cancellation.is_cancelled() {
            return Err(ModelGatewayError::Cancelled {
                model_id: Some(self.model_id.clone()),
                elapsed: DurationMs::new(25),
            });
        }
        Ok(Some(CompletionEvent::TextDelta("tick".to_string())))
    }
}

#[derive(Debug, Default)]
pub struct ManualClock {
    now_ms: Mutex<u64>,
}

impl ManualClock {
    pub fn advance(&self, delta_ms: u64) {
        *self.now_ms.lock().unwrap() += delta_ms;
    }
}

impl assistant_model_gateway::MonotonicClock for ManualClock {
    fn now_ms(&self) -> u64 {
        *self.now_ms.lock().unwrap()
    }
}

#[derive(Debug, Default)]
pub struct RecordingSleeper {
    slept: Mutex<Vec<DurationMs>>,
}

impl RecordingSleeper {
    pub fn slept(&self) -> Vec<DurationMs> {
        self.slept.lock().unwrap().clone()
    }
}

impl assistant_model_gateway::Sleeper for RecordingSleeper {
    fn sleep(&self, duration: DurationMs) {
        self.slept.lock().unwrap().push(duration);
    }
}
