# ADR-0058　生产装配根与阶段 1a 的 Plan 来源契约

状态：**Accepted**（2026-09-30 人类确认接受；TASK-214 已解锁，可开工）
日期：2026-09-30
Supersedes：—
Superseded by：—
关联：**TASK-105**（1a 唯一硬缺口）、TASK-103、TASK-213、TASK-029、TASK-035、TASK-036~038、
ADR-0053、ADR-0056、ADR-0057、ADR-0022、ADR-0055、`docs/spec/runtime-execution.md`、
`docs/audits/stage-1a-integration-audit-2026-09-29.md`、`docs/memory/open.md` M3

---

## 背景

TASK-029 交付的是**执行自检装配点**，不是生产装配根。`apps/agent-core/src/main.rs` 的文件头
自述即为此意：*"The production composition root will supply a real Provider, persistent
`SessionStore`, and adapter package."* 当前二进制只有 `--self-check` 一条路径，且它装配的是
`NoopProvider` / `EmptyRetriever` / `EmptyReader` / `NoopCompressor` 与**空** `ToolRegistry`。

TASK-103 已经把执行链路落进 binary 层：`RuntimeExecutor`（单步 Resolve→Policy→Execute→Verify→Commit）、
`ToolBusInvoker`（真 MCP 往返）、`EnvelopeObservationCollector`（指纹取自 Host 返回值）、
`HostAssembly`（会话 / 上下文 / Planner / Memory / ModelGateway / Policy / ToolBus / Audit）。
TASK-213 又把 UI 侧通路补齐，并留下**一行装配工作**：把 `RuntimeExecutor` 的引擎接成
`SnapshotEventSource` 的 provider。

但截至 2026-09-30，**没有任何卡拥有"生产装配根"这一实体**，因此三块硬前置全部悬空：

| 缺失实体 | 现状 | 后果 |
|---|---|---|
| **生产装配根** | 只有 `--self-check`；`HostAssembly` 不拥有 `TaskEngine` / `RuntimeExecutor` / `ToolRegistry` 内容 / `UiServer` / `SnapshotEventSource` | 没有可运行的 Host 进程 |
| **Notepad Host handler** | `crates/tool-bus` 的 handler 只在测试里存在（`apps/agent-core/tests/runtime_toolbus.rs` 的 `WriteTextHandler`）；生产 `ToolRegistry` 为空 | ToolBus 上没有任何可调工具 |
| **Plan 来源** | `Planner` 需要 `Arc<dyn ModelProvider>`；生产实现不存在，只有 `NoopProvider` | 无法从意图产生 `Plan` |

TASK-105（T1.1~T1.3 各 10 次靶机运行）依赖以上三者，因此它**不是"可以做但没做"，而是"没有可运行对象"**。
贸然开工只能得到伪造运行证据，违反铁律 1 与卡面禁项。

关键约束：`docs/memory/open.md` **M3** 已把「`local_only` 档所需的本地模型选型与硬件门槛（Ollama /
llama.cpp）」判给**阶段 1c 前**，即**真实 LLM 不在 1a 范围内**。而 1a 的 DoD 检验对象是
「受控执行链路」——真 UIA 定位与动作、真 MCP `ToolBus` 往返、真 `VerificationReceipt`、
真 task-engine 提交、真撤销与审批——**不是模型质量**。

这是一次决定**分层归属 + "1a 的真实运行"口径**的决策（漂移触发器 ③⑧），必须先有 ADR（铁律 10）。

## 决策（一句话）

**生产装配根由 `apps/agent-core` 的 binary 层独占；阶段 1a 的 Plan 来源是「声明式任务包 → `Plan`」的
确定性 `ModelProvider` 实现（不是 LLM）；Notepad Host handler 在 binary 层实现并注册进 `ToolBus`；
真实 LLM Provider（本地或云）不属 1a，另立卡并另起网络/依赖 ADR。**

## 决策细化

