# TASK-031　UI：元素拾取器 v0（悬停高亮 + 属性面板 + 一键生成 selector 候选链）+ 目标绑定向导

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A3**　依赖：029,017　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- 依赖：029,017　预估：M　难度：M
- 关联：架构 v2 §6.2 / §6.3 / §6.4 / §16.2、ADR-0022 D4~D6、`crates/platform/api/src/target.rs`、`plans/stage-1-pilots.md` 批次表 A4

## 目标（一句话）

在 `apps/desktop-ui` 内提供可复用的元素拾取器与目标绑定向导 feature：校验 Host 注入的元素快照，悬停/选中高亮目标 bounds，展示稳定属性与状态，一键生成有序 selector 候选链，并通过回调导出可审查的 Adapter selector 草稿。

## In scope

- `apps/desktop-ui/src/features/picker/**`（新增）
- `apps/desktop-ui/src/features/binding/**`（新增）

## Out of scope（做了算漂移）

- 修改 `App.tsx`、`main.tsx`、`src/ipc/client.ts`、Tauri command、`src-tauri/**`、`package.json`、锁文件或 tsconfig。
- 新增 Tauri IPC、公共 schema、Rust 类型、依赖或持久化格式。
- 真实跨应用鼠标拾取、全局置顶覆盖层、调用 UIA `ElementFromPoint` 或写 Adapter 文件。
- 发布计划、自愈分数回写、真实 Tool/postcondition 草稿、策略判定与审批。

## 必须遵守

- Host/UI 注入快照是**不可信输入**：先经 fail-closed 运行时校验；空 id、重复 id、越界 bounds、非法分数或未知 selector kind 一律显式报错。
- `AutomationId` / `ClassAndRole` / `RoleAndParent` 属稳定候选；`Name`、标题与无障碍路径只能作低分兜底并标 `locale_dependent=true`，不得成为首选。
- 候选链必须有序、链内 id 唯一、每项 score ∈ `[0,1]`，并至少含一个非本地化候选；否则拒绝导出草稿。
- 高亮只使用已校验的归一化 bounds；组件不得回退到未校验默认值。
- 绑定向导只生成内存草稿并通过 `onDraftReady` 交回调用方；本卡不负责落盘、IPC 或 Adapter 最终 schema 发布。
- 所有用户可见文案通过 `copy` 注入；组件使用语义化 HTML、可访问名称与键盘可操作控件。
- 单组件 ≤200 行，其他文件 ≤600 行；无 `any`、无非空断言、无 `console.*`、无 `fetch` / `WebSocket`。

## 验收命令（agent 必须全部执行并粘贴输出）

