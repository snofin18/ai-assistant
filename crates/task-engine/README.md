# assistant-task-engine

> Stage 1 task orchestration: 12 task states, Step DAG scheduling, checkpoints,
> crash recovery, cancellation, budgets, and watchdogs.

## Responsibilities

- Enforce the task state machine from architecture v2 section 8.1.
- Enforce the step lifecycle from `Pending` through `Committed`, including
  policy denial, retry, rollback, and skip dispositions.
- Validate Plan / Step DAGs and return a deterministic ready frontier.
- Persist a complete checkpoint before returning every successful transition.
- Restore a task from its latest checkpoint and apply externally supplied
  recovery evidence.
- Apply pause, cancellation, takeover, budget, and watchdog transitions.

## Boundaries

- Does not resolve targets or call any platform API.
- Does not execute tools, verify postconditions, acquire leases, or manage undo
  anchors. Those belong to Host, `verify`, `lease`, and `undo`.
- Does not request approval or execute the model loop. Those belong to `hitl`
  and `core`.
- Does not own database schema and does not write SQL. The SQLite adapter uses
  only public `assistant-storage` record APIs.
- Does not read wall-clock time or create background timers. Callers inject
  timestamps.

## Invariants

1. **No guessed recovery**: `Executing` or `Verifying` work after a crash needs
   explicit `Completed`, `NotCompleted`, or `Unknown` evidence. Missing or
   unknown evidence produces `NeedsHuman`.
2. **No implicit transition**: an illegal `(state, event)` pair is rejected.
3. **Persist before expose**: the next checkpoint is committed before the new
   snapshot is returned. A failed checkpoint write leaves the previous
   persisted snapshot authoritative.
4. **Write steps declare postconditions**: `StepEffect::Write` requires at
   least one postcondition.
5. **`L3Irreversible` is point-of-no-return**: such a step must set
   `point_of_no_return = true`.
6. **Bounded execution**: steps, elapsed time, tokens, and cost are checked
   against the task budget. Exhaustion is persisted as `NeedsHuman`; it is not
   silently retried.

## Typical Use

```rust
use std::collections::BTreeMap;

use assistant_task_engine::{
    MemoryCheckpointStore, RecoveryEvidence, StepId, TaskEngine, TaskId,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
let task_id = TaskId::new("t_1")?;
let step_id = StepId::new("s_1")?;
let evidence = BTreeMap::from([
    (step_id, RecoveryEvidence::NotCompleted),
]);
let assessment = engine.recover(&task_id, &evidence, 2_000)?;
assert_eq!(assessment.snapshot.task_id, task_id);
# Ok(())
# }
```

## Checkpoint Stores

- `MemoryCheckpointStore` is deterministic and intended for unit tests,
  replay, and pure state-machine exercises.
- `SqliteCheckpointStore` appends snapshots to the `checkpoints` table through
  `assistant-storage`. It creates the initial `tasks` row and checkpoint in one
  SQLite transaction.

## Known Limitations

- `assistant-storage` currently exposes insert/load functions for task rows but
  no task-row update. The latest status, usage, and state therefore live in the
  newest JSON checkpoint; the `tasks` row keeps creation-time metadata. A later
  card should add a storage-owned update API if UI queries need current status
  without reading checkpoints.
- Plan JSON is stored inline in the task row. The `> 64 KiB -> blob` rule from
  `docs/storage-design.md` is not wired yet.
- Recovery evidence is a caller-supplied classification. Producing that
  classification from fingerprints and business markers belongs to `verify`
  and later Core integration.
- Lease acquisition and rollback execution are deliberately outside this
  crate; recovery only returns `Resuming`, `Completed`, or `NeedsHuman`.

## Related Documents

- `cross-platform-ai-assistant-architecture-v2.md` sections 8.1 through 8.5 and
  8.9.
- `docs/storage-design.md` sections 3.1, 4, 6, and 8.
- `docs/spec/error-codes.md`.
- `tasks/TASK-022-task-engine-state-machine-dag-checkpoint.md`.
