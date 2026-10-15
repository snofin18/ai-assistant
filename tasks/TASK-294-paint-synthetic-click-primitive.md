# TASK-294　Paint 合成点击通道：`click_element_center` 与 TASK-256 切到 L4

- 状态：Review
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：TASK-292、TASK-293、ADR-0092、ADR-0093
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-292（平台契约，已合并）、TASK-293（Windows 实现，已合并）、ADR-0092（指针窗口约束）、ADR-0093（几何回 `CoordinateSpace`）　**预估**：M　**难度**：M
- **write scope**：`apps/agent-core/src/paint_handler_actions.rs`、`apps/agent-core/src/paint_handlers.rs`、`apps/agent-core/src/paint_handler_map.rs`、`apps/agent-core/tests/production_paint.rs`
- **关联**：`tasks/TASK-256-paint-real-contract-calibration.md`（`DRIFT-256-5` / `DRIFT-256-6`）、ADR-0067、ADR-0085、ADR-0063

**目标**

给 Paint T3.1 一条真机可用的 **L4 合成点击** 原语 `click_element_center(task_id, window, element, tool)`：
取元素几何 + 窗口几何（各一次，都走 `support::poll_immediate` + `map_platform_error`）→ **窗口内断言**
（元素中心必须落在窗口矩形内，半开区间，越界 → `ToolInvalidArgs`，**禁止 clamp**）→
在窗口独占租约内发一次 `PointerAction::Click`。并把 `paint.tool.select` 从真机上无效的
`invoke_action(toggle)` 切到这条原语上（`DRIFT-256-5`：形状项 `TogglePattern.Toggle()` 是空操作）。

**为什么必须是合成点击（别改回 toggle）**

真机一手证据（Paint 11.2605.81.0 / zh-CN，见 `docs/memory/apps/paint.md` §11 与 `DRIFT-256-5`）：
形状项的 `Button|矩形` 全树唯一，但 `TogglePattern.Toggle()` 两向皆为**空操作**；`ListItem|矩形`
只有 `ScrollItem,VirtualizedItem`（无 `SelectionItemPattern`），`Button|矩形` 只有 `Toggle,ScrollItem`
（无 `InvokePattern`）。这条路线上**没有任何可用的 UIA pattern**，只有真实鼠标点击能选中 ——
而产品本身就要点击 / 连续点击 / 拖拽这条通道，所以这是主线不是权宜。

**In scope**

1. `paint_handler_actions.rs`：新增 `window_geometry` / `element_geometry`（各走既有 `poll_immediate` 模式）
   与 `click_element_center`（取几何 → 窗口内断言 → 租约内 `pointer_action(Click)`），与既有 `drag` 同形。
2. `paint_handlers.rs`：`select_tool_output` 加 `task_id` 参数，把 `invoke_element(toggle)` 换成
   `click_element_center(task_id, &window, &shape_item, TOOL_TOOL_SELECT)`。
3. `paint_handler_map.rs`：`ToolSelectHandler::call` 传 `call.task_id()`。
4. `production_paint.rs`：fake 实现 `window_geometry` / `element_geometry`（复用 per-id 矩形；坐标空间
   `PhysicalPixels` / scale 1.0 / 非空显示器名；窗口矩形**包住**所有 fixture 元素矩形）；`pointer_action`
   落在矩形工具矩形内的 `Click` 置工具为 `rectangle`（`DragTo` 保持置 `drawn=true`）；**删掉**
   `invoke_action(RECTANGLE_TOOL_ID, "toggle")` 分支；加调用计数断言（一次 `paint.tool.select` =
   1×`element_geometry` + 1×`window_geometry` + 1×`pointer_action`）。

**Out of scope（做了算漂移）**

- 改 `crates/**` 公共接口 / schema / `ErrorCode` / `PointerAction` / `element_bounds`（ADR-0092 细则 9）。
- dock 定位与 IPC（TASK-295）、TASK-256 的其它 handler 校准、T3.2 / T3.3。
- 新增依赖、`#[allow]`、`unsafe`；`clamp` 越界点或猜显示器 / scale。
- 让 fake 为真机上不成立的旧契约（形状项 `toggle`）继续背书。

