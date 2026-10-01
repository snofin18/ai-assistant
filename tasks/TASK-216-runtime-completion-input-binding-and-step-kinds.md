# TASK-216　运行时补齐：任务输入绑定 + `hitl`/rollback/verify 步骤 + `tab.new` 观测

- 状态：**Ready（A 片可开工；B 片开工前置 = ADR-0059 Accepted）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：214、215、**DRIFT-105-3**
- 预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`tasks/TASK-105-notepad-t1-runtime-validation.md` §5 `DRIFT-105-3`、`docs/audits/stage-1a-runtime-validation-2026-10-01.md`、`docs/adr/0058-*`、`docs/spec/runtime-execution.md`、`adapters/com.microsoft.notepad/tasks/t1.{2,3}*.json`

---

## 目标（一句话）

让 T1.2 / T1.3 从"没有可运行对象"变成"能跑"：A 片补**任务输入绑定层**（解析任务包里的 `$input.*` / `$normalized_*` 占位）；B 片让 `hitl` / `host_service` / `verify` 这些步骤种类**要么真的执行、要么被明确移出 1a**，并让 `notepad.tab.new` 通过 `TabCountText` 观测 tab 计数。

## 背景（为什么现在做）

TASK-105 的 `DRIFT-105-3` 把 T1.2 / T1.3 一次都跑不起来的原因钉在三处（证据见 `docs/audits/stage-1a-runtime-validation-2026-10-01.md` §2）：

| # | 根因 | 证据 |
|---|---|---|
| 1 | 任务包的 `$input.` / `$normalized_*` 绑定**没有解析层**；确定性 Plan 来源刻意拒绝未绑定参数 | `apps/agent-core/src/task_package.rs` 的 `TaskPackageError::UnboundArguments` |
| 2 | 运行时**只执行 `kind = "tool"` 的步骤**；`hitl request_approval`（含 T1.3 的 `point_of_no_return: true`）/ `host_service prepare_rollback_anchors` / `verify verify_postconditions` 从不执行 | `apps/agent-core/src/task_package.rs` 的 `render_plan()` 跳过非 tool 步骤 |
| 3 | `notepad.tab.new` 被硬编码 fail-closed，而 T1.3 的第一个 tool 步骤正是它 | `apps/agent-core/src/notepad_handlers.rs` 约第 490 行 + `test_new_tab_fails_closed_without_tab_count_observation` |

后果：TASK-105 的「三组各 10 次」「审批与 point-of-no-return」「成功 + 失败恢复证据」三条 DoD **没有可观测对象**。
**这不只是"缺证据"**：`hitl` 与 rollback 不执行意味着这套运行时即便接上 UI，也拿不到审批交互与撤销 —— 而那正是本项目最核心的差异化能力。

## write scope

- `apps/agent-core/src/**`、`apps/agent-core/tests/**`（A 片 + B 片实现）
- `docs/adr/0059-*.md`、`docs/adr/README.md`、`docs/memory/decisions.md`（**B 片**契约；本卡授权起草并提交 Proposed，接受与否由人类裁决）
- `docs/spec/runtime-execution.md`（仅当 B 片裁决要求更新契约）
- `tasks/TASK-216-runtime-completion-input-binding-and-step-kinds.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅本卡条目标记 + 「当前进度」句）/ `docs/PARKING_LOT.md` / `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

**A 片（无需 ADR，可立即开工）**

- **任务输入绑定层**：生产根接受一份**显式注入**的任务输入（键值对，来自 CLI / 输入文件 / 调用方），在**渲染 Plan 之前**把步骤 `args` 里的 `$input.<name>` 替换为具体值。
- 派生引用（如 `$normalized_target_path`、`$canonical_text_before`）只有在**能由已解析值确定性算出**时才支持；否则**拒绝**（fail-closed），不许猜、不许留字面量。
- 未知引用、类型不匹配、必填输入缺失 → 构造期失败并给出可读原因。

**B 片（开工前置 = ADR-0059 Accepted）**

- 二选一并写进 ADR：**① 真的执行** `hitl` / `host_service` / `verify` 步骤（`request_approval` → HITL、`prepare_rollback_anchors` → Undo 锚点、`point_of_no_return` 语义）；或 **② 明确移出 1a**，说明 1a 的 DoD 相应改成什么、这些能力归哪个里程碑。
- `notepad.tab.new` 改为读靶机 `TabCountText`（TASK-215 已提供）观测 tab 计数并断言 +1；无法观测时仍 fail-closed。

## Out of scope（做了算漂移）

- **不改任务包声明**（`adapters/com.microsoft.notepad/tasks/**`）：`$input.` 是**任务输入**的占位语法，改的是"谁来绑定"，不是"要不要绑定"。
- 不改 `crates/**` 公共接口 / schema / `ErrorCode`；不新建 crate；不引第三方依赖。
- 不操作真实商业 Notepad。
- 不为了让 T1.x 通过而改测试断言或放宽门禁。

## 必须遵守

- **铁律 1**：未知引用 / 缺失输入 / 无法计算的派生值一律显式失败，不许把 `$input.x` 字面量塞进 Plan。
- **铁律 3**：审批是策略引擎的下游；B 片实现 `hitl` 步骤时不得绕过 `policy`。
- **铁律 10**：B 片先有 ADR-0059 再写代码。
- 绑定后的 Plan 仍只由可信来源决定 `effect` / `reversibility`（ADR-0055）。

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
```

A 片另需：**T1.2 的 `notepad.file.replace_text` 步骤能被渲染成合法 Plan**（当前会因 `UnboundArguments` 失败）+ 未知引用 / 缺失输入的负向用例。
B 片另需：`notepad.tab.new` 在靶机上读到 `TabCountText` 并断言 +1。

## 完成定义（DoD）

- [ ] A 片：任务输入绑定层落地，T1.2 / T1.3 的任务包能被渲染成**合法 Plan**（无残留 `$input.` / `$normalized_*` 字面量）
- [ ] A 片：未知引用 / 缺失输入 / 无法计算的派生值 → 构造期 fail-closed，各有负向用例
- [ ] B 片：ADR-0059 落地（先 Proposed，再由人类裁决）
- [ ] B 片：按 ADR-0059 执行或移出，并给出对应证据
- [ ] B 片：`notepad.tab.new` 读到 `TabCountText` 并断言 +1
- [ ] 未修改 Out of scope 文件；未新增第三方依赖；未改 `crates/**` 公共接口
- [ ] 上列 16 条验收命令全绿（`hygiene` 不高于 0E/3W 基线）
- [ ] §11.1 进度同步：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`

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
