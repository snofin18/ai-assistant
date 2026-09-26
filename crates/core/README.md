# assistant-core

Core orchestration components from architecture v2 section 11. TASK-028
implements session lifecycle, message trees, context selection, trimming,
compression, and token budgeting.

## Responsibilities

- Create, restore, mutate, and end a session through an injected
  `SessionStore`.
- Maintain a validated message tree with stable ids, parent links, sequence
  order, content validation, and explicit delete rules.
- Select one caller-chosen root-to-leaf branch for model context.
- Keep required anchors and evidence inside the context budget.
- Omit droppable history only with an auditable omission record.
- Replace older history with an injected structured summary when compression
  is required.
- Fail closed when required context or a summary cannot fit.

## Boundaries

- **No Host assembly.** The binary layer creates and injects components.
- **No SQLite connection or SQL.** Session persistence is behind the
  `SessionStore` trait. TASK-029 owns the production storage adapter.
- **No platform API calls.** The architecture test rejects platform
  implementation references and dependencies.
- **No policy decisions or tool execution.**
- **No Planner or Memory.** Those remain TASK-207 and TASK-208.
- **No system clock, random source, UUID library, filesystem, or network.**
  Time, ids, content, and persistence are supplied by the caller.
- **No model prompt policy.** The compressor is an injected strategy and may
  later bridge to `model-gateway`.

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
- Session deletion is leaf-only; branch deletion and tombstone semantics are
  not defined by TASK-028.
- The crate does not persist audit events for context omissions. Callers can
  project the returned omission records into audit events at the assembly
  boundary.

## Related Documents

- `docs/spec/core-orchestration.md`
- `docs/adr/0053-core-orchestration-layer-interface.md`
- `cross-platform-ai-assistant-architecture-v2.md` sections 7.1, 7.2, 11.3,
  and 15.
- `tasks/TASK-028-core-session-context.md`
