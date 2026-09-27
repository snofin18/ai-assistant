# TASK-029　二进制骨架：`apps/agent-core` + `apps/desktop-ui`（Tauri 2 + React + TS + Tailwind）+ capabilities 最小化 + CSP ＋ **Host 装配**（把 `core` 组件与 platform / tool-bus / policy / storage / audit 组装起来）

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A3**　依赖：028 / 206 / 207 / 208　预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。
- 来源：**ADR-0053 D1 / D5**（人类 2026-09-26 裁决「drift-028 的 5 点都按照你的建议做」→ 采纳 **DRIFT-028-4** 的建议 ②）—— 把「组装」从原 TASK-028 下沉到本卡（binary 才是**唯一装配点**）。
- 契约：`docs/spec/core-orchestration.md`（`core` 的依赖白名单与「不装配」口径）。

---

- **依赖**：028 / 206 / 207 / 208　**预估**：L　**难度**：L
- **write scope**：`apps/agent-core/**`、`apps/desktop-ui/**`、根 `Cargo.toml`、
  `docs/DEPENDENCIES.md`、`xtask/src/exemptions.rs`、
  `docs/adr/0032-doc-rule-exemption-registry.md`
- **关联**：`plans/stage-1-pilots.md` 批次表 A3（1a）、`docs/wbs-overview.md` §6（DoD）

**目标**

二进制骨架：`apps/agent-core` + `apps/desktop-ui`（Tauri 2 + React + TS + Tailwind）+ capabilities 最小化 + CSP；
**并承担 Host 装配**：把 `core` 的编排组件与 `platform` / `tool-bus` / `policy` / `storage` / `audit` 组装成可运行的 Host。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`apps/agent-core/**`、`apps/desktop-ui/**`、根 `Cargo.toml`、`docs/DEPENDENCIES.md`、
`xtask/src/exemptions.rs`、`docs/adr/0032-doc-rule-exemption-registry.md`

**Host 装配（本卡新增，2026-09-26 ADR-0053 D1 / D5）**

