# TASK-223　L1 文件通道 `read_utf8_prefix` 作为 binary 层保留 host_service

- 状态：**Done（2026-10-03；ADR-0064 文件通道 + ADR-0065 前置初始指纹步骤，T1.1 大文件 Plan fake 与真机双取证；DRIFT-223-1 / PL-100 闭环）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-219、**ADR-0064 Accepted**
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`docs/adr/0061-runtime-task-dataflow-and-resumable-approval.md` D5、`docs/adr/0064-*`、`adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`

---

## 目标（一句话）

让 T1.1 的**大文件路径**真正可用：为 `l1_file / read_utf8_prefix` 提供一个 binary 层保留的
`host_service` 工具（读 UTF-8 前缀并标 `truncated`），使 1 MB 文件走文件通道降级而不是被静默跳过。

## 背景（为什么现在做）

TASK-219 让 `pure` 步骤真实执行后，T1.1 仍只提交 `read_text` + `analyze` 两步；
`read_file_channel` 是 `kind = "l1_file"`，按 **ADR-0061 D5** 属"不由运行时执行"的
文件通道，当前被 `render_step` 静默跳过（只记 `declared_not_executed`）。

**这里必须纠正一个先前的口头建议**：我曾建议"把 L1 文件通道接入运行时"，但
ADR-0061 D5 明确把 `l1_file` 归给"文件通道 / 工具化"，**不属于**运行时步骤种类。
正确做法不是把 `l1_file` 塞进运行时，而是**另立一个 binary 层保留 `host_service` 工具**
（与 `inspect_target_path` / `set_editor_value` 同型），让任务包用它表达文件通道降级。
按铁律 10，先有 ADR-0064，再实现。

## write scope

