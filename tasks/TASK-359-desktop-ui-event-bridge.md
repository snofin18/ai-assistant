# TASK-359 desktop-ui/Tauri Core 事件桥与审批卡接线

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：T5.2 产品化
- 依赖：ADR-0057、ADR-0101、ADR-0103、TASK-344、TASK-355、TASK-358
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。分界线以上由 Orchestrator 所有。

---

- **write scope**：
  `apps/desktop-ui/src-tauri/src/event_bridge.rs`、
  `apps/desktop-ui/src-tauri/src/lib.rs`、
  `apps/desktop-ui/src/ipc/client.ts`、
  `apps/desktop-ui/src/ipc/eventStream.ts`、
  `apps/desktop-ui/src/ipc/eventStream.test.mjs`、
  `apps/desktop-ui/src/App.tsx`、
  `apps/desktop-ui/src/App.test.tsx`、
  `apps/desktop-ui/README.md`、
  `tasks/TASK-359-desktop-ui-event-bridge.md`、
  `docs/memory/{facts,pitfalls}.md`、
  `PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`MEMORY.md`、`LEDGER.md`

## 目标

把既有 Core → UI `UiEvent` 单向流接入 Tauri 和 React：Tauri 只启动一个有界后台订阅，
将事件原样作为可序列化 payload 推给 webview；前端在 zod 校验后把
`step_approval_requested` 映射为审批卡，并复用既有 `approve_request` / `deny_request`
命令把用户决定交回 Core。本卡不执行工具、不做授权判断。

## In scope

- 复用 `CorePipeTransport::subscribe` 与 ADR-0057 的既有事件流，不新增 wire 字段。
- Tauri 侧单实例事件泵；重复启动、事件发送失败和 Core 断连均显式上报。
- 前端统一订阅/清理 API；未知事件、非法 payload 与桥接失败不得静默吞掉。
- 主界面显示最新浏览器表单审批事件；approve/deny 只发送既有 UI 命令。
- 离线 fake source / mocked Tauri 测试与文档同步。

## Out of scope

- 不修改 `UiEvent`、`UiCommand`、Tool、protocol、DB schema 或 ErrorCode。
- 不新增第三方依赖、不启动真实浏览器或真实 GUI、不执行 `form_submit`。
- 不改 `browser_task_runtime.rs`、不把 TASK-057 / TASK-058 标为 Done。
- 不实现任务时间线的完整持久化、自动重连或命令自动重放。

## 必须遵守

1. UI 和 Core 返回都是不可信输入；前端必须经既有 zod 边界后才能渲染。
2. 事件泵最多一个；停止后必须清除运行标记，失败必须可见。
3. 只传可序列化数据；审批卡不得持有 DOM element、句柄或原始表单值。
4. 审批决定只调用既有 `sendUiCommand`，不得在 UI 推断权限或执行工具。
5. 断连不自动重放命令；重连只允许由用户/后续卡显式触发。

## 验收命令

```powershell
cargo test --manifest-path apps/desktop-ui/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-ui/src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
cargo run -p xtask -- replay --suite core
```

## DoD

- [ ] Tauri 事件泵单实例、断连/发送失败显式上报，并有 fake source 测试。
- [ ] 前端监听在渲染前执行 zod 校验，未知事件/坏 payload 有可见错误。
- [ ] 主界面能把浏览器 `step_approval_requested` 显示为审批卡并发送 approve/deny。
- [ ] 未新增依赖，未改 wire/schema/公共 Tool 接口，未启动真实浏览器。
- [ ] 全部门禁与专项测试通过，执行记录、LEDGER 与进度文件同步。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写；改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-359 desktop-ui/Tauri Core 事件桥与审批卡接线
【目标】把既有 Core→UI UiEvent 流接入 Tauri/React，并显示浏览器审批卡
【write scope】卡片正文所列 desktop-ui、事件桥测试、记忆与状态同步文件
【铁律】1 无静默失败；2 UI/IPC 先校验；8 只传可序列化数据；9 不扩范围；10 契约先行
【禁止】不改 UiEvent/Tool/protocol/DB schema，不加依赖，不启动真实浏览器/GUI
【验收】Tauri tests/clippy；UI lint/typecheck/test/build；AGENTS §6 全量门禁
【依赖】ADR-0057 / 0101 / 0103；TASK-344 / 355 / 358 Done
【疑问】无
```

### 2. 实际改动文件

- `apps/desktop-ui/src-tauri/src/event_bridge.rs`（新增）：单实例 Core 事件泵、tagged
  event/failure payload、fake source 负向测试。
- `apps/desktop-ui/src-tauri/src/lib.rs`：注册事件泵 command，并让真实 Core pipe
  同时供命令 transport 与事件 source 复用。