- 装配点**唯一**在本卡（binary 层）：`core` 只提供可装配组件，**不**依赖 `tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `lease`（黑名单见 `docs/spec/core-orchestration.md` 不变量 1 与 ADR-0053 D3）。
- 装配必须**显式**且可审计：每个组件的构造依赖（时钟 / 随机 / UUID / FS / 网络 / `ModelProvider` / storage 句柄）都在装配处注入，不在组件内部自取。
- **不新增第二装配点**：不得让 `core` 或 UI 侧各装配一份（否则「策略引擎是唯一放行点」与「无静默失败」都会出现第二个事实源）。
- 装配失败必须**显式**失败（缺组件 / 版本不匹配 / 句柄不可用 → 带 `ErrorCode` 报错退出），**禁止**用默认实现顶替。

**步骤**

1. 展开并固定 Host 装配接口：输入必须显式提供 storage / session store / memory retriever /
   App Map reader / model provider / model router / policy rules / tool registry / compressor /
   platform；缺任一项即带 `ErrorCode` 失败，不提供默认替代。
2. 在 `apps/agent-core` 实现唯一装配点：合并 storage + audit 迁移，打开唯一写连接，构造
   session / context / planner / memory / model-gateway / policy / tool-bus / audit sink。
   并把具体平台实现作为泛型注入。
3. 在 `apps/desktop-ui` 建立 Tauri 2 + React + TS + Tailwind 最小壳：严格 CSP、
   capabilities 空权限、无 shell/fs/http 插件、无远端内容；用静态安全测试覆盖配置。
4. 增加 Host 装配正向测试与缺组件负向测试；增加“装配代码只在 apps/agent-core”的源码断言。
4. 增加 Host 装配正向测试、缺组件负向测试与“装配代码只在 apps/agent-core”的源码断言。
5. 跑 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
   `cargo test --workspace`、全部 `xtask` 门禁与 `cargo deny check`；不合格 → DRIFT。
6. 更新 `docs/DEPENDENCIES.md` 的 Tauri / 前端依赖登记与本卡执行记录。

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] **Host 装配**：`apps/agent-core` 能在测试中装配出 Host（组件全部经注入构造）；缺组件 / 版本不匹配 / 句柄不可用 → **显式失败**（负向用例）
- [ ] **装配点唯一**：`crates/core` 的 `[dependencies]` ⊆ ADR-0053 D2 白名单（黑名单 crate 出现即失败）；装配代码只在 `apps/agent-core`
- [ ] `apps/desktop-ui` 存在 Tauri 2 + React/TS/Tailwind 最小壳；capabilities 不授予 shell/fs/http/系统权限，CSP 严格，且安全配置有静态测试
- [ ] 新增 Tauri / 前端依赖已登记 `docs/DEPENDENCIES.md`，根 `Cargo.toml` 只增加 `apps/agent-core` 的 workspace member
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-029 binary skeleton + Host assembly
【目标】在 binary 层唯一装配 Core/platform/tool-bus/policy/storage/audit，并建立 Tauri 2 UI 壳
【write scope】apps/agent-core/**、apps/desktop-ui/**、根 Cargo.toml、
docs/DEPENDENCIES.md、xtask/src/exemptions.rs、docs/adr/0032-doc-rule-exemption-registry.md
【铁律】装配点唯一；依赖显式注入；缺组件带 ErrorCode 失败；core 不装配
【禁止】core 依赖黑名单 crate；UI 授予系统权限；远端内容；默认 Provider/Store 顶替
【验收】fmt / clippy / test workspace / xtask 全门禁 / cargo deny / Tauri check / pnpm typecheck+build
【依赖】TASK-028 / 206 / 207 / 208 Done
【疑问】无（DRIFT-029-1 已由人类授权扩展 scope；DRIFT-029-2 为 refscan 基线修复）
```

### 2. 实际改动文件

- `apps/agent-core/**`：新增 Host assembly、storage/audit/clock/App Map adapters、可执行 `--self-check` 与 contract tests。
- `apps/desktop-ui/**`：新增 Tauri 2 + React/TS/Tailwind 壳、CSP/capabilities 安全配置、前端构建文件。
- 根 `Cargo.toml`：把 `apps/*` 改为显式 `apps/automation-host` + `apps/agent-core`，避免前端目录被当作 Cargo member。
- `docs/DEPENDENCIES.md`：登记 Tauri、React、Vite、Tailwind、TypeScript 等依赖。
- `xtask/src/exemptions.rs`、`docs/adr/0032-doc-rule-exemption-registry.md`：修复 refscan 豁免解析与登记行漂移。
- `apps/automation-host/tests/acceptance.rs`：范围外阻塞修复，kill 后先排空已排队心跳再断言真实断连（DRIFT-029-3）。
- `tasks/TASK-029-binary-skeleton-agent-core-desktop-ui.md`、`LEDGER.md`、`docs/memory/{facts,pitfalls}.md`：记录与进度。

### 3. 验收输出摘要

- `cargo fmt --all --check` → PASS。
- `cargo clippy --all-targets -- -D warnings` → PASS。
- `cargo test --workspace` → PASS；`assistant-agent-core` 8 个 assembly/security/self-check tests 通过。
- `cargo run -p assistant-agent-core -- --self-check` → PASS，输出 `assistant-agent-core self-check: ok`。
- `HostAssemblyInput<P>` 恢复受 `WindowProvider + UiAutomationProvider` 约束的泛型平台注入；非 Windows 目标在装配入口显式失败，避免 self-check 假阳性。
- `--self-check` 成功路径测试只在 Windows 运行；非 Windows 用 `#[cfg(not(windows))]` 负向测试断言 `platform` 配置错误。
- `assembly_contract` 中四个调用真实装配的 Windows 路径测试同样按目标平台分流；非 Windows 增加整体平台拒绝测试。
- Windows-only 测试专用 imports / helper 依赖用 `#[cfg(windows)]` 拆分，避免 Ubuntu/macOS clippy unused import。
- `cargo run -p xtask -- refscan` → **0 error / 0 warning**（修复前 151 error）。
- `hygiene` 0E/4W、`memory-counts`、`adr-index`、`check-ledger`、`check-migrations`、
  `verify-schemas`、`codegen --check`、`docscan`、`card-check` → 全 PASS。
