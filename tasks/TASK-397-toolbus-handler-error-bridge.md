# TASK-397 ToolBus handler failure ErrorCode bridge

- 状态：**Done**
- 阶段：2　子阶段：2b ToolBus error bridge　批次：治理　依赖：TASK-020、TASK-394、TASK-395、ADR-0108

---

## write scope

- `tasks/TASK-397-toolbus-handler-error-bridge.md`
- `tasks/TASK-395-excel-t21-fake-runtime-session-handlers.md`（仅状态行）
- `crates/tool-bus/src/error.rs`
- `crates/tool-bus/src/server.rs`
- `crates/tool-bus/tests/handler_contract.rs`
- `crates/tool-bus/tests/server_contract.rs`
- `crates/tool-bus/README.md`
- `docs/adr/0109-toolbus-handler-error-bridge.md`
- `docs/adr/README.md`
- `docs/memory/decisions.md`
- `MEMORY.md`（仅 `decisions.md` 派生规模行）
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

让受信任的内置 handler 能把既有 `ErrorCode` 无损带回 `ToolEnvelope.error`，不新增错误码、
不伪造 `Mcp` 数字码，也不建立第二套错误类型或桥接层。

## In scope

- 在 `ToolBusError` 增加加法式 `HandlerFailure { code: ErrorCode, message: String }`。
- `ToolBusError::error_code()` 对该变体原样返回 `code`。
- 保持 server 的统一错误信封路径；用 13 类 `ErrorCode` round-trip 测试锁定无损映射。
- 增加 handler 错误经 ToolBus round-trip 后仍保留 code/message 的合同测试。
- 同步 ADR、tool-bus README、decisions、PLAN/README/Stage 2/LEDGER。
- 将 TASK-395 的 `DRIFT-395-1` 状态从公共接口阻塞解除为可继续实现。

## Out of scope

- 不新增、删除或重命名任何 `ErrorCode`。
- 不修改 `ToolHandler`、`ToolOutput`、`ToolSchema`、IPC、DB schema 或 protocol schema。
- 不修改 `crates/tool-bus/src/server.rs` 的错误信封装配路径。
- 不改变 Policy、HITL、draft-07 参数校验或重试策略。
- 不实现 Excel COM runner、ToolBus 生产注册、`tools.json` 或真实 `.xlsx` 验收。

## 必须遵守

1. `HandlerFailure` 只接受 typed `ErrorCode`；禁止 `u16`、字符串错误码或 `Mcp` 数字码冒充。
2. 既有 `ToolBusError` 变体映射保持不变；`DuplicateTool` 等内部缺陷仍按原分类处理。
3. `Display` 必须保留 handler 的 message，不能改成只有 `Debug` 或空文本。
4. 公共 API 变更是本 ADR 的唯一授权范围，不得顺手改其他公共接口。
5. 变更先落 ADR-0109 与任务卡，再落实现和测试。
6. `DRIFT-395-1` 的解决只表示错误桥可用，不代表 TASK-395 或四个 Excel Tool 已完成。

## 验收命令

```powershell
cargo test -p assistant-tool-bus
cargo fmt --all --check
cargo clippy -p assistant-tool-bus --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
git diff --check
```

预期：ToolBus 合同测试与 workspace 全绿；13 类 ErrorCode 经 `HandlerFailure` 无损往返；
没有新增依赖、schema、ErrorCode 或产品 Excel 暴露。

## DoD

- [ ] ADR-0109 Accepted，明确 `HandlerFailure` 是唯一新增 bridge 变体。
- [ ] `error_code()` 对 `HandlerFailure` 返回原 typed code。
- [ ] 13 类 ErrorCode round-trip 测试全绿。
- [ ] handler 错误经 ToolBus 返回失败信封时 code/message 保真。
- [ ] `ToolHandler`、`ToolOutput`、ToolSchema、协议 schema 与依赖零变化。
- [ ] Excel `tools.json` 仍为空，未启动 Excel/COM/GUI。
- [ ] 文档门禁、PR、merge 与 post-merge main CI 回填。
- [ ] TASK-395 状态行与 `DRIFT-395-1` 的解除证据同步。

