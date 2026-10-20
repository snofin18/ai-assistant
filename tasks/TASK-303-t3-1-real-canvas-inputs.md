# TASK-303　T3.1 真机验收：画布参数按实测注入（尺寸 / 缩放）

- 状态：Done
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：TASK-256、TASK-294/296/297（真机链路前三步已通）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-256（真机 T3.1 验收）　**预估**：S　**难度**：S
- **write scope**：`apps/agent-core/tests/production_paint.rs`、`apps/agent-core/tests/support/paint_real_run.rs`
- **关联**：`adapters/com.microsoft.paint/tasks/t3.1.new-canvas-rectangle-color-screenshot.json`（前置条件）、`docs/memory/apps/paint.md` §7（画布坐标约定）

**目标**

让真机 T3.1 验收注入的画布参数与**真实文档**一致。当前真机跑通
`new_document` → `select_rectangle_tool` → `select_foreground_color` → `select_layer` 后，卡在
`resolve_end`：`canvas point (50,50) maps outside bounds (708,484)-(917,521)`。

**根因（一手实测）**

任务包把 `canvas_width` / `canvas_height` / `zoom_ratio` 声明为**调用方输入**，并把
「zoom ratio 与 viewport offsets 必须与 resolve 时的画布状态一致」写成**前置条件**。
而 harness 注入的是 fake 平台的常量（100×100、zoom 1.0）；真机文档实测为
**418×74 px、渲染 209×37 ⇒ zoom 0.5**。于是终点按 zoom 1.0 映射成 y=534，越过渲染底边 521，
`resolve_point` 按契约显式拒绝（fail-closed，**不是**产品缺陷）。

**In scope**

1. `paint_config` 拆分出 `paint_config_with_canvas(data_root, PaintCanvasInputs)`；
   `paint_config` 以 fake 常量（`PaintCanvasInputs::FAKE` = 100×100 / 1.0）委托，fake 路径行为不变。
2. 真机用例新增只读探测 `read_real_canvas_inputs`：`canvas`（aid `image`）渲染宽度 +
   `CanvasSizeTextBlock` 文本（`"418 × 74像素"`）→ `zoom_ratio = 渲染宽 / 画布宽`。
3. 矩形点沿用 (10,10)-(50,50)：画布任一边 ≤50 时**显式失败**，不静默 clamp。

**Out of scope（做了算漂移）**

- 产品侧 handler / adapter（`paint.canvas.resolve_point` 的语义不变）。
- fake 平台与其 fixture 常量。
- 画布滚动（viewport offset）建模：本轮按「新建文档未滚动」取 0。

**必须遵守**

- 铁律 1：探测不到画布尺寸 / 尺寸过小 → 显式失败，不猜。
- 不改任何测试断言来「让它过」。
- 真机操作仍需人类在场（测试只等前台、不抢焦点）。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_paint
cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_1_runs_on_real_paint   # 真机，需人类置前台
```

**DoD**

- [ ] `paint_config_with_canvas` + 只读探测落地，fake 路径零行为变更
- [ ] 上述静态验收全绿（真机 E2E 见 §7）
- [ ] `LEDGER.md` 追加一行；9 节记录填写
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-303（真机注入参数取实测画布状态）。
- **write scope**：上列两个测试文件 + 本卡 / 台账。
- **铁律**：1（测不到就显式失败）、4（写操作前状态必须与契约一致）、9（不静默 clamp）。
- **禁止**：改产品 handler / adapter / fake fixture。
- **疑问**：无。

### 2. 实际改动文件

- `apps/agent-core/tests/production_paint.rs`（`PaintCanvasInputs` + `paint_config_with_canvas`）
- `apps/agent-core/tests/support/paint_real_run.rs`（`read_real_canvas_inputs` + 单候选链 / 尺寸解析 helper）
- `LEDGER.md`、本卡

### 3. 验收输出摘要

- `cargo fmt --all --check`：exit 0。
- `cargo test -p assistant-agent-core --test production_paint`：**4 passed / 0 failed / 1 ignored**（fake 路径行为不变）。
- **真机（第 3 次触发，人类在场）**：`cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_1_runs_on_real_paint`
  → **`1 passed; 0 failed; 0 ignored`（11.11 s）** —— T3.1 全链路首次在真机跑通。

### 4. DoD 逐条核对

- [x] `paint_config_with_canvas` + 只读探测落地；`paint_config` 以 `PaintCanvasInputs::FAKE` 委托 → fake 路径零行为变更
- [x] 静态验收全绿 + **真机 E2E 通过**（见 §7）
- [x] `LEDGER.md` 追加一行
- [x] 无 Out of scope 文件被修改

### 5. 偏差

none（本卡是 T3.1 真机链路在 TASK-294/296/297 之后暴露的**下一处 harness 输入缺口**）。

### 6. 更合理做法

- 画布参数**实测注入**而不是写死：任务包已经把「与 resolve 时画布状态一致」写成前置条件，写死常量必然在真机（zoom 0.5）上越界。
- 探测是**只读**的（UIA 属性读），不改变 Paint 状态；与「设置缩放到 100% 再跑」相比不动用户界面。
- 尺寸/解析失败、画布过小都**显式失败**：宁可红，也不静默画到被 clamp 的点上。

### 7. 遗留问题

- **真机 E2E 已通过**（2026-10-16 第 3 次触发，人类在场）：`new_document` → `select_rectangle_tool` →
  `select_foreground_color` → `select_layer` → `resolve_start` / `resolve_end` → `draw_rectangle` →
  `capture_*` 全部 Committed，最终 `TaskStatus::Completed`，耗时 11.11 s。前两次触发都在前台等待阶段
  30 s 超时（未发输入）。**T3.1 真机链路至此打通**（DRIFT-256-5/6 的动作层与坐标层问题均已闭环）。
- **⚠ 像素级验证仍未取证**：本次通过证明的是**流程**跑通，不是画布像素被验证。整窗 `PrintWindow` 截图里
  只在工具栏找到测试选的前景色 `#ed1c24`，**画布区域是空白** —— 画布是独立合成层，窗口截图抓不到
  （与 `docs/memory/apps/paint.md` §11.5 同族）。T3.1 的 `rectangle_visual_assert` 因此在本轮**没有真正的
  像素证据**；要给视觉后置条件取证，需要走能抓到合成层的通道（另行裁决）。