- `cargo deny check` → advisories / bans / licenses / sources 全 ok。
- `cargo llvm-cov --workspace --fail-under-lines 75` → PASS，行覆盖 **75.13%**（新增 binary 路径测试后）。
- `cargo check --manifest-path apps/desktop-ui/src-tauri/Cargo.toml` → PASS。
- `pnpm typecheck`、`pnpm build` → PASS。
- PR #72 以 **merge commit `21e35bcb27497abb5b1a23a64136fb0ecfdf5ca0`** 合并到 `main`；push / pull_request 两个三平台 CI run 均 **success**，包含 Ubuntu/macOS/Windows 的 fmt、clippy、test、build、xtask 门禁与 coverage。

### 4. DoD 逐条核对

- [x] Host 可在测试中装配；缺 `session_store` 返回 `host_component_missing`。
- [x] `assistant-agent-core --self-check` 可执行；模型运行时显式注入；审计链非完整时 `host_audit_assembly_failed`。
- [x] 平台为受 trait 约束的泛型注入，`()` 等无效实现不能装配；非 Windows 目标 fail-closed。
- [x] self-check 测试按目标平台分流，避免 Ubuntu/macOS CI 因预期 fail-closed 而误报失败。
- [x] assembly contract 测试按目标平台分流；非 Windows 断言整体平台配置拒绝。
- [x] 跨平台 lint 通过；Windows-only imports 不污染非 Windows 编译。
- [x] 装配点只在 `apps/agent-core`；core manifest 黑名单断言通过。
- [x] Tauri/React/TS/Tailwind 壳存在；capabilities 空权限、CSP 无 `unsafe-inline`/`unsafe-eval`，静态安全测试通过。
- [x] Tauri / 前端依赖已登记；根 Cargo member 只新增 `apps/agent-core`。
- [x] fmt / clippy / workspace tests / xtask 全门禁 / deny 全绿。
- [x] LEDGER 与长期记忆已同步。
- [x] PR #72 已合并；远端三平台 CI 全绿。
### 5. 偏差

DRIFT-029-1
现象：本卡正文仍是「派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充」的占位版；
当前 write scope 仅列 `apps/agent-core/**`、`apps/desktop-ui/src-tauri/**`、
`apps/desktop-ui/*.config.*`，但标题要求 Tauri 2 + React + TS + Tailwind，
必然需要 `apps/desktop-ui/package.json`、`apps/desktop-ui/src/**`、锁文件/根级前端配置、
根 `Cargo.toml` 的 workspace member，以及 `docs/DEPENDENCIES.md` 的 Tauri/前端依赖登记。
影响：继续实现必然同时命中漂移触发器 ①（新增第三方依赖）与 ⑤（超出 write scope）；
且缺少展开后的 Host 装配接口、启动顺序、持久化 adapter 与 Provider 边界定义，
无法把「可运行 Host」写成可验收、可复现的任务。
我的建议：由 Orchestrator/人类先把 TASK-029 正文展开，并明确二选一：
① 扩展 write scope 到完整 `apps/agent-core/**`、`apps/desktop-ui/**`、根 `Cargo.toml`、
`docs/DEPENDENCIES.md`（含依赖批准）；或 ② 把 UI 壳与 Host 装配拆成新的独立卡号
（禁止 sub-suffix），本卡只保留可验收的装配骨架。
已停止的工作：未创建分支、未新增目录、未写产品代码；仅完成只读启动检查与漂移登记。
裁决：人类 2026-09-27 明确授权「按你说的继续，并且授权你直到 029 完成」。
范围扩展为 `apps/desktop-ui/**`、根 `Cargo.toml` 与 `docs/DEPENDENCIES.md`；
`DRIFT-029-1` 已闭环，实施继续。