<!-- ───── 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ─────
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发器 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-397 ToolBus handler failure ErrorCode bridge
【目标】让 handler 用既有 typed ErrorCode 无损返回失败，不建立第二套错误桥
【write scope】仅：本卡、tool-bus error/handler/server contract、tool-bus README、ADR-0109、decisions/MEMORY、Stage 2/PLAN/README/LEDGER；TASK-395 仅状态行
【铁律】单一错误事实源；不可信输入先校验；无静默失败；不改公共 schema；不扩大 scope
【禁止】不新增 ErrorCode；不改 ToolHandler/ToolOutput/ToolSchema/IPC/DB schema；不改 server 错误装配路径；不实现 Excel COM/ToolBus 注册或 tools.json
【验收】tool-bus tests / fmt / clippy / workspace / docs gates / diff-check 全绿
【依赖】TASK-020、ADR-0108、TASK-394、TASK-395；N_auto=0（本人类会话 reset）
【疑问】无；按人类 2026-11-24 批准的方案 A 实施
```

### 2. 实际改动文件

- `crates/tool-bus/src/error.rs`：新增 `HandlerFailure` 变体并映射 typed ErrorCode。
- `crates/tool-bus/src/server.rs`：handler error 继续走唯一信封路径，并对空/纯空白
  `HandlerFailure.message` 使用该 ErrorCode 的 canonical `message_for_model`。
- `crates/tool-bus/tests/handler_contract.rs`：13 类 ErrorCode round-trip 与 message 保真测试。
- `crates/tool-bus/tests/server_contract.rs`：handler failure 经 ToolBus error envelope 的
  13 类 code/message 合同测试与空消息 fallback 测试。
- `crates/tool-bus/README.md`：记录 handler failure 错误桥不变量。
- `docs/adr/0109-toolbus-handler-error-bridge.md`：Accepted ADR。
- ADR 索引、decisions、MEMORY 派生计数、Stage 2、PLAN、LEDGER：治理同步。

### 3. 验收输出摘要

- `cargo test -p assistant-tool-bus` → 全部 unit/contract/doc tests passed；新增
  `handler_contract` 13-code round-trip、`server_contract` 13-code envelope 与空消息
  fallback 用例 passed。
- `cargo fmt --all --check` → PASSED。
- `cargo clippy -p assistant-tool-bus --all-targets -- -D warnings` → exit 0；仅保留仓库既有
  `clippy::assert_is_empty` unknown-lint warning，未放宽判据。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅保留同一既有 unknown-lint warning。
- `cargo test --workspace` → PASSED；xtask 587 passed / 0 failed，workspace 全部测试通过。
- `cargo test -p assistant-core arch::` → arch 10 passed / 0 failed。
- `cargo run -p xtask -- adr-index` → PASSED，98 files。
- `cargo run -p xtask -- card-check` → PASSED，300 files，0 error。
- `memory-counts` 初次因 decisions 行/条数变化报 2 error；已按工具输出更正为 338 / 117。
- 首次新增 server 测试因误写未注册工具名失败；已改为 `demo.ambiguous.run` 后通过。
- 独立 review 的 P2 指出空/纯空白 message 可产生空错误说明，已在 server 信封边界补
  canonical fallback；P3 已通过 server `ErrorCode::ALL` 遍历测试关闭。
- PR #819 checks **22**（21 SUCCESS + 1 SKIPPED），pending 0 / failed 0，base=main，
  `MERGEABLE` / `CLEAN`；显式 squash merge 为
  `2c412d5d1b1d18f15af22675123726b06d28ead1`。
- Post-merge main CI `45305239489` success；commit 与 merge commit 一致。
- 独立 review 最终复核结论为 **no findings**；P2 空消息 fallback 与 P3 全量 ErrorCode 覆盖均已关闭。

### 4. DoD 逐条核对

- [x] ADR-0109 Accepted，`HandlerFailure` 是唯一新增 bridge 变体。
- [x] `error_code()` 对 `HandlerFailure` 返回原 typed code。
- [x] 13 类 ErrorCode round-trip 测试通过。
- [x] handler 错误经 ToolBus 返回失败信封时 code/message 保真。
- [x] `ToolHandler`、`ToolOutput`、ToolSchema、协议 schema 与依赖零变化。
- [x] Excel `tools.json` 仍为空，未启动 Excel/COM/GUI。
- [x] 文档门禁、PR、merge 与 post-merge main CI 回填。
- [x] TASK-395 状态行与 `DRIFT-395-1` 解除证据同步（需在 bridge merge 后做）。

### 5. 偏差

- `DRIFT-395-1` 是本次要解决的公共接口缺失，不是新增功能漂移。
- 首次 server 合同测试使用错误工具名，导致目标查找失败；这是测试夹具错误，已修复后复跑。
- `PROCESS-TASK397-SHELL-MISUSE`：最终权威机器重算为 **56** 次
  （主实现 48 = 47 次 `bash` + 1 次 `powerspace`，独立 review 8 次 `bash`）。
  均为只读、build、CI 或 closeout 查询；项目 hook 实际以 Windows PowerShell 执行，
  没有产品文件越界或外部副作用。

### 6. 更合理做法

- 选择加法式 `HandlerFailure`，复用既有 `error_code()` / server error envelope 单一路径；
  独立错误桥会形成第二套分类和可重试性映射。
- 13 类 code 用 `ErrorCode::ALL` 全量覆盖，避免只测试 ADR-0108 当前用到的几类。

### 7. 遗留问题

- TASK-395 已由 ADR-0109 / TASK-397 解除 `DRIFT-395-1` 并恢复 **Ready**；Windows COM runner、
  ownership journal、独立 watchdog、ToolBus 生产包装与人类在场 `.xlsx` 验收仍属后续实现。

### 8. 新增长期记忆

- 新增 ADR-0109 decision：ToolBus handler 失败通过 typed `HandlerFailure` 无损携带既有
  ErrorCode，不建立第二套错误桥。

### 9. 给审阅者的关注点

- 确认新增变体只走既有 `error_code()` / `Display` 信封路径，没有改变 server 装配。
- 确认 `ErrorCode::ALL` 13 类全部 round-trip，且没有新增 ErrorCode 或 schema。
- 确认 TASK-395 已恢复 Ready，且不把错误桥完成冒充四 Tool、COM runner 或真机验收完成。
