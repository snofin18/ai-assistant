# TASK-044　T3.1：新建画布 → 选矩形工具与颜色 → 指定画布坐标画矩形 → 截图验证形状与颜色

- 状态：**Review**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：043　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：043　**预估**：M　**难度**：M
- **write scope**：`adapters/com.microsoft.paint/tasks/**`、`eval/tasks/paint/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

T3.1：新建画布 → 选矩形工具与颜色 → 指定画布坐标画矩形 → 截图验证形状与颜色。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`adapters/com.microsoft.paint/tasks/**`、`eval/tasks/paint/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
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
【任务】TASK-044 T3.1：新建画布 -> 选矩形工具与颜色 -> 指定坐标画矩形 -> 截图验证
【目标】落地 Paint T3.1 任务包、10 用例评测契约、真实 UIA 探针证据；真机成功率/坐标误差不伪造
【write scope】adapters/com.microsoft.paint/tasks/**、eval/tasks/paint/**；按卡面步骤同步记录与必要记忆/台账
【铁律】无静默失败；不可信输入先校验；写操作必须有 postcondition；不静默扩大范围；契约先行
【禁止】改公共 trait/schema/spec；新增依赖/crate；改 TASK-043 selector 文件；把探针冒充真机 10 次成功
【验收】专项 validate.py；JSON/TOML 解析；fmt/clippy/tests；xtask 门禁
【依赖】TASK-043 已合并 `a3bd2e6`
【疑问】真机十次运行与 <=2 px 拖拽误差未测 -> DRIFT-044-1
```

### 2. 实际改动文件

- `adapters/com.microsoft.paint/tasks/README.md`
- `adapters/com.microsoft.paint/tasks/t3.1.new-canvas-rectangle-color-screenshot.json`
- `eval/tasks/paint/t3.1/README.md`
- `eval/tasks/paint/t3.1/cases.json`
- `eval/tasks/paint/t3.1/expected.json`
- `eval/tasks/paint/t3.1/probe-evidence.json`
- `eval/tasks/paint/t3.1/validate.py`
- 本卡记录区；`docs/memory/apps/paint.md`（真实 UIA 形状）；`LEDGER.md`（本轮事件）

### 3. 验收输出摘要

- 临时 Rust UIA 探针（不入库）通过 AUMID 启动 Paint，只读导出真实窗口树：
  `mspaint.exe`、`MSPaintApp`、`PencilTool`、`EraserTool`、
  `CanvasSizeTextBlock`、`ZoomValuesComboBox`、`ZoomSliderControl`、
  canvas `aid=image`、观察画布 `418 x 74` px、渲染 `209 x 37` px @ `0.5x`。
- `python eval/tasks/paint/t3.1/validate.py` ->
  `paint_t3_1_static_validation_ok cases=10 tolerance_px=2 probe=canvas_image_zoom_0.5`
- 专项 JSON 解析：`json.tool` 对 task / cases / expected / probe 全部通过。
- 2026-10-08 复跑：`cargo fmt --all --check` EXIT 0；`cargo clippy --all-targets -- -D warnings`
  EXIT 0；`cargo test --workspace` EXIT 0（xtask 469 passed）；xtask `hygiene`（0E/100W）/
  `memory-counts` / `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` /
  `check-comments` / `verify-schemas` 全 **PASSED**。
- 2026-10-08 复跑把本分支从 `codex/task-043-merge-backfill` 重基到 `main`（`2e5e184`），
  消解 PR #275 的 `CONFLICTING` 状态；重基提交 `78bddd7`。
- 2026-10-08 PR #275 已合并，**merge hash `c7db833`**（base `main`，pull_request 与 push
  两条工作流全部 job pass）。

2026-10-08 复跑后追加**真机 selector 解析探测**（临时 harness `target/paint_bind`，不入库，
用仓库自身的 `assistant-platform-windows` 去打真实 Paint 窗口）：

- `main_window` 声明候选 `class_and_role class=WinUIDesktopWin32WindowClass role=Window`
  → `TargetNotFound`（0 命中）；真实窗口 class 是 `MSPaintApp`，用 `MSPaintApp` + `Window`
  可解析。
- `canvas` 声明候选 `class_and_role class=Image role=Image` → `TargetAmbiguous`（窗口内
  38 命中）；改用 `automation_id=image` 可解析，`role=Group`、
  `bounds=(696,525,905,562)`、`size=209x37`（与 2026-10-07 探针一致）。
- `rectangle_tool_button` 声明候选 `class_and_role class=GridViewItem role=ListItem`
  → `TargetAmbiguous`（窗口内 43 命中）。
- 结果已落 `eval/tasks/paint/t3.1/probe-evidence.json` 的 `selector_resolution_probe`，
  并由 `validate.py` 断言锁定；`python eval/tasks/paint/t3.1/validate.py` 仍 PASSED。

### 4. DoD 逐条核对

- [x] card 标题声明的任务包与评测契约可被静态测试覆盖。
- [x] 10 个评测用例、2 px 误差上限、视觉容差与静默失败 0 已进入评测契约。
- [x] 真实 UIA 探针证据已落 `probe-evidence.json` 与 Paint 应用档案。
- [ ] 真机连续 10 次成功率 >= 75%（未运行，DRIFT-044-1；2026-10-08 实测被 selector
      未校准阻断 —— DRIFT-044-2）。
- [ ] 真机拖拽坐标误差 <= 2 px（未运行，DRIFT-044-1；同上被阻断）。
- [x] `cargo fmt --all --check` / `clippy` / `test --workspace` / xtask 全套 —— 2026-10-08 复跑全绿（见 §3）。

### 5. 偏差

`DRIFT-044-1`：本会话取得了真实 UIA 树和单次画布 bounds/zoom，但没有完成
真实拖拽、10 次成功率、混合 DPI 坐标误差与最终像素容差测量。TASK-044 的
评测契约因此是 `real_run_status = not_run`，不得把探针结果冒充验收结果。

`DRIFT-044-2`（漂移触发器 ⑤ + ⑧）：2026-10-08 用仓库自身 `WindowsPlatform` 打真实 Paint
窗口，测得**当前 selector 包无法驱动本卡的真机十次验收** ——
`main_window` 声明 class `WinUIDesktopWin32WindowClass` 与真实 class `MSPaintApp` 不符，
直接 `TargetNotFound`；`canvas` 与 `rectangle_tool_button` 的声明 class/role 在真实窗口内
分别命中 38 / 43 个元素，稳定 `TargetAmbiguous`。而 TASK-106 的 7 个 Paint handler 全部以
`resolve_window("main_window")` 起步，因此真机 `Plan` 在第 1 步就会失败。
**影响**：本卡的 DoD「真机连续 10 次成功率 ≥ 75%」在本会话不可达，且不是靠重跑能解决的。
**建议**：另立**Paint selector 校准卡**，把 `adapters/com.microsoft.paint/selectors/targets.json`
的 `main_window` / `canvas` / 工具 / 颜色 / 图层候选改成实测值（含 `automation_id` 主选、
class+role 兜底、`fallback_only` / `locale_dependent` 标注），再回来跑真机十次。
**已停工作**：未改 `selectors/**`（不在本卡 write scope，卡面已明确 selector 校正属后续卡），
未跑真实拖拽，未把探测数据冒充十次验收；只把实测结果落进 `eval/tasks/paint/**` 并加断言。

`DRIFT-044-3`（漂移触发器 ⑤ + ⑧）：同日抓取真实 Paint 的**完整 ControlView 树**后确认，
阻塞不止在 selector —— `apps/agent-core/src/paint_handlers.rs` 的 read-back 契约在真实控件上
不成立，而这些契约只在 fake 平台（`tests/production_paint.rs`）被验证过：
`paint.tool.select` 要求按钮文本等于英文工具 id（真实是本地化 `矩形` 的 `GridViewItem`、无 aid）；
`paint.color.select_foreground` 要求 `set_value(button,"R,G,B")` 后读回同值（真实是
`RadioButton`「颜色 1: 黑色」，无 set_value）；`paint.layer.select` 要求 `ListViewItem`（层面板
默认折叠，整棵树没有 `ListView`/`ListViewItem`）；`paint.document.new` 解析 `status_bar`（无该角色，
尺寸在 `TextBlock aid=CanvasSizeTextBlock`）。
**影响**：即使 selector 全部校准，T3.1 真机 `Plan` 仍会在 `select_rectangle_tool` /
`select_foreground_color` / `select_layer` 上失败 —— 这不是本卡能修的，也不在本卡 write scope。
**建议**：由 **TASK-256**（`tasks/TASK-256-paint-real-contract-calibration.md`）承接，
它同时拥有 `selectors/**` 与 `paint_handlers.rs` 写权限，并在卡面列出四条待人类裁决的设计点
（无 aid 的调色板锚点、任意 RGB 选择路径、图层折叠态、工具 read-back 语义）。
**已停工作**：未改 handler、未改 selector、未跑真机拖拽；只把实测树与契约错配落进
`eval/tasks/paint/t3.1/probe-evidence.json`（`real_control_tree` + `handler_contract_probe`）
并由 `validate.py` 断言锁定。

另：2026-10-08 本卡分支因 `main` 前进到 `2e5e184` 出现冲突，已重基并复跑全套门禁；
冲突只在 `LEDGER.md`（保留主库 TASK-253/254/255 行与本卡行）。本卡状态仍为
`Review`，实机十次运行完成前不得标记 Done。

另：TASK-043 selector 包仍标记 `probe_status=required`，其真实 ID 更新不在
TASK-044 write scope 内；本卡把真实 UIA 证据落到评测目录和应用记忆，供后续
集成卡校正 selector 包。

### 6. 更合理做法

- 先用真实 UIA 探针把“事实边界”钉死，再写任务包/评测契约；避免继续用
  `probe_status=required` 的 selector 猜坐标。
- 把矩形和调色板这类“无稳定 AutomationId、只有本地化可见文本”的控件标成
  `fallback_only=true`，不把中文名升级成主 selector。
- 用 canvas `aid=image`、`CanvasSizeTextBlock`、`ZoomValuesComboBox`、
  `ZoomSliderControl` 作为可复核的坐标/缩放锚点，后续只补拖拽误差。

### 7. 遗留问题

- `DRIFT-044-1` / `PL-113`：真机十次运行、拖拽误差、多屏 DPI 与像素容差待人工验收。
- `DRIFT-044-2`：**先决阻塞** —— Paint selector 包未按真实 UIA 校准（`main_window` 直接
  0 命中），真机十次验收在它修好前跑不起来。需要一张拥有
  `adapters/com.microsoft.paint/selectors/**` 写权限的校准卡。
- `DRIFT-044-3`：**先决阻塞** —— `paint_handlers.rs` 的工具 / 颜色 / 图层 read-back 契约
  只在 fake 平台成立，真实 Paint 给不出（无 aid 的本地化 `GridViewItem`、无 `set_value` 的
  `RadioButton`、折叠的层面板）。只校准 selector 仍跑不通。
- TASK-043 selector 文件需要后续集成卡按本卡探针证据更新；当前保持 provisional。
- 归属缺口**已开卡**：**TASK-256**（`tasks/TASK-256-paint-real-contract-calibration.md`，
  Ready）同时拥有 `adapters/com.microsoft.paint/**` 与 `apps/agent-core/src/paint_handlers.rs`，
  卡面列出四条待人类裁决的设计点；`DRIFT-044-1/2/3` 全部由它承接。

### 8. 新增长期记忆

- `docs/memory/apps/paint.md`：补真实 UIA 树的关键 AutomationId、canvas
  `aid=image`、`418 x 74 @ 0.5x` 观察值，以及矩形/调色板 fallback-only 边界。

### 9. 给审阅者的关注点

1. 矩形工具和调色板没有稳定 AutomationId，后续 selector 校正必须保留
   `locale_dependent=true` / `fallback_only=true`，不能把中文名当主选择器。
2. canvas 的 UIA bounds、zoom 与 scroll 偏移来自一次观察；混合 DPI 和多屏仍未证明。
3. `rectangle_visual_assert` 的 `max_changed_ratio=0.08` 与 `confidence_min=0.85`
   仍是契约值，不是实测结果；TASK-044/后续验收必须用真实截图校准。