- **时序假设**：探测发生在 plan 之前，而 `Ctrl+N` 在 plan 内执行。本次实测「探测时文档」与「新建后文档」同为 418×74；若将来两者不一致，`resolve_*` 会显式失败（fail-closed），不会静默点偏。若要彻底消除该假设，需要让 plan 支持「先跑 new_document、再探测、再注入」的两阶段执行。
- viewport offset 固定 0（新建文档未滚动）；滚动场景（T3.2/T3.3）需单独建模。
- 真机十次运行（TASK-044 的 `DRIFT-044-1` / `PL-113`）仍未开始；本轮只跑通 1 次。

**同日事后更正（2026-10-16 晚）**：上面那条「像素级验证仍未取证 —— 画布是独立合成层、窗口截图抓不到」
**是错的**，已作废。复测（同一 HWND，位图按 **物理** 尺寸 `2138×1096` 分配）抓到**完整窗口**，画布、
层层面板与**红色矩形**全在（`#ed1c24` 像素 2276 个），状态栏显示 `40 × 40像素` + `100%` 缩放。
真因是 **DPI 感知缺失**：不感知 DPI 的进程 `GetWindowRect` 拿虚拟化尺寸（1069×548），而 `PrintWindow`
按物理尺寸渲染 → 产品 `capture` 在缩放显示器上**只返回窗口左上角 1/4 图**。详见
`docs/memory/apps/paint.md` §12.5 与 `LEDGER.md` 的更正行。**TASK-303 本卡的实现无需改动**
（`zoom_ratio` 公式在进程自身坐标系里自洽）；要修的是 DPI 感知（另卡）。

### 8. 新增长期记忆

- **`docs/memory/apps/paint.md` §12.5**：缩放显示器上的窗口截取只拿到左上角 1/4 —— DPI 感知缺失同时
  破坏坐标口径与截图完整性（纠正本卡早前对「合成层抓不到」的误判）。

### 9. 给审阅者的关注点

- `PaintCanvasInputs::FAKE` 必须与 `PaintFakePlatform` 的 fixture（100×100、zoom 1.0）保持一致，否则 fake 路径会漂移。
- 探测只读 `element_geometry` + `read_text`，不新增产品调用面；请确认这不违反 TASK-294 的「合成点击调用上限」口径（该上限只约束 pointer 热路径）。

---

**2026-10-16 续（TASK-304 落地后的真机复跑）**：PMv2 让 `resolve_*` 通过（zoom 自动变 1.0），
下一步在 `draw_rectangle` 暴露出**同一类缺陷的另一处实例** —— 注入的 `coordinate_space` 仍是硬编码
`scale: 1.0`，而真机是 **scale 2**，被 `pointer_action` 按 ADR-0067 的显式坐标空间校验**正确拒绝**：
`coordinate space: display \\.\DISPLAY1 has scale 2, not 1`。修法与本卡同源：`read_real_canvas_inputs`
改为从画布元素的 `element_geometry().space()` 取 `origin_display()` 与 `scale()` 一并注入
（fake 路径仍走 `PaintCanvasInputs::FAKE`，行为零变更）。

修完后真机链路**跑到了最后一步**：`draw_rectangle` 的前 3 条运行时断言（含 **`visual_assert` 像素断言**）
**全部 Satisfied** —— 像素证据首次真实成立（TASK-304 的 PMv2 修复之功）；只剩运行时附加的
`state_changed{within_ms:2000}` 失败，那是**另一张卡**的契约问题（TASK-305 / `DRIFT-305-1`），不在本卡范围。

### 2026-10-20 TASK-324 收口补充

- PR #426（merge `a271ea2d5db157b580a6fc2621e8d426edb3599f`）与 PR #429
  （merge `0a87abac88a8aa4b0873f80e264308cd5e85c7d1`）均已合并到 `main`，各 11/11 success / pending 0。
- T3.1 真机端到端与后续十次运行均已通过，本卡目标完成。
- TASK-324 仅依据既有真实证据把状态从 `Review` 更新为 `Done`，未改正文或产品代码。
