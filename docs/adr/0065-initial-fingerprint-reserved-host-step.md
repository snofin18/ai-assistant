# ADR-0065　前置保留步骤产出初始指纹（condition-skip 的前序证据）

状态：**Accepted**（2026-10-03 人类在 TASK-223 派单中确认方案；DRIFT-223-1 / PL-100 闭环）
日期：2026-10-03
Supersedes：—
Superseded by：—
关联：**TASK-223**、`DRIFT-223-1`、`PL-100`、ADR-0060、ADR-0061 D5/D8、ADR-0064、
`docs/spec/runtime-execution.md` §5.1、`apps/agent-core/src/runtime_binding.rs`

---

## 背景

ADR-0061 D8 与 `runtime_binding.rs` 的 fail-closed 判据要求：一个 `when` 为假的步骤要被跳过，
必须**已有某个已提交步骤发布过 fingerprint**（跳过信封要携带真实指纹，不能凭空造）。

T1.1 的第一个可执行步骤是 `read_text`，它自带 `when: file_size_bytes <= max_text_bytes`。
大文件输入下该条件为假，而此前没有任何已提交步骤，于是运行时显式失败：
`condition for step read_text evaluated false before any fingerprint was published`。
把文件通道步骤调到前面也会让小文件分支撞上同一条限制。ADR-0064 的「影响」已明确把这个问题
留给后续 ADR/卡处理。

`docs/spec/runtime-execution.md` §5.1 本来就要求「Resolve/Precheck 获取 pre-fingerprint；
无法获取时必须显式 `NotEvaluable`，不能省略」。缺的不是新语义，而是**任务包里没有任何步骤
承载这个 pre-fingerprint**：`open_file` 是 `kind = platform` 的任务前置，按 ADR-0061 D5 不进 Plan。

## 决策（一句话）

**新增 binary 层保留 `host_service` 工具 `assistant.runtime.host_capture_initial_fingerprint`，
作为任务包第一个可执行步骤恒执行，用注入平台观测一次目标窗口指纹并发布，
为后续任何「首步条件为假」的跳过提供真实前序指纹。**

## 决策细化

| # | 内容 |
|---|---|
| **D1 恒执行** | 该步骤**不得**声明 `when`，且必须是 Plan 的第一个可执行步骤（`kind = platform` 的任务前置仍不进 Plan）。 |
| **D2 指纹来源 = 注入平台** | 实现只调用 `crates/platform/api` 的 `UiAutomationProvider::fingerprint(window, WholeWindow)`。测试注入 fake / replay provider，生产注入 `WindowsPlatform`：**同一段代码、同一条路径**，只有注入的实现不同。 |
| **D3 禁止构建配置分叉** | 严禁用 `cfg!(debug_assertions)`、feature 或 profile 决定指纹来源、是否执行该步骤、或是否接受占位值。构建配置分叉会让 `cargo test`（debug）不再证明 release 的生产行为，属于隐藏行为分叉。 |
| **D4 禁止伪造** | 指纹必须是平台真实返回值。窗口解析失败、平台不可用或指纹不可解析 → 显式失败（`CapabilityMissing` 未装配 / `VerifyFailed` 观测失败，附可读原因），不得回填常量、占位串或内容哈希。 |
| **D5 只读、状态不变** | 该步骤不改变应用状态：成功信封的 `fingerprint` 与 `previous_fingerprint` 是**同一次观测值**，postcondition 沿用保留工具默认的 `state_unchanged`。 |
| **D6 不改条件语义** | `runtime_binding.rs` 的 fail-closed 判据一字不改：没有前序指纹时仍然失败。本 ADR 只补「前序指纹的合法来源」。 |
| **D7 不改公共形状** | 不新增 crate、不新增第三方依赖、不改 `PlanStep` / `Plan` / `Planner` / `task-engine`；`l1_file` 语义仍按 ADR-0064 D1 不变。 |
| **D8 闭集与目录** | 工具名进 `RESERVED_RUNTIME_TOOLS` 闭集与 Planner 目录（`planner_schemas()`），**不注册进模型可见 ToolBus**（ADR-0060 D2）；模型看不到也调不到它。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 放宽 `runtime_binding` 的 fail-closed：首步条件为假时允许无指纹跳过 | ❌ | 跳过信封必须携带真实 fingerprint，否则 `state_unchanged` 无从断言；放宽等于把「无法证明状态未变」变成静默成功（铁律 1）。 |
| 用 `cfg!(debug_assertions)` 在测试里塞假指纹、生产里走真平台 | ❌ | 构建配置分叉使 debug 测试不再证明 release 行为（D3）。 |
| 用文件内容哈希或常量充当初始指纹 | ❌ | 伪造指纹（D4）；且后续真实平台指纹会与它比较，制造假冲突或假通过。 |
| 把 `open_file`（`kind = platform`）改成运行时步骤来发布指纹 | ❌ | 违反 ADR-0061 D5：platform 是任务前置，不由运行时执行。 |
| 在装配层预先写入一个「第 0 步快照」 | ❌ | 绕过 Plan/Step 状态机，快照里会出现无步骤归属的指纹，UI 时间线与审计无法对应。 |

## 影响

- T1.1 任务包新增第一个可执行步骤 `capture_initial_fingerprint`；可执行步骤数 3 → 4，
  快照数随之 +1（条件为假被跳过的步骤仍会 verify/commit，ADR-0061 D8 现状）。
- T1.2 / T1.3 **不需要**该步骤：它们的首个可执行步骤（`capture_pre_replace_text` / `validate_inputs`）
  没有 `when`，恒执行并已发布指纹。将来若给任何任务包的首步加条件，必须同批补该前置步骤。
- `ReservedHostOperations` 增加只读操作 `capture_initial_fingerprint()`，实现只出现在 binary 层。
- `DRIFT-223-1` / `PL-100` 闭环；`production_root_uia.rs` 的 T1.1 ignored 断言随真实步骤数同步。

## 验证方式

1. `runtime_tools`：闭集含新名字，`tool_for_step("host_service", Some("capture_initial_fingerprint"))`
   映射到新工具，`planner_schemas()` 仍覆盖全部保留名。
2. fake 平台执行 T1.1：第一个已提交步骤的 `post_fingerprint` 等于注入 fake 平台自己的指纹；
   大文件输入下 `read_text` 被跳过（UIA read 调用数 = 0），Plan 仍走到 `read_file_channel` 并 `Completed`。
3. 真机（`production_root_uia.rs`，ignored）：同一段代码经 `WindowsPlatform` 产出的初始指纹
   可被 `Fingerprint::parse` 接受且不等于 fake 平台的 revision 常量 → 证明来源是注入实现，不是构建配置。
4. 未装配 host operations 或平台观测失败时显式失败，绝不产出成功信封。
