# TASK-270　修复 model-gateway 无 fallback 错误被覆盖（DRIFT-269-1）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`crates/model-gateway/src/gateway.rs`、`crates/model-gateway/tests/retry_fallback_cancel.rs`、`tasks/TASK-270-model-gateway-error-preservation.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`docs/memory/pitfalls.md`、`MEMORY.md`、`docs/automations/2026-10-11-round-8.md`
- **关联**：`DRIFT-269-1`、`crates/model-gateway/README.md` 不变量 7、`docs/spec/error-codes.md`

**目标**

修复 `GatewayCompletion::start_available_attempt` 在无重试、无 fallback 时丢失原始
`ModelGatewayError` 的缺陷；provider 缺失、能力预检失败或 provider 启动失败必须原样返回，
不得退化为 `InternalInvariant`。

**背景**

`TASK-269` 的最小复现已证明：首候选缺少 `streaming` 能力且 fallback 链为空时，
`handle_stream_failure` 返回的 `CapabilityMissing` 被调用方丢弃，随后读取已取空的
`last_error`，最终错误码从 `ToolInvalidArgs` 变成 `Fatal`。该行为违反
`crates/model-gateway/README.md` 不变量 7「No silent capability substitution」。

**步骤**

1. 在 `retry_fallback_cancel.rs` 增加 capability 缺失且无 fallback 的回归测试。
2. 如回归测试先红，确认失败错误确为 `InternalInvariant`。
3. 修改 `start_available_attempt`，让三个失败分支直接返回 `handle_stream_failure` 的错误。
4. 重跑专项测试与全部现行硬门禁，确认真实错误变体与 `ErrorCode` 均保持稳定。

**DoD**

- [ ] 未修改公共 API、trait、schema、`ErrorCode`、依赖或 CI 配置
- [ ] 新增负向测试证明无 fallback 的 `CapabilityMissing` 保持原变体与 `ToolInvalidArgs`
- [ ] `cargo test -p assistant-model-gateway` 全绿
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步
- [ ] `DRIFT-269-1` 在执行记录中明确闭环

**验收命令**

```powershell
cargo test -p assistant-model-gateway test_unfallbackable_capability_missing_preserves_error
cargo test -p assistant-model-gateway
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- memory-counts; cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan; cargo run -p xtask -- docscan; cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger; cargo run -p xtask -- check-comments
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-270 修复无 fallback 能力缺失错误被覆盖
【目标】无重试、无 fallback 时保留 provider / capability 原始错误与稳定 `ErrorCode`
【write scope】`gateway.rs`、`retry_fallback_cancel.rs`、本卡、round 文件与状态 / memory 同步文件
【铁律】无静默失败；失败保留原错误；不扩 scope；热点文件先 guard
【禁止】改公共 API / trait / schema / `ErrorCode`；加依赖；碰 GUI / 网络 / 凭据
【验收】新增负向测试先红后绿；crate / workspace / xtask / deny / arch / replay 全绿
【依赖】`DRIFT-269-1` 已复现；无未完成前置卡
【疑问】无

### 2. 实际改动文件

- `crates/model-gateway/src/gateway.rs`
- `crates/model-gateway/tests/retry_fallback_cancel.rs`
- `tasks/TASK-270-model-gateway-error-preservation.md`
- `LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/memory/facts.md`、`docs/memory/pitfalls.md`、`MEMORY.md`
- `docs/automations/2026-10-11-round-8.md`

### 3. 验收输出摘要

- 修复前专项测试真实失败：
  `expected CapabilityMissing, got InternalInvariant { reason: "capability failure was not retained" }`。
- 修复后 `cargo test -p assistant-model-gateway --test retry_fallback_cancel`：
  **8 passed / 0 failed**；新增测试覆盖无 fallback 的 capability 缺失与 provider 启动失败。
- `cargo test -p assistant-model-gateway`：36 个测试与 1 个 doctest 全绿。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：EXIT 0；clippy 仅保留既有 `unknown lint: clippy::assert_is_empty` 噪声。
- `hygiene` scanned=402 / 0E / 128W / PASSED；`check-comments` 0E / 73W；
  `refscan` scanned=812 / 0E / 0W；`docscan` scanned=389 / 0E / 273W；
  `card-check` scanned=165 / 0E / 34W；`adr-index` scanned=76 / 0E / 0W；
  `verify-schemas` 5/5、`codegen --check` 0 drift、`check-migrations` 5/5。
- `cargo deny check` PASSED；`cargo test -p assistant-core arch::` 10 passed；
  `cargo run -p xtask -- replay --suite core` 2 steps / 5 changes / PASSED。
- `cargo llvm-cov --workspace --fail-under-lines 75`：行覆盖率 **79.01%** / PASSED。
- PR #318：pull_request CI run `38163000583` = **11/11 SUCCESS**；合并前
  base=`main`、mergeable=`MERGEABLE`、mergeStateStatus=`CLEAN`；merge
  `dbeb645`；post-merge main push CI run `38163555544` = **11/11 SUCCESS**。

### 4. DoD 逐条核对

- 公共 API / trait / schema / `ErrorCode` / 依赖 / CI 零改动：满足。
- 无 fallback capability 缺失保留原错误变体与 `ToolInvalidArgs`：满足，另有 provider
  启动失败保留 `ServerError` / `ModelNetworkFailure` 的回归测试。
- crate、workspace、xtask、deny、arch、replay 验收：满足，见 §3。
- 热点文件 guard：满足；本批 acquire 后编辑，随后 release。
- 状态同步：实现提交按实写 **Review**；本回填提交把卡面推进为 **Done**，并同步
  PLAN / README / plans / LEDGER / round 文件。
- `DRIFT-269-1`：满足，代码路径与负向回归均已闭环。

### 5. 偏差

- 无功能偏差；`DRIFT-269-1` 已由本卡闭环。
- 宿主机 PowerShell 5.1 启动再次零输出卡住，按既有逃生路径改用 `cmd.exe` +
  WSL 文本工具；已记录到 round 文件。

### 6. 更合理做法

- 三个失败分支统一改走 `self.handle_stream_failure(...)?`，既保留原错误，也删除了
  二次 `last_error.take()` 造成的覆盖窗口，并让后续新增失败源默认继承该契约。

### 7. 遗留问题

- 按 `TASK-269` 的历史命令复跑 crate 级 `--fail-under-lines 85` 本次得到 80.66%，
  与历史记录的 93.78% 不可复现；workspace 软门禁 75% 已通过，且本卡硬门禁全绿。
  未在本卡扩大范围修正覆盖率，建议下一轮先对 `origin/main` 复测再决定是否开卡。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录无 fallback 错误保留修复及 red→green 证据。
- `docs/memory/pitfalls.md`：记录“错误已由子树取走，调用方再次 take 会覆盖原错误”的模式。

### 9. 给审阅者的关注点

1. 三个 `start_available_attempt` 失败分支是否都保留原错误。
2. 两条负向测试是否同时证明错误变体与稳定 `ErrorCode`。
3. crate 级覆盖率历史数字不可复现的观察是否需要另开治理卡。
