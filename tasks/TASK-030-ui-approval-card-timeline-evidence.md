# TASK-030　UI：审批卡片（含 diff + 来源归因 + 授权范围）+ 执行时间线（含证据与撤销按钮）

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A3**
- 依赖：TASK-029（binary skeleton + desktop-ui shell）
- 关联：`cross-platform-ai-assistant-architecture-v2.md` §10.2~§10.5 / §16.4、`crates/hitl/README.md`、ADR-0048、`docs/spec/naming.md`、`plans/stage-1-pilots.md` A4
- 预估：L　难度：M

## 目标（一句话）

在 `apps/desktop-ui` 内提供可复用的审批卡片与执行时间线 feature，覆盖完整审批字段、来源归因、授权范围、diff 展示、证据引用和按可逆性控制的撤销入口。

## In scope

- `apps/desktop-ui/src/features/approval/**`（新增）
- `apps/desktop-ui/src/features/timeline/**`（新增）

## Out of scope（做了算漂移）

- 修改 `App.tsx`、`main.tsx`、IPC client、Tauri command、`package.json`、锁文件或 tsconfig。
- 引入 Vitest、Testing Library、图标库、i18n 库或任何其他依赖。
- 读取真实 Core/Host 数据、调用 Tauri IPC、截图、录制或重放执行。
- 元素拾取器、策略面板、出域开关、能力矩阵、成本面板。
- 持久化审批、授权撤销、真实变更参数后重试。

## 必须遵守

- 组件只消费调用方注入的视图模型；业务判定放独立纯函数或 hook，组件不自行放行权限。
- 所有运行时输入先校验；畸形、空字段、非法 scope 或缺失 diff 时显式返回错误，禁止用默认值冒充有效数据。
- `instruction_origin = app_content` 必须显著标红、默认拒绝，且只有用户显式覆盖后才能批准。
- `High` / `Critical` / `L3Irreversible` 只允许 `once`，不得渲染 `persistent` 等更宽范围。
- 不可逆或缺少可用 Anchor 的步骤必须禁用撤销，并显示明确原因。
- 所有用户可见文案通过 `copy` 属性提供的 i18n key 或键值注入，不在组件中硬编码自然语言。
- 使用语义化 HTML、可访问名称、键盘可操作controls，并为状态变化提供 live region。
- 组件文件不超过 200 行；其他文件不超过 600 行硬上限；无 `any`、无非空断言、无 `console.*`、无 `fetch` / `WebSocket`。

## 验收命令（agent 必须全部执行并粘贴输出）

