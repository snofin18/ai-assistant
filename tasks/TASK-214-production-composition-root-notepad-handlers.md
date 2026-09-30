# TASK-214　生产装配根：真实 Host 进程 + Notepad Host handler + 1a Plan 来源

- 状态：**InProgress（第 1 片已落地：确定性 Plan 来源 + 1a 断言映射表；装配根与 handler 本体未做）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：103、213、**ADR-0058 Accepted**
- 预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 关联：TASK-029（self-check 装配点）、TASK-103（执行链路）、TASK-213（UI 传输 + `SnapshotEventSource`）、TASK-035（adapter 声明式包）、TASK-036~038（任务包）、TASK-105（下游验收）、`docs/adr/0058-*.md`、`docs/spec/runtime-execution.md`
- 来源：**TASK-105 无法开工的根因** —— 生产装配根、Notepad Host handler、Plan 来源三者都不存在（`apps/agent-core/src/main.rs` 自述"production composition root will supply…"）。人类 2026-09-30 指示「按你说的下一步去做吧」→ 立本卡。

---

## 目标（一句话）

在 `apps/agent-core` 的 binary 层落一个**真实可运行**的 Host 装配根：把 `RuntimeExecutor` 的引擎接成
`SnapshotEventSource`，注册 5 个走真实 UIA 的 Notepad handler，注入确定性「任务包 → `Plan`」的
`ModelProvider`，并启动 TASK-213 的 `UiServer` —— 让 TASK-105 有对象可跑。

## 背景（为什么现在做）

| # | 事实 | 证据 |
|---|---|---|
| 1 | 现有二进制只有 `--self-check`，装配 `NoopProvider` / 空 `ToolRegistry` | `apps/agent-core/src/main.rs` 文件头 + `run_self_check()` |
| 2 | `HostAssembly` 不拥有 `TaskEngine` / `RuntimeExecutor` / `ToolRegistry` 内容 / `UiServer` / `SnapshotEventSource` | `apps/agent-core/src/assembly.rs` 的 `HostComponents` 字段表 |
| 3 | 生产 `ToolRegistry` 为空；handler 只在测试里（`WriteTextHandler`） | `apps/agent-core/tests/runtime_toolbus.rs` |
| 4 | `SnapshotEventSource` 已就绪，但"接引擎"那一行没人做 | `LEDGER.md` 2026-09-30 TASK-213 收口行 |
| 5 | TASK-105 的三条硬前置全部悬空 | `PLAN.md` 当前状态块的「阻塞项」 |
| 6 | 真实 LLM 归 1c 前（M3），1a 不该引网络/凭据 | `docs/memory/open.md` M3；ADR-0058 D3 |

## write scope

- `apps/agent-core/src/**`（新增生产模式入口、`ModelProvider` 实现、handler 模块；**不改**既有文件的公共语义）
- `apps/agent-core/tests/**`（新增验收测试；**不得改**既有断言 —— 漂移触发器 ⑦）
- `apps/agent-core/README.md`（仅当装配根边界需要写清）
- `apps/agent-core/Cargo.toml`（**仅当**需要启用 `assistant-platform-windows` 的既有 feature；**不加**第三方依赖）
- `tasks/TASK-214-production-composition-root-notepad-handlers.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块 4 行）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅本卡条目完成标记 + 「当前进度」句）/ `MEMORY.md`（仅规模表）：AGENTS.md §11.1 强制的进度同步
- `docs/PARKING_LOT.md` / `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- 生产装配根：`TaskEngine` + `RuntimeExecutor` + `ToolRegistry`（含 handler）+ `UiServer` + `SnapshotEventSource` 的显式装配与启动。
- 确定性 `ModelProvider`：读 `adapters/com.microsoft.notepad/tasks/t1.*.json` → 渲染计划 JSON → 同一输入同一 `Plan`。
- 5 个 Notepad handler（`notepad.file.read_text` / `notepad.file.replace_text` / `notepad.file.save` / `notepad.tab.new` / `notepad.file.save_as`），经 `ToolBus` 分发，走 `assistant_platform_windows`。
- 目标绑定：显式 app 身份 + adapter selector 候选链；窗口定位走 **owner PID**（ADR-0022 D1）。
- 指纹采集：pre/post fingerprint 放进 `ToolEnvelope.data.fingerprint`；缺指纹 = `NeedsHuman`。
- 事件接线：`RuntimeExecutor` 的引擎接成 `SnapshotEventSource<Provider>` 的 provider。
- fail-closed 启动校验 + 契约/验收测试（靶机 `fixtures/apps/notepad-like`）。

