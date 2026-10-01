# ADR-0061　运行时任务包数据流、确定性本地操作与可恢复审批

状态：**Proposed**（待人类接受；接受前 TASK-217 不得开工）
日期：2026-10-01
Supersedes：ADR-0059 D6 中 `pure` 一律“不执行、由包外注入”的单行处置（**其余 D1~D5/D7 保留**）
Superseded by：—
关联：**TASK-216 `DRIFT-216-4`**、TASK-217、TASK-105、ADR-0047、ADR-0056、ADR-0058、
ADR-0059、ADR-0060、`docs/spec/runtime-execution.md`

---

## 背景

TASK-216 已把 `$input.*` 的静态注入、`hitl` / `host_service` / `verify` 保留工具和一次性审批
落到 binary 层，但 `DRIFT-216-4` 证明 T1.2/T1.3 仍不能运行：任务包还声明了

- **前序步骤输出**：`$canonical_text_before`、`$approval_diff`、`$normalized_target_path`、
  `$target_path` 等；
- **条件步骤**：`when: "!target_existed_before"`；
- **按 operation 区分的 host/pure 操作**：`inspect_target_path`、`set_editor_value`、
  `compute_literal_replacement`、`build_text_diff`、`validate_t1_3_inputs`；
- **交互式审批暂停/恢复**：无批准时必须停在原步骤，UI 批准后继续，不能把“等待人类”降级成
  普通工具失败后丢弃进程。

直接支持 `$a + $b`、函数调用或任意表达式会重新打开 ADR-0047 已否决的表达式语言。把任务包里的
动态输出要求调用方预先算好也不可行：例如 `$canonical_text_before` 只有在运行时读过目标文档后
才存在。

同时，ADR-0060 目前只按 `kind` 映射保留工具，无法区分 `host_service` 的多个 operation；任务包
的 `when` 字段也尚未进入任何执行语义。继续维持现状只会得到“Plan 看似完整、执行语义缺失”的
第二种静默失败。

## 决策（一句话）

在 binary 层增加**只做整值引用解析的任务执行上下文**，按任务包声明的步骤输出、白名单 `pure`
operation 和 `(kind, operation)` 映射执行步骤；条件只允许布尔引用；审批未决时生成可恢复的
`AwaitingApproval` 状态，UI 决策后从同一快照恢复。**不引入表达式语言，不改 `PlanStep` /
`task-engine` 公共形状。**

## 决策细化

