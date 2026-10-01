# ADR-0059　1a 运行时真的执行 `hitl` / `host_service` / `verify` 步骤

状态：**Proposed**（待人类接受；接受前不得据本 ADR 写 B 片代码）
日期：2026-10-01
Supersedes：—
Superseded by：—
关联：**TASK-216**、TASK-105、TASK-214、ADR-0056、ADR-0058、`docs/spec/runtime-execution.md`、
`docs/audits/stage-1a-runtime-validation-2026-10-01.md`、`tasks/TASK-105-*.md` §5 `DRIFT-105-3`

---

## 背景

`DRIFT-105-3` 查明：阶段 1a 的运行时**只执行** `kind = "tool"` 的步骤，于是 T1.2 / T1.3
任务包里的这些步骤**从不执行**（证据见审计报告 §2.2）：

| 步骤种类 | 出现在 | 后果 |
|---|---|---|
| `hitl \| request_approval` | T1.2 ×2、T1.3 ×1（含 `point_of_no_return: true`） | TASK-105 DoD 的「审批与 point-of-no-return 行为符合声明」**没有可观测对象** |
| `host_service \| prepare_rollback_anchors` | T1.2 | **撤销 / 锚点证据没有对象** |
| `verify \| verify_postconditions` | T1.2、T1.3 | 任务包声明的 `set` 级断言没有落地（`RuntimeExecutor` 仍按 PlanStep 的 `postconditions` 验证，那层没问题） |

这不只是"缺证据"：**审批交互与撤销是本项目最核心的差异化能力**。如果 1a 的运行时永远不执行
这两类步骤，那么即便 UI 与 IPC 全部接通，用户也拿不到审批卡片和撤销 —— 阶段 1a 的
"受控、可审计、可撤销"就是一句空话。

这是一次**决定运行时执行哪些步骤种类**的决策（漂移触发器 ③⑧），必须先有 ADR（铁律 10）。

## 决策（一句话）

**阶段 1a 的运行时真的执行 `hitl` / `host_service` / `verify` 三类步骤**：由 `RuntimeExecutor`
在单步之间按声明顺序调用 HITL、Undo 锚点与任务包声明的后置断言集合；`point_of_no_return`
按声明生效；任何一类缺失实现即 fail-closed，**不降级、不静默跳过**。

## 决策细化

| # | 内容 |
|---|---|
| **D1 执行者** | 仍由 binary 层 `RuntimeExecutor` 独占编排（ADR-0056 D1）；新增的步骤种类**不**把编排权下放给 core 或 UI。 |
| **D2 `hitl` 步骤** | `request_approval` 走 `crates/hitl`：`risk` / `show_diff` / `scope_options` / `diff` 映射到审批请求；`AllowWithConfirmation` 的步骤**必须**经过它。`point_of_no_return: true` 的步骤按"不可逆"处理（铁律 6：永不无人值守）。 |
| **D3 `host_service` 步骤** | `prepare_rollback_anchors` 走 `crates/undo`：按 `required_anchor_levels` 建锚点；建锚失败即**不进入 Execute**（与 `docs/spec/runtime-execution.md` §5.3 一致）。 |
| **D4 `verify` 步骤** | `verify_postconditions`（任务包里的 `set`）在 Step 提交前求值；它与 `RuntimeExecutor` 现有的 `postconditions` 校验**同源**，不新增第二套断言语言（ADR-0047 已否决自由字符串断言）。 |
| **D5 步骤顺序** | 任务包声明的顺序即执行顺序；`hitl` 出现在 `tool` 之前意味着"先批准再执行"，出现在之后意味着"执行后确认"。执行器不得重排。 |
| **D6 fail-closed** | 遇到**未实现**的步骤种类（如 `l1_file` / `pure` 若本 ADR 未覆盖）→ 显式失败并给 `ErrorCode`。**禁止**继续沿用"跳过非 tool 步骤"的旧行为。 |
| **D7 与 ADR-0058 的关系** | 只扩展"执行哪些步骤"，**不改** D2 的 Plan 来源口径与"真实 LLM 不属 1a"。 |
| **D8 任务输入绑定** | 属 TASK-216 **A 片**，不需要本 ADR；本 ADR 只覆盖步骤种类。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 维持现状（只执行 `tool`，其余静默跳过） | ❌ | 静默跳过 = 静默失败（铁律 1）；且直接导致 1a 拿不到审批与撤销证据 |
| 明确把 `hitl` / rollback 移出 1a | ❌ | 人类 2026-10-01 选择"执行"；且移出等于承认 1a 不含本项目最核心的可控性能力 |
| 让 UI 直接发起审批与撤销 | ❌ | 违反铁律 3（策略引擎唯一放行点）与 ADR-0057 D1（UI 零系统权限） |
| 为这些步骤新引入一套声明语言 | ❌ | 与 ADR-0047（否决自由字符串断言）冲突；复用任务包既有字段 |
| 把 `hitl` / undo 的实现放进 `crates/core` | ❌ | 违反 ADR-0053 D3（core 黑名单含 `hitl` / `undo`） |

## 影响

- **TASK-216 B 片**开工前置由本 ADR 满足（Accepted 后）。
- TASK-105 的「审批与 point-of-no-return」「成功 + 失败恢复证据」两条 DoD **首次具备可观测对象**。
- `docs/spec/runtime-execution.md` §5.2 / §5.3 / §5.6 需按 D2~D5 补步骤种类的执行语义（属 TASK-216）。
- `crates/**` 的**公共接口不需要改动**：`crates/hitl` / `crates/undo` / `crates/verify` 均已存在。若实现中发现必须改 → **回本 ADR 补充**（触发器 ③）。
- 不新增第三方依赖、不新增 crate。

## 验证方式

1. **审批**：`point_of_no_return: true` 的 T1.3 步骤在无人类批准时**不得执行**；批准后按 `once` 作用域放行一次（负向 + 正向各一）。
2. **锚点**：`prepare_rollback_anchors` 失败时不进入 Execute，且错误可读。
3. **声明顺序**：`hitl` 在 `tool` 之前的步骤，执行顺序在事件流里可复核（先 `step_approval_requested` 再 `step_execution_started`）。
4. **fail-closed**：遇到本 ADR 未覆盖的步骤种类 → 显式失败带 `ErrorCode`，不得跳过。
5. **端到端**：T1.3 在靶机上出现 `SaveAsDialogWindow` 之前，必须先有审批事件。

## 重新评估触发条件

- 若实现时发现 `crates/hitl` / `crates/undo` 的现有接口不足以表达 `diff` / `scope_options` → 回本 ADR 补充；
- 若 1a 的 DoD 被人类改成不含审批与撤销 → 本 ADR 作废，改走"移出 1a"的新 ADR；
- 若 `l1_file` / `pure` 步骤也需要执行 → 另立 ADR，别在本 ADR 里扩。

## 相关 ADR

- **ADR-0056**：运行执行链路契约（D1 的依据）。
- **ADR-0058**：生产装配根与 1a Plan 来源（D7 的边界）。
- **ADR-0053**：core 依赖白名单（否决选项的依据）。
- **ADR-0047**：否决自由字符串断言（D4 的依据）。
- **ADR-0057**：UI↔Core 传输（否决"UI 直接发起审批"的依据）。
