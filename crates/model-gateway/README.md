# assistant-model-gateway

Provider-neutral routing and execution for architecture v2 section 11.

## Responsibilities

- Define a pull-based streaming provider contract with explicit cancellation, capabilities, token
  counting, and integer pricing.
- Route each request from typed rules over task stage, context size, sensitivity, remaining budget,
  and mounted-tool count.
- Retry retryable provider failures with exponential backoff and an injected deterministic jitter
  source, then move through a configured fallback chain.
- Forward stable prompt-cache hints without reordering or rewriting the prefix.
- Record per-step model, input/output/cached tokens, integer cost, latency, cache-hit state, and each
  provider attempt for later audit or UI aggregation.

## Boundaries

- **No network or vendor SDK.** Concrete providers own HTTP, WebSocket, CLI, or local-runtime I/O.
- **No policy decision.** The caller obtains permission before invoking the gateway. Budget checks
  only enforce the limits attached to the request.
- **No context management.** Tree trimming, history compression, App Map selection, and message
  construction belong to the Core context manager.
- **No persistence or audit sink.** [`CostLedger`] is in-memory; storage projection and audit event
  emission belong to their owning crates.
- **No tool-protocol normalization implementation.** The gateway validates normalized tool-call
  deltas but does not parse or repair vendor formats.
- **No versioned provider API yet.** This card defines the stable Core-facing contract; concrete
  OpenAI/Anthropic/local adapters are later integrations.

## Invariants

1. **Fail closed.** Invalid requests, provider events, usage records, and terminal sequences return
   typed errors. The gateway never converts malformed output into an empty successful response.
2. **Retry only before visible output.** After the first non-empty text or tool-call delta, a stream
   failure returns `PartialOutput`; the gateway neither retries nor falls back.
3. **Cancellation is terminal.** A cancelled token is checked before every provider call and poll.
   Provider implementations must honor the poll timeout and must never return success after
   cancellation.
4. **Terminal usage is mandatory.** A successful stream contains one usage event followed by one
   finish event.
5. **Routing is deterministic.** Rules are evaluated by `(priority, declaration order)` and the
   first matching rule wins.
6. **Costs are conservative integers.** Pricing is expressed in micro-USD per million tokens; each
   component rounds upward, cached input uses a separate rate, and totals never use floating point.
7. **No silent capability substitution.** A provider that cannot stream or cannot accept a
   request's tools is skipped through the configured fallback chain or reported as
   `CapabilityMissing`.

## Typical Use

```rust
use std::sync::Arc;
use assistant_model_gateway::{
    CancellationToken, ModelGateway, ModelRouter, RetryPolicy,
};

// A concrete ModelProvider is supplied by the caller.
# fn configure_gateway(
#     router: ModelRouter,
#     providers: Vec<Arc<dyn assistant_model_gateway::ModelProvider>>,
# ) -> Result<ModelGateway, assistant_model_gateway::ModelGatewayError> {
let gateway = ModelGateway::new(router, RetryPolicy::default(), providers)?;
# Ok(gateway)
# }
```

Provider implementations must:

1. validate or normalize vendor responses before emitting `CompletionEvent`;
2. check `CancellationToken::is_cancelled` before I/O and before each event poll;
3. return `Timeout` when no event arrives within the supplied poll timeout;
4. emit exactly one `Usage` event before `Finished`.

## Known Limitations

- The stream contract is synchronous pull-based to avoid adding a runtime dependency. Async
  providers can bridge into it with a runtime adapter, but that adapter must preserve the timeout
  and cancellation rules.
- The gateway cannot preempt a provider thread that ignores cancellation or blocks beyond its
  declared poll timeout. Such a provider is non-conforming and must fail conformance tests.
- Routing rules are typed and intentionally smaller than a general expression language. New
  predicate families require a contract update rather than embedding expressions in strings.
- Prompt-cache hints are forwarded even when a provider reports no cache capability; the router
  does not currently make cache support a hard eligibility condition.
- Cost records are per request, not persisted. Task/day/month aggregation is left to storage and the
  future UI cost panel.
- Provider adapters, structured tool-call parsing, and actual network retry classification are
  deferred to provider implementation cards.

## Related Documents

- Architecture v2 sections 11.1 through 11.5.
- `docs/spec/error-codes.md`.
- `docs/spec/naming.md`.
- `tasks/TASK-026-model-gateway-provider-router-fallback.md`.
