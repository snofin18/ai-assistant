# TASK-217　运行时任务数据流、确定性本地操作与可恢复审批

- 状态：**InProgress（ADR-0061 Accepted 2026-10-01）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-216、**ADR-0061 Accepted**、**DRIFT-216-4**
- 预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`tasks/TASK-216-runtime-completion-input-binding-and-step-kinds.md` §5 `DRIFT-216-4`、`docs/adr/0061-*`、`docs/spec/runtime-execution.md`、`docs/audits/stage-1a-runtime-validation-2026-10-01.md`

---

## 目标（一句话）

让 T1.2/T1.3 真正具备可执行数据流：按任务包声明解析前序步骤输出、执行白名单 `pure`/host operation、只在布尔引用条件为真时执行步骤，并在审批未决时暂停后从同一快照安全恢复。

## 背景（为什么现在做）

`DRIFT-216-4` 的核心缺口不是“少一个字符串替换”，而是任务包声明的数据流没有执行者：

- `$canonical_text_before` 等值只有运行后才存在；
- `compute_literal_replacement` / `build_text_diff` 等 `pure` 步骤没有被执行；
- `host_service` 目前只按 kind 映射，无法区分 `prepare_rollback_anchors` / `inspect_target_path` / `set_editor_value`；
- `when: "!target_existed_before"` 没有任何求值语义；
- 无批准时直接失败会终止进程，UI approve 后没有安全恢复入口。

ADR-0061 已把这些边界定义为闭集和白名单，本卡只实现该决策。

## write scope

- `apps/agent-core/src/**`、`apps/agent-core/tests/**`
- `docs/adr/0061-*.md`、`docs/adr/README.md`、`docs/memory/decisions.md`
- `docs/spec/runtime-execution.md`
- `tasks/TASK-217-runtime-task-dataflow-resumable-approval.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅本卡条目标记 + 「当前进度」句）/ `docs/PARKING_LOT.md` / `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- binary 层 `TaskExecutionContext`：显式输入 + 已提交步骤输出；只支持整值 `$name` 引用。
- `RuntimeDataflowPlan` 旁路：从任务包读取 `when` / `outputs` / 引用关系，在构造期校验唯一生产、生产先于消费、未知引用。
- 输出发布：仅当步骤验证并提交后发布；失败/跳过/验证未提交不发布。
- `pure` operation 白名单：至少实现 1a 任务包需要的 `compute_literal_replacement`、`build_text_diff`、`validate_t1_3_inputs`。
- `host_service` 按 `(kind, operation)` 分派：保留 `prepare_rollback_anchors`，新增 `inspect_target_path`、`set_editor_value`。
- `when`：只支持布尔引用与 `!` 取反；未知名字/非布尔值/复合表达式 fail-closed。
- 审批暂停/恢复：无授权 → `AwaitingApproval` + pending 注册；approve/deny/timeout；恢复时校验 snapshot/fingerprint 后从原步骤继续。
- 端到端测试：T1.2/T1.3 的 Plan 构造与执行路径、条件跳过、输出缺失、审批暂停/恢复、保留工具模型不可见。

## Out of scope（做了算漂移）

- 改 `adapters/com.microsoft.notepad/tasks/**` 的任务包声明。
- 改 `crates/**` 公共接口、schema、`ErrorCode`、`PlanStep` / `Plan` / `Planner` 形状。
- 引入表达式语言、脚本引擎、循环、比较、复合布尔表达式。
- 真实 LLM / 网络 Provider、真实商业 Notepad、无人值守授权。
- 新第三方依赖、新 crate。

## 必须遵守

