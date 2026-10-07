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
- 待合并后补跑：`fmt` / `clippy` / `cargo test --workspace` / xtask 全套门禁。

### 4. DoD 逐条核对

- [x] card 标题声明的任务包与评测契约可被静态测试覆盖。
- [x] 10 个评测用例、2 px 误差上限、视觉容差与静默失败 0 已进入评测契约。
- [x] 真实 UIA 探针证据已落 `probe-evidence.json` 与 Paint 应用档案。
- [ ] 真机连续 10 次成功率 >= 75%（未运行，DRIFT-044-1）。
- [ ] 真机拖拽坐标误差 <= 2 px（未运行，DRIFT-044-1）。
- [ ] `cargo fmt --all --check` / `clippy` / `test --workspace` / xtask 全套（提交前补跑）。

### 5. 偏差

`DRIFT-044-1`：本会话取得了真实 UIA 树和单次画布 bounds/zoom，但没有完成
真实拖拽、10 次成功率、混合 DPI 坐标误差与最终像素容差测量。TASK-044 的
评测契约因此是 `real_run_status = not_run`，不得把探针结果冒充验收结果。

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
- TASK-043 selector 文件需要后续集成卡按本卡探针证据更新；当前保持 provisional。

### 8. 新增长期记忆

- `docs/memory/apps/paint.md`：补真实 UIA 树的关键 AutomationId、canvas
  `aid=image`、`418 x 74 @ 0.5x` 观察值，以及矩形/调色板 fallback-only 边界。

### 9. 给审阅者的关注点

1. 矩形工具和调色板没有稳定 AutomationId，后续 selector 校正必须保留
   `locale_dependent=true` / `fallback_only=true`，不能把中文名当主选择器。
2. canvas 的 UIA bounds、zoom 与 scroll 偏移来自一次观察；混合 DPI 和多屏仍未证明。
3. `rectangle_visual_assert` 的 `max_changed_ratio=0.08` 与 `confidence_min=0.85`
   仍是契约值，不是实测结果；TASK-044/后续验收必须用真实截图校准。
