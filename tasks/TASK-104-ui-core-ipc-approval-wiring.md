# TASK-104　UI ↔ Core typed IPC 与审批接线

- 状态：**Done（2026-09-29；LEDGER Done + PR #100 / merge `e397f9a`；UI/IPC approval wiring）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：102、103
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：TASK-029/030/031/032、TASK-102/103、`apps/desktop-ui/**`

## 目标

把桌面 UI 与 Core 真正接起来：typed IPC command、审批请求/结果、执行时间线、
暂停/接管操作只通过受校验的边界传递，禁止 UI 自行判断权限或直接调用平台层。

## In scope

- `apps/agent-core/src/**` 的 IPC command / event 适配层。
- `apps/desktop-ui/src/**` 的调用、状态与审批界面接线。
- `apps/desktop-ui/src-tauri/**` 的必要的 command 注册。
- 相关测试与 README。

## Out of scope

- 真实 Notepad 10 次运行验收。
- 修改 Policy 或 Host 核心逻辑。
- 新 Provider、DLP 或浏览器功能。

## 必须遵守

- UI 不直接调用 Host 或平台 provider。
- 审批 scope、diff、来源归因、point-of-no-return 全部经运行时校验。
- IPC 输入是不可信输入，必须拒绝未知字段/非法状态。
- 审批拒绝或接管后必须停止对应执行。
- 仍保持 UI 无系统权限和严格 CSP。

## 验收命令

```powershell
cargo test --workspace
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
pnpm --dir apps/desktop-ui build
cargo run -p xtask -- docscan
```

## 完成定义（DoD）

