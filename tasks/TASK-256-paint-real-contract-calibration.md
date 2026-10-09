# TASK-256　Paint 真机契约校准：selector 与 handler read-back 对齐实测树

- 状态：**Review**
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

- 任务：TASK-256 Paint 真机契约校准恢复；ADR-0086 / TASK-257 已合并，允许继续 selector 与 handler read-back 校准。
- write scope：`adapters/com.microsoft.paint/**`、`apps/agent-core/src/paint_handlers.rs`、`apps/agent-core/tests/production_paint.rs`、`eval/tasks/paint/**`。
- 关键约束：selector 只能在已解析窗口 scope 内解析；可见文本仅最低分兜底；写操作必须有可验证 postcondition；错误必须带 `ErrorCode`。

### 2. 实际改动文件

- `apps/agent-core/src/paint_handlers.rs`：恢复并收敛真实 Paint 11.2605.81.0 的工具 / 调色板 / 图层选择契约；调色板 RGB → 实测 child index；图层折叠时显式展开；文件压到 900 行以内，`hygiene` 硬门禁恢复。
- `apps/agent-core/tests/production_paint.rs`：保留 fake 4 passed，并新增显式 `#[ignore]` 的真实 Paint 生产装配验收入口。
- `eval/tasks/paint/t3.1/cases.json`、`eval/tasks/paint/t3.1/expected.json`：恢复真机实测颜色 / 坐标评测数据。
- `adapters/com.microsoft.paint/selectors/targets.json`：冲突按已合并 TASK-257 的 ExactName 契约保留，不回退为 `name_regex`。

### 3. 验收输出摘要

- `cargo test --workspace`：通过，469 passed；doc-tests 全部通过。
- `cargo test -p assistant-agent-core --test production_paint`：fake 4 passed。
- `cargo test -p assistant-agent-core --lib paint_handlers`：4 passed。
- `cargo test -p assistant-agent-core --test production_paint --no-run`：真实 ignored 测试编译通过。
- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py`：PASSED。
- `python eval/tasks/paint/t3.1/validate.py`：PASSED。
- `cargo fmt --all --check`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过；仅既有 `clippy::assert_is_empty` unknown-lint warning。
- `hygiene`：0 errors / 119 warnings；`paint_handlers.rs` 为 900 行，超过 600 行软建议线但未超过 900 行硬上限。
- 其余 `memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` / `check-comments`：通过。

### 4. DoD 逐条核对

- [x] selector 与 handler fake 契约已按实测 Paint 树对齐，TASK-257 ExactName 未回退。
- [x] 真实 `#[ignore]` 验收入口已加入并完成编译；实际真机运行待有人在场执行。
- [ ] 真机 Paint 运行结果尚未采集，不能宣称 TASK-256 Done。
- [x] workspace / Paint fake / handler 单测 / adapter / eval / xtask 门禁全绿。
- [x] 未改 `crates/**` 公共接口、schema、ErrorCode，未引入依赖。

### 5. 偏差

`DRIFT-256-1`（漂移触发器 ③ + ⑨）：`1A` 在当前 selector 能力下不可实现。

- **现象**：2026-10-08 真机单次探针确认，`形状` / `颜色` / `层` 的
  `name_regex` 是 UIA 原生子串匹配，分别命中 **3 / 5 / 6** 个元素
  （例如 `形状` 同时命中 `形状轮廓`、`形状填充`）。把它们作为
  `RoleAndParent.parent_id` 的作用域候选时，平台会先因父候选歧义返回
  `TargetAmbiguous`，根本不会继续解析子候选。
- **影响**：现有候选种类无法同时表达“非本地化 class/role + 精确名称”或
  “容器内第 N 个可选项”。WIP 中“父作用域 + `Selection::ByIndex`”可按
  逻辑完成 handler，但真机仍在第一步 selector 失败；只跑 fake 会制造
  假绿。
- **已停工作**：停止继续改 selector / handler / fake，不提交 WIP，不把
  fake 通过冒充真机校准，不开 PR。
- **建议**：按卡面选项 C 另立 ADR/放行，选择最小候选扩展：
  ①新增精确名称 + class/role 候选，或②为容器增加受版本约束的子项索引
  候选。裁决前不要放宽 `TargetAmbiguous`，也不要新增依赖。

保留的已验证事实：展开 `层` 后，`automation_id=layersList` 可唯一解析，
第一项可见名为 `图层 1`；`SelectionItemPattern::IsSelected` 强后置条件可用。
调色板 20 色及 RGB/index 映射已实测并保存在
`eval/tasks/paint/t3.1/probe-evidence.json` 的 `real_contract_calibration`。

**2026-10-09 人类裁决 + 补充真机实测**：按用户「就按这个执行」，
① 把本轮真机结论写入 `probe-evidence.json` 的 `keytip_and_palette_probe` 与
`docs/memory/apps/paint.md` §11 —— Ribbon KeyTips 可用（工具 `Alt+T,*` / 画笔 `Alt+B` /
图层 `Alt+L` / 编辑颜色 `Alt+E,C`）；**形状库无 KeyTip 且形状项 `IsKeyboardFocusable=false`**
（L2 选形状不成立）；固定调色板 = 公开的 MS Paint 20 色表且与实测 RGB 逐色相等；
② 按建议①立 **ADR-0086**（`SelectorKind::ExactName`：UIA `PropertyCondition` 精确相等、
零依赖、`locale_dependent=true`、只作兜底、0/多命中 fail-closed）+ 落地卡 **TASK-257**。
本卡当时保持 **Blocked**，待 TASK-257 落地后恢复。

**2026-10-09 TASK-257 合并后恢复**：ADR-0086 已进入 `main`，本卡已取回 WIP，
恢复 handler / fake / eval 校准；当前状态改为 **Review**，等待真实 Paint 交互验收。

### 6. 更合理做法

先复用已合并的 ExactName 契约，再把真实控件可提供的 `SelectionItemPattern::IsSelected`
作为工具 / 调色板 / 图层的强后置条件，避免重新引入本地化文本 read-back。

### 7. 遗留问题

- 真实 Paint 11.2605.81.0 交互验收尚未运行；需先打开 Paint，再执行 production_paint ignored 测试。
- `paint_handlers.rs` 当前 900 行，仍有 600 行软建议 warning；若继续增长，需另立拆文件卡，不在本卡 write scope 内静默扩模块。
- TASK-256 尚未达到 Done，不能解锁 TASK-044 的十次运行声明。

### 8. 新增长期记忆

- 无新增长期事实；本轮使用既有 Paint 真机证据与 ADR-0086，未新增应用坑。

### 9. 给审阅者的关注点

- `Selection::ByIndex` 的 child index 必须对应 Paint 11.2605.81.0 实测 RGB / 矩形索引，不能改成猜第一个。
- 图层面板折叠时必须先通过稳定 `layersList` / `Alt+L` 路径展开，再做 layer child selection。
- 真机 ignored 测试没有 Paint 窗口时必须显式失败，不得以 fake 结果替代。
