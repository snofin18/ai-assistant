# TASK-318　Paint 形状轮廓 / 形状填充按组拆工具（ADR-0097 落地）

- 状态：**InProgress**
- 阶段：1　子阶段：**1b**　批次：Paint 真机链路　依赖：TASK-309、TASK-317
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `docs/adr/0097-paint-shape-outline-and-fill-group-tools.md`
  - `docs/adr/README.md`（登记表追加一行 + 下一个可用编号）
  - `adapters/com.microsoft.paint/tools/tools.json`
  - `adapters/com.microsoft.paint/selectors/targets.json`
  - `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`
  - `adapters/com.microsoft.paint/app_map.json`、`adapters/com.microsoft.paint/memory/app_map.v1.json`
  - `adapters/com.microsoft.paint/README.md`
  - `apps/agent-core/src/paint_handlers.rs`、`paint_handler_actions.rs`、`paint_handler_map.rs`、`paint_handler_support.rs`、`paint_registry.rs`
  - `apps/agent-core/tests/production_paint.rs`
  - `docs/memory/apps/paint.md`
  - `LEDGER.md`、`PLAN.md` 当前状态块、`plans/stage-1-pilots.md`、`README.md` 三处
  - 本卡
- **关联**：ADR-0097、ADR-0096、ADR-0091、TASK-309、TASK-317、
  `docs/memory/apps/paint.md` §13.9 / §13.11 / §13.12 / §13.13 / §13.15 / §13.18

**目标**

按 ADR-0097 把 Paint 的 `形状轮廓` / `形状填充` 两个候选组拆成语义工具
`paint.outline.select` / `paint.fill.select`，沿用 ADR-0096 的组内词汇表 + 有界 `path`。

**现象**

TASK-317 只落了工具 / 形状 / 画笔三组，`形状轮廓` / `形状填充` 两个 flyout 的 7 项各自
没有 schema、没有 selector target、没有 handler；`paint.md` §13.13 一度把这两组判成
「必须先选中形状才可用」，但 2026-10-19 的只读 ControlView 复测显示二者**未选中形状时
也能展开并枚举全部 7 项**（`focusable=True`、`Invoke+Toggle`）。

**本卡修法**

按 ADR-0097 D1–D6：新增两个 mounted tool（词汇表各 7 项、`path` 长度 1..=1、逐元素校验、
非法即既有 `ToolInvalidArgs`）→ selector pack 各加 7 个 `observed` + `allow_fallback_entry`
的 `exact_name_and_role` 目标 → handler 走「先展开形状库，再展开目标 flyout」的两段式 →
校验器加钉桩 → 单测覆盖正反路径。

**Out of scope（做了算漂移）**

- `大小`（形状笔宽）组进 schema：无非本地化锚点，且未选中形状时 flyout 为空。
- 图像 / 选择 / Copilot 组进 schema（ADR-0096 已定）。
- 真机端到端落笔（TASK-309 真机段，需要人类在场）。
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

- [ ] ADR-0097 落档 Accepted 并登记进 `docs/adr/README.md`
- [ ] `paint.outline.select` / `paint.fill.select` 各 7 项词汇表，`path` 有界且逐元素校验
- [ ] 同名项（蜡笔 / 记号笔 / 油画笔 / 普通铅笔 / 水彩画笔）跨三组不再歧义（正反测试）
- [ ] selector pack 两个 flyout 按钮 + 14 个 `MenuItem` 升 `observed` 并被校验器钉住
- [ ] 读回不使用会粘住的 `ToggleState`；输出显式 `ui_state_readback=false`
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-318（Paint 形状轮廓 / 形状填充按组拆工具）。
- **write scope**：ADR-0097 与登记表、Paint tools / selectors / validator / app_map、
  Paint handler 与 registry、production_paint 测试、paint.md 记忆、状态同步文件、本卡。
- **铁律**：1（无静默失败）、3（策略唯一放行点）、9（不扩范围）、10（先 ADR 后 schema）。
- **禁止**：`大小` / 图像 / 选择 / Copilot 进 schema；真机端到端；新 ErrorCode / 依赖。
- **验收**：见正文六条命令。
- **依赖**：ADR-0096 Accepted（PR #473 / merge `fb690c7`）；TASK-317 三组已落（PR #474 /
  merge `fdbd3564`）；TASK-309 提供 §13.8–§13.16 只读探针证据。
- **疑问**：无。`大小` 组证据不足，按 ADR-0097 D6 明确留在 schema 之外。

### 2. 实际改动文件

- `docs/adr/0097-paint-shape-outline-and-fill-group-tools.md`：新增 ADR（Accepted），定义两个
  组工具、组内词汇表、两级 ExpandCollapse 通道、`ui_state_readback=false` 读回语义，并明确
  `大小` 不进 schema。
- `docs/adr/README.md`：登记 0097，下一可用编号改为 0098。
- `adapters/com.microsoft.paint/tools/tools.json`：新增 `paint.outline.select` 与
  `paint.fill.select`（各 7 项 enum、`path` 1..=1、postcondition `selected_path` / `path_depth`）；
  工具数 12 → 14。
- `adapters/com.microsoft.paint/selectors/targets.json`：新增 16 个 `observed` 目标
  （`shape_outline_button` / `shape_fill_button` + 轮廓 7 项 + 填充 7 项），全部
  `exact_name_and_role` + `allow_fallback_entry=true`；目标数 55 → 71。
- `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`：`measured_group_targets` 增补
  这 16 个 target 的 id / name / role 钉桩。
- `adapters/com.microsoft.paint/README.md`：`Current Status` 更新为 71 个 observed target；
  `Known Gaps` 增加 `大小` 仍无契约的说明。
