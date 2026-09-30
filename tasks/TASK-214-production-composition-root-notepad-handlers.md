# TASK-214　生产装配根：真实 Host 进程 + Notepad Host handler + 1a Plan 来源

- 状态：**Ready（ADR-0058 已于 2026-09-30 转 Accepted；可开工）**
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

（待填）

### 3. 验收输出摘要

（待填）

### 4. DoD 逐条核对

（待填）

### 5. 偏差

**DRIFT-214-1（ADR-0058 D2 的落地缺口：任务包的 postcondition 不是 `verify` 的形状）**

1. **现象**：ADR-0058 D2 规定 1a 的 Plan 来源是「读 `adapters/com.microsoft.notepad/tasks/t1.*.json` → 渲染计划 JSON」的确定性 `ModelProvider`，而 `RuntimeExecutor` 只走 `verify::parse_postconditions`（`crates/verify/src/postcondition.rs`，**11 种 `kind` 的闭集**，未知 kind 与多余字段一律拒绝）。但任务包的 `postconditions[].id` 是**说明性标识**（`source_file_unchanged` / `line_count_matches_text` / `keyword_paragraphs_match` / `truncation_is_explicit`），`adapters/com.microsoft.notepad/tools/tools.json` 的服务端后置条件同样是说明性标识（`read_only_no_state_change` / `canonical_text_compare` / `text_equals` / `replacement_count_equals` / `title_has_no_unsaved_marker` / `file_content_or_mtime_verified` / `tab_count_increased_by_one` / `file_exists` / `existing_file_not_overwritten`）。**两处都不是可被 `parse_postconditions` 接受的形状**。
2. **影响**：确定性 provider 必须把说明性 id 翻成断言。若让它**凭空生成**断言值（例如猜 `text_equals` 的期望文本），那就是伪造验证 —— 直接违反铁律 1 与铁律 4，并让「写操作必有 postcondition」退化成装饰。仅凭现有任务包与工具声明，机器无法推出这些断言。
3. **建议（不改任务包，也不改 `crates/**`）**：把断言模板做成 **binary 层的显式、可审阅映射表**（每个 1a 工具一条，随确定性 provider 一起提交并接受 review），并把「映射缺失」当作注册期失败（fail-closed）。理由：① 任务包与工具声明属 `adapters/**`，不在本卡 write scope（触发漂移触发器 ⑤）；② 映射表放在 binary 层符合 ADR-0058 D1（装配单点在 binary）；③ 断言值一旦是人工写死且可审阅，就不是"伪造验证"，而是"1a 的确定性断言集"。
4. **已停工作**：本卡**未写任何产品代码**。先记录本 DRIFT 并等人类裁决（漂移触发器 ③⑧：涉及"哪种 postcondition 才是 1a 的合法断言"这一语义决定）。

### 6. 更合理做法

（待填）

### 7. 遗留问题

（待填）

### 8. 新增长期记忆

（待填）

### 9. 给审阅者的关注点

（待填）