- `docs/adr/0064-*.md`、`docs/adr/README.md`、`docs/memory/decisions.md`
- `apps/agent-core/src/runtime_tools.rs`、`apps/agent-core/src/reserved_invoker.rs`、`apps/agent-core/src/reserved_host.rs`、`apps/agent-core/src/notepad_registry.rs`、`apps/agent-core/src/runtime_host_ops.rs`
- `apps/agent-core/tests/task_package.rs`、`apps/agent-core/tests/production_root.rs`
- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`（`read_file_channel` 改声明）
- `tasks/TASK-223-l1-file-channel-read-utf8-prefix.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（本卡标记 + 「当前进度」句）/ `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- ADR-0064：授权"文件通道以 binary 层保留 `host_service` 工具表达"，**不改 `l1_file` 的语义**
  （它仍是任务前置/通道标记，不是运行时步骤种类），并说明与 ADR-0061 D5 的关系。
- 新增保留工具 `assistant.runtime.host_read_utf8_prefix`：输入绝对路径 + `max_text_bytes`，
  按 UTF-8 边界读前缀（不得在码点中间截断），输出 `text` / `truncated` / `bytes_read` /
  `bytes_total` / `fingerprint`；路径必须是绝对路径且不含 `..`；文件缺失 → `TargetNotFound`。
- 任务包 `read_file_channel` 改声明为 `kind = "host_service"` + `operation = read_utf8_prefix`，
  保留 `when: file_size_bytes > max_text_bytes` 与 `outputs: ["text", "truncated"]`。
- 负向用例：相对路径/含 `..` 拒绝；文件缺失 `TargetNotFound`；UTF-8 多字节边界不截断；
  `max_text_bytes` 非正数拒绝。

## Out of scope（做了算漂移）

- 改 `crates/**` 公共接口、`PlanStep` / `Plan` / `Planner` 形状。
- 把 `l1_file` 定义成运行时步骤种类（那会违反 ADR-0061 D5）。
- 真实 LLM / 商业应用 / 网络。

## 必须遵守

- **铁律 1**：读失败、非绝对路径、非正预算一律显式失败。
- **铁律 10**：ADR-0064 Accepted 前不写实现。
- **铁律 9**：只改 write scope 内文件。
- ADR-0063：读文件不得引入无界缓冲（只读前缀，不整文件入内存）。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test task_package
cargo test -p assistant-agent-core --test production_root
cargo run -p xtask -- docscan
```

并附大文件走文件通道、UTF-8 边界不截断、三类负向四条证据。

## 完成定义（DoD）

- [ ] ADR-0064 落档并 Accepted（人类裁决）。
- [ ] `assistant.runtime.host_read_utf8_prefix` 实现并通过负向用例。
- [ ] T1.1 大文件用例在 Plan 中走 `host_service` 文件通道分支。
- [ ] 读取只取前缀，`truncated` 显式，不把整文件读入内存。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

**第 1 轮（2026-10-03 WIP，ADR-0064 + 文件通道工具）**

```text
【任务】TASK-223 L1 文件通道 read_utf8_prefix
【目标】以 binary 层保留 host_service 表达文件通道，支持有界 UTF-8 前缀读取并显式 truncated
【write scope】仅：card 列出的 ADR / agent-core / adapter / 状态与记忆文件
【铁律】1 无静默失败；9 不静默扩大范围；10 ADR-0064 Accepted 后实现；ADR-0061 D5；ADR-0063
【禁止】改 crates/**、PlanStep/Plan/Planner 形状、把 l1_file 变成运行时步骤、整文件读入内存
【验收】fmt / clippy / workspace tests / task_package / production_root / docscan
【依赖】TASK-219 Done；ADR-0064 已按用户授权 Accepted
【疑问】完整大文件执行需要改 runtime_binding 条件跳过语义，超出卡面 write scope；按规则记 DRIFT-223-1，不越权修改
```

**第 2 轮（2026-10-03 闭环 DRIFT-223-1，人类派单确认方案）**

```text
【任务】TASK-223 续做：用「前置步骤产生初始指纹」闭环 DRIFT-223-1
【目标】新增恒执行的前置保留 host_service 步骤，用注入平台产出真实初始指纹，使 T1.1 大文件 Plan 跑通
【write scope】仅：apps/agent-core/src/**、apps/agent-core/tests/**、adapters/.../tasks/t1.1*、本卡与状态同步文件
【铁律】1 无静默失败；2 平台返回不可信；9 不静默扩大范围；10 契约先行（先落 ADR-0065）；ADR-0061 D5/D8；ADR-0063
【禁止】改 crates/** 公共形状、PlanStep/Plan/Planner；用 cfg!(debug_assertions) 或任何构建配置分叉运行时行为；伪造/占位指纹；放宽断言判据
【验收】fmt / clippy -D warnings / test --workspace / task_package / production_root / 真机 production_root_uia T1.1 / docscan
【依赖】TASK-219 Done；ADR-0064 Accepted（已核 LEDGER 末行与 8cf173e 为 main 祖先）
【疑问】① 卡面 write scope 只列 docs/adr/0064-*，而 ADR-0064 影响段明写「必须另立 ADR」→ 按铁律 10 新增 ADR-0065 文件并登记（见 §5）；
       ② 派单第 5 步要求 T1.2/T1.3 同批加前置步骤，但 write scope 限定 t1.1*，且实测两者首步无 when → 只改 T1.1（见 §5 DRIFT-223-2）
```

### 2. 实际改动文件

**第 1 轮（WIP，commit `8cf173e`，已在 main）**

- `docs/adr/0064-l1-file-channel-as-reserved-host-service.md`（新增）、`docs/adr/README.md`、`docs/memory/decisions.md`
- `apps/agent-core/src/runtime_tools.rs`、`reserved_invoker.rs`、`reserved_host.rs`、`runtime_host_ops.rs`、`notepad_registry.rs`
- `apps/agent-core/tests/task_package.rs`、`production_root.rs`
- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`

**第 2 轮（本轮，闭环 DRIFT-223-1）**

- `docs/adr/0065-initial-fingerprint-reserved-host-step.md`（新增）、`docs/adr/README.md`（§1 登记 + 下一号 0066）、`docs/memory/decisions.md`
- `apps/agent-core/src/runtime_tools.rs`（闭集 +1、`tool_for_step`、Planner schema、声明数组 10→11、模块不变量注释纠正）
- `apps/agent-core/src/runtime_host_ops.rs`（`ReservedHostOperations::capture_initial_fingerprint()`）
- `apps/agent-core/src/notepad_registry.rs`（实现：解析主窗口 → 注入平台 `fingerprint(WholeWindow)`）
- `apps/agent-core/src/reserved_host.rs`、`reserved_invoker.rs`（分派 + 真实指纹 ok 信封 / 显式失败）
- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`（`capture_initial_fingerprint` 作为第一个可执行步骤）
- `apps/agent-core/tests/task_package.rs`（+2 用例）、`production_root.rs`（+1 大文件用例、快照数 3→4、step-event helper 参数化）
- `apps/agent-core/tests/production_root_uia.rs`（ignored T1.1 断言 2→4 + 真机指纹断言；新增真机大文件用例）
- `apps/agent-core/tests/runtime_toolbus.rs`（+1 未装配即 `CapabilityMissing` 的负向用例）
- 本卡；`LEDGER.md` / `PLAN.md`（当前状态块）/ `README.md`（三处）/ `plans/stage-1-pilots.md`（标记 + 当前进度句）/
  `MEMORY.md`（规模表 3 行）/ `docs/memory/{facts,pitfalls}.md`（各追加 1 条）/ `docs/PARKING_LOT.md`（PL-100 闭环 + PL-103/104 新提）

### 3. 验收输出摘要

全部按退出码核（本机 Windows 11 25H2，rustc/cargo 1.98.1）：

- `cargo fmt --all --check`：EXIT 0。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅既有 unknown-lint warning）。
- `cargo test --workspace`：EXIT 0。
- `cargo test -p assistant-agent-core`：lib 44 passed、`task_package` **17** passed、`production_root` **14** passed、
  `runtime_toolbus` **6** passed、`runtime_execution` 6、`assembly_contract` 5、`ui_ipc` 12、其余全绿。
- `cargo test -p assistant-core arch::`：5 passed（依赖方向）。
- **真机（交互式桌面 + `notepad-like` 靶机）**：`cargo test -p assistant-agent-core --test production_root_uia test_production_t1_ -- --ignored --test-threads=1`
  → **4 passed**（`test_production_t1_1_dry_run_over_real_uia`、**新增** `test_production_t1_1_large_file_over_real_uia`、
  `test_production_t1_2_dry_run_over_real_uia`、`test_production_t1_3_dry_run_over_real_uia`）。
  并行跑同一批会因 4 个同 AutomationId 靶机窗口同时在场而 `TargetAmbiguous` 全红 → 见 **PL-104**。
- xtask：`hygiene` / `docscan` / `check-comments` / `refscan` / `verify-schemas` / `card-check` / `codegen --check` /
  `adr-index`（scanned=53，0E/0W）/ `memory-counts`（0E/0W）/ `check-ledger` 全 **PASSED**。

**卡面要求的四条证据**

1. **大文件走文件通道**：`production_root.rs::test_production_t1_1_large_file_skips_read_text_and_uses_the_file_channel`
   （5200 B 文件 / 1024 B 预算）→ `Completed`、4 快照、`read_calls == 0`（UIA 读取从未发生）、
   `analyze` 只能靠文件通道发布的 `text` 提交；真机版 `test_production_t1_1_large_file_over_real_uia` 同样 4 步全提交。
2. **UTF-8 边界不截断**：`notepad_registry::tests::test_read_utf8_prefix_trims_incomplete_multibyte_suffix`
   （`"ab€"` / 预算 4 → `text == "ab"`、`bytes_read == 2`）。
3. **三类负向**：相对路径与 `..` 拒绝、文件缺失 `TargetNotFound`、`max_text_bytes == 0` 拒绝（同上 tests 模块）；
   本轮再加保留工具层负向：`runtime_toolbus.rs::test_capture_initial_fingerprint_without_host_operations_fails_closed`
   → `CapabilityMissing` + `data == None`（不产出任何指纹载荷）。
4. **`truncated` 显式**：`read_utf8_prefix_data` 输出 `truncated = bytes_total > bytes_read`，1 MiB 用例
   （1,048,577 B 文件 / 1,048,576 B 预算）断言 `truncated == true` 且 `bytes_read == 1048576`。

**「不是构建配置分叉」的证据（ADR-0065 D2/D3）**

- fake 侧：`production_root.rs` 两个 T1.1 用例断言第一个已提交步骤 `id == "capture_initial_fingerprint"` 且
  `post_fingerprint == FakePlatform::fingerprint()`（revision 常量 `sha256:0000…0001`）。
- 真机侧：`production_root_uia.rs` 断言同一字段可被 `Fingerprint::parse` 接受，且 **不等于** 上述 fake 常量。
- 两侧走的是同一份 `NotepadReservedHostOperations::capture_initial_fingerprint` → `NotepadHandlerContext::fingerprint_event`
  → 注入 `P: UiAutomationProvider`；仓库内 `grep -n "debug_assertions" apps/agent-core/src` 无运行时行为分叉（仅 1 处
  `debug_assert!` 在 UTF-8 回退长度校验上，不改变返回值）。

**合并与 CI 证据（回填）**

- PR **#190**（`codex/task-223-initial-fingerprint` → `main`）：CI run `37089459934` = **11/11 SUCCESS**
  （`check` windows-latest 6m16s / ubuntu-latest 2m50s / macos-latest 4m31s、`cargo deny` ×2、
  doc consistency、desktop-ui checks、desktop-ui tauri (windows)、commitlint、
  gate negative verification #6、xtask deferred inventory）；合并前 `mergeable=MERGEABLE`、
  `mergeStateStatus=CLEAN`、`baseRefName=main`。
- 实现提交 `af08e43`，合并提交 **`685d4a9`**（2026-10-03T02:29:08Z，`state=MERGED`）。
- 回填走独立分支 `codex/task-223-merge-backfill`：`LEDGER.md` 只追加一行「merge hash 回填」，不改写原 WIP/Done 行。

### 4. DoD 逐条核对

- [x] ADR-0064 落档并 Accepted（第 1 轮）；**ADR-0065 落档并 Accepted（本轮，人类派单确认方案）**。
- [x] `assistant.runtime.host_read_utf8_prefix` 实现并通过负向用例。
- [x] **T1.1 大文件用例完整执行走 `host_service` 文件通道分支**：fake 平台与真机 UIA 各一次 `Completed`，
      `read_text` 被条件跳过（`read_calls == 0`），`read_file_channel` + `analyze` 真实提交。
- [x] 读取只取前缀，`truncated` 显式，不把整文件读入内存（`take(read_limit)` + 最多 3 字节回退；16 MiB 硬上限）。
- [x] 未修改 Out of scope 文件：`crates/**`、`PlanStep` / `Plan` / `Planner` 形状、`l1_file` 语义均未动
      （`git diff --stat` 全部落在 §2 清单内）。

### 5. 偏差

**DRIFT-223-1（第 1 轮提出 → 本轮闭环）**

- 原现象：T1.1 第一个可执行步骤 `read_text` 的 `when` 在大文件输入下为假，而此前无任何已提交步骤，
  `BindingInvoker` 返回 `VerifyFailed: condition for step read_text evaluated false before any fingerprint was published`。
- 闭环方式（**不改判据**）：按 ADR-0065 增加恒执行的前置保留步骤 `capture_initial_fingerprint`，
  由注入平台真实观测一次 `WholeWindow` 指纹并提交，后续条件跳过因此有合法前序指纹可引用。
  `runtime_binding.rs` 一行未改；`docs/spec/**` 未改。
- 证据：§3 的 fake + 真机双用例；`PL-100` 已在 `docs/PARKING_LOT.md` 标为已关闭。

**DRIFT-223-2（本轮新提，已按规则停下不越权）**

- 现象：派单第 5 步要求「T1.2 / T1.3 任务包同批加前置步骤」，但同一份派单把 write scope 限定为
  `adapters/com.microsoft.notepad/tasks/t1.1*`，卡面 write scope 亦只列 t1.1 那一个文件 —— 指令自相矛盾（漂移触发器 ⑤）。
- 实测判定：T1.2 的首个可执行步骤是 `capture_pre_replace_text`（`kind = tool`，**无 `when`**），
  T1.3 的是 `validate_inputs`（`kind = pure`，**无 `when`**）；两者恒执行并已发布指纹（T1.2 为真实平台指纹，
  T1.3 为 pure 输出内容哈希），因此「首步条件为假」场景在这两张包里**不存在**。
- 处置：**未修改 t1.2 / t1.3 的 JSON**（既无必要，也超 scope）。真机 T1.2 / T1.3 干跑仍全绿（§3）。
  规则已写进 ADR-0065 影响段：将来若给任何任务包的首步加条件，必须同批补该前置步骤。

**本轮 scope 说明（主动申报，未静默扩大）**

1. `docs/adr/0065-*.md` 是新文件，卡面 write scope 只列 `docs/adr/0064-*`；但 ADR-0064 影响段明写
   「必须另立 ADR/卡处理条件跳过语义」，且铁律 10 要求契约先行 → 按 ADR-0026 的登记表取下一可用号 0065，
   并在 `docs/adr/README.md` §1 与 `docs/memory/decisions.md` 登记（`adr-index` 0E/0W 通过）。
2. `apps/agent-core/tests/runtime_toolbus.rs` 与 `production_root_uia.rs` 不在卡面文件清单内，但在派单 scope
   （`apps/agent-core/tests/**`）内；改动只是新增负向用例与同步「多了一个真实前置步骤」导致的快照数/步骤序断言。
3. `MEMORY.md` 只改「各文件当前规模」表 3 行（facts/pitfalls/decisions），这是 AGENTS.md §11.2 第 6 步的强制动作，
   由 `memory-counts` 机器校验；§1 快照与其他段落未动。
4. 断言更新属漂移触发器 ⑦ 的形态，但方向是**收紧**而非放宽：`production_root.rs` 的 step-event helper 由
   「任取第一条 step 事件」改为「按 step_id + `committed` 精确匹配」，并新增对前置步骤事件的断言；
   `production_root_uia.rs` 由写死 2 改为写死 4 且新增真机指纹可解析 / 非 fake 常量两条断言。
5. `production_root.rs` 加完新用例后达到 **899 行**，距 `hygiene/file-too-long` 硬上限 900（gov §5.4 / ADR-0033）
   只剩 1 行；本轮把大文件用例的 config 构造收敛回既有 `production_config` helper（行为不变，只换 task inputs），
   文件降到 **888 行**。结构性拆分记入 **PL-105**，未做 drive-by 重构。

### 6. 更合理做法

补一个「恒执行的前置观测步骤」而不是放宽 fail-closed：`state_unchanged` 的可评估性依赖真实指纹，
放宽判据等于把「无法证明状态未变」变成静默成功（铁律 1）。同理，用注入平台而不是 `cfg!(debug_assertions)`
分叉 —— 后者会让 `cargo test`（debug）不再证明 release 的生产行为，属于隐藏行为分叉。
文件通道仍按 ADR-0064 用 `host_service` 表达，不扩展运行时步骤种类（那会违反 ADR-0061 D5 并改公共形状）。

### 7. 遗留问题

- **PL-103**：`docs/spec/runtime-execution.md` §3 仍写「三个 `assistant.runtime.*` 保留工具」，实际闭集已是 11 个；
  `docs/spec/**` 对 Implementer 只读，需 Orchestrator 或契约卡改为指向 `RESERVED_RUNTIME_TOOLS` 唯一事实源。
- **PL-104**：`production_root_uia.rs` 的 ignored 真机用例不能并行跑（同 AutomationId 靶机窗口互相歧义 → `TargetAmbiguous`）；
  卡面验收命令未写 `--test-threads=1`，容易被误判成回归。修法涉及 `fixtures/apps/notepad-like/**`，超出本卡 scope。
- **PL-105**：`production_root.rs` 现 888 行，距 hygiene 硬上限 900 只剩 12 行；下一张动到该文件的卡应先拆分，
  否则新增一个用例就会把 Warning 变成 Error 并卡住 CI。
- 「条件为假的步骤仍 verify/commit」这一现状本轮**未改**（ADR-0061 D8 语义），因此每加一个前置步骤，
  所有写死快照数的断言都要同步，含 ignored 真机用例 —— 已写入 `docs/memory/pitfalls.md`。
- 阶段 1a 的 **TASK-040**（首次点击校准真机验收）与 **TASK-220**（真实 UIA 三用例）仍需人工操作真实 GUI 才能收口，
  本轮未触碰（PL-101 / DRIFT-220-1 仍开着）。

### 8. 新增长期记忆

- FACT（`docs/memory/facts.md`）：前置初始指纹步骤已让 T1.1 大文件 Plan 真跑通；fake 与 `WindowsPlatform`
  各自返回自己的指纹，`runtime_binding.rs` 判据未改。
- PITFALL（`docs/memory/pitfalls.md`，`[supersedes:2026-10-03]` 第 1 轮那条）：修法是补恒执行前置步骤而非放宽
  fail-closed；同时坐实「跳过的步骤仍提交 → 写死快照数的断言（含 ignored 真机用例）必须同步」。
- DECISION（`docs/memory/decisions.md`）：ADR-0065 全文摘要（恒执行 / 注入平台 / 禁构建配置分叉 / 禁伪造 / 只读 /
  不改条件语义 / 不改公共形状 / 闭集与 Planner 目录）。
- 第 1 轮的 FACT / PITFALL 条目按「只追加不改写」保留原样。

### 9. 给审阅者的关注点

1. **指纹来源**：`apps/agent-core/src/notepad_registry.rs` 的 `capture_initial_fingerprint` 是本卡风险最高处 ——
   请确认它只有「解析主窗口 + 调注入平台 `fingerprint(WholeWindow)`」两步，没有任何常量、内容哈希或
   `cfg!(debug_assertions)` 兜底；失败路径是否都带可读原因（`VerifyFailed` / `CapabilityMissing`）。
2. **`fingerprint` 与 `previous_fingerprint` 同值**：这是只读观测的诚实表达（本步骤不改变状态），
   但它也让 `state_unchanged` 变成必然通过 —— 请确认这符合 ADR-0065 D5 的意图，而不是掩盖真实变化
   （真正的变化检测仍由后续写步骤的 before/after 指纹承担）。
3. **T1.1 可执行步骤 3→4**：这是任务包对外可见的契约变化，`task_package.rs` 的 Plan 形状断言与
   `production_root*.rs` 的快照数断言都已同步；请核对没有其它地方（评测集、UI 时间线快照、审计基线）
   仍写死 3 步 / 3 快照。
4. **DRIFT-223-2**：T1.2 / T1.3 未加前置步骤是按「首步无 `when`」的实测判定 + write scope 双重理由，
   若人类认为仍应统一加，请另开卡（会改动两张任务包与其真机断言）。
