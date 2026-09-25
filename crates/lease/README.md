# assistant-lease

Target leases for architecture v2 section 8.8.

## Responsibilities

- Validate lease keys and owners before they enter manager state.
- Model `shared`, `intent`, and `exclusive` lease modes.
- Enforce at most one writer per target while allowing compatible readers and planners.
- Apply caller-injected timestamps for TTL expiry and renewal.
- Force-release agent leases when a user takes over a target.
- Acquire multiple leases in canonical key order with all-or-nothing commit.

## Boundaries

- **No platform access.** Target resolution, window handles, UIA, Win32, and process access stay
  outside this crate.
- **No background timer.** The manager never reads wall-clock time. Callers inject `now_ms` and can
  call `reap_expired` for an explicit sweep.
- **No policy decision.** Whether an operation is allowed is decided by `crates/policy`.
- **No persistence or audit writes.** Lease values are in-memory only at this stage.
- **No queue or UI.** A conflict is a typed error; scheduling a visible wait queue belongs to a
  later Host/UI integration.

## Invariants

1. **One writer per target.** An `exclusive` lease blocks every other owner's active lease on the
   same key.
2. **Intent is conservative.** Two owners may both hold `intent`, but an `intent` blocks another
   owner's `exclusive` acquisition.
3. **TTL is fail-closed.** A zero TTL is rejected. Expiration occurs when
   `now_ms == expires_at_ms`.
4. **No silent ownership transfer.** Renewal and release require the original owner.
5. **Multi-lease acquisition is atomic.** Requests are sorted by canonical key and fully
   conflict-checked before identifiers are allocated or records are inserted.
6. **User preemption is complete for a key.** Every active agent lease for that key is removed and
   returned; already-expired leases are returned separately.

## Typical Use

```rust
use assistant_lease::{LeaseKey, LeaseManager, LeaseMode, LeaseOwner};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let key = LeaseKey::new("com.microsoft.notepad")?.with_document_id("doc_1")?;
let owner = LeaseOwner::parse("task_1")?;
let mut manager = LeaseManager::new();

let lease = manager.acquire(key.clone(), LeaseMode::Exclusive, &owner, 30_000, 1_000)?;
let renewed = manager.renew(lease.id(), &owner, 30_000, 20_000)?;
assert!(renewed.is_active_at(49_999));

manager.preempt_for_user(&key, 21_000)?;
# Ok(())
# }
```

## Known Limitations

- The manager is process-local and in-memory. Cross-process or crash-recovery persistence requires
  a later storage integration.
- Waiting and fair queueing are not implemented. A conflict returns `ErrorCode::Transient` so the
  caller can wait or reroute.
- Same-owner upgrade is represented by acquiring the stronger mode while retaining the weaker one.
  Atomic downgrade/replacement is outside this card.
- There is no lease-specific `ErrorCode`. Input and ownership failures map to `ToolInvalidArgs`,
  conflicts and expiration map to `Transient`, and clock/overflow failures map to `Fatal`.

## Related Documents

- Architecture v2 section 8.8 and sections 10.6 / S1 / S9 / S12.
- `docs/spec/naming.md` section 7.
- `docs/spec/error-codes.md`.
- `tasks/TASK-025-lease-target-exclusive-shared-intent.md`.