- `apps/agent-core/src/paint_handler_support.rs`：新增 `OUTLINE_GROUP_VOCABULARY` /
  `FILL_GROUP_VOCABULARY` 与 `outline_target` / `fill_target` 映射。
- `apps/agent-core/src/paint_handlers.rs`：新增两个 tool 常量、两个 target 常量、两个 selection
  state 字段与 `ShapeGroupFlyout` 枚举；新增 `select_outline_output` / `select_fill_output`
  与共用实现 `select_shape_group_flyout_output`；新增 2 条单测（同名跨三组 + 两组互不通约）。
- `apps/agent-core/src/paint_handler_actions.rs`：把画笔 flyout 的两段式解析抽成
  `wait_for_shape_group_flyout_item`，新增轮廓 / 填充包装（先展开形状库再展开目标 flyout）。
- `apps/agent-core/src/paint_handler_map.rs`、`paint_registry.rs`：注册两个新 handler 并把
  mounted 工具集从 9 扩到 11。
- `docs/memory/apps/paint.md`：新增 §13.19（轮廓 / 填充未选中形状时也可展开并枚举；同刻只有
  一个 flyout 的条目在树里）与 §13.20（`大小` flyout 未选中形状时展开为空）。
- `LEDGER.md`：追加本卡一行。
- 本卡。

### 3. 验收输出摘要

- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` →
  `PASSED: app_id=com.microsoft.paint targets=71 tools=14 recipes=8 probe_status=required`（exit 0）。
- 修改后 CRLF / JSON 解析校验：适配包全部 JSON `json.loads` 通过（工具数 14）。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0（Finished dev profile）。
- `cargo test -p assistant-agent-core` → 库 65 passed / 0 failed；各集成套件 0 failed；
  Paint production `6 passed / 2 ignored`。
- 只读 ControlView 复测（`eval/tasks/paint/probe-toolset.ps1`，未点击、结束时恢复折叠）：
  `形状轮廓` 展开枚举 7 项、`形状填充` 展开枚举 7 项；两者与 `大小` 的空 flyout 形成对照。
- 见 §3 说明：本段**未**跑真机落笔端到端，属 TASK-309 真机段。

### 4. DoD 逐条核对

- [x] ADR-0097 落档 Accepted 并登记进 `docs/adr/README.md`（`adr-index` 0E/0W PASSED）。
- [x] 两个工具各 7 项词汇表，`path` 有界且逐元素校验，非法即既有 `ToolInvalidArgs`。
- [x] 同名项跨三组不再歧义（`蜡笔` 在 brush / outline / fill 各自解析到 `*_crayon` 单测）。
- [x] 两个 flyout 按钮 + 14 个 `MenuItem` 升 `observed` 并被 validator 钉住（`PASSED targets=71`）。
- [x] 读回不使用粘住的 `ToggleState`；两个工具输出显式 `ui_state_readback=false`。
- [x] 静态验收全绿（fmt / clippy / agent-core 测试 / 适配包 validator）；`LEDGER.md` 追加一行。
- [x] 未修改 Out of scope 文件；未新增 ErrorCode / envelope / audit-event / 依赖。

### 5. 偏差

- **无越权偏差**。主动把「展开形状库」这一步加进画笔 flyout 的公共解析路径，使
  `wait_for_brush_item` 与两个新 flyout 复用同一段代码；这是纯重构，画笔行为不变
  （`cargo test -p assistant-agent-core` 全绿），且新路径对画笔只是多做一次幂等的
  `CollapsedButton` 展开。

### 6. 更合理做法

- 轮廓 / 填充的 14 个 `MenuItem` target 名字本身提供的作用有限（全部 `fallback_only`），
  真正的解析靠「展开后只剩本组条目」这一 UI 事实；如果 Paint 未来改成同时保留多个 flyout，
  按 `exact_name_and_role` 解析会立刻变成 `TargetAmbiguous`。届时需要把 flyout 容器做成
  可见的父候选（`role_and_parent`），那要新的 ADR。

### 7. 遗留问题

- 本卡保持 **InProgress**：轮廓 / 填充只是「声明 + 分发 + 单测」已落地，真实 GUI 的按钮
  通道仍只有只读 pattern 证据，没有端到端落笔验收。
- `大小`（形状笔宽）仍无契约：按钮只有本地化 name，未选中形状时 flyout 展开为空；需要
  真机段先选中形状才能取证。
- 图像 / 选择 / Copilot 组仍按 ADR-0096 在 schema 之外。
- 真机端到端（选中形状后落笔并验证像素）属 TASK-309 真机段，本卡不声称完成。

### 8. 新增长期记忆

- 无新增 `FACT` / `PITFALL` / `REJECTED` 条目；新发现的「轮廓 / 填充未选中形状时也能展开
  并枚举」「同刻只有一个 flyout 的条目在树里」「`大小` 未选中形状时展开为空」已落
  `docs/memory/apps/paint.md` §13.19 / §13.20。

### 9. 给审阅者的关注点

- 重点核对 `validate_adapter_pack.py` 里新增的 16 个钉桩是否与 `paint.md` §13.11 / §13.12
  的 7 项顺序与措辞逐条一致（尤其 `无边框` / `无填充` 是各自第一项）。
- 重点核对 `wait_for_shape_group_flyout_item`：它把「展开形状库」抽成共用步骤后，画笔路径
  是否仍能先解到已展开的条目（`test_paint_t3_1_runs_to_completed_on_the_fake_platform` 覆盖）。
- 两个新工具的 `ui_state_readback=false` 是**有意**的：ADR-0096 D5 记过 `ToggleState` 会粘住，
  本项目不拿它冒充 UI 当前值。
