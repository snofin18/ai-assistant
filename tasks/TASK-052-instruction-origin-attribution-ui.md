# TASK-052　来源归因：每个动作记录 `instruction_origin`（user_request/plan_derived/app_content/tool_suggestion）+ UI 展示

- 状态：**Ready**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：027,030　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：027,030　**预估**：M　**难度**：M
- **write scope**：`crates/core/src/origin**`、`apps/desktop-ui/src/features/approval/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

来源归因：每个动作记录 `instruction_origin`（user_request/plan_derived/app_content/tool_suggestion）+ UI 展示。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/core/src/origin**`、`apps/desktop-ui/src/features/approval/**`

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

【任务】TASK-052 来源归因　　【目标】为动作提供 canonical `instruction_origin` 归因值，并让审批 UI 展示四类来源，`app_content` 默认拒绝。

【write scope】最小扩权后：`crates/core/src/origin**`、`apps/desktop-ui/src/features/approval/**`，加 `docs/adr/0082-*`、`docs/adr/README.md`、`docs/memory/decisions.md`、`docs/spec/core-orchestration.md`、`crates/core/src/lib.rs`、`crates/core/README.md`，以及状态同步文件。

【铁律】1 无静默失败；2 不可信输入先校验；3 policy 是唯一放行点；9 不静默扩范围；10 公共契约先有 ADR。

【禁止】不改 `PlanStep` / `PolicyDecision` / protocol / audit-event schema / IPC / DB；不新增依赖或 crate；不把 `app_content` 的 UI 默认拒绝实现成策略放行；不操作真实 GUI。

【验收】`cargo test -p assistant-core origin::`、`pnpm --dir apps/desktop-ui test`、`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、xtask 门禁全绿。

【依赖】TASK-027 / TASK-030 均已在 `LEDGER.md` 记录 Done。

【疑问】默认处理：ADR-0082 只冻结 core 归因值 / 记录的纯模型与 UI token 一致性；把归因写入 audit / `ApprovalRequest` 的跨层 schema 变更留给后续卡。

### 2. 实际改动文件

- `crates/core/src/origin.rs`（新增 `InstructionOrigin` / `InstructionAttribution` / `OriginError`）
- `crates/core/src/lib.rs`（注册并导出 origin 模块）
- `crates/core/README.md`（职责 / 边界 / 不变量 / 已知限制）
- `apps/desktop-ui/src/features/approval/approvalModel.test.mjs`（token 契约和 `plan_derived` / `tool_suggestion` 展示测试）
- `docs/adr/0082-instruction-origin-attribution.md`（新增 Accepted ADR）
- `docs/adr/README.md`（登记 0082，下一个可用号改 0083）
- `docs/memory/decisions.md`（追加 ADR-0082 决策条目）
- `docs/spec/core-orchestration.md`（新增 InstructionOrigin 行、不变量 12、ADR-0082 关联）
- `MEMORY.md`（decisions 规模表 250/93 → 254/94）
- `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`（TASK-052 完成状态同步）
- `tasks/TASK-052-instruction-origin-attribution-ui.md`（本执行记录）

### 3. 验收输出摘要

- `cargo test -p assistant-core origin::` → **7 passed / 0 failed**。
- `pnpm --dir apps/desktop-ui test` → Node model 78 passed + Vitest 3 passed / 0 failed。
- `pnpm --dir apps/desktop-ui typecheck` → EXIT 0。
- `pnpm --dir apps/desktop-ui lint` → EXIT 0。
- `cargo test --workspace` → 全绿；`xtask` 467 passed / 0 failed。
- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0。
- `xtask hygiene` → PASSED，0E/109W。
- `xtask memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments` / `verify-schemas` / `codegen --check` → 全 PASSED。

### 4. DoD 逐条核对

- [x] card 标题声明的能力可被测试用例覆盖：core 覆盖四类 token、非法 token、元数据组合、`app_content` 高风险；UI 覆盖 token 契约与展示路径。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED；另跑 `check-ledger` / `check-comments` / `verify-schemas` / `codegen --check` 全 PASSED。
- [x] `LEDGER.md` 追加一行；ADR-0082 决策已追加 `docs/memory/decisions.md`。

### 5. 偏差

none。人类已预先授权 ADR-0082 + 最小扩权；本卡没有新增实现偏差。

### 6. 更合理做法

采用 **core 纯模型 + UI token 一致性测试**，而不是直接改 `PolicyDecision` / `ApprovalRequest` / audit schema：这样先冻结四类来源和归因校验，不把提示注入来源与权限放行耦合，也不触发 protocol / DB 迁移。

### 7. 遗留问题

跨层写入归因（audit event / IPC / `ApprovalRequest`）仍待后续 ADR + schema 卡；本卡明确不把 `is_high_risk` 当作权限决策。

### 8. 新增长期记忆

- [2026-10-07][DECISION][src:ADR-0082] 指令来源归因采用 core 纯模型，四类稳定 token、按来源校验元数据，`app_content` 仅作高风险信号。（原文见 `docs/memory/decisions.md`）

### 9. 给审阅者的关注点

1. token 集合是否与架构 v2 §10.5 和 UI approval 完全一致，且未知 token 不做默认回退。
2. `plan_derived` / `app_content` / `tool_suggestion` 的元数据约束是否 fail-closed。
3. `app_content` 是否只作为信号，UI 仍默认拒绝并需要显式覆盖，policy / HITL 的放行权没有被本模块绕开。