```powershell
pnpm --dir apps/desktop-ui typecheck; pnpm --dir apps/desktop-ui build
node --test --experimental-strip-types apps/desktop-ui/src/features/approval/approvalModel.test.mjs apps/desktop-ui/src/features/timeline/timelineModel.test.mjs
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

## 完成定义（DoD）

- [ ] 审批卡片覆盖 v2 §10.2 全部字段：动作、目标、影响范围、diff、来源归因、工具选择理由、风险级、可逆性与撤销方式、授权范围、证据链接。
- [ ] 时间线覆盖 v2 §16.4 每步字段：动作、目标、前后指纹、验证结果、证据、耗时、成本、可逆性、撤销入口。
- [ ] `app_content` 默认拒绝；高风险范围收敛只允许 `once`；不可逆/无 Anchor 的撤销按钮禁用。
- [ ] 纯逻辑与交互控制器测试覆盖正常、畸形、高风险、来源为 `app_content`、撤销不可用五类路径。
- [ ] UI typecheck 与 production build 全绿。
- [ ] 上述 Rust 与 xtask 验收命令全绿。
- [ ] 无任何 Out of scope 文件被修改。
- [ ] `tasks/TASK-030-ui-approval-card-timeline-evidence.md` 执行记录 9 节填完。
- [ ] `LEDGER.md` 追加一行；若有新事实或坑，追加 `docs/memory/{facts,pitfalls}.md`。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

已按会话协议核对 `main`：TASK-029 已合并（`21e35bc`），实际下一张为 TASK-030；本卡正文由本轮按 gov §3.2 代 Orchestrator 展开。write scope、禁止项、验收命令和依赖均见正文区；实现未越过该 scope，也未新增依赖或改公共接口。

### 2. 实际改动文件

- `apps/desktop-ui/src/features/approval/**`：审批卡片、diff/origin/scope/evidence 组件、严格模型校验、交互 reducer、copy 类型与 11 个专项测试。
- `apps/desktop-ui/src/features/timeline/**`：执行时间线、证据/撤销/重放入口、严格模型校验、交互 reducer、copy 类型与测试。
- `tasks/TASK-030-ui-approval-card-timeline-evidence.md`：正文展开与执行记录。
- 后续同步文件：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` / `MEMORY.md` / `docs/memory/facts.md` / `docs/PARKING_LOT.md` / `docs/automations/2026-09-27-round-1.md`。

### 3. 验收输出摘要

- `pnpm --dir apps/desktop-ui typecheck` → exit 0。
- `pnpm --dir apps/desktop-ui build` → exit 0；Vite 29 modules transformed。
- `node --test --experimental-strip-types ...` → 13 tests / 13 passed（review 修复后）。
- `cargo fmt --all --check` → 0 diff。
- `cargo clippy --all-targets -- -D warnings` → exit 0。
- `cargo test --workspace` → 全绿；`xtask` 374 passed，无失败。
- `xtask hygiene` → 0 error / 4 existing warnings；`memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` → PASSED。
- PR #74 的 push run `36327695575` 与 pull_request run `36327698217` 均 16/16 success；merge commit `4823cfd`。

### 4. DoD 逐条核对

- [x] v2 §10.2 审批字段齐备。
- [x] v2 §16.4 时间线字段齐备。
- [x] `app_content` 默认拒绝、显式覆盖后才可批准。
- [x] 高风险/不可逆只允许 `once`，L3 另有二次确认词。
- [x] 不可逆、失败、无 Anchor 或未提供回调时撤销入口禁用并显示原因。
- [x] 畸形输入显式报错，不静默补默认值。
- [x] UI typecheck/build 与 Rust/xtask 验收全绿。
- [x] 未修改 write scope 外产品文件，未新增依赖。
- [x] 执行记录 9 节完成；LEDGER 与长期记忆同步。

### 5. 偏差

none。没有放宽 lint、没有改测试断言、没有加依赖、没有改公共接口。卡面从占位版展开为完整正文、预估由 M 修正为阶段表既有的 L，均发生在实现前并由本轮 Orchestrator 代行。

### 6. 更合理做法

为保持依赖零新增，专项测试使用 Node 24 内置类型擦除直接验证自包含 TS 模型，而不是在 TASK-030 内引入 Vitest / Testing Library；这会绕过 package/lockfile 修改和依赖审批。组件仍走 `tsc` + Vite production build，交互状态放在纯 reducer 中测试。

### 7. 遗留问题

- PL-093：`apps/desktop-ui` 尚未接入 gov §6.3 要求的 Vitest + Testing Library；本卡只覆盖模型与交互控制器，DOM 级键盘/点击回归需要在单独获准的 UI 测试栈卡中补齐。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 Node 24 `--experimental-strip-types` 可在零新增依赖下运行自包含 TS 模型测试，同时明确 DOM 交互测试仍需单独补测试栈。

### 9. 给审阅者的关注点

1. 重点核对 `app_content` 覆盖、高风险 scope 收敛、L3 二次确认三条安全路径。
2. 核对时间线撤销可用性判定是否始终要求 `succeeded` + 可用 Anchor，且无证据时不会伪装成功。
3. 核对 copy 注入是否覆盖所有可见文案，尤其是 diff warning、evidence label 与 disabled reason。

独立 review 追加修复：证据与 UI 截图引用现只接受 `evidence://` 内部引用，拒绝 `javascript:` / 外部 URL；时间线拒绝“succeeded + failed verification”的矛盾组合，并补两条负向测试。

最终 merge：PR #74 → `main` 的 `4823cfd`（2026-09-27）。