- `apps/desktop-ui/src/ipc/eventStream.ts`（新增）：bridge envelope strict parser。
- `apps/desktop-ui/src/ipc/client.ts`：监听注册、zod 校验、StrictMode
  `already_running` 重连语义。
- `apps/desktop-ui/src/App.tsx`、`App.test.tsx`（新增）：审批卡、脱敏字段 diff、
  approve/deny 命令接线与 DOM 测试。
- `apps/desktop-ui/src/ipc/eventStream.test.mjs`（新增）：bridge parser 正负样本。
- `apps/desktop-ui/README.md`、本卡、`docs/memory/{facts,pitfalls}.md`、
  `MEMORY.md`、`plans/stage-1-pilots.md`、`LEDGER.md`：证据与状态同步。

### 3. 验收输出摘要

- `cargo test --manifest-path apps/desktop-ui/src-tauri/Cargo.toml`：16 passed。
- `cargo clippy --manifest-path apps/desktop-ui/src-tauri/Cargo.toml --all-targets -- -D warnings`：
  exit 0。
- `pnpm --dir apps/desktop-ui lint` / `typecheck`：exit 0。
- `pnpm --dir apps/desktop-ui test`：model 89 passed；Vitest 2 files / 5 tests passed。
- `pnpm --dir apps/desktop-ui build`：exit 0。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`：exit 0。
- `cargo test --workspace`：510 passed；`cargo test -p assistant-core arch::`：10 passed。
- `verify-schemas` / `codegen --check` / `hygiene` / `memory-counts` / `adr-index` /
  `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments` /
  `check-migrations`：PASSED，0 error。
- `cargo deny check`：advisories / bans / licenses / sources ok。
- `cargo run -p xtask -- replay --suite core`：2 steps、5 changes、0 mismatch。
- 2026-11-02 merge evidence：PR #691 head `a6d4836a7e1f9c3e08c6e79dd0b6e0eb4d736baf`
  的 11 项 checks 全部 success；合并提交
  `ebe7a3fd4696c2a1154f5b40257f055149caa9cd`；post-merge main CI
  `41425763958` 为 success。

### 4. DoD 逐条核对

- [x] Tauri 事件泵单实例、断连/发送失败显式上报，并有 fake source 测试。
- [x] 前端监听在渲染前执行 zod 校验，未知事件/坏 payload 有可见错误。
- [x] 主界面能把浏览器 `step_approval_requested` 显示为审批卡并发送 approve/deny。
- [x] 未新增依赖，未改 wire/schema/公共 Tool 接口，未启动真实浏览器。
- [x] 专项与全量门禁通过；执行记录、LEDGER 与阶段计划已同步。

### 5. 偏差

- 本会话机器统计到 **22** 次误填 `shell:"bash"`；ADR-0100 的 PreToolUse hook 均将其
  重写为显式 PowerShell，没有进入 WSL/bash。误填本身仍记缺陷；所有写入均为预期文件，
  无目录外写入或产品文件意外改动。
- 初版 bridge parser 在 Node type-stripping 下无法解析运行时 `contract.js`；改为注入
  既有 `parseUiEvent` parser，保持单一契约源。
- 现有 `ApprovalCard` 不变量要求 L3 使用 `irreversible` diff，而浏览器事件合法地同时
  携带字段 diff；App 层独立展示字段 diff，同时给卡片使用不可逆警告模型，不改 wire。
- 代码与测试净增超过 400 行建议预算（事件泵、bridge parser、客户端、App 接线及
  正负测试约 760 行）；本卡保持单卡闭环，未为过线拆出只可编译的半桥接。

### 6. 更合理做法

- 事件泵与 webview listener 分离生命周期：Core 流保持单实例，React StrictMode
  只重建 listener，不重启泵。
- 审批数据仍只从既有 `UiEvent` 和 zod 边界进入；UI 不自行推断权限或执行工具。

### 7. 遗留问题

- desktop-ui 目前为骨架页面；任务时间线的持久化展示仍非本卡范围。
- 未启动真实 Core pipe / 真实浏览器做人工端到端；TASK-057 / TASK-058 保持 Blocked。
- merge hash / PR checks / post-merge main CI 与最终 shell misuse 已由本回填提交记录。

### 8. 新增长期记忆

- FACT：Core→Tauri→React 事件桥已落地，详见 `docs/memory/facts.md`。
- PITFALL：React StrictMode 的双 effect 必须把 `already_running` 当作重挂 listener，
  详见 `docs/memory/pitfalls.md`。

### 9. 给审阅者的关注点

- `event_bridge.rs` 是否始终只有一个线程，且事件/失败都通过同一 tagged channel 可见。
- `connectUiEventStream` 的 StrictMode 重连分支是否没有启动第二条 Core 订阅。
- App 是否在 wire 不变的前提下同时保留 redacted 字段 diff 与 L3 `once` 确认边界。
