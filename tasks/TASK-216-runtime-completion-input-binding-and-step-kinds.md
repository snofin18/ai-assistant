# TASK-216　运行时补齐：任务输入绑定 + `hitl`/rollback/verify 步骤 + `tab.new` 观测

- 状态：**Done（2026-10-02；LEDGER Done + PR #157 / merge `91e41ff`；DRIFT-216-4 由 TASK-217 闭环）**
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

【任务】TASK-216 运行时补齐：任务输入绑定 + `hitl`/rollback/verify 步骤 + `tab.new` 观测
【目标】把 A/B 片已经落地的运行时能力与 TASK-217 的数据流闭环合并收口为 Done
【write scope】仅：本卡记录区、`LEDGER.md`、`PLAN.md` 当前状态块、`README.md` 三处、`plans/stage-1-pilots.md` 本卡标记与进度句、`docs/PARKING_LOT.md`
【铁律】证据必须可复核；不得伪造测试或运行结果；公共热点文件先取 guard；不得改契约或放宽门禁
【禁止】不改 `apps/**` / `crates/**`；不操作真实 GUI；不把 TASK-105 的运行验收冒充为已完成
【验收】本卡 16 条命令 + TASK-217 已合并的 `production_root` / `ui_ipc` / `runtime_toolbus` 证据
【依赖】TASK-214、TASK-215 已 Done；`DRIFT-216-4` 已由 TASK-217 闭环
【疑问】无

### 2. 实际改动文件

- **改** `apps/agent-core/src/task_package.rs`：新增 `TaskPackageError::MissingInput` / `UnsupportedReference`；新增 `TaskPackageProvider::from_package_json_with_inputs()`；`normalize_arguments` → `resolve_arguments`（递归解析 `$input.<name>`，空输入表仍走旧的 `UnboundArguments` 语义）。
- **改** `apps/agent-core/tests/task_package.rs`：新增 3 条（T1.2 绑定后能渲染且无 `$input.` 残留、缺输入 → `MissingInput`、派生引用 → `UnsupportedReference`），测试数 9 → **12**。
- **改** `docs/adr/0059-*.md`、`docs/adr/README.md`：ADR-0059 转 Accepted（人类 2026-10-01）。
- **改** `apps/agent-core/src/runtime_tools.rs`、`reserved_invoker.rs`、`notepad_registry.rs`、`notepad_handlers.rs`、`notepad_tab.rs`：三类保留运行时工具映射、执行器与 `tab_count` 观测。
- **改** `apps/agent-core/src/approval_grants.rs`：有界授权表（TTL / uses / Persistent 拒绝）。
- **改** `apps/agent-core/src/ui_control.rs`、`production.rs`、`main.rs`：UI 批准写入运行时消费的同一张授权表；`ProductionHost::approvals()` 共享该表。
- **改** `apps/agent-core/tests/ui_ipc.rs`、`runtime_toolbus.rs`：证明一次 UI 批准只放行一次、`persistent` 不授予、保留工具永不到达模型可见 ToolBus。
- **改** `docs/adr/0060-*.md`：ADR-0060 转 Accepted（保留运行时工具）。
- **改** `docs/spec/runtime-execution.md`：补三类保留步骤、一次性审批和 `point_of_no_return` 的执行语义。
- **改** 本卡、`LEDGER.md`。

### 3. 验收输出摘要

```text
2026-10-02 closeout gates (all PASS):
cargo fmt --all --check                                  -> clean
cargo clippy --all-targets -- -D warnings                -> EXIT 0
cargo test --workspace                                   -> EXIT 0
cargo test -p assistant-agent-core                       -> EXIT 0
cargo run -p xtask -- verify-schemas                     -> 0 error(s), verdict PASSED
cargo run -p xtask -- codegen --check                    -> 0 drift(s), 0 error(s), verdict PASSED
cargo run -p xtask -- hygiene                            -> 331 scanned, 0 error(s), 3 warning(s), verdict PASSED
cargo run -p xtask -- docscan                            -> 0 error(s), 370 warning(s), verdict PASSED
cargo run -p xtask -- card-check                         -> 0 error(s), 27 warning(s), verdict PASSED
cargo run -p xtask -- refscan                            -> 0 error(s), 0 warning(s), verdict PASSED
cargo run -p xtask -- memory-counts                      -> 8 scanned, 0 error(s), 0 warning(s), verdict PASSED
cargo run -p xtask -- adr-index                          -> 45 scanned, 0 error(s), 0 warning(s), verdict PASSED
cargo run -p xtask -- check-ledger                       -> 0 error(s), 0 warning(s), verdict PASSED
cargo run -p xtask -- check-migrations                   -> 0 error(s), 0 warning(s), verdict PASSED
cargo run -p xtask -- check-comments                     -> 331 scanned, 0 error(s), 68 warning(s), verdict PASSED
cargo deny check                                         -> advisories / bans / licenses / sources all ok
```

