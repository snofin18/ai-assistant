# TASK-319　Paint 形状笔宽（大小）按组拆工具（ADR-0098 落地）

- 状态：**Done**
- 阶段：1　子阶段：**1b**　批次：Paint 真机链路　依赖：TASK-309、TASK-318
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `docs/adr/0098-paint-shape-size-group-tool.md`
  - `docs/adr/README.md`（登记表追加一行 + 下一个可用编号）
  - `adapters/com.microsoft.paint/tools/tools.json`
  - `adapters/com.microsoft.paint/selectors/targets.json`
  - `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`
  - `adapters/com.microsoft.paint/README.md`
  - `apps/agent-core/src/paint_handlers.rs`、`paint_handler_support.rs`、`paint_handler_flyout.rs`、
    `paint_handler_actions.rs`、`paint_handler_map.rs`、`paint_registry.rs`、`paint_handlers_tests.rs`
  - `docs/memory/apps/paint.md`
  - `LEDGER.md`、`PLAN.md` 当前状态块、`README.md` 三处
  - 本卡
- **关联**：ADR-0098、ADR-0097、ADR-0096、ADR-0091、TASK-309、TASK-318、
  `docs/memory/apps/paint.md` §13.13 / §13.14 / §13.16 / §13.18 / §13.20 / §13.21

**目标**

按 ADR-0098 把 Paint 形状组的 `大小`（形状笔宽）做成一个语义工具 `paint.size.select`：
组内词汇表 = 像素宽度（`1 像素` / `3 像素` / `5 像素` / `8 像素`），锚点 = 稳定
`AutomationId`（`Size1` / `Size3` / `Size5` / `Size8`），有界 `path` + 两级 `ExpandCollapse` 通道。

**现象**

`大小` 是 `形状` 组最后一个没有 schema 的 flyout。ADR-0097 曾以「无非本地化锚点 + 未选中形状时
flyout 为空」把它排除；2026-10-19 的只读 ControlView 复测证明**形状组有活动形状时四项都能
枚举且都带稳定 aid**，而 `大小` 按钮本身仍是本地化 name + `Button` role（与轮廓 / 填充同形），
`Size1` 那一项的 UIA name 是 `大小`（§13.16 的三重同名陷阱）。

**本卡修法**

新增一个 mounted tool `paint.size.select`（词汇表 4 项、`path` 1..=1、逐元素校验、非法即既有
`ToolInvalidArgs`）→ selector pack 加 `shape_size_button`（`exact_name_and_role` +
`allow_fallback_entry`；4 个 `size_*_option` 目标已在 TASK-309 第四段落档）→ handler 走
「先展开形状库，再展开 `大小`，再按 `AutomationId` 有界重解 → `Invoke`」→ 校验器钉住按钮 name/role
与四个 aid ↔ 工具枚举的对应 → 单测覆盖正反路径。

**Out of scope（做了算漂移）**

