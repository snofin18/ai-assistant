# TASK-225　合成输入目标租约独占

- 状态：**InProgress（2026-10-03：承接 TASK-040 唯一遗留 DoD / PL-101）**
- 阶段：1　子阶段：**1b**　批次：A5-REMEDIATION / 1b 桥接　依赖：018、025、040（真机校准部分已完成）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线以上是正文（Orchestrator 所有，Implementer 只读）；以下是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

## 目标（一句话）

在 Host 装配层复用 `assistant-lease`，使 L4 `pointer_action` / `key_action` 在真正发送前必须持有目标窗口的独占租约；租约冲突显式返回既有 `ErrorCode`，成功与失败路径都释放，只读路径不受影响。

## 背景

TASK-040 已补齐拖拽原子性、坐标校准和真机点击命中；其唯一未闭环 DoD 是跨层目标租约独占，因超出原卡 write scope 留在 `PL-101`。`crates/lease` 已由 TASK-025 提供进程内 exclusive lease、TTL、冲突错误和显式释放。本卡只做 binary 装配层接线，不重新设计租约。

## write scope

- `tasks/TASK-225-synthetic-input-target-lease.md`（本卡记录区可写）
- `apps/agent-core/src/**`
- `apps/agent-core/tests/**`
- `apps/agent-core/Cargo.toml`（仅加入既有 workspace crate `assistant-lease` 的直接依赖）
- `crates/lease/**`（仅在确有必要且不改公共形状时）
- `docs/PARKING_LOT.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`MEMORY.md`（按 AGENTS.md §11 和 ADR-0028 的锁协议同步状态）

## In scope

- 在 `apps/agent-core` 的 Notepad Host 输入调用点前获取目标窗口 exclusive lease。
- `key_action` 的 window / element 目标统一映射到窗口级 lease key；`pointer_action` 使用调用方已解析的窗口 key。
- 冲突复用 `crates/lease` 既有 `Transient` 映射；不新增 `ErrorCode`。
- 成功、平台失败、租约释放失败三条路径均显式处理。
- fake / unit 证据覆盖：两任务争抢同一目标时后者失败；前者释放后后者成功；失败路径租约表回基线。

## Out of scope

- 不改 `crates/platform/api/**` 公共 trait、`ErrorCode`、schema、IPC 或 Tool schema。
- 不新增 crate、顶层目录或第三方依赖。
- 不把 lease 逻辑放进 `crates/platform/windows`。
- 不实现跨进程 / 持久化 lease、等待队列、UI 排队或用户抢占 UI。
- 不改既有卡分界线以上正文，不顺手重构无关模块。

## 必须遵守

- 铁律 1：无静默失败；租约冲突、平台失败、释放失败都必须可观测。
- 铁律 3：本卡不新增 policy 放行点；租约只在 Host 输入边界执行，policy 仍先于执行。
- 铁律 9 / 10：不扩公共形状；需要改公共 trait、schema 或依赖分类时停止并记 DRIFT。
- ADR-0028：热点文件逐个 `guard acquire` / `release`；拿不到锁就记 LEDGER 并只做不涉锁部分。
- ADR-0056 D3 / D7：执行顺序与 cancel / takeover 语义不由本卡改写；本卡只补 Lease 边界。
- ADR-0063：租约管理器必须有界；本卡复用 TASK-025 的进程内管理器，新增状态不随历史无界增长。
- 只读路径（`read_text`、`fingerprint`、窗口枚举）不获取写租约。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo test -p assistant-agent-core synthetic_input_lease
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
```

## DoD

- [ ] `key_action` 在平台发送前取得目标窗口 exclusive lease；无租约不发送。
- [ ] pointer 输入使用同一租约 gate；当前生产链无 pointer 调用时仍有专项契约测试覆盖。
- [ ] 两任务争抢同一目标时后者显式失败，协议码稳定为既有 `Transient`，不是 panic / 静默继续。
- [ ] 前者释放后后者可成功；失败路径释放后租约表回到基线。
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿。
- [ ] `cargo test -p assistant-core arch::` 与 TASK-225 专项测试全绿。
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check` 全绿。
- [ ] LEDGER、PLAN、README、plans、MEMORY 规模表按实际结果同步；PL-101 / TASK-040 的记录区按证据更新。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-225 合成输入目标租约独占
【目标】在 apps/agent-core 装配层复用 assistant-lease，使 L4 key_action / pointer_action 发送前持有目标窗口 exclusive lease，冲突稳定返回既有 Transient，成功与失败路径都释放，只读路径不变。
【write scope】仅：新建 tasks/TASK-225-*.md、apps/agent-core/src/**、apps/agent-core/tests/**、crates/lease/**（仅在不改公共形状时）、docs/PARKING_LOT.md、状态同步文件；另需最小机械接线 apps/agent-core/Cargo.toml，加入既有 workspace crate assistant-lease 的直接路径依赖。
【铁律】1 无静默失败；3 policy 仍是唯一放行点；7 core 不碰平台 API；9 不静默扩范围；10 契约先行；ADR-0056 D3 / D7、ADR-0028、ADR-0063。
【禁止】新增 crate / 第三方依赖；改 crates/platform/api/** 公共 trait、ErrorCode、schema；放宽 lint 或新增 #[allow]；把 lease 逻辑塞进 crates/platform/windows；改既有卡分界线以上正文；顺手重构。
【验收】fmt / clippy / workspace tests / arch / TASK-225 专项 / xtask 十项门禁。
【依赖】TASK-025 Done；TASK-040 真机校准已完成但跨层 lease 仍 WIP；ADR-0056 Accepted；main 干净。
【疑问】agent-core 当前没有生产 pointer_action 调用点；默认实现共享 TargetLeaseGate 契约测试覆盖 pointer-shaped 路径，并让全部现有 key 输入走同一 gate。Cargo.toml / Cargo.lock 为直接依赖机械接线。
```

### 2. 实际改动文件

- `apps/agent-core/Cargo.toml`、`Cargo.lock`：加入既有 workspace crate `assistant-lease` 直接依赖；无第三方依赖、无新 crate。
- `apps/agent-core/src/target_lease.rs`：新增 `TargetLeaseRegistry` / `TargetLeaseGate`，复用 `assistant-lease` 的 exclusive / TTL / 冲突 / 显式 release，并保留既有错误码映射。
- `apps/agent-core/src/notepad_input.rs`：把 `key_action` 的窗口/元素目标统一映射为窗口级 lease key；provider 调用在 gate 内执行。
- `apps/agent-core/src/notepad_handlers.rs`、`notepad_registry.rs`、`notepad_rollback.rs`、`production.rs`、`production_tests.rs`：把 task id 接到 save / save_as / rollback undo；`ProductionConfig` 克隆共享同一 registry。
- 本卡、TASK-040 记录区、`docs/PARKING_LOT.md`、`docs/memory/{facts,pitfalls}.md`、`MEMORY.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`。

### 3. 验收输出摘要

```text
cargo test -p assistant-agent-core target_lease -- --nocapture
  target_lease::tests::test_two_tasks_conflict_then_release_allows_retry ... ok
  target_lease::tests::test_failed_operation_releases_lease ... ok
  target_lease::tests::test_conflicting_operation_is_not_invoked ... ok
  target_lease::tests::test_pointer_operation_uses_the_same_exclusive_gate ... ok
  target_lease::tests::test_two_task_gates_share_the_registry_and_conflict ... ok
  test result: ok. 5 passed; 0 failed

cargo test -p assistant-agent-core production_config_clones -- --nocapture
  test_production_config_clones_share_one_lease_registry ... ok
  test result: ok. 1 passed; 0 failed

cargo test --workspace
  finished with EXIT 0; agent-core lib 50 passed / main 2 passed / production_root 14 passed / runtime_toolbus 6 passed

cargo test -p assistant-core arch::
  arch_dependencies 5 passed; arch_layering 5 passed

cargo fmt --all --check
  EXIT 0

cargo clippy --all-targets -- -D warnings
  EXIT 0（仅既有 unknown-lint `clippy::assert_is_empty` warning）

cargo run -p xtask -- hygiene
  machine-summary: hygiene: scanned=338 errors=0 warnings=107 verdict=PASSED

cargo run -p xtask -- memory-counts / adr-index / refscan / check-ledger
  each: 0 error(s), 0 warning(s), PASSED

cargo run -p xtask -- docscan / card-check / check-comments
  docscan 0E/342W PASSED；card-check 0E/27W PASSED；check-comments 0E/69W PASSED

cargo run -p xtask -- verify-schemas / codegen --check
  verify-schemas 0 error(s) PASSED；codegen 0 drift(s), 0 error(s) PASSED

PR #196 CI run 37106343253
  11/11 SUCCESS; merge commit 0fe4bdb; mergeStateStatus=CLEAN; baseRefName=main
```

`TargetLeaseGate::run` 的冲突分支在 provider 闭包前返回，测试断言第二个 task 的 operation 未执行；成功与失败分支后的新增 acquisition 均成功，直接证明 lease 表回到基线。

### 4. DoD 逐条核对

- [x] `key_action` 在平台发送前取得目标窗口 exclusive lease；当前 save / save_as / rollback undo 全部走 `TargetLeaseGate::run`。
- [x] pointer-shaped 路径使用同一 exclusive gate；生产链当前无 pointer 调用，专项测试覆盖冲突与释放。
- [x] 两任务争抢同一目标时后者显式失败，`ToolBusError::error_code() == ErrorCode::Transient`，不是 panic / 静默继续。
- [x] 前者释放后后者成功；失败与冲突路径均断言租约表回到基线。
- [x] `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` 全绿。
- [x] `cargo test -p assistant-core arch::` 与 TASK-225 专项测试全绿。
- [x] `xtask` 门禁见 §3；`hygiene` 为 PASSED（新增软 warning 已在 §5 如实记录）。
- [x] 状态同步文件与 PL-101 / PL-107 已更新；merge hash 回填阶段另记。

### 5. 偏差

- scope 机械接线：Rust 直接依赖要求修改 `apps/agent-core/Cargo.toml` 与锁文件，虽未逐字列入派单 scope，但只加入既有 workspace crate，未新增 crate / 第三方依赖 / 公共 trait。
- 当前生产链没有 pointer_action 调用点，故 pointer 的生产接线以共享 gate 契约和专项测试覆盖；不做平台层越权改动。
- `hygiene` 新增 `notepad_handlers.rs` / `production.rs` 的软超长 warning；`0 error` 不变，未改规则或放宽判据。

### 6. 更合理做法

先把 lease manager 提升为 `ProductionConfig` 可克隆共享的 registry，再把单个 input action 的 gate 放在 Host 装配层；这样每个 task host 共享一张表，同时保留 `crates/lease` 纯逻辑、`crates/platform/*` 无 lease 依赖的边界。

### 7. 遗留问题

- TASK-040 正文 DoD 复选框因正文只读无法勾选；其跨层 lease 工程项已由本卡闭环，`PL-101` 已在 PARKING_LOT 标记关闭。
- `PL-107` 记录 TASK-224 的 ADR 编号误写，待 Orchestrator 更正。
- 没有实现跨进程 / 崩溃恢复 lease；该边界仍属 `crates/lease` README 的已知限制。

### 8. 新增长期记忆

- FACT：目标 lease 在 binary 层共享 registry，synthetic input 每个 action fail-closed。
- PITFALL：每个 `ProductionHost` 各自 new manager 无法阻止跨 task 冲突。

### 9. 给审阅者的关注点

- 审阅 `ProductionConfig::clone` 后共享同一 `TargetLeaseRegistry` 是否满足后续多 task 装配方式。
- 审阅 `key_action` 的窗口级 key 映射是否会过度串行化同一窗口内的独立输入。
- 审阅 `pointer_action` 尚无生产调用点这一事实是否需要在后续卡补入口；当前没有用 `#[allow]` 隐藏死代码。
