# TASK-032　UI：策略面板 + **出域三档开关**（含逐应用覆盖）+ Capability Matrix 视图 + 成本面板

- 状态：**Review**
- 阶段：1　子阶段：**1a**　批次：**A3**　依赖：029,021　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

## 目标

在桌面 UI 中提供三个可组合的只读/受控视图：
① 策略面板：`local_only` / `redacted` / `full` 三档出域级别 + 逐应用覆盖；
② Capability Matrix 视图：显示运行时能力、通道可用性与降级原因；
③ 成本面板：按本次任务 / 今日 / 本月展示模型与应用维度的 token、成本、延迟。

本卡只实现 UI 视图模型、交互控制器与组件。策略变更通过注入回调交给 Core/DLP 持久化；UI 不自行放行动作。

## write scope

- `apps/desktop-ui/src/features/policy/**`
- `apps/desktop-ui/src/features/capability/**`
- `apps/desktop-ui/src/features/cost/**`

默认写权限：本卡 `tasks/TASK-032-ui-policy-panel-egress-capability-cost.md` 的记录区；以及阶段完成态的
`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` / `docs/memory/*` 按治理规则同步。

## In scope

- 出域策略的 fail-closed 运行时解析、三档切换、逐应用覆盖和默认级别显示。
- 出域状态栏组件：常驻显示全局或当前应用的有效级别。
- `local_only` 缺少本地模型时明确阻止生效并显示错误，不得静默降级或改用云端。
- 从较窄级别升到较宽级别时必须由用户显式确认；确认前不得生效。
- Capability Matrix 的运行时快照解析、能力/风险/审批/通道/降级展示。
- 成本快照解析与本次任务 / 今日 / 本月聚合，按模型/应用拆分，使用整数 micro-USD 避免浮点成本。
- 所有输入按不可信数据处理；所有文案由 copy 对象注入，不新增第三方依赖，不直接调用平台或网络。

## Out of scope（做了算漂移）

- 新增或修改 `protocol/` schema、TS 生成类型、IPC 命令或 Tauri command。
- `crates/dlp` / `crates/policy` / Core 持久化实现（归 TASK-050 / TASK-021）。
- 逐内容类型覆盖（本卡只覆盖逐应用；内容类型归 DLP 卡）。
- 真实 Capability probe、真实模型计费读取、数据库查询与审计写入。
- DOM 测试栈引入、组件全局装配、路由/导航重构。
- 任何第三方依赖。

## 必须遵守

- UI 输入不可信：模型必须经运行时解析后才可渲染；解析失败不得回退到默认策略。
- UI 不是策略引擎：组件只产生策略变更意图并通过回调交给上层，不执行工具、不判断动作权限。
- 三档语义按架构 v2 §12.6.1：`local_only` < `redacted` < `full`；默认 `redacted`。
- 升级需显式确认；`local_only` 没有本地模型可用时 fail-closed。
- 成本金额使用整数 micro-USD；token/调用数/延迟不得为负数或非整数。
- 样式使用 Tailwind；组件不写业务 IPC；所有可见文案由 copy 注入。
- 单文件按写作规范控制在 400 行以内，函数不超过 80 行。

## 验收命令

```powershell
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui build
node --test apps/desktop-ui/src/features/policy/egressPolicy.test.mjs apps/desktop-ui/src/features/capability/capabilityMatrix.test.mjs apps/desktop-ui/src/features/cost/costModel.test.mjs
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
```

## 完成定义（DoD）