- [ ] 至少一条 UI 发起的执行请求能到达 Agent-core。
- [ ] 审批卡能收到并回传结构化决策。
- [ ] 时间线能显示真实执行与验证结果。
- [ ] IPC 拒绝非法输入且不产生静默失败。
- [ ] 前端 lint/test/typecheck/build 全绿。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-104 UI ↔ Core typed IPC 与审批接线        【目标】UI 经受校验边界发出执行请求、回传审批决策、显示真实时间线；非法 IPC fail-closed
【write scope】仅：apps/agent-core/src/**、apps/desktop-ui/src/**、apps/desktop-ui/src-tauri/**、相关测试与 README、docs/DEPENDENCIES.md（登记规则 1 要求）、本卡记录区、LEDGER.md、PLAN.md、README.md、plans/stage-1-pilots.md、docs/PARKING_LOT.md
【铁律】1 无静默失败（拒绝带 ErrorCode）；2 UI 输入与 IPC 返回都是不可信输入；3 Policy 是唯一放行点（UI/审批卡不判权限）；9 不得静默扩大范围（新增 zod 依赖已先升级获批 + 登记）；10 契约先行（ADR-0056 D9 / runtime-execution §6）
【禁止】真实 Notepad 10 次运行；改 Policy 或 Host 核心逻辑；新 Provider / DLP / 浏览器功能
【验收】cargo test --workspace；pnpm --dir apps/desktop-ui lint/typecheck/test/build；xtask docscan → 全绿
【依赖】TASK-102、TASK-103（均 Done，已核对 LEDGER）
【疑问】AGENTS §5.4 要求 IPC 运行时校验用 zod，而 zod 未安装（漂移触发器 ①）。人类 2026-09-29 裁决「可以引入 zod 依赖」→ 已登记 docs/DEPENDENCIES.md 后引入。
```

### 2. 实际改动文件

| 文件 | 改动 |
|---|---|
| `apps/agent-core/src/ui_ipc.rs`（新增） | UI↔Core 1.0 契约：`UiCommand` / `UiCommandOutcome` / `UiEvent`、`UiCommandError`（→ `ErrorCode`）、`parse_ui_command`（信封版本 + kind 白名单 + 未知字段拒绝）、`dispatch_ui_command`（解析通过才触达处理器）、`project_snapshot_events` |
| `apps/agent-core/src/ui_control.rs`（新增） | `TaskControlHandler`：把 approve/deny/pause/cancel/takeover 应用到真实 task-engine；pending-approval 注册表；引擎错误 → 稳定错误码 |
| `apps/agent-core/src/lib.rs`、`Cargo.toml` | 导出两个模块；新增 `serde` 依赖（已登记的既有依赖，新增使用方） |
| `apps/agent-core/tests/ui_ipc.rs`（新增） | 10 个契约用例：黄金样本解析、命令恰好触达处理器一次、三类非法样本 0 次触达、deny 真的把步骤停掉、scope 未提供/未知审批/未知任务拒绝、pause→takeover→cancel 状态推进、committed 步骤投影出真实 post fingerprint |
| `apps/agent-core/tests/fixtures/ui_ipc/*.json`（新增 7 个） | 两侧共享的黄金样本（4 正 + 3 负） |
| `apps/desktop-ui/src/ipc/contract.ts`（新增） | zod 1.0 契约（`z.strictObject` + discriminatedUnion）与 `parseUiCommandEnvelope` / `parseUiEvent` / `parseUiCommandOutcome` |
| `apps/desktop-ui/src/ipc/client.ts` | `sendUiCommand`：发送前校验命令、渲染前校验 Core 返回；`HostIpcError` 带稳定 code |
| `apps/desktop-ui/src/ipc/contract.test.mjs`（新增） | 解析同一批黄金样本 + 未知字段/空标识/非法 outcome 拒绝 |
| `apps/desktop-ui/src/features/approval/approvalCommand.ts` + 测试（新增） | 审批决定 → `approve_request` / `deny_request`；空 reason 拒绝 |
| `apps/desktop-ui/src/features/timeline/timelineEvents.ts` + 测试（新增） | Core 事件 → 时间线状态；未知状态直接丢弃，不猜成功 |
| `apps/desktop-ui/src/features/intent/intentModel.ts` / `intentCopy.ts` / `IntentLauncher.tsx` + 测试（新增） | 第一个真实 UI → Core 调用点（提交用户意图），失败显式展示 |
| `apps/desktop-ui/src/App.tsx` | 渲染 `IntentLauncher` |
| `apps/desktop-ui/package.json` / `pnpm-lock.yaml` | 新增 `zod@4`；`test` glob 扩到 `src/**/*.test.mjs` |
| `apps/desktop-ui/src-tauri/src/commands.rs`（新增）、`src/lib.rs` | `send_ui_command`：本地 fail-closed 校验信封 + 经 `CoreCommandTransport` 转发；`run_with_state` 注册 command |
| `docs/DEPENDENCIES.md` | 登记 `zod@4`（Approved，人类 2026-09-29）；`serde` 使用方补 `apps/agent-core` |
| 三个 `README.md` | agent-core / desktop-ui 职责、不变量、已知限制同步 |

### 3. 验收输出摘要

```text
cargo fmt --all --check                         → PASS（0 diff）
cargo clippy --all-targets -- -D warnings       → PASS（exit 0）
cargo test --workspace                          → PASS（1009 passed / 0 failed）
pnpm --dir apps/desktop-ui lint                 → PASS
pnpm --dir apps/desktop-ui typecheck            → PASS
pnpm --dir apps/desktop-ui test                 → PASS（76 passed / 0 failed）
pnpm --dir apps/desktop-ui build                → PASS（dist 产出）
cd apps/desktop-ui/src-tauri && cargo check/test → PASS（4 个边界校验用例；该 workspace 不在 CI 内，见 §7）
cargo run -p xtask -- check-migrations / hygiene（0E/4W）/ verify-schemas / codegen --check / memory-counts / adr-index / refscan / docscan → 全 PASS
```

合并证据：PR #100（base `main`）—— push run `36523017341` 与 pull_request run `36523044998` 均 `completed/success`，各 9/9 job success，PR 汇总 18 个 status context 全 success（含新增 `zod` 后的 `desktop-ui checks`）；合并前 `mergeable=MERGEABLE` / `merge_state_status=CLEAN`；merge commit `e397f9a`。

### 4. DoD 逐条核对

- [x] **至少一条 UI 发起的执行请求能到达 Agent-core**：`IntentLauncher` 是真实调用点；`apps/agent-core/tests/fixtures/ui_ipc/submit_intent.json` 由 TS 客户端与 Core 解析器**两侧各解析一遍**，Core 侧 `dispatch_ui_command` 用例断言命令恰好触达处理器一次。
- [x] **审批卡能收到并回传结构化决策**：`approvalCommand.ts` 把 approved/denied 翻成 `approve_request` / `deny_request`；Core `TaskControlHandler` 真的执行 approve（校验 scope 已提供）与 deny（`deny_step` 把步骤置 `PolicyDenied`、任务置 `Failed`）。
- [x] **时间线能显示真实执行与验证结果**：`project_snapshot_events` 从 task snapshot 投影，committed 步骤带真实 post fingerprint；`timelineEvents.ts` 只在事件匹配且状态已知时更新，未知状态丢弃。
- [x] **IPC 拒绝非法输入且不产生静默失败**：两侧各有未知字段/未知 kind/版本漂移的负向用例，且断言非法样本 **0 次触达处理器**；拒绝全部带稳定 code。
- [x] **前端 lint/test/typecheck/build 全绿**：见 §3。
- [x] **未修改 Out of scope 文件**：未触碰 Policy、Host 核心逻辑、Provider、DLP、浏览器代码。

### 5. 偏差

- **DRIFT-104-1（新增第三方依赖，已裁决）**：AGENTS §5.4 要求 IPC 运行时校验用 `zod`，但仓库没有该依赖 → 命中漂移触发器 ①。已按协议停止并向人类升级；人类 2026-09-29 回复「可以引入 zod 依赖」。落地顺序遵守登记规则 1（先在 `docs/DEPENDENCIES.md` 登记 `zod@4`，再改 `package.json` / `pnpm-lock.yaml`）。`docs/DEPENDENCIES.md` 不在卡面 write scope 列表内，但登记规则 1 明文要求「先登记后引入」，故属必要动作。
- 无其他偏差。

### 6. 更合理做法

用**两侧共享的黄金样本**而不是各自手写断言：同一批 JSON 同时被 zod schema 与 Rust 解析器解析，任何一侧改契约都会让另一侧红，避免"两份实现各自自证"。Core 侧的未知字段判定用显式 allow-list 而不是 `serde(deny_unknown_fields)`，因为内部标签枚举不支持该属性 —— 显式 allow-list 也更容易给出「哪个字段越界」的可读错误。

### 7. 遗留问题

- **UI↔Core 的真实传输未接通**：`src-tauri` 已本地校验并暴露 `CoreCommandTransport` 边界，但未注入实现时显式返回 `core_transport_unavailable`。Core 进程侧的 UI 命令监听端（NamedPipe 服务 + 认证）需要新的线上契约 → 应立卡并走 ADR（已记 PL-095）。
- **Core → UI 事件推送未接通**：事件解析与时间线投影已实现并测试，但推送通道同样待接（同 PL-095）。
- **`src-tauri` 不在任何 CI 门禁内**：`cargo test --workspace` 覆盖不到这个独立 workspace，本卡只能本地 `cargo check` / `cargo test` 取证；归 TASK-210。
- **审批授权尚未落库**：`TaskControlHandler` 只在进程内记录 pending approval，重启即失（与 PL-092 同源）。

### 8. 新增长期记忆

无新增 `docs/memory/*` 条目；契约与边界写入 ADR-0056、`docs/spec/runtime-execution.md` 与两个 crate README，缺口记 PL-095。

### 9. 给审阅者的关注点

1. **DoD #1 的口径**：本卡以「同一批黄金样本被 UI 客户端与 Core 解析器双向验证 + Core 侧 dispatch 用例」证明请求能到达 Agent-core；真实 NamedPipe 传输留待 PL-095。若你认为必须端到端跑通管道才算达标，请退回并先立传输卡。
2. **`docs/DEPENDENCIES.md` 越出卡面 write scope**：属登记规则 1 的必要动作（先登记后引入），已随依赖批准一并记录为 DRIFT-104-1。
3. **`send_ui_command` 默认 fail-closed**：未注入传输时返回 `core_transport_unavailable` 而不是空成功；请确认这符合"不伪造成功"的要求。
