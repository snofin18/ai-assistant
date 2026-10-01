# TASK-216　运行时补齐：任务输入绑定 + `hitl`/rollback/verify 步骤 + `tab.new` 观测

- 状态：**InProgress（A 片 Done；B 片已解锁，未开工）**
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

- **改** `apps/agent-core/src/task_package.rs`：新增 `TaskPackageError::MissingInput` / `UnsupportedReference`；新增 `TaskPackageProvider::from_package_json_with_inputs()`；`normalize_arguments` → `resolve_arguments`（递归解析 `$input.<name>`，空输入表仍走旧的 `UnboundArguments` 语义）。
- **改** `apps/agent-core/tests/task_package.rs`：新增 3 条（T1.2 绑定后能渲染且无 `$input.` 残留、缺输入 → `MissingInput`、派生引用 → `UnsupportedReference`），测试数 9 → **12**。
- **改** `docs/adr/0059-*.md`、`docs/adr/README.md`：ADR-0059 转 Accepted（人类 2026-10-01）。
- **改** 本卡、`LEDGER.md`。

### 3. 验收输出摘要

```text
cargo test -p assistant-agent-core --test task_package   → 12 passed / 0 failed
cargo clippy -p assistant-agent-core --all-targets -- -D warnings → 0 warning
cargo fmt --all --check                                  → clean
```

关键点：**T1.2 的任务包在给出 `old_text` / `new_text` / `expected_replacements` 之后能被渲染成合法 Plan**（当前 `test_t1_2_package_renders_once_inputs_are_bound` 断言 `notepad.file.replace_text` 进入 Plan 且无 `$input.` 字面量残留）—— 这正是 DRIFT-105-3 的根因 1。

### 4. DoD 逐条核对

- [x] A 片：任务输入绑定层落地，T1.2 任务包能被渲染成合法 Plan（无 `$input.` 残留）—— **已做**（T1.3 待 B 片 `tab.new` 生效后一并验证）
- [x] A 片：未知引用 / 缺失输入 → 构造期 fail-closed，各有负向用例 —— **已做**（`MissingInput` / `UnsupportedReference` 各一）
- [x] B 片：ADR-0059 落地 —— **已做**（2026-10-01 人类确认接受）
- [ ] B 片：按 ADR-0059 执行，并给出对应证据 —— **未做**
- [ ] B 片：`notepad.tab.new` 读到 `TabCountText` 并断言 +1 —— **未做**
- [x] 未修改 Out of scope 文件；未新增第三方依赖；未改 `crates/**` 公共接口 —— 已核对（本轮只动 `apps/agent-core/src/task_package.rs`、其测试、ADR 与本卡）
- [ ] 上列 16 条验收命令全绿 —— 本轮跑了 A 片相关的 fmt / clippy / 定向测试；全量门禁待 B 片收口时一次跑齐
- [ ] §11.1 进度同步 —— **Done 时执行**（当前 InProgress）

### 5. 偏差

**B 片第 1 步（2026-10-01）：`tab_count` 采用「可选 target」**

按人类 2026-10-01「按你的计划去做」，`tab_count` 不塞进 `REQUIRED_TARGETS`（那会把 TASK-215 已裁决的六条全声明设计改掉，并逼真实 Notepad 包声明一个它没有的元素），而是新增 **`OPTIONAL_TARGETS`**：适配包声明了就启用、`load()` 会像必选一样校验它；**没声明不是错误** —— 语义是"该适配包不提供这项观测"，调用方必须 fail-closed，**不许编一个数**。靶机包已声明 `tab_count` → `TabCountText`，真实 Notepad 包不变。

**注意**：handler（`notepad.tab.new`）**还没消费**这个 target，所以它仍然 fail-closed —— 本轮只把"能声明"这一步做完。

**B 片第 2 步（2026-10-01）：`notepad.tab.new` 消费 `tab_count`**

`NewTabHandler` 从单元结构改成与其他 handler 同形的 `NewTabHandler<P>{ context }`；新增 `NotepadHandlerContext::new_tab_output()`：解析 `tab_count` → 读 `Tabs: N` → 解析 `add_tab_button` 并 invoke → 再读一次 → **断言恰好 +1**，否则报 `-32_004`（VerifyFailed）并带 before/after；成功后回指纹。配套纯函数 `parse_tab_count()`：`Tabs: <n>` 之外一律 `-32_601`（CapabilityMissing）。旧的无条件 fail-closed 自由函数已删除，其单元用例替换为 `parse_tab_count` 的正/负两条。

**适配包没声明 `tab_count` 时**：`resolve_element` 会以该 target 名报错（fail-closed），**不会**在动作前编造计数，也不会假装成功 —— 这正是"可选 target"想要的语义。

**仍未证明**：本步只做到了"能编译 + 纯函数有单测 + 全量测试与 UIA 干跑不回归"；**`notepad.tab.new` 尚未经端到端跑通**（T1.3 的 Plan 仍渲染不了，见 ADR-0059 执行侧未做）。端到端证据待 B 片第 3 步之后补。

**DRIFT-216-2（ADR-0059 D6 照字面实现会打断 T1.1 —— 需要人类裁决覆盖集合）**

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

### 6. 更合理做法

（待填）

### 7. 遗留问题

（待填）

### 8. 新增长期记忆

（待填）

### 9. 给审阅者的关注点

（待填）