- 画笔组的 `尺寸` / `不透明度` Slider 进 schema（本次探测的那一态里不在树上，无 Min/Max 证据）。
- 图像 / 选择 / Copilot / 层组进 schema（ADR-0096 / ADR-0097 已定）。
- 真机端到端落笔（TASK-309 真机段，需要人类在场）；本卡不跑真机 GUI。
- 新增 ErrorCode / envelope / audit-event / 第三方依赖。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-agent-core
cargo test --workspace
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
cargo run -p xtask -- adr-index
```

**DoD**

- [ ] ADR-0098 落档 Accepted 并登记进 `docs/adr/README.md`
- [ ] `paint.size.select` 4 项词汇表，`path` 有界且逐元素校验，跨组取值被拒（正反测试）
- [ ] 词汇表 ↔ `AutomationId` 对应（`1 像素`→`Size1` … `8 像素`→`Size8`）有单测 + 校验器钉桩
- [ ] `shape_size_button` 按 `exact_name_and_role`（`大小` + `Button`）解析并被校验器钉住
- [ ] 读回不使用会粘住的 `ToggleState`；输出显式 `ui_state_readback=false`
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-319（Paint 形状笔宽「大小」按组拆工具）。
- **write scope**：ADR-0098 与登记表、Paint tools / selectors / validator / README、
  Paint handler 六个源文件 + 单测文件、paint.md 记忆、状态同步文件、本卡。
- **铁律**：1（无静默失败）、5（API 优先：aids 优先于 name / 合成输入）、9（不扩范围）、
  10（先 ADR 后 schema）。
- **禁止**：画笔 `尺寸` / `不透明度` Slider 进 schema；图像 / 选择 / Copilot / 层；真机端到端；
  新 ErrorCode / 依赖。
- **验收**：见正文六条命令。
- **依赖**：ADR-0097 Accepted 且 TASK-318 三组已落（PR #476 / #477，merge
  `b5be949` / `6aa3be8`）；TASK-309 提供 §13.8–§13.16 只读探针证据。
- **疑问**：无。`大小` 的前置条件（形状组需有活动形状）按 ADR-0098 D4 写成调用方义务，
  未取证的部分继续 fail-closed。

### 2. 实际改动文件

- `docs/adr/0098-paint-shape-size-group-tool.md`：新增 ADR（Accepted），冻结 `paint.size.select`
  的词汇表（像素宽度）、aid 锚点、两级 `ExpandCollapse` 通道、前置条件与 `ui_state_readback=false`。
- `docs/adr/README.md`：登记 0098，下一可用编号 0098 → 0099。
- `adapters/com.microsoft.paint/tools/tools.json`：新增 `paint.size.select`（4 项 enum、`path` 1..=1、
  postcondition `selected_path` / `path_depth`）；工具数 14 → 15。
- `adapters/com.microsoft.paint/selectors/targets.json`：新增 `shape_size_button`
  （`exact_name_and_role`：`大小` + `Button`，`observed` + `allow_fallback_entry`）；
  四个 `size_*_option` 目标沿用 TASK-309 第四段的 aid 锚点不动；目标数 71 → 72。
- `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`：`measured_group_targets` 增补
  `shape_size_button` 钉桩；新增 ADR-0098 段——钉住工具 enum 顺序、`path` 边界的
  `minItems` / `maxItems`、`ui_state_readback=false`，并逐个钉住四个 target 的
  `AutomationId`（`Size1` / `Size3` / `Size5` / `Size8`）。
- `adapters/com.microsoft.paint/README.md`：`Current Status` 71 → 72 targets 并写明
  `大小` flyout 与其 aid 锚点；`Known Gaps` 把「`大小` 无契约」改为 ADR-0098 的真实边界。
- `apps/agent-core/src/paint_handler_support.rs`：新增 `SIZE_GROUP_VOCABULARY` 与 `size_target`。
- `apps/agent-core/src/paint_handler_flyout.rs`：`ShapeGroupFlyout` 增 `Size` 变体；把
  `tool()` / `vocabulary()` / `resolve_target()` 收进该 enum（顺带把共享体的参数从 6 降到 3、
  消掉既有的 `too-many-parameters` 告警）；新增 `select_size_output` 与 `size_leaf` 读写。
- `apps/agent-core/src/paint_handler_actions.rs`：新增 `wait_for_size_flyout_item`
  （复用「展开形状库 → 展开目标 flyout → 有界重解」三步，目标按 aid 解析）。
- `apps/agent-core/src/paint_handlers.rs`：新增 `TOOL_SIZE_SELECT`、`TARGET_SHAPE_SIZE_TOGGLE`
  与 `size_leaf` 选中态字段。
- `apps/agent-core/src/paint_handler_map.rs`：新增 `SizeSelectHandler` 并注册；把三个 canvas
  handler 的注册抽成 `insert_canvas_handlers`，让 `build_handler_map` 回到 80 行以内
  （纯搬移，注册集合不变）。
- `apps/agent-core/src/paint_registry.rs`：`PAINT_EXPECTED_TOOL_NAMES` 增加 `TOOL_SIZE_SELECT`
  （mounted 工具 11 → 12）。
- `apps/agent-core/src/paint_handlers_tests.rs`：补 `//!` 模块头；新增 2 条单测
  （词汇表 ↔ aid 映射 / 本地化标签与邻组取值被拒）。
