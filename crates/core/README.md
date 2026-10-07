# assistant-core

Core orchestration components from architecture v2 section 11. TASK-028
implements session lifecycle, message trees, context selection, trimming,
compression, and token budgeting. TASK-207 adds model-output planning into the
task-engine Plan contract.

## Responsibilities

- Create, restore, mutate, and end a session through an injected
  `SessionStore`.
- Maintain a validated message tree with stable ids, parent links, sequence
  order, content validation, and explicit delete rules.
- Maintain runtime taint state for each cached session: Tool messages taint,
  User messages clear, and restoration recomputes from the role sequence.
- Select one caller-chosen root-to-leaf branch for model context.
- Keep required anchors and evidence inside the context budget.
- Omit droppable history only with an auditable omission record.
- Replace older history with an injected structured summary when compression
  is required.
- Fail closed when required context or a summary cannot fit.
- Convert one injected `ModelProvider` response into a validated
  `assistant-task-engine::Plan`.
- Reject malformed JSON, invalid DAGs, and tools absent from the catalog.
- Load and validate versioned App Map files through an injected file reader.
- Build bounded memory projections from caller-selected App Map entries and
  storage retrieval hits, preserving source references and omission reasons.
- Run clean-context review for high-risk Steps through an injected
  `ModelProvider`, using only the session goal and a validated Step summary.
- Define the canonical `InstructionOrigin` tokens and validate per-origin
  attribution metadata (`parent_goal` / `source_ref`).

## Boundaries

- **No Host assembly.** The binary layer creates and injects components.
- **No SQLite connection or SQL.** Session persistence is behind the
  `SessionStore` trait. TASK-029 owns the production storage adapter.
- **No platform API calls.** The architecture test rejects platform
  implementation references and dependencies.
- **No policy decisions or tool execution.**
- **No model routing, retries, or fallbacks.** Planner consumes the supplied
  provider directly; routing remains a model-gateway concern.
- **No system clock, random source, UUID library, filesystem, or network.**
  Time, ids, content, and persistence are supplied by the caller.
- **No SQLite connection or SQL in Memory.** App Map reads use an injected
  reader; retrieval uses an injected `MemoryRetriever` that the binary layer
  maps to the storage API.
- **No model prompt policy.** The compressor is an injected strategy and may
  later bridge to `model-gateway`.
- **No permission decision or session mutation in clean-context review.** The
  reviewer returns a typed decision or fail-closed error; policy / HITL remain
  the final gates.
- **No permission decision or persistence in instruction attribution.** An
  `app_content` origin is a high-risk signal; policy / HITL still decide whether
  an action may proceed.

## Invariants

1. `crates/core/Cargo.toml` dependencies stay inside ADR-0053 D2.
2. Session ids, message ids, goals, content, token estimates, budgets, and
   restored snapshots are validated before use.
3. A failed `SessionStore` update leaves the previous in-memory snapshot
   authoritative.
4. A tree path is always selected explicitly by leaf id. Core never guesses
   which branch to send to a model.
5. Required context over budget returns an error; it is never truncated.
6. Droppable and summarized messages produce `ContextOmission` records.
7. Compression failure or summary/source mismatch returns a typed error; it
   never degrades into silent history loss.
8. `used_tokens` never exceeds `available_tokens`.
9. Planner never repairs malformed model output and always validates the
   resulting Plan with `Plan::validate`.
10. Every planned step uses a tool present in the caller-supplied catalog.
11. App Map paths are relative and traversal-free; App Map content, version,
    fields, and entry ids are validated before use. Canonical provenance is
    generated from the validated path and array index, and the declared token
    estimate can only raise the conservative character-count estimate.
12. Memory never injects an entire App Map implicitly. Only caller-selected
    entry ids plus retrieval hits are considered, and every omitted candidate
    has an explicit origin, token estimate, and `MemoryOmissionReason`.
13. Taint never becomes clean silently: only a User message or the
    `SessionManager::clear_taint` entry point clears it; restore recomputes the
    conservative state from persisted messages.
14. Clean-context review only accepts `SessionSnapshot::goal()` plus a
    validated `HighRiskStepSummary`; Tool / document / web content never enters
    the prompt, and review calls never append messages or change taint.
15. Instruction origins use only `user_request` / `plan_derived` /
    `app_content` / `tool_suggestion`; unknown tokens and invalid metadata
    combinations fail closed.