| # | 内容 |
|---|---|
| **D1 装配归属** | 生产装配根 = `apps/agent-core` binary 层新增的**生产模式**（与 `--self-check` 并列的独立入口/子命令）。它独占构造 `TaskEngine` + `RuntimeExecutor`、`ToolRegistry`（含 Notepad handlers）、`UiServer`（复用 TASK-213 的 `ui_server`）、`SnapshotEventSource`。**不进入 `crates/core`**（ADR-0053 D2：组装单点在 binary，core 只提供可装配组件）。 |
| **D2 Plan 来源（1a）** | 确定性 `ModelProvider` 实现：读 `adapters/com.microsoft.notepad/tasks/t1.*.json`（TASK-036~038 的声明式任务包）→ 渲染为计划 JSON 的确定性 completion；`model_id = "task_package"`。**同一输入必得同一 `Plan`**，不引入随机性。这是 1a 对"真实 provider"的口径。 |
| **D3 真实 LLM 不在 1a** | 任何 HTTP/云/本地推理 Provider（新增 `reqwest` 等依赖、网络出口、凭据）都与 **M3** 一致地推迟到 **1c 前**，届时另立 ADR + 卡。本 ADR **不授权**任何新第三方依赖。 |
| **D4 Notepad Host handler** | 在 binary 层实现 5 个 handler（`notepad.file.read_text` / `notepad.file.replace_text` / `notepad.file.save` / `notepad.tab.new` / `notepad.file.save_as`，形状取自 `adapters/com.microsoft.notepad/tools/tools.json`），经 `ToolBus` 分发；定位与动作走 `assistant_platform_windows` 的 `UiAutomationProvider` / `WindowProvider`，**在目标窗口 scope 内**解析（ADR-0043）；`Element`/句柄不跨进程（铁律 8）。 |
| **D5 目标绑定** | 1a 的目标 = 显式注入的 app 身份 + adapter 的 selector 候选链；窗口定位走 **owner PID**（`GetWindowThreadProcessId`，ADR-0022 D1），**禁止**用启动返回的 PID 或 `MainWindowHandle` 启发式。 |
| **D6 指纹与验证** | pre/post fingerprint 由 Host 采集并放进 `ToolEnvelope.data.fingerprint`；验证只走 `verify_postconditions_with_receipt`（ADR-0056 D3）。缺指纹 = `NeedsHuman`，不得编造默认值。 |
| **D7 事件接线** | `RuntimeExecutor` 的 `TaskEngine` 接成 `SnapshotEventSource<Provider>`（TASK-213 留下的那一行）；provider 由装配根注入，`SnapshotEventSource` 不持有引擎、不加锁。 |
| **D8 fail-closed 启动** | 缺 provider / handler 数 ≠ 声明数 / 空 `ToolRegistry` / 缺 `UiServer` 配置 —— 一律**拒绝启动**并返回带 `ErrorCode` 的错误，不得降级成 self-check 或 Noop。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 让 `--self-check` 顺便充当生产根（用 `NoopProvider` 跑 T1.x） | ❌ | 等于伪造运行证据，违反铁律 1 与 TASK-105 卡面「不得从静态用例推算」；且空 `ToolRegistry` 根本调不到工具 |
| 1a 就引入真实 LLM Provider（本地 OpenAI-compatible / 云） | ❌ | 触发漂移触发器 ①（新第三方依赖：HTTP client）与 ⑪（真实网络/凭据）；且与 `open.md` **M3**（本地模型选型归 1c 前）冲突 |
| 用通用 `run_shell` / `run_powershell` 工具驱动 Notepad | ❌ | AGENTS §7 永久禁止注册通用 shell；且等于放弃可控性（`rejected.md` 2026-09-16） |
| 把装配根写进 `crates/core` | ❌ | 违反 ADR-0053 D2/D3（core 黑名单含 `tool-bus` / `policy` / `audit` 等，装配单点在 binary） |
| 让 UI 进程持有 provider / handler 并直接执行 | ❌ | 违反铁律 3（策略引擎是唯一放行点）与 ADR-0057 D1（UI 零系统权限、不链接 core） |
| 为 1a 新建独立 crate 放 handler | ❌ | 漂移触发器 ②；且 handler 依赖 binary 层已注入的 `ToolBus` 与目标绑定，没有独立生命周期理由 |

## 影响

- 新增卡 **TASK-214**（生产装配根 + Notepad Host handler + Plan 来源）——状态 = Ready，**开工前置 = 本 ADR 转 Accepted**。
- **TASK-105 在本 ADR + TASK-214 完成前仍不可开工**；阶段 1a 结论不变（**NO-GO**）。
- 1a 的"真实运行"口径被本 ADR 钉死为**执行链路真实 + Plan 来源确定性**；`PLAN.md` 当前状态块里
  「真实 ModelProvider」的措辞在人类接受本 ADR 后需一并澄清（否则同一句话有两个事实源）。
- `crates/**` 的**公共接口不需要改动**：`ModelProvider` trait、`ToolHandler` trait、`ToolRegistry`、
  `UiServer`、`SnapshotEventSource` 均已存在。若实现中发现必须改公共接口 / schema / ErrorCode
  → **回本 ADR 补充**，不得顺手改（触发器 ③）。
- 不新增第三方依赖、不新增 crate、不改 lint 政策。

## 验证方式

1. **负向（fail-closed）**：缺 provider / `ToolRegistry` 为空 / handler 数与声明不符 → 启动拒绝且错误带
   `ErrorCode`（N1 单测 + N2 CI 显式失败步骤，ADR-0019 负向验证）。
2. **正向（靶机）**：在 `fixtures/apps/notepad-like` 上跑 T1.1 一次 → `ToolBus` 调用数 = 1、
   Host 返回值带合法 `fingerprint`、verify 铸造 receipt、task-engine 提交成功。
3. **事件**：同一次运行经 `ui_server` 推出 `step_state_changed`，UI 侧经 zod 校验后落到时间线模型。
4. **静默失败 = 0**：任何失败路径都必须带 `ErrorCode` 与可读说明，不得吞错。
5. **确定性**：同一任务包跑两次得到同一 `Plan`（D2 的可回放性判据）。

## 重新评估触发条件

- 若 TASK-214 实现时发现必须改动 `crates/**` 的公共接口 / schema → 回本 ADR 补充决策；
- 若 1a 的 DoD 被人类改为包含真实模型 → 本 ADR D2/D3 作废，改走"1a 引入 Provider"的新 ADR；
- 若 Linux/macOS 的 Host handler 需要不同的目标绑定协议 → 抽平台差异，**不复制本契约**；
- 若 `SnapshotEventSource` 的去重口径（按 `TaskSnapshot.revision`）在长任务下产生堆积 → 另立卡，
  不在本 ADR 范围。

## 相关 ADR

- **ADR-0053**：core 依赖白名单与 binary 装配点（D1 的依据）。
- **ADR-0056**：运行执行链路契约（D6/D7 的依据）。
- **ADR-0057**：UI↔Core 传输契约（D1 复用 `ui_server` 的依据）。
- **ADR-0022**：Windows 目标身份（D5 的依据）。
- **ADR-0043**：元素解析必须有 scope（D4 的依据）。
- **ADR-0055**：`effect` / `reversibility` 由可信工具目录注入（D4 handler 注册期的依据）。