关键点：**TASK-217 的已合并实现**已在 workspace 回归中证明 T1.2 / T1.3 到 `Completed`、UI approve/resume 与只消费一次授权；本卡只做收口，没有新增或放宽任何断言。

### 4. DoD 逐条核对

- [x] A 片：任务输入绑定层落地（无 `$input.` 残留）—— **已做**（`MissingInput` / `UnsupportedReference` 负向用例齐备）
- [x] ~~A 片：T1.2 任务包能被渲染成合法 Plan~~ —— **已被 B 片第 3 步推翻，见 §5 `DRIFT-216-3`**：绑定层本身没问题，但 T1.2 现在因 `host_service` 无执行器而**应当** fail-closed。原断言作废，改成 `test_t1_2_package_fails_closed_on_unexecuted_step_kinds`。
- [x] A 片：未知引用 / 缺失输入 → 构造期 fail-closed，各有负向用例 —— **已做**（`MissingInput` / `UnsupportedReference` 各一）
- [x] B 片：ADR-0059 落地 —— **已做**（2026-10-01 人类确认接受）
- [x] B 片：按 ADR-0059 执行，并给出对应证据 —— **已由 TASK-217 完成**：`DRIFT-216-4` 的数据流、pure/host operation、审批暂停与 UI 批准恢复均已落地；T1.2/T1.3 在 fake platform 到 `Completed`，见 §5 `DRIFT-216-4` 的闭环行
- [x] B 片：`notepad.tab.new` 读到 `TabCountText` 并断言 +1 —— **已做**（`notepad_tab.rs`，失败仍 fail-closed）
- [x] 未修改 Out of scope 文件；未新增第三方依赖；未改 `crates/**` 公共接口 —— 已核对（本轮只动 `apps/agent-core/src/task_package.rs`、其测试、ADR 与本卡）
- [x] 上列 16 条验收命令全绿 —— **已完成**（2026-10-01，第 4 步与 `DRIFT-216-4` 落档后按顺序复跑；全部返回退出码 0，`hygiene` 维持 0E/3W）
- [x] §11.1 进度同步 —— **已执行**（2026-10-02 收口：本卡 Done 标记 + LEDGER / PLAN / README / plans 同批更新）

### 5. 偏差

**B 片第 1 步（2026-10-01）：`tab_count` 采用「可选 target」**

按人类 2026-10-01「按你的计划去做」，`tab_count` 不塞进 `REQUIRED_TARGETS`（那会把 TASK-215 已裁决的六条全声明设计改掉，并逼真实 Notepad 包声明一个它没有的元素），而是新增 **`OPTIONAL_TARGETS`**：适配包声明了就启用、`load()` 会像必选一样校验它；**没声明不是错误** —— 语义是"该适配包不提供这项观测"，调用方必须 fail-closed，**不许编一个数**。靶机包已声明 `tab_count` → `TabCountText`，真实 Notepad 包不变。

**注意**：handler（`notepad.tab.new`）**还没消费**这个 target，所以它仍然 fail-closed —— 本轮只把"能声明"这一步做完。

**B 片第 2 步（2026-10-01）：`notepad.tab.new` 消费 `tab_count`**

`NewTabHandler` 从单元结构改成与其他 handler 同形的 `NewTabHandler<P>{ context }`；新增 `NotepadHandlerContext::new_tab_output()`：解析 `tab_count` → 读 `Tabs: N` → 解析 `add_tab_button` 并 invoke → 再读一次 → **断言恰好 +1**，否则报 `-32_004`（VerifyFailed）并带 before/after；成功后回指纹。配套纯函数 `parse_tab_count()`：`Tabs: <n>` 之外一律 `-32_601`（CapabilityMissing）。旧的无条件 fail-closed 自由函数已删除，其单元用例替换为 `parse_tab_count` 的正/负两条。

**适配包没声明 `tab_count` 时**：`resolve_element` 会以该 target 名报错（fail-closed），**不会**在动作前编造计数，也不会假装成功 —— 这正是"可选 target"想要的语义。

