# TASK-214　生产装配根：真实 Host 进程 + Notepad Host handler + 1a Plan 来源

- 状态：**Ready（开工前置 = ADR-0058 转 Accepted）**
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