- `docs/memory/apps/paint.md`：新增 §13.21（两态实测 + aid 锚点 + 三重同名），并追加剧中更正
  §13.20 的过度推广；文件 379 → 400 行（ADR-0021 阈值 400 内）。
- `MEMORY.md`：§1 规模表 `apps/paint.md` 行 379 → 400 并更新读法摘要。
- `LEDGER.md`：追加本卡一行。`PLAN.md`：当前状态块补 TASK-319 与新的下一步动作。本卡。

### 3. 验收输出摘要

- 适配包：`python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` →
  `PASSED: app_id=com.microsoft.paint targets=72 tools=15 recipes=8 probe_status=required`（exit 0）。
- 只读 ControlView 复测（`eval/tasks/paint/probe-toolset.ps1`，未点击、结束时恢复折叠）：
  形状组有活动形状（画布 name `在画布上使用 向下箭头 工具`）时
  `PROBE: expanded aid="CollapsedButton" state=Expanded` /
  `PROBE: expanded name="大小" state=Expanded` 后有
  `[5] name="大小" aid="Size1"` / `3 像素 Size3` / `5 像素 Size5` / `8 像素 Size8`
  （均 `type=MenuItem focusable=True patterns=[Invoke,Toggle,ScrollItem,VirtualizedItem]`），
  `seen=180 emitted=18 truncated=False elapsed_ms=559`；同一 run 里
  `[6] name="大小" aid="" type=Button ... patterns=[Invoke,ExpandCollapse,ScrollItem]`。
- `cargo fmt --all --check` → exit 0；`cargo clippy --all-targets -- -D warnings` → exit 0。
- `cargo test -p assistant-agent-core` → lib **67 passed / 0 failed**（+2 条新单测），
  各集成套件 0 failed（Paint production `6 passed / 2 ignored`）。
- `cargo test --workspace` → exit 0（0 failed）。
- xtask：`hygiene` 0E/121W、`check-comments` 0E/68W、`docscan` 0E/264W、`card-check` 0E/34W、
  `refscan` 0E/0W、`verify-schemas` PASSED、`codegen --check` 0 drift、`check-migrations` 0E/0W、
  `memory-counts` PASSED、`adr-index` 0E/0W、`check-ledger` PASSED。告警数与改动前基线（122）
  相比少 1 条，未新增告警类别。
- 契约 PR 状态：**PR #479**（ADR-0098 + 登记表 + 本卡）pull_request CI = **11 checks / 11 pass /
  pending 0**，合并前 base=main、mergeable=MERGEABLE、mergeStateStatus=CLEAN，merge
  `ff8337d`。

### 4. DoD 逐条核对

- [x] ADR-0098 落档 Accepted 并登记进 `docs/adr/README.md`（`adr-index` 0E/0W PASSED）。
- [x] `paint.size.select` 4 项词汇表，`path` 有界且逐元素校验，跨组取值被拒（2 条新单测 + 校验器钉桩）。
- [x] 词汇表 ↔ `AutomationId` 对应有单测 + 校验器钉桩（`1 像素`→`Size1` … `8 像素`→`Size8`）。
- [x] `shape_size_button` 按 `exact_name_and_role`（`大小` + `Button`）解析并被校验器钉住。
- [x] 读回不使用会粘住的 `ToggleState`；两个新输出路径都带 `ui_state_readback=false`（五金具共享同一实现）。
- [x] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行。
- [x] 未修改 Out of scope 文件（画笔 Slider / 图像 / 选择 / Copilot / 层 / 真机执行均未碰）。

### 5. 偏差

