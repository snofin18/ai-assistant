# TASK-354 Browser form approval event and strict UI parser

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 前置
- 依赖：TASK-344、TASK-352、TASK-353
- 本文件 = **卡片正文 + 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。

---

## 目标

把 TASK-352 已校验的 `FormApprovalPreview` 投影成 ADR-0101 定义的
`step_approval_requested` UI 事件，并让 TypeScript 侧对事件做 strict parse 后映射到现有
`ApprovalCardModel`。事件只携带已脱敏字段 diff、已校验来源归因和最小 form origin/action
证据，不携带原始字段值或 `form_fill` evidence。

## In scope

- `apps/agent-core/src/ui_ipc.rs`：新增 `step_approval_requested` 事件及其封闭数据形状。
- `apps/agent-core/src/browser_task_approval.rs`：从 `FormApprovalPreview` 构造该事件。
- `apps/agent-core/src/browser_task_approval_tests.rs`：Core 侧投影、负向边界测试。
- `apps/agent-core/tests/ui_ipc.rs` 与 `apps/agent-core/tests/fixtures/ui_ipc/`：共享黄金样本。
- `apps/desktop-ui/src/ipc/contract.ts` 与测试：zod strict parse。
- `apps/desktop-ui/src/features/approval/**`：事件到审批卡模型的纯函数映射与测试。
- `docs/spec/browser-form-confirmation.md`：冻结事件线上形状。
- 本卡、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/facts.md`、`MEMORY.md`（仅规模表变化时）。

## Out of scope

- 不登记 `PendingApprovals`、不消费 `ApprovalGrants`、不执行 `form_submit`。
- 不接实时 UI event source、不启动真实 Edge、不修改 App 页面布局。
- 不新增 ADR、依赖、数据库 schema、ErrorCode 或通用点击工具。
- 不修改 `form_read` / `form_fill` / `form_submit` Tool schema。
- 不把 `ToolEnvelope.evidence.extra`、原始 before/after 或页面可见文本写入 UI 事件。

## 必须遵守

1. 事件固定包含 `request_id`、`task_id`、`step_id`、`action`、`target`、`summary`、
   `impact`、结构化 field diff、已校验 `instruction_attribution`、`risk`、
   `reversibility`、`scope_options`、最小 form 证据与 `expires_at_ms`。
2. form submit 事件固定为 `risk=high`、`reversibility=l3_irreversible`、
   `scope_options=["once"]`；未知值、缺字段、空 diff、重复 selector 或额外字段一律拒绝。
3. diff 的 before/after 只能来自 TASK-352 已脱敏的 `DiffPreview::Fields`；
   before 与 after 相同、字段名空或数量越界均拒绝。
4. attribution 复用 `InstructionAttribution` 四类 token 的条件规则；
   `app_content` 仍只作为高风险信号，不改变 policy。
5. Core 与 UI 必须解析同一份黄金样本；任何一侧放宽字段都会让另一侧测试失败。
6. UI 映射只生成审批卡模型，不直接调用 IPC、不授权、不触发提交。

## 验收命令

```powershell
cargo test -p assistant-agent-core browser_task_approval
cargo test -p assistant-agent-core --test ui_ipc
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
pnpm --dir apps/desktop-ui build
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
```

## DoD

- [ ] `FormApprovalPreview` 能构造封闭的 `step_approval_requested` 事件，原始字段值不可见。
- [ ] Rust 与 TypeScript 对同一黄金样本 strict parse 成功，非法组合全部 fail-closed。
- [ ] UI 映射生成 high/L3/once 的 `ApprovalCardModel`，过期状态由事件 expiry 决定。
- [ ] 事件中不含 `evidence.extra`、原始 before/after、DOM、title、Cookie 或页面文本。
- [ ] 全部门禁命令通过；TASK-057/058 状态不被本卡改写。

<!-- ══ 分界线：以上是**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下是 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤，
     并会被 xtask card-check 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写，9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-354 Browser form approval event and strict UI parser
【目标】把已校验表单预览投影成 step_approval_requested，并由 UI 严格解析和映射为审批卡
【write scope】仅：agent-core ui/browser approval 与测试、desktop-ui IPC/approval、共享 fixture、本卡与状态同步文件
【铁律】1 无静默失败；2 UI/IPC/工具返回不可信；3 Policy 唯一放行点；6 L3 不可逆只允许 once；9 不扩范围；10 契约先行
【禁止】不登记/消费授权、不执行 form_submit、不接实时事件源、不启动真实浏览器、不改 Tool schema/依赖/DB/ErrorCode
【验收】Core 专项与 UI 黄金样本双向解析；workspace 与 pnpm 全套门禁；文档门禁全绿
【依赖】TASK-344、TASK-352、TASK-353 均已核对为 Done；TASK-057/058 保持 Blocked
【疑问】无；ADR-0101 D6 已授权新增事件，不另立 ADR
```

### 2. 实际改动文件