## Typical Use

```rust
use std::sync::Arc;

use assistant_core::{
    ContextBudget, ContextRetention, MemorySessionStore, MessageContent, MessageId,
    MessageRole, NewMessage, SessionClock, SessionId, SessionManager, TokenCount,
};

struct Clock;

impl SessionClock for Clock {
    fn now_unix_ms(&self) -> i64 {
        1_700_000_000_000
    }
}

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let store = Arc::new(MemorySessionStore::new());
let mut sessions = SessionManager::new(store, Arc::new(Clock));
let session_id = SessionId::new("s_1")?;
sessions.create_session(session_id.clone(), "summarize a document")?;
sessions.append_message(
    &session_id,
    NewMessage {
        id: MessageId::new("m_1")?,
        parent_id: None,
        role: MessageRole::User,
        content: MessageContent::new("Read the document")?,
        token_estimate: TokenCount::new(8),
        retention: ContextRetention::Required,
    },
)?;

let snapshot = sessions.snapshot(&session_id)?;
let leaf = snapshot.latest_leaf().ok_or("missing message")?;
let budget = ContextBudget::new(TokenCount::new(4_096), TokenCount::new(1_024))?;
# let _ = (snapshot, leaf, budget);
# Ok(())
# }
```

The example intentionally stops before `build_context` because a production
`HistoryCompressor` is injected by the binary. Tests and replay use an
in-memory deterministic compressor.

## Session Store Contract

`SessionStore` separates orchestration from persistence:

- `insert_session` fails when an id already exists.
- `update_session` accepts exactly the next snapshot revision.
- `load_session` returns a validated `SessionSnapshot`.

`MemorySessionStore` is deterministic and intended for unit tests, replay, and
early assembly. The TASK-029 binary adapter must map the same contract to the
public `assistant-storage` record APIs without exposing a SQLite connection to
Core.

## Known Limitations

- There is no production storage adapter in this crate yet. The in-memory store
  does not survive process restart.
- The context code consumes caller-supplied token estimates. Provider-specific
  token counting and compressor prompts remain assembly/provider concerns.
- Compression operates on one contiguous older suffix of the selected branch.
  More sophisticated summarization windows require a later contract change.
- Planner intentionally requires exact JSON with a single `steps` field. It
  does not repair Markdown fences, missing fields, or provider-specific tool
  call payloads.
- Planner callers supply plan/task identity and budget; model output controls
  only the Step DAG shape.
- Session deletion is leaf-only; branch deletion and tombstone semantics are
  not defined by TASK-028.
- Runtime taint is not persisted as a separate column or snapshot field. A
  restored session recomputes it from message roles, so an explicit live-only
  clear can become tainted again after a process restart unless a User message
  followed the Tool result.
- The crate does not persist audit events for context omissions. Callers can
  project the returned omission records into audit events at the assembly
  boundary.
- App Map schema v1 is intentionally small. Adapters that need richer metadata
  must bump the version and add validation rather than silently accepting
  unknown fields.
- Production retrieval still needs a TASK-029 adapter from `MemoryRetriever`
  to the storage API; tests use deterministic in-memory implementations.
- The clean-context reviewer is a review primitive, not an automatic hook. The
  binary assembly layer must invoke it before high-risk Steps and treat every
  provider or parsing failure as fail-closed; the model-provided reason is
  untrusted display text.
- Instruction attribution is a canonical pure value. This crate does not yet
  persist it in audit events or carry it through IPC / ApprovalRequest; those
  cross-layer schema changes require a later contract and are intentionally out
  of scope for this card.

## Related Documents

- `docs/spec/core-orchestration.md`
- `docs/adr/0053-core-orchestration-layer-interface.md`
- `docs/adr/0080-taint-tracking-and-permission-decay.md`
- `docs/adr/0081-clean-context-review-component.md`
- `docs/adr/0082-instruction-origin-attribution.md`
- `cross-platform-ai-assistant-architecture-v2.md` sections 7.1, 7.2, 11.3,
  10.5, 12.4, and 15.
- `tasks/TASK-028-core-session-context.md`
- `tasks/TASK-207-core-planner-plan-step-dag.md`
- `tasks/TASK-208-core-memory-app-map-fts-retrieval.md`