- **无越权偏差**。两处同文件内的结构性调整，都是为守住既有护栏并已记录：
  ① `ShapeGroupFlyout` 的三个查表函数（`tool` / `vocabulary` / `resolve_target`）收进 enum，
     把共享体参数从 6（触发 `hygiene/too-many-parameters`）降到 3，行为不变；
  ② `paint_handler_map.rs` 的三个 canvas handler 注册抽成 `insert_canvas_handlers`，
     让 `build_handler_map` 回到 80 行以内（新增一个 handler 会把原函数顶到 81 行）。
     两处都只改 Paint 自有 handler 文件，注册集合与调用语义逐字不变（测试全绿）。

### 6. 更合理做法

- 「词汇表 = 显示名」这个在工具 / 形状 / 画笔 / 轮廓 / 填充五组都成立的做法，到 `大小` 组就
  第一次失效（`Size1` 的 name 是分组标题 `大小`）。更省事的长期做法是让**校验器**对每个组
  同时钉「词汇表 ↔ 锚点（name 或 aid）」两列，而不是各组各自发明词汇表口径；本卡按 ADR-0098
  的最小面先钉住 `大小` 这一组，未顺手统一其余五组。
- 与轮廓 / 填充的 14 个 target 相比，`大小` 组有个额外好处：aid 锚点让「同刻只有一个 flyout
  在树里」这条 UI 事实不再必要（aid 唯一），所以那一类「flyout 互斥」风险在 `大小` 上不存在。

### 7. 遗留问题

- **TASK-319 已收口**：本卡声明范围（契约 + 声明 + 分发 + 单测 + 钉桩）已全部落地，
  且真实 GUI 的 `paint.size.select(["8 像素"])` 已在生产 Plan 中 `Committed`。
- 真机端到端（选中形状 → 设轮廓 / 填充 / 大小 → 落笔并验证像素）属 **TASK-309 真机段**，
  需要人类在场，本卡不声称完成。
- 画笔组的 `尺寸` / `不透明度` Slider 在本次探测的那一态里**不在 UIA 树上**（整棵树只有
  `ZoomSliderControl` 一个 Slider），因此仍无 Min/Max/CurrentValue 证据，继续留在 schema 之外。
- `大小` 的前置条件「形状组需有活动形状」是**调用方义务**：`paint.size.select` 不会代选形状；
  形状未激活时以既有 `TargetNotFound` fail-closed（ADR-0098 D4）。
- 本次只做只读展开与枚举取证，**未**调用新 handler、**未**改变 Paint 状态（探针结束时已恢复折叠）。
- 2026-10-19 真机会话补充：`paint.size.select(["8 像素"])` 走
  `CollapsedButton → 大小 → aid=Size8` 并成功执行；选中项后同样等待 MenuItem 从树里消失，
  以目标物化 / 消失作为异步 ExpandCollapse 的真实效果判据。

### 8. 新增长期记忆

- 无新增 `FACT` / `PITFALL` / `REJECTED` 条目。本轮的新事实（`大小` flyout 两态、四个 aid 锚点、
  三重同名、形状组本身是程序化路径）按 ADR-0021 的应用专属规则落
  `docs/memory/apps/paint.md` §13.21，并对 §13.20 追加更正；`MEMORY.md` 只同步规模表行。

### 9. 给审阅者的关注点

- 词汇表口径值得逐条核对：`1 像素` / `3 像素` / `5 像素` / `8 像素` 是**本项目自定的稳定 token**，
  不是 UI 显示名（`Size1` 的 name 是 `大小`）。请核对 `size_target`、`tools.json` 的 enum 与
  `validate_adapter_pack.py` 的 `measured_size_vocabulary` 三处是否一一对应。
- `shape_size_button` 只用 `name + role` 解析：`大小` 同时在 `Button`（flyout 按钮）/
  `Slider`（画笔尺寸）/ `MenuItem`（`Size1`）上出现，role 是唯一消歧手段，改动时不要退化成 name-only。
- `wait_for_size_flyout_item` 复用轮廓 / 填充的两级展开路径但**不做**任何形状选择：
  若审阅者认为应当自动选形状，那属于新的 ADR（会引入静默副作用）。