**仍未证明**：本步只做到了"能编译 + 纯函数有单测 + 全量测试与 UIA 干跑不回归"；**`notepad.tab.new` 尚未经端到端跑通**（T1.3 的 Plan 仍渲染不了，见 ADR-0059 执行侧未做）。端到端证据待 B 片第 3 步之后补。

**DRIFT-216-2（ADR-0059 D6 照字面实现会打断 T1.1 —— 需要人类裁决覆盖集合）**

**DRIFT-216-3（自伤：把"未执行种类"塞进模型输出，打红了 3 个生产测试）**

**B 片第 3b 步（2026-10-01）：保留运行时工具落地（ADR-0060 Accepted）**

**B 片第 3c 步的契约（动手前先写清，避免再踩一次"渲染能过、执行过不去"）**

第 3b 步给了三个保留工具 **`state_unchanged`** 作为后置断言。**这在执行侧会撞墙**，必须先讲清楚：

1. `EnvelopeObservationCollector` 要求工具信封里带 **`data.fingerprint`**；而 `state_unchanged` 正是拿它比对的。三个保留步骤**没有应用副作用**，但它们的信封里也**没有指纹** —— 所以按现状执行到它们会因"没有指纹"而进 `NeedsHuman`，而不是干净地通过。
2. **执行器的正确形状**（下一步按这个做）：
   - 新增 **`ReservedRuntimeInvoker`**（binary 层）：实现 `ToolInvoker`，**按工具名分流** —— 保留名走本地执行、其余转交 `ToolBusInvoker`。`RuntimeExecutor::new` 的第三个参数由它接替。
   - 它需要三样注入：① `latest_snapshot`（生产根已有 `Arc<Mutex<Option<TaskSnapshot>>>`）用于 `verify`；② 平台 provider + target 目录，用于 **fingerprint**（主窗口）—— 保留步骤虽不改应用，仍应**用指纹证明"确实没改"**，而不是空口声称；③ `crates/hitl` / `crates/undo` 的句柄用于另外两个。
   - `verify_postconditions`：读 `latest_snapshot`，断言**此前所有写步骤都已提交**；再取一次主窗口指纹放进信封 → `state_unchanged` 可求值。任一不满足 → 返回**带 `ErrorCode` 的错误信封**（不是 `Err`，避免被当成"结果未知"）。
   - `prepare_anchors` / `request_approval`：同形状；`request_approval` 还要守 `point_of_no_return` 与事件顺序。
3. **为什么不在本步顺手做**：这是新增一个实现 `ToolInvoker` 的组件 + 三处注入 + 指纹证明，属独立一步；而且第 3b 步刚因为"只测了渲染、没测执行"红过一次 CI，这一条正是那个教训的具体化。

新增 `apps/agent-core/src/runtime_tools.rs`：三个保留名（`assistant.runtime.request_approval` / `prepare_anchors` / `verify_postconditions`）、闭集 `RESERVED_RUNTIME_TOOLS`、`tool_for_step_kind()`（kind → 保留工具）、`planner_schemas()`（三个 Planner 目录用的 `ToolSchema`）。

`render_plan()`：`hitl`/`host_service`/`verify` 不再 fail-closed，而是**映射到保留工具**；`assertion_table` 给这三类加了一条诚实且可求值的后置断言 —— **`state_unchanged`**（它们没有应用副作用，只动运行时状态）。`notepad_registry::build_notepad_registry` 把保留 schema **追加进 Planner 目录**（因此**不进 ToolBus 挂载**，模型够不着 —— ADR-0060 D2）。测试：T1.2 **现在能渲染出完整 Plan**（含 `replace_text` + 三个保留工具），`runtime_tools` 自带两条单测。

**明确未做（下一步）**：三个保留工具的**执行器**还没有 —— 它们不在 ToolBus 里，所以一旦真的执行到它们，调用会失败。本轮只做到"目录 + 映射 + 可渲染"。

**两处自伤并已修**：① `none_readonly` 不是协议 `ToolReversibility` 的合法变体（协议只有 l0~l3），两个只读保留工具改用 `l0_undo_stack`，与 `notepad_registry` 对 `none_readonly` 的既有归一化（DRIFT-214-2）保持一致；② 把目录扩展写进 `production.rs` 把它顶到 **611 行**、hygiene 又变 4W —— 改为写在 `notepad_registry::build_notepad_registry`（它本来就负责产出 Planner 目录），`production.rs` 复原，**hygiene 回到 0E/3W**。