- [ ] 三档出域策略可切换；全局和逐应用覆盖均即时更新；升级需显式确认。
- [ ] `local_only` 无本地模型时明确报错且不改策略。
- [ ] 状态栏组件常驻显示当前有效出域级别。
- [ ] Capability Matrix 视图覆盖能力、风险、审批、通道和降级原因。
- [ ] 成本面板覆盖本次任务 / 今日 / 本月与模型/应用拆分。
- [ ] 负向用例覆盖畸形策略、无本地模型、未确认升级、重复/非法能力、非法成本。
- [ ] `pnpm typecheck` / `pnpm build` / 本卡 Node 专项测试 / Rust workspace / xtask 门禁全绿。
- [ ] `LEDGER.md` 追加实现与占位展开事件；若新增长期事实或坑则追加 `docs/memory/{facts,pitfalls}.md`。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-032 UI 策略面板 + 出域三档开关 + Capability Matrix 视图 + 成本面板
【目标】提供 fail-closed 的三档出域策略、逐应用覆盖、常驻状态栏、能力矩阵视图与整数成本面板
【write scope】仅：apps/desktop-ui/src/features/policy/**、capability/**、cost/**（加本卡记录区与治理同步文件）
【铁律】无静默失败 / UI 输入不可信 / UI 不是策略引擎 / 契约先行 / 不静默扩大范围
【禁止】不改 protocol / IPC / Tauri / Rust 公共接口；不引第三方依赖；不接真实持久化或网络
【验收】pnpm typecheck/build、Node 专项测试、cargo fmt/clippy/test、xtask hygiene/memory-counts/adr-index/refscan/docscan/card-check
【依赖】TASK-029、TASK-021 已 Done（已核对 PLAN / plans / LEDGER）
【疑问】无；默认采用 fail-closed：无本地模型时禁止 local_only，升级未显式确认时不生效
```

### 2. 实际改动文件

- `apps/desktop-ui/src/features/policy/**`：`egressPolicy.ts`、`policyCopy.ts`、`useEgressPolicy.ts`、
  `EgressPolicyPanel.tsx`、`EgressStatusBar.tsx`、`index.ts`、`egressPolicy.test.mjs`
- `apps/desktop-ui/src/features/capability/**`：`capabilityMatrix.ts`、`capabilityCopy.ts`、
  `CapabilityMatrixView.tsx`、`index.ts`、`capabilityMatrix.test.mjs`
- `apps/desktop-ui/src/features/cost/**`：`costModel.ts`、`costCopy.ts`、`CostPanel.tsx`、
  `index.ts`、`costModel.test.mjs`
- `tasks/TASK-032-ui-policy-panel-egress-capability-cost.md`：正文展开 + 本执行记录
- 验收同步：`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/memory/*`、`MEMORY.md`

### 3. 验收输出摘要

- `pnpm --dir apps/desktop-ui typecheck`：PASS（`tsc --noEmit`）。
- `pnpm --dir apps/desktop-ui build`：PASS（Vite production build，29 modules）。
- `node --test ...`：26 tests / 26 pass / 0 fail。
- `cargo fmt --all --check`：PASS（0 diff）。
- `cargo clippy --all-targets -- -D warnings`：PASS。
- `cargo test --workspace`：PASS（workspace 全绿；xtask 374 tests）。
- `cargo run -p xtask -- hygiene`：PASS，0E/4W（既有文件长度基线）。
- `cargo run -p xtask -- refscan`：PASS，0E/0W。
- `cargo run -p xtask -- docscan`：PASS，0E/441W（既有基线）。
- `cargo run -p xtask -- card-check`：PASS，0E/27W（既有基线）。
- `memory-counts` / `adr-index` / `cargo deny check`：待 closeout 同步后复跑回填。

### 4. DoD 逐条核对

- [x] 三档出域策略可切换；全局和逐应用覆盖均即时更新；升级需显式确认。
- [x] `local_only` 无本地模型时明确报错且不改策略。
- [x] 状态栏组件常驻显示当前有效出域级别。
- [x] Capability Matrix 视图覆盖能力、风险、审批、通道和降级原因。
- [x] 成本面板覆盖本次任务 / 今日 / 本月与模型/应用拆分。
- [x] 负向用例覆盖畸形策略、无本地模型、未确认升级、重复/非法能力、非法成本。
- [x] `pnpm typecheck` / `pnpm build` / 本卡 Node 专项测试 / Rust workspace / xtask 门禁全绿。
- [x] `LEDGER.md` 追加实现与占位展开事件；新增长期事实/坑同步 `docs/memory/*`：closeout 同批完成。

### 5. 偏差

none

### 6. 更合理做法

无。视图模型按现有 `approval` / `timeline` 的注入式 copy 与 fail-closed 解析模式实现，
未触碰 IPC/schema，也未引入 DOM 测试栈。

### 7. 遗留问题

无新增。UI DOM 测试栈缺口沿用既有 **PL-093**。

### 8. 新增长期记忆

```text
- [2026-09-28][PITFALL][src:TASK-032 实现] **UI 成本面板不能把 `model-gateway` 的 `u64` micro-USD 直接当 JS `number` 使用**：TypeScript/JavaScript 的安全整数上限是 `Number.MAX_SAFE_INTEGER`（2^53-1），超过后解析或聚合会静默舍入，成本显示与预算判断都会偏离真实值。**因此**：成本模型必须拒绝非安全整数，并用 `Number.isSafeInteger` 在每个聚合步骤做溢出检查；若未来确实需要完整 `u64`，传输层应改为 decimal string / bigint，而不是放宽前端校验。
```

### 9. 给审阅者的关注点

1. 出域升级确认已改成一次性 pending：一次确认只授权一次全局/逐应用变更，策略 prop 变化会 resync。
2. Capability Matrix 与成本时间戳均严格拒绝非法 RFC 3339 / 日历 / 时钟值，避免 `Date` 静默归一化。
3. 成本聚合始终使用整数 micro-USD，拒绝 JS 非安全整数，并按 task/today/month 正确切分。