- `apps/agent-core/src/ui_ipc.rs`：新增 `UiEvent::StepApprovalRequested` 及封闭 diff/attribution/evidence 类型。
- `apps/agent-core/src/browser_task_approval_event.rs`（新增）：从 `FormApprovalPreview` 构造事件，并在 `form_submit` policy 求值前一次性生成。
- `apps/agent-core/src/browser_task_approval.rs`、`browser_task_runtime.rs`：接入事件生成和 recorder 读取，未登记或消费授权。
- `apps/agent-core/src/browser_task_approval_tests.rs`、`browser_task_runtime_tests.rs`、`tests/ui_ipc.rs`：Core 投影、redaction、非法 action、黄金样本与 runtime 专项。
- `apps/agent-core/tests/fixtures/ui_ipc/step_approval_requested.json`（新增）：Rust/TS 共用的审批事件样本。
- `apps/desktop-ui/src/ipc/contract.ts` 与测试：zod strict parse、归因条件、once/high/L3、origin/action 与 expiry 校验。
- `apps/desktop-ui/src/features/approval/browserApproval.ts` 与测试（新增）：事件到 `ApprovalCardModel` 的纯映射。
- `apps/desktop-ui/src/features/approval/approvalModel.ts`、`approvalCopy.ts`、`EvidenceList.tsx`、`index.ts`：支持无外链的 `form_state` 内存证据。
- `docs/memory/facts.md`、`MEMORY.md`：记录事件形状、redaction 与 UI parser 事实。
- `tasks/TASK-354-browser-form-approval-event.md`、`LEDGER.md`：本卡记录与执行台账。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core browser_task_approval`：11 passed。
- `cargo test -p assistant-agent-core browser_task_runtime`：38 passed。
- `cargo test -p assistant-agent-core --test ui_ipc`：13 passed。
- `cargo test --workspace`：510 passed / 0 failed；doctests 全绿。
- `cargo clippy --all-targets -- -D warnings`：exit 0；仅仓库既有 `unknown lint: clippy::assert_is_empty` 提示。
- `pnpm --dir apps/desktop-ui test`：85 model tests + 3 DOM tests passed。
- `pnpm --dir apps/desktop-ui lint / typecheck / build`：全绿。
- `verify-schemas` / `codegen --check` / `hygiene` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments` / `cargo deny check` / `replay core`：全绿。

### 4. DoD 逐条核对

- [x] `FormApprovalPreview` 能构造封闭的 `step_approval_requested` 事件，原始字段值不可见。
- [x] Rust 与 TypeScript 对同一黄金样本 strict parse 成功，非法组合全部 fail-closed。
- [x] UI 映射生成 high/L3/once 的 `ApprovalCardModel`，过期状态由事件 expiry 决定。
- [x] 事件中不含 `evidence.extra`、原始 before/after、DOM、title、Cookie 或页面文本。
- [x] 全部门禁命令通过；TASK-057/058 状态未被本卡改写。

### 5. 偏差

- `DRIFT-354-1`：卡面 In scope 含 `docs/spec/browser-form-confirmation.md`，但 `AGENTS.md` §8 将
  `docs/spec/*` 设为只读。实现中曾试改线上形状说明，发现冲突后已完整撤回；最终 spec 无 diff，
  事件形状由 ADR-0101 D6、`ui_ipc.rs` 与共享黄金样本冻结，不改变产品行为。
- 本轮接续段最终机器统计到显式 `shell="bash"` 误填 **55** 次；均为只读、测试、门禁、guard 或状态
  查询命令，未通过 bash 写入产品文件。后续命令改用显式 PowerShell。

### 6. 更合理做法

- 用一份共享 JSON 同时驱动 Rust 与 zod 解析，避免两侧各自手写断言；事件类型用 boxed diff /
  attribution 控制 `UiEvent` 枚举体积，既保留封闭形状又通过 clippy `large_enum_variant`。
- 将事件生成留在既有 approval event 子模块，`browser_task_runtime.rs` 保持 600 行边界内，不新增
  file-too-long warning。

### 7. 遗留问题

- 事件尚未登记到 `PendingApprovals`、尚未进入实时 `UiEventSource`，也尚未消费 `ApprovalGrants`。
- T5.2 loopback 端到端任务链、`form_submit` 人工 once 批准与真实 Edge 验收仍待下一张卡。
- TASK-057/058 仍为 Blocked，本卡未改写其状态或正文。

### 8. 新增长期记忆

- FACT：browser form submit 审批事件已有封闭线上形状与 UI strict parser；runtime 在 policy 前构造，
  UI 只映射已脱敏 diff 和 `form_state` 内存证据，不触碰原始字段值。

### 9. 给审阅者的关注点

- 重点检查 `browser_task_approval_event.rs` 是否只在 `form_submit` 前一次性取走 preview，且没有登记授权。
- 重点检查 zod `instructionAttributionSchema` 与 form evidence 的 origin/action 条件是否和 ADR-0101 一致。
- 重点检查 `UiEvent` boxed 字段序列化后仍是同一扁平 JSON，没有把内部 Rust 表示泄漏到线上形状。

---

**2026-10-31 merge evidence**

- PR #678 checks **22**（21 success + 1 skipped，pending 0），Mergeable / Clean，merge `a0280d2`。
- post-merge main CI `41167108842` = **success**。
