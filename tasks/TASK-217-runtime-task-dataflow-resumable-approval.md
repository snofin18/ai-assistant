# TASK-217　运行时任务数据流、确定性本地操作与可恢复审批

- 状态：**Blocked（ADR-0061 Proposed；接受后开工）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-216、**DRIFT-216-4**
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

（待填）

### 2. 实际改动文件

（待填）

### 3. 验收输出摘要

（待填）

### 4. DoD 逐条核对

（待填）

### 5. 偏差

（待填）

### 6. 更合理做法

（待填）

### 7. 遗留问题

（待填）

### 8. 新增长期记忆

（待填）

### 9. 给审阅者的关注点

（待填）