| # | 内容 |
|---|---|
| **D1 上下文归属** | `TaskExecutionContext` 只在 binary 装配层存在，持有 `name -> serde_json::Value`。它是单次运行的内存状态，不持久化成第二份会话真相。 |
| **D2 引用语法** | `$name` 仅在**整个字符串就是这个引用**时允许，替换后保留原 JSON 类型；`"prefix-$name"`、`"$a + $b"`、嵌套函数等一律拒绝。未知名字、类型不匹配、已跳过步骤的输出 → 执行前 fail-closed。 |
| **D3 输出发布** | 任务包 `steps[].outputs` 是输出声明。只有当产生该输出的步骤完成验证并提交后，声明值才进入上下文；失败、被拒绝、被跳过或验证未提交都不发布输出。输出名必须唯一，且生产者必须早于所有消费者。 |
| **D4 数据流旁路** | 任务包 provider 额外暴露 binary 内部使用的 `RuntimeDataflowPlan`（步骤 id → `when` / outputs / 引用）。它**不进入模型输出、不进入 Plan JSON**；`Planner` 仍只接受 `steps` 单字段。 |
| **D5 运行时覆盖面** | 运行时执行闭集为 `{tool, hitl, host_service, verify, pure}`。`platform` 与 `l1_file` 仍是任务前置条件/文件通道，不由运行时执行；若后续步骤依赖它们的输出，任务包在构造期失败，不能靠猜值继续。此项**取代 ADR-0059 D6 中 `pure` 不执行的单行处置**。 |
| **D6 `pure` operation** | `pure` 不变成自由表达式，而是**闭集白名单 operation**：每个 operation 有结构化 args / outputs、纯函数、无 IO / 网络 / 进程副作用。1a 至少包含 `compute_literal_replacement`、`build_text_diff`、`validate_t1_3_inputs`。未知 operation 一律拒绝。 |
| **D7 `host_service` operation** | 保留工具名按 operation 区分，建议形式 `assistant.runtime.host.<operation>`；`prepare_rollback_anchors` 保持既有语义，1a 还需 `inspect_target_path`（只读文件系统检查）与 `set_editor_value`（经已注入的平台写路径）。它们只进 Planner 目录，不进模型可见 ToolBus。 |
| **D8 `when` 条件** | 只允许 `name` 或 `!name` 两种形式，指向上下文中已有的布尔值；禁止逻辑与、逻辑或、比较、括号和函数。条件在执行策略/工具调用前求值；未知名字或非布尔值 → fail-closed。被跳过步骤不产生输出，若后续真正执行且引用该输出则失败。 |
| **D9 审批暂停/恢复** | 无授权时 `request_approval` 返回 **`AwaitingApproval`**，注册稳定的 pending 项（由 task_id + step_id 派生）并保留任务快照；它**不是** `ToolFailed`。UI approve 写入有界授权，deny 使步骤/任务失败，超时保持失败关闭。恢复时重新读取同一任务快照、核验 fingerprint 与待审批步骤后从该步骤继续，**不得重放已提交步骤或重复副作用**。 |
| **D10 无人值守边界** | 没有可交互 UI、pending 无法注册或恢复条件不成立时，审批步骤不得继续；`point_of_no_return` 仍遵守 ADR-0059 的人工一次批准语义，永久禁止无人值守。 |
| **D11 形状不变** | `PlanStep` / `Plan` / `Planner` / `task-engine` 的公共形状不改；数据流职责放在 binary 层的上下文与 invoker 包装中。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 支持任意 `$` 表达式 | ❌ | 与 ADR-0047 冲突；会引入 lexer / parser / evaluator / scope，且模型无法枚举可写形式。 |
| 要求调用方预先算好全部派生值 | ❌ | 动态值（如读回文本、替换计数、diff）只有在运行时才存在，预计算等于伪造或重复执行。 |
| 把输出声明或上下文放进 Plan JSON | ❌ | Planner 只接受 `steps` 单字段；TASK-216 `DRIFT-216-3` 已证明会污染模型契约。 |
| 改 `PlanStep` 增加 outputs / condition 字段 | ❌ | 是 `crates/**` 公共接口变更，波及 Planner、TaskEngine、Executor 与全部既有测试。 |
| 继续静默跳过 `pure` / `when` / 未知 operation | ❌ | 静默失败，T1.3 更可能在没有条件保护时执行写操作。 |
| 把“等待审批”当普通工具失败并结束进程 | ❌ | UI 无法安全恢复；重跑会重放已提交步骤和副作用。 |

## 影响

- `DRIFT-216-4` 获得可实施路径，TASK-217 可在本 ADR Accepted 后开工。
- `docs/spec/runtime-execution.md` 需补上下文、输出发布、`when`、operation 分派与暂停/恢复语义。
- `RuntimeDataflowPlan`、操作白名单和恢复检查都留在 `apps/agent-core`；**不改 `crates/**` 公共接口、不加第三方依赖、不新建 crate**。
- 模型可见工具集不变：`assistant.runtime.*` 仍只在 Planner 目录中。

## 验证方式

1. 引用解析只接受整值 `$name`；`$a + $b`、部分插值、未知名字、错误类型各有负向用例。
2. 输出只在验证提交后可见；失败/跳过步骤的输出被引用时 fail-closed。
3. T1.2 的 `compute_literal_replacement` / `build_text_diff` 产出可被后续 `hitl` 使用，且 diff 来自真实读回文本。
4. T1.3 的 `inspect_target_path` 在目标已存在时使条件路径拒绝，绝不执行 `save_as`。
5. `when` 只支持布尔引用；未知条件、非布尔值、复合表达式均拒绝。
6. 无授权时得到 `AwaitingApproval`；approve 后从同一快照恢复一次；deny/超时不继续。
7. 恢复检查不重放任何已提交步骤，事件中可看到暂停、批准、恢复与提交顺序。
8. 模型侧调用任一 `assistant.runtime.*` 均失败。

## 重新评估触发条件

- 若 1a 之后确实需要一般表达式、循环或复杂条件语言 → 另立 ADR，不能在本 ADR 上就地扩张；
- 若某个 operation 需要网络、凭据或真实商业应用 → 回本 ADR 检查是否超出 1a；
- 若暂停/恢复需要改变 `task-engine` 的公共状态或持久化接口 → 先补 ADR，再改 `crates/**`。

## 相关 ADR

- **ADR-0047**：否决自由字符串断言/表达式语言。
- **ADR-0056**：运行执行链路与 receipt 提交门。
- **ADR-0058**：生产装配根与确定性任务包 Plan 来源。
- **ADR-0059 / ADR-0060**：步骤覆盖闭集与保留运行时工具。
