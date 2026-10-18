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
5. `element_bounds` is replayed only from recorded node bounds; missing nodes,
   non-positive rectangles, and `i64 -> i32` overflow fail closed.
6. Replay never pretends an unrecorded write action succeeded.
7. `xtask replay --suite core` may consume version 2 sequence fixtures for
   deterministic tree diffing; the provider API in this crate remains the
   version 1 single-snapshot offline provider.

相关：架构 v2 §17.4、`docs/spec/testing.md`、`tasks/TASK-034-record-replay-framework-xtask-replay.md`。

## 已知限制

- **只回放读操作。** 写操作在 v0 不被回放，一律返回 `CapabilityMissing`，不伪造成功。
- **provider API 只认 schema 版本 1 的单快照录制。** 版本 2 的序列录制只由
  `xtask replay --suite core` 消费，本 crate 的离线 provider 不读它。
- **不做真实桌面采集，也没有录制器 GUI。** 录制内容必须由外部工具产出；
  本 crate 只校验并回放已经存在的录制。
- **不做树级 diff。** 两次录制之间的树级比对在 `xtask` 侧（Recording v2 序列），
  本 crate 只提供单快照的确定性回放。
