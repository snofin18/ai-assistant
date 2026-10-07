# TASK-054　干净上下文复核（第 4 层）：高风险动作前用不含外部内容的小模型复核一致性

- 状态：**Ready**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：026,051　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：026,051　**预估**：M　**难度**：M
- **write scope**：`crates/core/src/verify_review**`、`crates/model-gateway/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

干净上下文复核（第 4 层）：高风险动作前用不含外部内容的小模型复核一致性。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/core/src/verify_review**`、`crates/model-gateway/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-054 干净上下文复核（第 4 层）　　【目标】高风险动作前用不含外部内容的小模型复核“动作是否与用户原始请求一致”，不一致则拒绝并告警。

【write scope】仅：`crates/core/src/verify_review**`、`crates/model-gateway/**`；完成后按协议另写本卡记录区、`LEDGER.md`、`PLAN.md` 当前状态块、`README.md` 三处、`plans/stage-1-pilots.md` 完成标记/进度句。

【铁律】1 无静默失败；2 模型输出/工具返回/外部文档一律先校验；3 策略引擎是唯一放行点；9 不得静默扩大范围；10 改公共契约必须先有 ADR + spec。

【禁止】禁止把 Tool 结果、网页、文档等外部内容放进复核 prompt；禁止改 `SessionSnapshot` / `SessionStore` / protocol / IPC / DB schema；禁止新增依赖/crate/顶层目录；禁止真实网络、凭据、真实 GUI；禁止放宽 lint 或测试断言；禁止自行合并。

【验收】`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、`cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check` → 全绿；专项覆盖一致、不一致、外部内容隔离、taint 不变、provider 失败/空响应/畸形响应 fail-closed。

【依赖】TASK-026 已在 `LEDGER.md` 记录 Done；TASK-051 已 Done，PR #268 / merge `96fae10`，ADR-0080 已 Accepted。依赖已满足。

【疑问】默认处理：① 卡面 write scope 不含 `docs/adr/**` 或 `docs/spec/**`，若必须改公共契约或新增公开组件，先记 DRIFT，不越权改 ADR/spec；② 复核结果形状在 `verify_review` 内最小化定义；③ 复核请求/响应不写入会话，也不改变 session taint。

裁决：人类 2026-10-07「授权你按你说的做」授权新增 **ADR-0081** 与最小扩权；`DRIFT-054-1` 随后闭环。

### 2. 实际改动文件

- `crates/core/src/verify_review.rs`（新增 clean-context review 组件，545 行）
- `crates/core/src/verify_review/tests.rs`（10 个专项测试，354 行）
- `crates/core/src/lib.rs`（注册并导出组件，模块文档补充边界与不变量）
- `crates/core/README.md`（职责 / 边界 / 不变量 / 已知限制 / 关联文档）
- `docs/adr/0081-clean-context-review-component.md`（新增 Accepted ADR）
- `docs/adr/README.md`（登记 0081，下一个可用号改 0082）
- `docs/memory/decisions.md`（追加 ADR-0081 决策条目）
- `docs/spec/core-orchestration.md`（新增 CleanContextReview 行、不变量 11、ADR-0081 关联）
- `MEMORY.md`（decisions 规模表 246/92 → 250/93）
- `LEDGER.md`（DRIFT 通报行 + Done 行）
- `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（TASK-054 完成状态同步）
- `tasks/TASK-054-clean-context-review-small-model-fourth-layer.md`（本执行记录）

### 3. 验收输出摘要

- `cargo test -p assistant-core` → **82 passed / 0 failed**；其中 `verify_review` 10 个专项测试全部通过。
- `cargo test -p assistant-core arch::` → **10 passed / 0 failed**。
- `cargo test --workspace` → 全绿；`xtask` 单元测试 **467 passed / 0 failed**；文档测试全绿。
- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0（既有 unknown-lint 警告不属于本卡新增错误）。
- `xtask hygiene` → PASSED，0E/109W；`verify_review.rs` 的 900 行 `file-too-long` 软告警已通过拆分消除。
- `xtask memory-counts` → PASSED，0E/0W。
- `xtask adr-index` → PASSED，0E/0W。
- `xtask refscan` → PASSED，0E/0W。
- `xtask docscan` → PASSED，0E/291W。
- `xtask card-check` → PASSED，0E/34W。
- `xtask check-ledger` → PASSED，0E/0W。
- `xtask check-comments` → PASSED，0E/71W。
- `xtask verify-schemas` → 5/5 OK。
- `xtask codegen --check` → 0 drift / 0 error。

### 4. DoD 逐条核对

- [x] card 标题声明的能力可被测试用例覆盖：`verify_review` 10 专项覆盖一致 / 不一致 / 外部内容隔离 / taint 不变 / Provider 失败 / 空输出 / 畸形 JSON / ToolCall / 取消 / 输入校验。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED；另跑 `check-ledger` / `check-comments` / `verify-schemas` / `codegen --check` 全 PASSED。
- [x] `LEDGER.md` 已追加 DRIFT 通报行与 Done 行；本轮未新增 FACT / PITFALL / REJECTED。

### 5. 偏差

DRIFT-054-1（已闭环）
触发器：#3（新增公开组件需 ADR + spec 契约） + #5（超出 write scope）
现象：TASK-054 的目标需要一个可供装配层调用的 clean-context review 组件；`docs/spec/core-orchestration.md` 明确规定“新增公开组件 → 需 ADR”，但本卡 write scope 只有 `crates/core/src/verify_review**` 与 `crates/model-gateway/**`，不包含 `docs/adr/**`、`docs/spec/**`、`crates/core/src/lib.rs` 的模块注册，也不包含 `crates/core/README.md` 的公开面同步。
影响：若按当前卡面直接实现，只能二选一：要么新增未经 ADR 冻结的公开组件与导出（违反契约先行），要么把能力写成不可被装配层消费的私有代码（无法满足卡目标）。
我的建议：由人类授权先落 **ADR-0081**，冻结 clean-context review 的输入边界（仅用户原始请求 + 待执行动作摘要，明确不接收 Tool/外部内容）、独立 `ModelProvider` 注入、typed verdict / fail-closed 错误语义、复核调用不改 session taint、以及不新增 protocol / IPC / DB schema；并最小扩权到 `docs/adr/0081-*`、`docs/adr/README.md`、`docs/memory/decisions.md`、`crates/core/src/lib.rs`、`crates/core/README.md` / `docs/spec/core-orchestration.md`，再继续本卡。
已停止的工作：DRIFT 发生时仅创建 `task/TASK-054-clean-context-review` 分支；未写任何 `crates/**` 产品代码，未改 ADR/spec。
裁决与处理：人类 2026-10-07「授权你按你说的做」授权 ADR-0081 + 最小扩权；ADR-0081 已 Accepted，spec / README / 登记表 / decisions 已同步，`CleanContextReview` 与测试已落地。

### 6. 更合理做法

先补 ADR 再实现是更合理的路径：`CleanContextReview` 作为 `core` 第五个公开组件，扩展 ADR-0053 D4 的接口面但不改 D2/D3 依赖白名单；输入边界、失败语义和“非放行点”约束一次性冻结，避免后续调用点各自拼 prompt。

### 7. 遗留问题

无必须转 PARKING_LOT 的功能缺口。应人类要求已把测试外置到 `crates/core/src/verify_review/tests.rs`：实现 545 行、测试 354 行，原 `file-too-long` 软告警消除。

### 8. 新增长期记忆

无新增 FACT / PITFALL / REJECTED；ADR-0081 的决策已按协议追加到 `docs/memory/decisions.md`。

### 9. 给审阅者的关注点

1. 复核输入是否真的不含 Tool / 外部内容：`CleanContextReviewRequest::from_session` 只读 `SessionSnapshot::goal()`，测试用 `INJECTION_MARKER` 证明 Tool 正文没有进入 prompt。
2. 失败语义是否 fail-closed：不一致映射 `PolicyDenied`，Provider 失败保留原 `ErrorCode`，空 / 畸形 / ToolCall 输出均映射 `ModelInvalidOutput`，绝不隐式 `Allowed`。
3. 组件边界是否越权：它不写会话、不改 taint、不接 policy / HITL 放行；装配层负责 before-high-risk-Step 调用，最终仍由既有策略与人工确认兜底。