1. **现象**：B 片第 3 步第一版把 `declared_not_executed` 数组写进渲染出的 Plan JSON。`cargo test -p assistant-agent-core --test production_root` 立刻两条红：`Planner { reason: "invalid planner output: planner output must contain only the steps field" }`；真 UIA 干跑也红。
2. **根因**：`Planner` 对模型输出**只接受 `steps` 一个字段**（这是它有意的 fail-closed 契约）。我把运行时自己的 bookkeeping 塞进了模型输出，等于污染了模型→Planner 的契约。
3. **处置**：Plan JSON 回到 `{"steps": ...}` 一个字段；"未执行种类"改由 **provider 的独立访问器** `declared_not_executed()` 暴露（装配点可自行记录/打日志），并用测试锁住"plan 只有一个字段"这条不变量。**没有**为了让测试过而放宽 Planner。
4. **教训**：模型输出是**契约**，不是内部日志的载体；要给运行时记账，另开出口。

1. **现象（实测清点）**：三份任务包用到的 `kind` 一共 **8 种**，而运行时今天只执行 `tool`：

   ```text
   t1.1  platform ×1, tool ×1, l1_file ×1, pure ×1
   t1.2  platform ×2, tool ×3, host_service ×1, pure ×3, hitl ×2, verify ×1
   t1.3  pure ×2, platform ×2, host_service ×2, policy ×1, tool ×3, hitl ×1, l1_file ×1, verify ×1
   ```

   ADR-0059 只覆盖 `hitl` / `host_service` / `verify`；**`platform` / `l1_file` / `pure` / `policy` 四种未被覆盖**。
2. **影响**：ADR-0059 **D6 明文规定**"未覆盖的步骤种类 → 显式失败"。照字面实现，**T1.1 会立刻跑不起来** —— 它现在能渲染并跑通，恰恰依赖"非 tool 步骤被跳过"这个被 D6 否掉的行为。也就是说：**ADR-0059 已接受的决定与现有任务包互相矛盾**（漂移触发器 ④⑧）。
3. **建议（二选一，需人类裁决）**：
   - **① 扩覆盖面**：把 `platform` / `l1_file` / `pure` / `policy` 一并纳入 ADR-0059 的执行范围。代价大：`pure` 需要表达式求值器，而 ADR-0047 已否决自由字符串断言/表达式语言，等于要重新开一扇门；`policy` 需要与 `crates/policy` 的放行点对齐。
   - **② 定义"声明但不执行"的闭集**（**推荐**）：运行时覆盖集合 = `{tool, hitl, host_service, verify}`；其余四种必须**显式声明为"由别处负责"**并给出理由 —— `platform` 是**任务前置条件**（包内自己写着 "Open mode is a task precondition until a registered open_file tool exists"）、`l1_file` 是文件通道（归 Adapter/工具化）、`pure` 是包内纯计算（**不由运行时求值**，其结论以 `args` 形式注入）、`policy` 的放行归 `crates/policy` 而不是工具调用。这样 D6 仍然成立（**闭集之外才失败**），T1.1 也不会被打断。
4. **已停工作**：本轮**未改任何执行侧代码**（动手前先做这个清点，正是为了不把 T1.1 打红）。等人类选 ① 还是 ②，再落 B 片第 3 步。

**DRIFT-216-1（A 片的一处取舍：空输入表仍报 `UnboundArguments`）**

1. **现象**：有了输入绑定层之后，"参数里还有 `$input.`"其实有两种含义 —— ① 调用方**根本没给**输入；② 给了输入但**这个键缺失**。
2. **处置**：两者分开报：空输入表 → 沿用旧的 `UnboundArguments`；非空但键缺失 → 新的 `MissingInput`。理由是旧语义（"任务包从未被绑定"）已被既有测试与文档引用（TASK-214 的 `DRIFT-214-1`），改掉它等于悄悄改历史语义；而"给了输入却少一个键"是**另一类**错误，值得有自己的名字。
3. **代价**：多一个错误变体。可接受 —— 它让排障时不用猜是"没喂输入"还是"喂漏了"。

**DRIFT-216-4（T1.2/T1.3 仍无可执行对象：前序步骤输出没有绑定层）**

