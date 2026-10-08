# TASK-256　Paint 真机契约校准：selector 与 handler read-back 对齐实测树

- 状态：**Ready**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：043、044、106　预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：043、044、106　**预估**：L　**难度**：L
- **write scope**：`adapters/com.microsoft.paint/**`、`apps/agent-core/src/paint_handlers.rs`、
  `apps/agent-core/tests/production_paint.rs`、`eval/tasks/paint/**`
- **关联**：ADR-0022 D4（禁用可见文本作主 selector）、ADR-0043（元素解析必须有 scope）、
  ADR-0085（`element_bounds`）、ADR-0084 D7（真机验收另卡）、
  `eval/tasks/paint/t3.1/probe-evidence.json`、`tasks/TASK-044-*.md`、`tasks/TASK-106-*.md`

**目标**

让 Paint T3.1 在**真实 Paint 11.2605.81.0** 上真正跑起来：把 `selectors/targets.json`
按实测 UIA 树校准，并把 `paint_handlers.rs` 的 read-back 契约改成真实控件能提供的语义，
从而解锁 TASK-044 的真机十次验收（`DRIFT-044-1` / `PL-113`）。

**为什么不是「只改 selector」**

2026-10-08 实测（证据见 `eval/tasks/paint/t3.1/probe-evidence.json`）：

1. `main_window` 声明 `class=WinUIDesktopWin32WindowClass` → `TargetNotFound`；真实 class 是 `MSPaintApp`。
2. `canvas` 声明 `class=Image role=Image` → `TargetAmbiguous`（38）；真实是 `NamedContainerAutomationPeer` / `Group` / `aid=image`。
3. `status_bar` 声明的 `Grid`/`StatusBar` 不存在；画布尺寸在 `TextBlock aid=CanvasSizeTextBlock`（`418 × 74像素`）。
4. `rectangle_tool_button` 声明 `AppBarButton`/`Button`；真实是 `GridViewItem`/`ListItem`、**无 aid**、Name 本地化（`矩形`）。
5. `foreground_color_button` 声明 `Button`/`Button`；真实是 `RadioButton`、**无 aid**、Name 本地化（`颜色 1: 黑色`）。
6. `layers_panel` / `layer_item`：真实树里**没有** `ListView` / `ListViewItem`（层面板默认折叠）。
7. `paint_handlers.rs` 的 read-back 契约与真实控件不兼容：
   - `paint.tool.select` 要求按钮文本 == 英文工具 id（`rectangle`）；
   - `paint.color.select_foreground` 要求 `set_value(button,"R,G,B")` 后读回 `"R,G,B"`；
   - `paint.layer.select` 要求 `ListViewItem` + 读回 `Layer 1`。
   这三条在 fake 平台（`production_paint.rs`）里成立，在真实 Paint 上不成立 —— 即
   **TASK-106 的 Paint handler 只被 fake 验证过**。

**In scope**

1. 按 `probe-evidence.json` 的 `real_control_tree` 校正 `selectors/targets.json` 里 T3.1 用到的
   target（`main_window` / `toolbar` / `rectangle_tool_button` / `foreground_color_button` /
   `layers_panel` / `layer_item` / `status_bar` / `canvas_host` / `canvas` 等），候选链必须写清
   主选与兜底、`locale_dependent` / `fallback_only` 标注。
2. 对齐 `paint_handlers.rs` 的 read-back 契约（工具 / 颜色 / 图层）到真实控件能提供的语义。
3. 在 `production_paint.rs` 之外补一条**真机 `#[ignore]` 验收**（或在 TASK-044 的 eval 目录里
   给出可复跑的实机脚本），使 fake 与真实行为不再各自为政。
4. 若实现中发现必须改 `crates/**` 公共接口 / schema / ErrorCode → 停下记 DRIFT，另立 ADR。

**Out of scope**

