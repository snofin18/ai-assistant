# assistant-replay

## 职责

- Define the versioned recording format for UI tree snapshots and recorded read outcomes.
- Validate recordings fail-closed before they are used by tests.
- Provide offline `UiAutomationProvider` / `WindowProvider` implementations backed by recordings.
- Keep platform API types as the only public platform contract.

## 边界

- Does not capture from a real desktop or implement a recorder GUI.
- Does not drive applications.
- Does not modify `crates/platform/api` or add a second platform data model.
- Write actions are not replayed in v0; they return `CapabilityMissing`.

## 不变量

1. Recording schema version must be `1`.
2. Node handles are unique, parents exist, and the parent graph is acyclic.
3. Recorded text references must point to an existing node.
4. Element resolution follows selector-chain order and fails closed on ambiguity.
5. Replay never pretends an unrecorded write action succeeded.
6. `xtask replay --suite core` may consume version 2 sequence fixtures for
   deterministic tree diffing; the provider API in this crate remains the
   version 1 single-snapshot offline provider.

相关：架构 v2 §17.4、`docs/spec/testing.md`、`tasks/TASK-034-record-replay-framework-xtask-replay.md`。