## Out of scope（做了算漂移）

- **真实 LLM Provider**（HTTP / 云 / 本地推理）、任何新第三方依赖、任何网络出口或凭据。
- 改 `crates/**` 的公共接口 / trait / schema / `ErrorCode`（需要 → 回 ADR-0058 补充）。
- 新建 crate 或顶层目录。
- 在 `crates/core` 内做装配（ADR-0053 D2）。
- 注册通用 shell / PowerShell / `execute_code` 工具（AGENTS §7 永久禁止）。
- 操作真实商业 Notepad；只对 `fixtures/apps/notepad-like` 靶机取证。
- 完整 DDL/DLP / 污点追踪（阶段 1c）。
- 顺手重构既有装配、测试或 UI 代码。

## 必须遵守

- 策略仍是唯一放行点（铁律 3）：handler 不做权限判断，`RuntimeExecutor` 不绕过 Policy。
- 每个写工具必须有 postconditions；缺声明 → 拒绝注册（ADR-0055）。
- `Element` / 句柄不跨进程（铁律 8）：跨 `ToolBus` 只传可序列化参数与结果。
- 无静默失败（铁律 1）：缺组件 / 空目录 / handler 数与声明不符 → **拒绝启动**并带 `ErrorCode`。
- 失败不得被转成成功；验证失败不得提交（ADR-0056 D3）。
- 不改测试断言来通过（漂移触发器 ⑦）；发现问题记 DRIFT 并停。

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
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-migrations
cargo run -p xtask -- refscan
cargo deny check
pnpm --dir apps/desktop-ui lint; pnpm --dir apps/desktop-ui format:check; pnpm --dir apps/desktop-ui test
```

并附一次**靶机 T1.1 干跑**的实测输出（真实 UIA + 真 ToolBus + 真 receipt），以及 5 个 handler 的负向用例输出。

## 完成定义（DoD）

- [ ] 生产模式可启动：显式装配 `TaskEngine` + `RuntimeExecutor` + `ToolRegistry` + `UiServer` + `SnapshotEventSource`；`--self-check` 仍可用
- [ ] 5 个 Notepad handler 注册进 `ToolBus`，工具名与 `adapters/com.microsoft.notepad/tools/tools.json` 一致
- [ ] 确定性 Plan 来源：同一任务包两次运行产出同一 `Plan`
- [ ] 靶机 T1.1 干跑成功：ToolBus 调用数 = 1、Host 返回值带合法 fingerprint、verify 铸 receipt、task-engine 提交
- [ ] fail-closed：缺 provider / 空 registry / handler 数不符 → 启动拒绝且带 `ErrorCode`（含负向用例）
- [ ] 事件：同一次运行经 `ui_server` 推出 `step_state_changed`，UI 侧 zod 校验通过
- [ ] 未修改 Out of scope 文件；未新增第三方依赖；未改 `crates/**` 公共接口
- [ ] 上列 16 条验收命令全绿（`hygiene` 保持 0E/4W 基线）
- [ ] §11.1 进度同步：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-214 生产装配根：真实 Host 进程 + Notepad Host handler + 1a Plan 来源
【目标】在 `apps/agent-core` binary 层落一个可运行的生产装配根，让 TASK-105 有对象可跑
【write scope】仅：`apps/agent-core/src/**`、`apps/agent-core/tests/**`、`apps/agent-core/README.md`、`apps/agent-core/Cargo.toml`（仅 feature）、本卡、`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` / `MEMORY.md`（进度同步）、`docs/PARKING_LOT.md` / `docs/memory/{facts,pitfalls}.md`（仅追加）
【铁律】1 无静默失败；3 策略是唯一放行点；4 每个写操作必须有 postcondition；8 element/句柄不跨进程；10 契约先行
【禁止】真实 LLM Provider / 新第三方依赖 / 改 `crates/**` 公共接口 / 新 crate / 通用 shell 工具 / 操作真实商业 Notepad / 改测试断言
【验收】16 条命令全绿 + 靶机 T1.1 干跑实测输出 + 5 个 handler 负向用例
【依赖】103 ✅、213 ✅、**ADR-0058 ✅（2026-09-30 Accepted）**
【疑问】开工即撞到 plan 来源的 postcondition 形状缺口 → 见 §5 **DRIFT-214-1**。默认处理 = 断言模板作为 binary 层显式映射表，不猜、不改任务包，先记录再落地。

### 2. 实际改动文件

- **新增** `apps/agent-core/src/task_package.rs`：确定性 `TaskPackageProvider`（ADR-0058 D2）＋ **1a 断言映射表**（DRIFT-214-1 方案 ①）。
- **新增** `apps/agent-core/tests/task_package.rs`：9 条契约测试（确定性、只取 `kind=tool`、断言被 `verify` 接受、四类 fail-closed）。
- **改** `apps/agent-core/src/lib.rs`：注册 `mod task_package` 并导出 `TaskPackageProvider` / `TaskPackageError` / `TASK_PACKAGE_MODEL_ID`。
- **改** 本卡、`tasks/TASK-105-*.md`、`LEDGER.md`、`docs/PARKING_LOT.md`：记录与停车位（均为 §8 允许的追加面）。

本轮**未**触碰 `crates/**`、`adapters/**`、`fixtures/**`、`.github/**`，未加依赖。

### 3. 验收输出摘要

```text
cargo test -p assistant-agent-core --test task_package  → 9 passed / 0 failed
cargo test --workspace                                  → 全部 test result: ok（0 failed；含 xtask 411 passed）
cargo fmt --all --check                                 → clean
cargo clippy --all-targets -- -D warnings               → Finished，0 warning
xtask hygiene                                           → 0 error / 4 warning（= 基线）
xtask card-check / docscan / refscan / memory-counts     → PASSED
xtask adr-index / check-ledger / check-migrations       → PASSED
xtask verify-schemas / codegen --check / check-comments  → PASSED（0 drift；check-comments 0 error / 67 warning）
```

### 4. DoD 逐条核对

- [x] 生产模式可启动：显式装配 `TaskEngine` + `RuntimeExecutor` + `ToolRegistry` + `SnapshotEventSource`，并提供 `--production` / `--serve-ui`；`--self-check` 仍可用 —— **第 2 片已做**
- [x] 5 个 Notepad handler 注册进 `ToolBus` —— **第 2 片已做**（`notepad.tab.new` 因平台缺 tab count 观测而显式 fail-closed，不是假成功）
- [x] 确定性 Plan 来源：同一任务包产出同一 `Plan` —— **已做**（`test_t1_1_package_renders_identical_plan_twice` 断言逐字相同）
- [x] 靶机 T1.1 干跑成功 —— **第 2 片已做**：`production_root_uia`（ignored，显式运行）真实启动 `notepad-like`，经真 UIA + 真 ToolBus + 真 receipt 完成 1 step
- [ ] fail-closed：缺 provider / 空 registry / handler 数不符 → 启动拒绝 —— **部分**（已有缺失 task package / 空 UI peer / 空 registry 校验与 `new_tab` 负向；缺 provider 与 handler 数不足仍缺专门负向用例）
- [ ] 事件经 `ui_server` 推出 `step_state_changed` —— **部分**（`SnapshotEventSource` 投影已测；`serve_with_events` 已接生产入口，但尚未补“同一次运行走真管道”验收）
- [x] 未修改 Out of scope 文件；未新增第三方依赖；未改 `crates/**` 公共接口 —— 已核对（`git diff --stat` 仅本卡 write scope）
- [x] 上列 16 条验收命令全绿 —— **第 2 片已跑**：fmt / workspace clippy / workspace tests / agent-core tests / 全部 xtask 门禁 / `cargo deny check` / UI lint-format-test / ignored 真 UIA 干跑 **全 PASS**；`hygiene` 0E/4W
- [ ] §11.1 进度同步 —— **Done 时执行**（本轮为 InProgress，只追加 LEDGER）

### 5. 偏差

**DRIFT-214-1（ADR-0058 D2 的落地缺口：任务包的 postcondition 不是 `verify` 的形状）**

1. **现象**：ADR-0058 D2 规定 1a 的 Plan 来源是「读 `adapters/com.microsoft.notepad/tasks/t1.*.json` → 渲染计划 JSON」的确定性 `ModelProvider`，而 `RuntimeExecutor` 只走 `verify::parse_postconditions`（`crates/verify/src/postcondition.rs`，**11 种 `kind` 的闭集**，未知 kind 与多余字段一律拒绝）。但任务包的 `postconditions[].id` 是**说明性标识**（`source_file_unchanged` / `line_count_matches_text` / `keyword_paragraphs_match` / `truncation_is_explicit`），`adapters/com.microsoft.notepad/tools/tools.json` 的服务端后置条件同样是说明性标识（`read_only_no_state_change` / `canonical_text_compare` / `text_equals` / `replacement_count_equals` / `title_has_no_unsaved_marker` / `file_content_or_mtime_verified` / `tab_count_increased_by_one` / `file_exists` / `existing_file_not_overwritten`）。**两处都不是可被 `parse_postconditions` 接受的形状**。
2. **影响**：确定性 provider 必须把说明性 id 翻成断言。若让它**凭空生成**断言值（例如猜 `text_equals` 的期望文本），那就是伪造验证 —— 直接违反铁律 1 与铁律 4，并让「写操作必有 postcondition」退化成装饰。仅凭现有任务包与工具声明，机器无法推出这些断言。
3. **建议（不改任务包，也不改 `crates/**`）**：把断言模板做成 **binary 层的显式、可审阅映射表**（每个 1a 工具一条，随确定性 provider 一起提交并接受 review），并把「映射缺失」当作注册期失败（fail-closed）。理由：① 任务包与工具声明属 `adapters/**`，不在本卡 write scope（触发漂移触发器 ⑤）；② 映射表放在 binary 层符合 ADR-0058 D1（装配单点在 binary）；③ 断言值一旦是人工写死且可审阅，就不是"伪造验证"，而是"1a 的确定性断言集"。
4. **已停工作**：本卡**未写任何产品代码**。先记录本 DRIFT 并等人类裁决（漂移触发器 ③⑧：涉及"哪种 postcondition 才是 1a 的合法断言"这一语义决定）。

**DRIFT-214-1 后续（2026-09-30，人类指示「按推荐先做了试试」）**

人类采纳建议 ①，本卡恢复编码：1a 断言映射表落在 `apps/agent-core/src/task_package.rs` 的 `assertion_table()`，覆盖 5 个 Notepad 工具名，**每个断言的取值要么固定、要么取自该步骤自己的参数**（例如 `replace_text` 用 `new_text`、`save_as` 用 `target_path`），不做任何推断。表外工具、未绑定 `$input.` 参数、无工具步骤、非法 JSON 四类一律构造期失败；`tests/task_package.rs` 用 `assistant_verify::parse_postconditions` 反向证明产出的断言确实被 `verify` 接受。任务包与 `crates/**` 未改。

### 6. 更合理做法

把「声明」与「可执行断言」分开是对的：任务包描述的是**意图**（`source_file_unchanged`），`verify` 要的是**可求值的断言**。本轮没有让 provider 去猜，而是把翻译显式化在 binary 层，代价是"新增一个工具就要新增一条映射"——这条成本被 `MissingAssertion` 的 fail-closed 挡住，不会静默漏掉。

### 7. 遗留问题

- 装配根本体（`TaskEngine` + `RuntimeExecutor` + `UiServer` + `SnapshotEventSource` + 生产入口）尚未落地。
- 5 个 Notepad handler 的**实现体**（走 `UiAutomationProvider` / `WindowProvider`）尚未落地；`TargetDescriptor` 与 selector 链目前还没有注入点。
- 靶机 `com.example.notepad-like` 的 selector 与工具声明（`DRIFT-105-2` / `PL-097`）需另立卡。
- `notepad.file.save` / `notepad.tab.new` 目前只断言 `state_changed`；「保存后标题无未保存标记」「标签数 +1」需要 `state_assert` / `value_equals` 的字段口径，待 handler 落地时一并收紧。

### 8. 新增长期记忆

（无长期记忆新增 —— 本轮的结论已由 `DRIFT-214-1` 与 `docs/PARKING_LOT.md` 的 `PL-096` / `PL-097` 承载。）

### 9. 给审阅者的关注点

1. **断言映射表是本轮的审查重点**：`apps/agent-core/src/task_package.rs` 的 `assertion_table()` 是"1a 的合法断言集"的唯一落点。请重点看三个取值是否可接受：`read_text → state_unchanged`、`replace_text → text_contains(new_text) + state_changed`、`save/tab.new → state_changed`、`save_as → file_changed(target_path, any)`。
2. `read_text` 用 `state_unchanged` 而不是 `text_equals`：因为我们没有可信的"期望文本"来源，用等值断言就必须猜，属伪造验证。
3. 本卡仍是 **InProgress**，`PLAN.md` / `README.md` / `plans/*` 尚未按 Done 同步（符合 InProgress 约定）；阶段 1a 仍 **NO-GO**。

### 10. 第 2 片补充记录（2026-10-01：生产装配根 + 5 handler + 真 UIA 干跑）

#### 10.1 实际改动

- 新增生产装配根：`apps/agent-core/src/production.rs`、`production_run.rs`、`production_support.rs`。
- 新增 Adapter 加载与 handler：`notepad_targets.rs`、`notepad_registry.rs`、`notepad_handlers.rs`、`notepad_files.rs`。
- 新增测试：`tests/production_root.rs`（内存平台，真 ToolBus / receipt / event projection）、
  `tests/production_root_uia.rs`（ignored，交互式 Windows 真 UIA 干跑）。
- 改 `main.rs`：新增 `--production` 与可选 `--serve-ui`；保留 `--self-check`。
- 改 `runtime.rs`：`EnvelopeObservationCollector` 读取 `previous_fingerprint` / `elapsed_ms` / `files`，使 verify 不再因缺观测而假失败。
- 改 `ui_server.rs`：新增 `serve_with_events` 包装，复用既有 session loop；未改既有函数语义。
- 改 `task_package.rs`：保留声明 task id，并确定性归一化为 task-engine 合法 id；新增访问器。
- 更新 `apps/agent-core/README.md` 的生产模式边界与已知限制。

#### 10.2 新增偏差/兼容处理

- **DRIFT-214-2（兼容处理，已记 pitfall）**：`tools.json` 对只读工具声明
  `reversibility = "none_readonly"`，但 `ToolSchema` 闭集没有该值。写 scope 不含
  `adapters/**`，因此在 binary 解析层只对该精确组合归一为 `l0_undo_stack`；其他未知值仍 fail-closed。
- **DRIFT-214-3（兼容处理）**：任务包 `task_id`（如
  `notepad.t1_1.open_read_full_text`）含点号，而 task-engine 的 `TaskId` 只接受
  ASCII 字母/数字/`_`/`-`。provider 保留 declared id，并确定性归一化给 task-engine；不猜测语义。
- `notepad.tab.new` 当前**不做不可验证动作**：平台 API 尚无稳定 tab-count observation，
  handler 在动作前返回 `CapabilityMissing`。该缺口与 `DRIFT-105-2` / PL-097 同源。

#### 10.3 验收输出摘要

```text
cargo fmt --all --check                                      → PASS
cargo clippy --all-targets -- -D warnings                    → PASS
cargo test --workspace                                       → 全部 test result: ok（0 failed）
cargo test -p assistant-agent-core                           → 56 passed / 0 failed（另有 1 ignored）
cargo test -p assistant-agent-core --test production_root_uia -- --ignored --nocapture
                                                             → 1 passed / 0 failed
cargo run -p xtask -- hygiene                                → 0E / 4W（= 基线）
xtask verify-schemas / codegen --check                       → PASS
xtask docscan / card-check / memory-counts / adr-index       → PASS
xtask check-ledger / check-migrations / refscan              → PASS
xtask check-comments                                         → 0E / 67W（= 基线）
cargo deny check                                             → advisories/bans/licenses/sources OK
pnpm desktop-ui lint / format:check / test                   → PASS（76 model + 3 DOM）
```

真 UIA 干跑细节：测试临时生成靶机 selector 包，启动 `notepad-like`，经生产根完成
T1.1 的 1 个 read step；断言最终 `TaskStatus::Completed`、1 个 committed snapshot。

#### 10.4 本轮遗留（因此仍不标 Done）

1. `notepad.tab.new` 仍不能在无 tab-count observation 的平台上真实完成。
2. `notepad.file.save_as` 已有实现路径，但靶机缺少跨进程对话框，未取得真实成功证据。
3. “同一次运行经 `ui_server` + 真管道推出 `step_state_changed`”尚未补端到端验收。
4. 缺 provider / handler 数不符的专门负向用例仍未补齐；当前只有装配校验与部分负向。
5. TASK-105 仍不可开工；阶段 1a 仍 **NO-GO**。