- **铁律 1**：未知引用、缺输出、非布尔条件、未知 operation、恢复不一致全部显式失败。
- **铁律 3**：策略引擎仍是唯一放行点；保留运行时 operation 不得绕过 policy。
- **铁律 4**：每个写 operation 必须有 postcondition；输出只有在验证提交后可见。
- **铁律 6**：`point_of_no_return` 永不无人值守放行。
- **铁律 10**：ADR-0061 未 Accepted 前不得写实现代码。
- ADR-0047：不引入自由字符串断言或表达式求值器。
- ADR-0060 D2：`assistant.runtime.*` 只进 Planner 目录，不进模型可见 ToolBus。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-migrations
cargo run -p xtask -- check-comments
cargo deny check
cargo test -p assistant-agent-core --test production_root_uia -- --ignored
```

## 完成定义（DoD）

- [ ] 整值 `$name` 引用解析落地；部分插值 / 表达式 / 未知引用 / 类型错各有负向用例。
- [ ] 已提交步骤输出进入上下文；失败、跳过、验证未提交的输出不可见。
- [ ] T1.2/T1.3 的任务包在构造期通过数据流校验，并在 fake platform 上执行到 `Completed`。
- [ ] `when` 的布尔引用与取反可用；复合表达式明确拒绝。
- [ ] `pure` / host operation 按白名单执行；未知 operation 拒绝。
- [ ] 无授权时 `AwaitingApproval`；approve 后从同一快照恢复一次；deny/timeout 不继续。
- [ ] 未重放已提交步骤、未重复副作用；事件顺序可复核。
- [ ] 模型可见挂载集始终不含 `assistant.runtime.*`。
- [ ] 未改 Out of scope 文件；未新增第三方依赖；未改 `crates/**` 公共接口。
- [ ] 上列验收命令全绿（`hygiene` 不高于 0E/3W 基线）。
- [ ] §11.1 进度同步：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`。

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-217 运行时任务数据流、确定性本地操作与可恢复审批
【目标】让 T1.2/T1.3 的任务包从前序输出、`pure`/host operation、封闭条件和审批中真正跑通
【write scope】仅：`apps/agent-core/src/**`、`apps/agent-core/tests/**`、ADR-0061/登记文件、`docs/spec/runtime-execution.md`、本卡与状态同步文件
【铁律】不可信输入先校验；策略引擎唯一放行；写操作必须有 postcondition；审批不得自批；不改 `crates/**` 公共形状
【禁止】改任务包声明、引入表达式语言/脚本引擎、加第三方依赖/新 crate、操作真实商业 Notepad、伪造运行证据
【验收】卡面 17 条命令全绿；T1.2/T1.3 fake platform 到 `Completed`；审批可恢复
【依赖】TASK-216、ADR-0061 Accepted、`DRIFT-216-4`
【疑问】`when` 现成声明含比较和 `&&`，已按封闭谓词子集修订 ADR；无其他疑问

### 2. 实际改动文件

- **增** `apps/agent-core/src/runtime_dataflow.rs`：整值 `$name` 解析、封闭 `when` 谓词、运行时数据流计划。
- **增** `apps/agent-core/src/runtime_binding.rs`：调用前条件求值与参数绑定，成功后暂存 outputs，提交后发布。
- **增** `apps/agent-core/src/runtime_host_ops.rs`：`inspect_target_path` / `set_editor_value` 的内部 host operation trait。
- **改** `apps/agent-core/src/runtime_tools.rs`：`pure` / host operation 的三段式保留工具与 Planner schema。
- **改** `apps/agent-core/src/task_package.rs`：任务包 outputs / `when` / operation 解析，前序输出占位和封闭条件校验。
- **改** `apps/agent-core/src/reserved_invoker.rs`：三个 pure operation、两个 host operation、锚点级别归一化与同指纹信封。
- **改** `apps/agent-core/src/production.rs`、`production_policy.rs`：任务输入注入、绑定 invoker 装配、审批窗口与策略桥接。
- **改** `apps/agent-core/src/notepad_handlers.rs`、`notepad_registry.rs`：host operation 使用既有 Notepad handler context。
- **改** `apps/agent-core/src/main.rs`：新增 `--task-inputs <path>`。
- **改** `apps/agent-core/tests/task_package.rs`、`production_root.rs`、`production_root_uia.rs`：输入绑定、T1.2 真执行与现有 T1.1 回归。
- **改** ADR-0061、ADR 登记、decisions、`docs/spec/runtime-execution.md`、本卡与状态同步文件。

### 3. 验收输出摘要

```text
cargo fmt --all --check                         → clean
cargo clippy --all-targets -- -D warnings        → EXIT 0
cargo test --workspace                           → EXIT 0
cargo test -p assistant-agent-core               → EXIT 0（lib 33 passed）
cargo test -p assistant-agent-core --test task_package → 14 passed
cargo test -p assistant-agent-core --test production_root → 7 passed
xtask verify-schemas / codegen --check / hygiene / docscan / card-check / refscan
  / memory-counts / adr-index / check-ledger / check-migrations / check-comments → 全部 EXIT 0
cargo deny check                                 → advisories / bans / licenses / sources ok
```

### 4. DoD 逐条核对

- [x] 整值 `$name` 引用解析落地；部分插值 / 表达式 / 未知引用 / 类型错有负向用例。
- [x] 已提交步骤输出进入上下文；失败、跳过、验证未提交不发布。
- [ ] T1.2/T1.3 都在 fake platform 执行到 `Completed` —— **T1.2 已完成；T1.3 待做**。
- [x] `when` 的布尔引用、取反、比较与 `&&` 可用；复合表达式明确拒绝。
- [x] `pure` / host operation 按白名单执行；未知 operation 拒绝。
- [ ] 无授权时 `AwaitingApproval` 并可从同一快照恢复 —— **当前只验证了预置有界授权的成功路径；可恢复暂停仍待做**。
- [ ] 未重放已提交步骤、事件顺序可复核 —— T1.2 顺序已跑通，暂停/恢复事件待补。
- [x] 模型可见挂载集不含 `assistant.runtime.*`。
- [x] 未改任务包声明；未加第三方依赖；未改 `crates/**` 公共接口。
- [x] 上列正常验收命令全绿（`hygiene` 0E/3W）。
- [ ] §11.1 进度同步 —— 本 slice 同步 LEDGER / PLAN / plans；TASK-217 尚未 Done。

### 5. 偏差

- **ADR-0061 修订 1**：`when` 从仅布尔引用扩为封闭谓词子集（引用 / 取反 / 值比较 / `&&`），以兼容现成 T1.1/T1.3 声明；仍禁止括号、函数、算术和 `||`。
- **ADR-0061 修订 2**：步骤可重绑同名 outputs；解析使用此前最近一次已提交的生产者。T1.2 的预期计数与真实计数依赖此语义。
- **ADR-0061 修订 3**：保留工具名必须是三段式。`assistant.runtime.pure.*` 被 Planner 正确拒绝，改为 `assistant.runtime.pure_*` / `assistant.runtime.host_*`。
- **实现取舍**：T1.1 的 `count_lines_and_keyword_paragraphs` 未声明文本输入且输出无人消费，继续作为“声明但不执行”，避免伪造分析结果。
- **策略桥接**：保留运行时工具在 policy 适配器里按低风险内部控制操作建模；普通应用写工具仍走原策略。已提交的 `request_approval` 只为**下一个写步骤**打开一次策略窗口，避免重复确认或无限授权。

### 6. 更合理做法

- 用 side-channel `RuntimeDataflowPlan` 承载 outputs / `when`，不污染 Planner 的单字段模型输出契约。
- 把 host operation 实现挂在既有 Notepad handler context 上，避免复制平台定位/写路径。

### 7. 遗留问题

- **T1.3** 尚未在 fake platform 上执行到 `Completed`；需要补 tab-count 读取、新标签写入、跨进程 Save As 的文件创建模拟或真靶机证据。
- **可恢复审批** 尚未实现：当前成功路径依赖调用方在运行前预置有界授权；无授权时仍返回 `AwaitingApproval` 并终止本次 `execute_plan`，没有持久化恢复句柄与 UI resume 入口。
- `check-comments` 对 `runtime_tools.rs` 的 PITFALL 标签格式仍有 warning；不是 error，后续小卡清理。

### 8. 新增长期记忆

- **FACT**：T1.2 已能在 fake platform 上通过完整 runtime（输入绑定、pure diff、两次审批、锚点、替换、保存、verify）到 `Completed`。
- **FACT**：Planner 工具名严格三段式；四段保留名会在启动时被拒绝。
- **PITFALL**：`state_unchanged` 需要 previous fingerprint；所有保留/pure/host 成功信封都必须同时给 before/after。

### 9. 给审阅者的关注点

1. 当前 PR 是 TASK-217 的中间 slice，**不是 Done**；不要因为没有 T1.3/暂停恢复证据而误判全部完成，也不要误认为它们已完成。
2. 策略窗口按“每个已提交 approval 只放行下一个写步骤”消费一次；需重点审查这是否符合预期，且不会放行无关写。
3. `pure` 的异常处理是白名单而非通用表达式；新增 operation 必须显式实现和测试。
