# ADR-0060　`hitl` / `host_service` / `verify` 以「保留运行时工具」落地

状态：**Proposed**（待人类接受；接受前不得据本 ADR 写执行器）
日期：2026-10-01
Supersedes：—
Superseded by：—
关联：**TASK-216**、TASK-105、ADR-0059、ADR-0056、ADR-0058、ADR-0053、`docs/spec/runtime-execution.md`、
`docs/spec/tool-schema.md`、`tasks/TASK-216-*.md` §5

---

## 背景

ADR-0059（2026-10-01 修订）把运行时覆盖面定为闭集
`{tool, hitl, host_service, verify}`，并要求**真的执行**后三类。TASK-216 第 3a 步已让
`render_plan()` 对它们 **fail-closed**（`UnexecutedStepKind`），但一直没实现执行器。

动手时撞到一个**形状问题**：

1. `assistant_task_engine::PlanStep` 只有一个 `tool: String` 字段，**没有**"步骤种类"的位置；
2. `Planner` 要求每个步骤的 `tool` **能在调用方给出的工具目录里查到**（`PlannerRequest.tools`）；
3. 于是"把 `hitl` 变成 PlanStep"要么改 `task-engine` 的公共形状，要么给这三类各注册一个**工具**。

改 `PlanStep` 是 `crates/**` 的公共接口变更（触发漂移触发器 ③），影响面覆盖 Planner、
TaskEngine、RuntimeExecutor、verify 与全部既有测试；而"注册为工具"只需在 binary 层做。

同时有一个**安全约束**必须处理：这三类**绝不能被模型当作工具直接调用** ——
模型若能直接调 `request_approval`，审批就形同虚设（铁律 3）。

## 决策（一句话）

**三类以「保留运行时工具」落地**：在 binary 层为它们各注册一个 `assistant.runtime.*` 工具，
**只进入 Planner 的目录、不进模型可见的挂载集**；`render_plan()` 把对应 `kind` 映射成该工具名，
由 binary 层的 handler 真正执行；`PlanStep` 与 `task-engine` 的公共形状**不改**。

## 决策细化

| # | 内容 |
|---|---|
| **D1 保留名** | 三个工具名固定为 `assistant.runtime.request_approval`、`assistant.runtime.prepare_anchors`、`assistant.runtime.verify_postconditions`。与 `toolset.*` 同属**闭集例外**：名字是契约，不得漂移。 |
| **D2 可见性（安全）** | 这三个工具**只在 Planner 的目录里**（`PlannerRequest.tools`），**不**出现在模型可见的 `ToolBus` 挂载集（`MountSelection`）里；模型无法调用它们。**这一条是本 ADR 的安全核心**（铁律 3）。 |
| **D3 映射** | `render_plan()` 的映射：`kind = hitl` → `request_approval`；`host_service` → `prepare_anchors`；`verify` → `verify_postconditions`。步骤 `args` 原样带过（`risk` / `show_diff` / `scope_options` / `diff` / `set` 等）。 |
| **D4 执行** | handler 在 binary 层：`request_approval` 走 `crates/hitl`；`prepare_anchors` 走 `crates/undo`；`verify_postconditions` 断言"该任务此前所有写步骤都已提交且带 receipt"。**都不绕过 `crates/policy`**（铁律 3）。 |
| **D5 `set` 的语义** | `verify` 步骤的 `args.set` **不是**新的断言语言（ADR-0047 已否决那扇门）。本 ADR 把它定义为：**"该任务此前所有 tool 步骤都已验证并提交"**；`set` 名只作**记录用**，不参与求值。若将来需要逐条断言，另立 ADR。 |
| **D6 不改 `task-engine`** | `PlanStep` / `Plan` / `Planner` 的公共形状**一字不改**；这是本 ADR 相对于"改 PlanStep"的主要收益。 |
| **D7 与 ADR-0059 的关系** | 只规定**怎么落地**；覆盖面闭集本身仍由 ADR-0059（含 2026-10-01 修订）决定。 |
| **D8 fail-closed** | 三个保留工具缺席、挂载集误包含它们、或 handler 不可用 → **拒绝启动**并给 `ErrorCode`。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 改 `PlanStep` 加"步骤种类"字段 | ❌ | `crates/**` 公共接口变更，波及 Planner / TaskEngine / Executor / verify 与全部既有测试；收益不如在 binary 层解决 |
| 让这三类成为模型可见工具 | ❌ | 模型能直接调 `request_approval` 等于审批形同虚设（铁律 3） |
| 把 `set` 做成一套断言 DSL | ❌ | 与 ADR-0047（否决自由字符串断言/表达式语言）冲突 |
| 把执行器放进 `crates/core` | ❌ | 违反 ADR-0053 D3（core 黑名单含 `hitl` / `undo`） |
| 继续"静默跳过"或干脆不执行 | ❌ | ADR-0059 D6 已明文禁止；TASK-105 的审批/撤销证据将永远拿不到 |

## 影响

- **TASK-216 B 片第 3b 步**的开工前置由本 ADR 满足（Accepted 后）。
- `render_plan()` 的 `UnexecutedStepKind` 分支改为映射到保留工具；T1.2 / T1.3 由此**可以渲染**。
- `ToolBus` 的挂载集与 Planner 目录**首次出现差异** —— 这是 D2 有意为之，需在 `docs/spec/tool-schema.md` 与 `crates/tool-bus/README.md` 各加一句（属 TASK-216）。
- **不改** `crates/**` 公共接口、不加依赖、不加 crate。

## 验证方式

1. **安全（最重要）**：`ToolBus` 的模型可见挂载集**不含** `assistant.runtime.*`；负向用例断言模型侧调用这三个名字**失败**。
2. **正向**：T1.2 的 `hitl` 步骤在无人类批准时**不执行**；批准后按 `once` 放行一次。
3. **锚点**：`prepare_anchors` 失败 → 不进入 Execute，错误可读。
4. **顺序**：`hitl` 出现在 `tool` 之前时，事件流里先 `step_approval_requested` 再 `step_execution_started`。
5. **fail-closed**：保留工具缺席 / 被误挂载 → 启动即拒绝。

## 重新评估触发条件

- 若实现时发现 `TaskBus` 的挂载机制无法表达"只在 Planner 目录、不在模型挂载集" → 回本 ADR 补充；
- 若 `verify` 的 `set` 确实需要逐条断言 → 另立 ADR（ADR-0047 的边界）；
- 若将来某能力**必须**由运行时在步骤之间执行且无法表达为工具 → 回到 ADR-0059 的表。

## 相关 ADR

- **ADR-0059**：覆盖面闭集（D7 的边界）。
- **ADR-0056**：运行执行链路（D4 的依据）。
- **ADR-0053**：core 依赖白名单（否决选项的依据）。
- **ADR-0047**：否决自由字符串断言（D5 的依据）。
- **ADR-0058**：生产装配根（binary 层落地的依据）。