裁决补充：人类 2026-09-27 明确授权“按你说的继续，并且授权你直到 029 完成”。
范围扩展为完整 `apps/desktop-ui/**`、根 `Cargo.toml`、`docs/DEPENDENCIES.md`。

DRIFT-029-2
现象：卡片 DoD 要求 `xtask refscan` PASSED，但基线为 151 error；根因是 ADR-0032
豁免登记中的规则/路径被 Markdown 反引号包裹，解析器未剥离，部分行号已漂移，
且缺少 refscan 自身负向测试夹具的豁免。
影响：本卡无法在不修门禁的情况下满足 DoD，长期红灯会让后续会话误判仓库状态。
建议：修复豁免解析、更新登记行号、补齐测试夹具豁免。
已处理：`xtask/src/exemptions.rs` 剥离反引号；ADR-0032 更新 5 行并新增 4 条豁免；
`refscan` 实测 0E/0W。

DRIFT-029-3
现象：独立 review 后复跑 `cargo test --workspace` 时，既有 `automation-host` 的
`test_kill_host_client_detects_disconnect` 稳定失败；该文件不在本卡 write scope。
影响：workspace 测试无法全绿，TASK-029 不能收口；但失败不是 Host 装配行为回归。
根因：kill 后管道里可能已有 host 排队的心跳，原断言直接要求下一次读取为
Disconnect/Timeout，未先排空心跳。
处理：仅在 2 s 截止时间内跳过 `WireMessage::Heartbeat`，再断言真实断连；
未放宽断连判据，也未删除测试。

Closeout：TASK-029 已由 PR #72 / merge `21e35bc` 合并；push 与 pull_request
两个 CI run 均 success。最终 review 结论：代码层面通过，无剩余阻塞项。

### 6. 更合理做法

Host 装配采用显式输入 + async `assemble`，把 SQLite/audit 句柄留在 binary 层；
Tauri 壳独立 workspace，避免根 `cargo test --workspace` 依赖 Linux WebKit 系统包。
`WindowsPlatform` 当前未实现 `PlatformService`，因此装配用
`WindowProvider + UiAutomationProvider` 约束泛型平台，并显式检查 Windows Host 可用性；
不把未实现的 trait 当作已满足的能力。

### 7. 遗留问题

- PL-092：生产 SessionStore 仍等待 storage conversation/session 记录 API；本卡注入接口。
- 具体模型 Provider 仍未实现；本卡只装配 trait 注入点，Provider 归后续 integration 卡。
- `WindowsPlatform` 未实现 `PlatformService`，需后续平台卡决定是否补齐。
- 卡片正文步骤 4 仍重复一处；正文区只读，未在本卡改写，留给 Orchestrator 整理。

### 8. 新增长期记忆

见 `docs/memory/facts.md` 与 `docs/memory/pitfalls.md` 的本轮追加。

### 9. 给审阅者的关注点

1. `HostAssembly` 是否真正唯一，且 `crates/core` 未出现装配依赖。
2. Tauri capabilities 空权限与无 `unsafe-inline` 的 CSP 是否足以满足“webview 零系统权限”。
3. `refscan` 修复是否只是豁免遮蔽；本次同时修正了解析器反引号与真实行号漂移。
4. 豁免清单解析已改为扫描全部表并拒绝格式错误行；确认没有靠静默跳过维持绿灯。
5. 平台泛型约束与非 Windows fail-closed 是否真正消除了 self-check 假阳性。
6. 合并后三平台 CI 是否仍全绿；最终 merge commit 为 `21e35bc`。