1. **现象**：第 4 步完成后，UI 的 approve/deny 已能写入运行时消费的有界授权表；但实际核对 T1.2/T1.3 任务包时发现，它们不只需要 `$input.*`，还大量引用**前序步骤输出**：`$canonical_text_before`、`$canonical_text_expected`、`$approval_diff`、`$normalized_target_path`、`$target_path` 等。当前 `TaskPackageProvider` 只能从调用方注入的 map 中取字符串；没有值就 fail-closed，因此 T1.2/T1.3 **仍渲染不出完整 Plan**，不能声称已能端到端运行。
2. **性质**：这不是"少写一行接线"，而是"任务包声明的数据流由谁求值"的新契约问题。直接支持任意 `$name` 会滑向表达式语言；只支持字符串替换又无法覆盖 `replacement_count` 等非字符串输出。它与 ADR-0047（否决自由表达式语言）和 ADR-0059 D6（`pure` 由包外注入）都有交集，属于漂移触发器 ③⑧，必须先裁决，不能在本步猜测实现。
3. **建议**：另立一张小 ADR + 卡，二选一定义清楚：① **运行时输出绑定层**：按任务包 `steps[].outputs` 记录每个已执行步骤的结构化输出，只允许后续 `args` 引用**已记录的名字**；`pure` 计算明确归谁（运行时或包外注入），仍不引入表达式语言；② **任务包显式化**：把需要外部计算的值全部列为 `inputs`，由调用方在 Plan 渲染前注入，任务包不再声明 `pure` 步骤。推荐 ①，因为它才是 T1.2/T1.3 真实闭环需要的最小数据流。
4. **当前处置**：保留已验证的 A 片绑定层、B 片保留工具与 UI 授权表，不伪造 T1.2/T1.3 运行证据；TASK-105 仍不得开工。阶段 1a 维持 **NO-GO**。

**DRIFT-216-4 闭环（2026-10-02，由 TASK-217 完成）**

1. **裁决落地**：ADR-0061 采纳方案 ① 的最小形式：仅支持整值 `$name` 引用、已提交步骤 outputs、封闭 `when` 谓词、按 `(kind, operation)` 分派的 `pure` / host operation；不引入表达式语言。
2. **证据**：TASK-217 slice 1 / slice 2 已合并至 `main`（`d68d7f5`，backfill `e8f72c3`）；`production_root` 9 passed，覆盖 T1.2 / T1.3 fake-platform `Completed`、无授权暂停、UI approve 后从同一快照恢复且已提交步骤 attempts 不增。
3. **本卡收口**：A 片绑定层、B 片保留工具、`notepad.tab.new` 的 `TabCountText` 观测与可恢复审批形成一条可运行链路，因此 `DRIFT-216-4` 从本卡遗留项中移除；TASK-105 的剩余工作是真实 GUI 靶机 10 次运行取证，不是本卡范围。

### 6. 更合理做法

运行时数据流由 TASK-217 以 side-channel `RuntimeDataflowPlan` 承载，保留步骤仍不进模型可见 ToolBus；这样既不污染 Planner 的单字段输出契约，也不把 `$name` 扩展成任意表达式语言。

### 7. 遗留问题

- **`DRIFT-216-4` 已闭环**：T1.2/T1.3 的前序输出绑定、pure/host operation 与可恢复审批由 TASK-217 落地；保留步骤仍保持在模型可见 ToolBus 之外。
- `docs/spec/tool-schema.md` 与 `crates/tool-bus/README.md` 仍需按 ADR-0060 影响节补一句"Planner 目录可含保留工具、模型挂载集不可含"的说明；当前卡 write scope 不含这两个文件，留待专门小卡处理。
- TASK-105 的真实 GUI 取证仍受自动化黑名单约束，应由允许操作靶机窗口的会话或人工触发执行。

### 8. 新增长期记忆

- **FACT**：UI 的 `ApproveRequest` 可通过共享 `ApprovalGrants` 放行保留运行时步骤一次；`Persistent` 永远拒绝，且保留工具永不到达模型可见 ToolBus。
- **PITFALL**：任务包里的 `$name` 同时承载"外部输入"与"前序步骤输出"，只做字符串注入会把两种语义混为一谈；需要明确输出绑定契约后再实现。

### 9. 给审阅者的关注点

1. 本卡现在关闭，依据是 TASK-217 的实际实现与测试证据，而不是本卡早期记录的静态渲染结果；审阅时应沿 `production_root` / `runtime_toolbus` / `ui_ipc` 用例核验。
2. 审批接线已证明"一次批准只消费一次"；本期只关闭 fake-platform 闭环，真实 `notepad-like` 十次运行仍归 TASK-105。
3. 保留工具刻意不进模型可见 ToolBus；任何为了"方便"把它们挂进 `MountSelection` 的改动都违反 ADR-0060 D2。