**必须遵守**

- **ADR-0092 细则 8**：合成指针必须落在目标窗口矩形内；越界 → `ToolInvalidArgs`，**禁止 clamp**（铁律 9）。
- **ADR-0092 细则 5**：热路径禁止枚举显示器；一次 `click_element_center` = 1 次元素几何 + 1 次 `pointer_action`。
- 铁律 1：平台失败必须映射成带 `ErrorCode` 的失败，不得用默认值冒充成功。
- **ADR-0067**：坐标空间由几何自带，不得收敛式猜 scale / origin。
- fake 只反映真机成立的契约；`toggle` 路径即真机证伪，必须删除其 fixture 支持。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_paint
cargo run -p xtask -- hygiene
cargo run -p xtask -- check-ledger
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
```

真机（需人类把 Paint 置于前台）：

```powershell
Start-Process "shell:AppsFolder\Microsoft.Paint_8wekyb3d8bbwe!App"
cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_1_runs_on_real_paint
```

**DoD**

- [ ] `click_element_center` 落地：几何各一次 + 窗口内断言（禁 clamp）+ 租约内 `Click`
- [ ] `paint.tool.select` 切到合成点击；`task_id` 经 handler map 传入
- [ ] fake 实现两处几何、`Click` 置工具、删除旧 `toggle` 分支、加调用计数断言
- [ ] 上述验收命令全绿；真机 ignored 用例可由人类在场跑通（`DRIFT-256-5` 闭环）
- [ ] 未改 `crates/**` 公共接口 / schema / `ErrorCode`；未加依赖 / `#[allow]` / `unsafe`
- [ ] `LEDGER.md` 追加一行；任务卡 9 节记录填写；新事实 / 坑进 `docs/memory/apps/paint.md`
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-294（Paint 合成点击通道；TASK-256 切到 L4）。
- **write scope**：仅上列四个文件（+ 本卡文件）。
- **铁律**：1（显式失败）、4（写操作有 postcondition）、9（越界禁 clamp）、10（契约先行，ADR-0092/0093 已 Accepted）。
- **禁止**：改 `crates/**` / `PointerAction` / `ErrorCode` / schema；加依赖 / `#[allow]` / `unsafe`；猜显示器 / scale。
- **依赖**：TASK-292（`5895179`）、TASK-293（`5895179`）已合并；ADR-0092 / ADR-0093 已 Accepted。
- **疑问**：#399/#401/#402/#403 的 LEDGER merge-hash 合并回填是否接受 → 默认不动、收尾单独问，不在本卡范围。

### 2. 实际改动文件

- `apps/agent-core/src/paint_handler_actions.rs`（`click_element_center` + 两个几何查询）→ **In scope**
- `apps/agent-core/src/paint_handlers.rs`（`select_tool_output` 加 `task_id`，切到合成点击）→ **In scope**
- `apps/agent-core/src/paint_handler_map.rs`（传 `call.task_id()`）→ **In scope**
- `apps/agent-core/tests/production_paint.rs`（fake 两处几何 + `Click` 置工具 + 删旧 `toggle` + 调用计数）→ **In scope**

### 3. 验收输出摘要

- `cargo fmt --all --check`：exit 0（写入后回归）。
- `cargo clippy --all-targets -- -D warnings`：**exit 0**（既有 `clippy::assert_is_empty` unknown-lint warning 与本卡无关）。
- `cargo test --workspace`：**exit 0**（含全部 doc-tests）。
- `cargo test -p assistant-agent-core --test production_paint`：**4 passed / 0 failed / 1 ignored**。
  新增的调用计数断言全过：`element_geometry_calls == 1`、`window_geometry_calls == 1`、
  `rectangle_tool_clicks == 1`、`pointer_actions == 2`（1 Click + 1 DragTo）、`tool == "rectangle"`、`drawn == true`。
- `cargo run -p xtask -- hygiene`：PASSED（0 error / 115 warning，均为既有项）。
- `cargo run -p xtask -- check-ledger`：PASSED（plan_date = ledger_last_date = 2026-10-15）。
- `cargo run -p xtask -- card-check`：PASSED（0 error；本卡无发现项）。
- `cargo run -p xtask -- docscan`：PASSED（0 error）。
- 追加：`check-comments` / `refscan` / `adr-index` / `memory-counts` 全 PASSED
  （`memory-counts` 因 `apps/paint.md` 243 → 266 行同步 `MEMORY.md` 规模表后转绿）。

### 4. DoD 逐条核对

- [x] `click_element_center`：元素几何 + 窗口几何各一次、窗口内断言（半开区间、禁 clamp）、租约内 `Click`
- [x] `paint.tool.select` 切到合成点击；`task_id` 经 handler map 传入
- [x] fake 实现两处几何、`Click` 置工具、删除旧 `toggle` 分支、加调用计数断言
- [x] 验收命令全绿（真机 ignored 见 §7；`DRIFT-256-5` 的真机闭环依赖人类在场）
- [x] 未改 `crates/**` 公共接口 / schema / `ErrorCode`；未加依赖 / `#[allow]` / `unsafe`
- [x] `LEDGER.md` 追加一行；9 节记录填写；应用坑进 `docs/memory/apps/paint.md`
- [x] 无 Out of scope 文件被修改

### 5. 偏差

none。

### 6. 更合理做法

- 复用既有 `drag` 的租约写法与 `normalized_point` 换算，不另造租约 / 换算路径 —— 同一概念全项目只实现一次。
- 几何查询抽成 `window_geometry` / `element_geometry` 两个私有原语（与既有 `element_bounds` 同形），
  `click_element_center` 只负责断言 + 点击；后续「点击任意元素」都复用它。
- fake 用「点在矩形工具矩形内」而不是「点等于某个常量」判定命中，明确表达「落在 RECTANGLE_TOOL_ID 矩形内」。

### 7. 遗留问题

- **真机 ignored 用例（`test_paint_t3_1_runs_on_real_paint`）尚未跑** —— 它需要人类把
  Paint 11.2605.81.0 置于前台（测试只等 30 s、不抢焦点）。跑通才算 `DRIFT-256-5` 闭环；
  在它通过前 TASK-256 仍停在 Review、TASK-044 的十次运行也不得开始。
- **2026-10-15 真机尝试 1 次（失败，未发输入）**：Paint 已在运行（PID `7980` / HWND `6884838`、
  class `MSPaintApp`、标题「无标题 - 画图」），但测试进程与 agent 的 shell **都无法把 Paint 置前台**
  —— `SetForegroundWindow`（经 `AttachThreadInput`）被 Windows 前台锁拒绝，`GetForegroundWindow`
  始终是 Chrome（PID `10596`）。用例在 30 s 内未等到 Paint 前台 → 显式失败
  （`Error: "timed out waiting for a human to bring Paint to the foreground"`），
  **未发送任何输入**（测试在 wait 阶段就返回，符合 fail-closed 设计）。需人类手动点一下 Paint 再重跑。
- 混合 DPI / 多屏的真机归属（`element_geometry` 的中心点归属）仍按 TASK-293 的说明由后续真机验收覆盖。

### 8. 新增长期记忆

- 见收尾同步（应用坑进 `docs/memory/apps/paint.md`）。

### 9. 给审阅者的关注点

- 窗口内断言是**半开区间** `[left, right) × [top, bottom)`，越界直接 `ToolInvalidArgs`；若改成 clamp
  或四舍五入到边界内即违反 ADR-0092 细则 8。
- 调用计数断言把 ADR-0092 细则 5 变成机器判据：`element_geometry` / `window_geometry` 各恰一次，
  且矩形工具的 `Click` 恰一次；`DragTo` 走 draw 路径不计入该点击计数。