- Paint 其余工具（`paint.canvas.rollback_last_write` / `paint.file.open` / `paint.file.save_as`）。
- T3.2 / T3.3 任务包（TASK-045 / 046）。
- 改 `crates/**` 公共接口、schema、ErrorCode、新增 crate 或第三方依赖。
- 绕过 Paint 自身确认、静默覆盖文件。

**必须遵守**

- **ADR-0022 D4**：可见文本只能作最低分兜底且必须 `locale_dependent=true`；不得把中文名升级成主 selector。
- **ADR-0043**：元素解析必须在窗口 scope 内；父候选只作作用域。
- 铁律 1：read-back 失败必须返回带 `ErrorCode` 的失败，不得用默认值冒充成功。
- 铁律 4：每个写步骤必须有可验证的 postcondition。
- 真机操作必须有人在场；不得无人值守操作真实 GUI（章程 §3.7 / ADR-0084 D7）。

**待裁决（动手前需要人类拍板；不裁决则停下记 DRIFT）**

1. **形状 / 调色板这类无 aid 的 `GridViewItem` 用什么锚点？**
   - 选项 A：`role_and_parent`（`ListItem` + 父 `GridView`）—— 但真实树里有**两个** `GridView`
     （形状、颜色），父本身也只能靠本地化 Name（`形状` / `颜色`）区分，等于把本地化依赖下移。
   - 选项 B：直接用本地化 Name 作 `name_regex` 兜底（`locale_dependent=true`，最低分）——
     与 ADR-0022 D4 相容，但换语言即失效。
   - 选项 C：先点击父容器（`NamedContainerAutomationPeer`）再取子项 —— 需要新的候选种类（漂移触发器 ⑨）。
2. **任意 RGB 前景色怎么选？** Paint 调色板只有固定色；`0,180,0` / `0,128,255` 这类值不在调色板里。
   - 选项 A：走「编辑颜色」对话框输入 RGB（新增一条对话框交互路径）。
   - 选项 B：把 T3.1 的评测用例收窄到调色板已有的颜色（改 `eval/tasks/paint/**`，属 TASK-044 契约）。
3. **图层步骤怎么处理？** 层面板默认折叠且没有 `ListViewItem`。
   - 选项 A：先展开层面板再选层（需要新的展开动作 + 真实图层项身份）。
   - 选项 B：把 `select_layer` 从 T3.1 任务包移出（改 `adapters/com.microsoft.paint/tasks/**`）。
4. **工具 read-back 用什么？** 真实 ToggleButton / GridViewItem 的 Name 是本地化文本。
   - 选项 A：读 UIA 选中态（`IsSelected`）而不是文本 —— 需要平台层暴露该只读属性（可能触发 ADR）。
   - 选项 B：把期望值改成适配包声明的本地化名（随语言失效）。

**验收命令**

```powershell
python eval/tasks/paint/t3.1/validate.py
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_paint
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments
```

**DoD**

- [ ] `selectors/targets.json` 的 T3.1 target 全部按实测树校准，主选非本地化、兜底显式标注
- [ ] `paint_handlers.rs` 的工具 / 颜色 / 图层 read-back 与真实控件语义一致
- [ ] 真机 `#[ignore]` 验收（或等价可复跑脚本）存在，且 fake 与真机行为不再各自为政
- [ ] TASK-044 的真机十次运行可在本卡之后执行（不要求本卡跑满十次）
- [ ] 上述验收命令全绿
- [ ] LEDGER.md 追加一行；新事实/坑进 `docs/memory/apps/paint.md` 或 `docs/memory/{facts,pitfalls}.md`
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

（未开工 —— 待 Orchestrator 裁决「待裁决」四条后填写。）

### 2. 实际改动文件

（未开工。）

### 3. 验收输出摘要

（未开工。）

### 4. DoD 逐条核对

（未开工。）

### 5. 偏差

（未开工。）

### 6. 更合理做法

（未开工。）

### 7. 遗留问题

（未开工。）

### 8. 新增长期记忆

（未开工。）

### 9. 给审阅者的关注点

（未开工。）