```powershell
pnpm --dir apps/desktop-ui typecheck; pnpm --dir apps/desktop-ui build
node --test --experimental-strip-types apps/desktop-ui/src/features/picker/pickerModel.test.mjs apps/desktop-ui/src/features/binding/bindingModel.test.mjs
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

## 完成定义（DoD）

- [ ] 拾取器覆盖悬停/选中高亮、稳定属性面板、父路径、状态、actions/patterns 与 bounds。
- [ ] 对 Notepad 编辑区快照可生成 ≥3 个有序候选，且首选不是 `NameRegex` 或其它本地化候选。
- [ ] 绑定向导可生成含 `TargetDescriptor` 形状的 Adapter selector 草稿，并通过导出回调交回 JSON。
- [ ] 模型与控制器测试覆盖正常、畸形输入、重复 id、非法 bounds、仅本地化候选、导出失败六类路径。
- [ ] UI typecheck 与 production build 全绿。
- [ ] 上述 Rust 与 xtask 验收命令全绿。
- [ ] 无任何 Out of scope 文件被修改。
- [ ] `tasks/TASK-031-ui-element-picker-selector-candidates.md` 执行记录 9 节填完。
- [ ] `LEDGER.md` 追加一行；若有新事实或坑，追加 `docs/memory/{facts,pitfalls}.md`。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

已按会话协议核对 `main`：TASK-030 已合并，实际下一张为 TASK-031；本卡正文由本轮按 gov §3.2 从占位版展开。实现遵守 `picker/**` 与 `binding/**` write scope，未修改 Tauri、IPC、schema、依赖或应用壳。自动化指令已授权按最优解推进，因此回执后继续实现。

### 2. 实际改动文件

- `apps/desktop-ui/src/features/picker/**`：严格快照解析、元素高亮、属性面板、候选链生成、候选列表与 11 个专项测试。
- `apps/desktop-ui/src/features/binding/**`：绑定向导、Adapter selector 草稿生成、导出回调与 8 个专项测试。
- `tasks/TASK-031-ui-element-picker-selector-candidates.md`：正文展开与执行记录。
- 后续同步：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`。

### 3. 验收输出摘要

- `pnpm --dir apps/desktop-ui typecheck` -> exit 0。
- `pnpm --dir apps/desktop-ui build` -> exit 0；Vite 29 modules transformed。
- `node --test --experimental-strip-types ...pickerModel.test.mjs ...bindingModel.test.mjs` -> 19 tests / 19 passed。
- `cargo fmt --all --check` -> 0 diff。
- `cargo clippy --all-targets -- -D warnings` -> exit 0。
- `cargo test --workspace` -> 全绿。
- `xtask hygiene` -> 0 error / 4 existing warnings；`memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` -> PASSED。

### 4. DoD 逐条核对

- [x] 拾取器覆盖悬停/选中高亮、属性、父路径、状态、actions/patterns 与 bounds。
- [x] Notepad 编辑区快照生成 3 个正分候选，首选为稳定 `class_and_role`，不使用本地化文本。
- [x] 绑定向导生成 `TargetDescriptor` 形状草稿并通过 `onDraftReady` 回调导出 JSON。
- [x] 19 个模型/控制器测试覆盖正常、畸形、重复 id、越界/偏移 bounds、本地化候选越级、非法/临时候选、kind/value 不一致、仅本地化候选与导出失败路径。
- [x] UI typecheck 与 production build 全绿。
- [x] Rust 与 xtask 验收命令全绿。
- [x] 未修改 Out of scope 文件，未新增依赖。
- [x] 执行记录 9 节已填；进度文件与 LEDGER 在 review 通过后同步。

### 5. 偏差

none。占位正文在实现前展开；未放宽 lint、未改测试断言、未新增依赖、未改公共接口。

独立 review 首轮发现 1 个 P0 与 3 个 P1 均已在同一 write scope 内闭环：

- P0：`RoleAndParent` 的父候选会被平台解析器当普通候选尝试，可能在 target 未命中时误返回父元素。修复为**不生成该不安全候选**；负向测试断言导出链不含 `role_and_parent`、零分 helper 或 `runtime_id`。
- P1：本地化候选可能越级为首选。修复为验证链严格降序、首选必须非本地化且分数严格高于所有本地化候选、本地化分数上限 `0.49`。
- P1：workspace 非零原点被忽略。修复为 bounds containment 与高亮百分比都减去 workspace 原点，并加非零原点正/负测试。
- P1：binding 最外层 `null` / 非对象会抛异常。修复为 `parseBindingWizardInput(input: unknown)` fail-closed，并加负向测试。

P2/P3 同步收紧：组件不再直出原始 parser error / error key，布尔状态与兜底文案走 `copy`；workspace 增加 group label，向导步骤增加 `aria-current="step"`。

第二轮独立复验确认 P0/P1 清零。另关闭两项非阻塞质量缺口：候选 `kind/value` 与 TTL 语义校验；草稿工厂对 raw snapshot / metadata 再走一次运行时解析。剩余限制已登记为 PL-094：`RoleAndParent` 的 helper 候选目前无法在平台 chain 中安全表达，因此 v0 不生成该 kind。

### 6. 更合理做法

- 将候选生成、验证、高亮几何与向导状态全部做成纯函数/纯 reducer，组件只负责语义化渲染和事件转发；Node 内置类型擦除即可覆盖核心路径。
- binding 测试注册一个仅作用于测试进程的 `.js -> .ts` resolve hook，复用 Node 24 类型擦除，同时保持生产源码使用 bundler 规范的 `.js` specifier，不引入测试依赖。

### 7. 遗留问题

- PL-093：`apps/desktop-ui` 仍未接入 Vitest + Testing Library；本轮保持模型级测试，DOM 键盘交互回归需在独立 UI 测试栈卡补齐。
- 真实 UIA `ElementFromPoint` 与 Tauri command 接线不在本卡 scope，由后续 Host/IPC 卡负责。
- PL-094：平台 `RoleAndParent` 父候选会被 `resolve_element` 当普通候选尝试；本卡 fail-closed 不生成该 kind，待平台契约增加 helper 标记或解析器过滤。

### 8. 新增长期记忆

`docs/memory/pitfalls.md`：记录 `RoleAndParent` helper 候选可能被平台解析器返回为错误目标，以及 v0 的 fail-closed 处置。

### 9. 给审阅者的关注点

1. 核对 `NameRegex` / `A11yPath` / `TitleRegex` / `VisualAnchor` 是否始终被标为本地化依赖，且不能成为首选。
2. 核对候选验证是否拒绝 `role_and_parent` / `runtime_id`、未排序链、本地化越级和窗口无稳定候选。
3. 核对 UI 文案是否全部来自 `copy`，以及高亮 bounds 是否正确处理 workspace 非零原点。

最终 merge：PR #76 -> `main` 的 `2cb0139`（2026-09-28）；push / PR 两轮各 16 项 check-run 全绿，`merge_state_status=clean`。
