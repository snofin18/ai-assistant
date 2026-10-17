# TASK-309　Paint 完整工具集真机落地（画笔 / 形状 / 轮廓 / 填充 / 大小 / 颜色）

- 状态：**InProgress**
- 阶段：1　子阶段：**1b**　批次：Paint 真机链路　依赖：TASK-308
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `apps/agent-core/src/paint_handlers.rs`
  - `apps/agent-core/src/paint_handler_actions.rs`
  - `apps/agent-core/src/paint_handler_selection.rs`
  - `apps/agent-core/src/task_package_render.rs`
  - `apps/agent-core/tests/production_paint.rs`
  - `adapters/com.microsoft.paint/**`
  - `eval/tasks/paint/**`（探针脚本）
  - `docs/memory/apps/paint.md`
  - `docs/memory/archive/paint-early-probes.md`（ADR-0021 归档拆分）
  - `MEMORY.md`、`LEDGER.md`、`PLAN.md` 当前状态块、`plans/stage-1-pilots.md`、`README.md` 三处
  - 本卡
- **关联**：TASK-307 / TASK-308、`docs/memory/apps/paint.md` §13（人类提供的工具集清单）、
  ADR-0021（记忆归档）、ADR-0087（ContentViewWalker）、ADR-0091（`ExactNameAndRole`）

**目标**

把 Paint 工具选择从「`paint.tool.select` 实际只支持 `rectangle`」推进到 §13 清单里的完整集合：
画笔 9 型、形状 23 个、轮廓 7 项、填充 7 项、大小 4 档，以及颜色选择。**先取证，后实现。**

**现象**

`paint.tool.select` 的 schema enum 已声明 `pencil` / `brush` / `eraser` / `fill` / `rectangle` /
`ellipse` / `line` / `color_picker`，但 handler 只真正实现 `rectangle`，其余显式拒绝。
selector pack 里 pencil / brush / fill / eraser 的候选仍是 provisional，`brush_tool_button`
的 class / name 与真机形状不完全匹配。

**本卡修法**

1. 写有界 UIA 探针（`eval/tasks/paint/probe-toolset.ps1`），对每个 flyout 取证
   name / AutomationId / ControlType / `IsKeyboardFocusable` / 可用 pattern。
2. 按取证结果校准 selector pack，并扩展 handler 覆盖 §13 的工具与形状。
3. 真机端到端：至少覆盖「选画笔 → 选形状 → 设轮廓 / 填充 / 大小 → 选色 → 落笔」的可验证路径。
4. 按 ADR-0021 把 `docs/memory/apps/paint.md` §9–§11 整体移入
   `docs/memory/archive/paint-early-probes.md`，原处留指针，登记 MEMORY.md 归档索引 + LEDGER。

**Out of scope（做了算漂移）**

- Planner / 模型自动判断「续画 vs 新建画布」（另立 TASK-310，需先落 ADR）。
- 新增 Tool schema / 公共 trait / ErrorCode；本卡只填已声明但未实现的 enum 变体。
- dock 摆放、IPC 窗口管理。
- 撤销 / 重做通道扩展。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-agent-core
cargo test --workspace
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
```

**DoD**

- [ ] 探针脚本对 §13 六个 flyout 逐个取证，输出粘进记录区（含原始 name / aid / ControlType）
- [ ] selector pack 里被取证覆盖的候选从 provisional 升为实测
- [ ] `paint.tool.select` 对已取证的工具不再显式拒绝；未取证的工具仍 fail-closed
- [ ] 至少一条真机端到端路径可复跑并给出原始输出
- [ ] `docs/memory/apps/paint.md` 归档拆分完成且回到 ADR-0021 阈值内
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-309（Paint 完整工具集真机落地）。
- **write scope**：上列 paint handler / 任务包渲染 / 适配包 / 探针脚本 / Paint 档案 + 归档件 + 台账。
- **铁律**：1（无静默失败）、5（API 优先：L1 > L2 > L3 > L4 > L5）、9（不得静默扩大范围）、10（契约先行）。
- **禁止**：新增 Tool schema / 公共 trait / ErrorCode；Planner 自动判断文档模式（TASK-310）；dock / IPC。
- **验收**：见正文四条命令 + 适配包 validate。
- **依赖**：TASK-308 **Done**（`LEDGER.md` 已核，merge `9c8fd09`）；§13 工具集清单已落档。
- **疑问**：形状 23 项、画笔 9 项、轮廓 / 填充各 7 项只在对应 flyout **展开时**才进 UIA 树，
  本轮只读探测拿不到。默认处理：先试 §11.1 已实测的键盘路径（`Alt+B`、`Alt+T,P` 等）展开 flyout，
  拿不到再考虑有界合成点击；**未取证的枚举变体保持 fail-closed，不放行猜测值**。

### 2. 实际改动文件

- `eval/tasks/paint/probe-toolset.ps1`：新增只读、有界（元素数 / 深度 / 墙钟三重封顶）的
  ControlView 探针；纯 ASCII（refscan 要求）。
- `docs/memory/apps/paint.md`：新增 §13 工具集清单（13.1–13.7，人类截图）与
  §13.8 真机探针实测表。
- `MEMORY.md`：`apps/paint.md` 规模表行同步。
- `adapters/com.microsoft.paint/selectors/targets.json`：按 §13 实测把 `pencil_tool_button` /
  `eraser_tool_button` / `brush_tool_button` 由 `required` 升 `observed`，主候选改为实测
  `automation_id`（`PencilTool` / `EraserTool` / `BrushesSplitButton`，score 0.94），
  另附本地化 `exact_name_and_role` 作为 `fallback_only`。`observed` 目标数 8 → 11。
- `docs/memory/archive/paint-early-probes.md`：新增归档件，逐字接收 `paint.md` §9–§12 与
  §13.1–§13.7（429 行），并在头部标明来源与现行档案入口。
- `docs/memory/apps/paint.md`：删除已迁出的早期探针 / 运行时装配 / KeyTip / 静态工具集清单，
  原处保留 §9–§12 与 §13.1–§13.7 的指针；主档案 709 → 299 行，继续保留 §13.8–§13.16 的真机探针表。
- `MEMORY.md`：规模表把 `apps/paint.md` 更新为 299 行，§7 归档索引登记首条 L3 归档；读法改为
  “现行探针表 + 归档入口”，不再手抄已迁出的旧探针摘要。
- 本卡。
- **第四段（2026-10-18 round-3 automation）**：
  - `adapters/com.microsoft.paint/selectors/targets.json`：按 §13.14 / §13.16 的实测 aid
    新增四个 `observed` 目标 `size_1px_option` / `size_3px_option` / `size_5px_option` /
    `size_8px_option`（`automation_id` = `Size1` / `Size3` / `Size5` / `Size8`，score 0.94，
    `locale_dependent=false`，无 name 兜底）。适配包目标数 17 → 21、`observed` 11 → 15。
  - `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`：新增 `measured_size_options`
    钉桩（目标必须 `observed`、候选必须是 `automation_id`、取值必须等于实测 aid、不得本地化 /
    兜底）。负向验证：把 `Size8` 改成 `Size9` 后校验器 `exit 1` 并报
    `must use AutomationId 'Size8'`。
  - `adapters/com.microsoft.paint/README.md`：`Current Status` 不再声称「每个 selector 目标都是
    `required`」；改为整包仍 provisional / 逐个目标升 `observed`（15 / 21）、并写明 `observed`
    只代表 UIA 形状实测而非成功率；`Known Gaps` 首条改写为「覆盖率不完整」的真实缺口清单。
  - `docs/memory/apps/paint.md`：按行等量改写 5 处与实测冲突的陈述（§3 的
    「所有 selector 目标 required」、§6 第 8 条「探针挂起、selector 仍 provisional」、§8
    「形状和调色板仍无稳定 AutomationId」、§13.15 标题与其结论段）—— 行数仍为 **299**，
    未新增/删除行。

### 3. 验收输出摘要

- `powershell -NoProfile -File eval/tasks/paint/probe-toolset.ps1`（只读，未点击）→
  `seen=91 emitted=91 truncated=False elapsed_ms=177`；窗口
  `name="无标题 - 画图" class=MSPaintApp proc=mspaint`。原始表见 `docs/memory/apps/paint.md` §13.8。
- 探针**一次就修正了两条既有记录**：§11.1 的 `ToggleButton` / `SplitButton` 是 XAML 类名，
  UIA ControlType 实测为 `Button` / `RadioButton`；§13.7 的调色板色名与真机 UIA name 不符，已改。
- 归档拆分验收：`docs/memory/apps/paint.md` = **299 行**（ADR-0021 阈值 400），
  `docs/memory/archive/paint-early-probes.md` = **429 行**；迁移内容由原文切片生成，未改写条目。
  机器计数见 §3 的命令输出。
- 待补：`paint.tool.select` handler 扩展、真机端到端；本轮未碰真实 GUI。
- 静态验收：`cargo fmt --all --check` exit 0；`cargo clippy --all-targets -- -D warnings`
  exit 0；`cargo test -p assistant-agent-core` 176 passed / 0 failed / 11 ignored；
  `cargo test --workspace` exit 0；`cargo test -p assistant-core arch::` 10 passed；
  `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` PASSED；
  `verify-schemas` / `codegen --check` / `hygiene` / `memory-counts` / `adr-index` /
  `check-comments` / `refscan` / `docscan` / `card-check` / `check-ledger` /
  `check-migrations` 全 PASSED；`cargo deny check` advisories / bans / licenses / sources 全 ok。
- **第四段（2026-10-18 round-3 automation）**：
  - `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` →
    `PASSED: app_id=com.microsoft.paint targets=21 tools=10 recipes=6 probe_status=required`（exit 0）。
  - 适配包 JSON 全量解析（`json.loads` 遍历 `*.json`）exit 0。
  - 钉桩负向用例：临时副本把 `Size8` 改成 `Size9` → 校验器 `negative_exit=1`，报
    `selector target size_8px_option: size-8px-option-automation-id must use AutomationId 'Size8'`。
  - `docs/memory/apps/paint.md` = **299 行**（与 `MEMORY.md` 规模表一致，`memory-counts` 无告警）。
  - 本轮未碰真实 GUI；handler 扩展与真机端到端仍未做（见 §7）。

### 4. DoD 逐条核对

- [x] 探针脚本对 §13 六个 flyout 逐个取证，输出已粘进 `docs/memory/apps/paint.md` §13.8–§13.16
      （含原始 name / aid / ControlType / patterns）。**但用的是 UIA pattern 展开，不是合成点击** ——
      实测证明画笔 / 形状 / 轮廓 / 填充 / 大小全部支持 `ExpandCollapse` 或 `Toggle`，L4 通道不必要。
- [x] selector pack 候选升为实测（**部分完成**）：三个工具按钮已按实测 aid 升 `observed`。
  `fill_tool_button` / `toolbar` / `zoom_status_text` / `canvas_host` / `choose_color_dialog` /
  `save_as_dialog` 仍是 `required` —— 前两者只有本地化 name，而适配包校验器规定
  `observed` 目标的本地化候选必须 `fallback_only`，不能当主选择器；后四者本轮未取证。
- [ ] `paint.tool.select` 放行已取证工具：**未做**
- [ ] 真机端到端路径：**未做**
- [x] 归档拆分：**完成**（`apps/paint.md` 709 → 299 行；新增 `docs/memory/archive/paint-early-probes.md`
  429 行；`MEMORY.md` §7 归档索引已登记；`memory-counts` 不再报该文件阈值告警）
- [x] 静态验收：见 §3；`LEDGER.md` 追加行见本次提交
- [x] 无 Out of scope 文件被修改（本次只动探针脚本 + 记忆 + 本卡）
- **第四段（2026-10-18 round-3 automation）**：DoD 第 2 条（selector pack 候选升为实测）继续推进
  —— 尺寸四档由「未落档」变为 `observed` 且被校验器钉住；第 3 / 4 条（handler 放行与真机端到端）
  仍**未完成**，本卡继续保持 `InProgress`，PR 仍按 WIP 提交。

### 5. 偏差

- **归档范围按最优解自决扩大**：卡面第 4 条写“把 §9–§11 整体移入”，但 §9–§11 仅 145 行；
  只移这些后 `apps/paint.md` 仍约 564 行，不满足同一 DoD 的“回到 ADR-0021 阈值内”。
  因此本轮按同一主题继续迁出 §12（形状项 pattern / DPI / 弹层 / 图层面板实测）与
  §13.1–§13.7（人类提供的静态工具集清单），保留 §13.8–§13.16 的现行真机探针表。
  迁移过程只做逐字节切片移动，未改写归档条目；原处留指针，`MEMORY.md` §7 登记。

### 6. 更合理做法

- 归档拆分应一次性按“主档案保留现行可操作事实、历史/静态清单进 archive”的口径做，而不是只按
  卡面写死的三个章节号；否则会出现“迁移完成但文件仍超阈值”的假闭环。
- `docs/memory/archive/README.md` 仍写“当前为空”，该文件不在本卡 write scope；本轮在 §7 留待
  下一张治理卡或 Orchestrator 归档维护时同步。

### 7. 遗留问题

- **本卡尚未 Done**，本次只落地「探针取证 + 记忆落档」这一段，PR 按 WIP 提交，卡保持 `InProgress`。
- 剩余三段：① selector pack 按 §13 实测结论校准；② `paint.tool.select` 扩到画笔 / 形状 / 轮廓 /
  填充 / 大小 / 颜色；③ 至少一条真机端到端路径并贴原始输出。
- ① 已部分完成（三个工具按钮）。有意留在 `required` 的三类要单列：
  - **只有本地化 name 的控件**（`fill_tool_button`、颜色选取器）——校验器不允许它们当 `observed` 主选择器，
    要么找到非本地化锚点，要么维持 `required` 并接受"每次执行前需人确认"；
  - **`形状轮廓` / `形状填充` / `大小` 三个 flyout 按钮**：实测 aid 全空、只有 name，
    且**必须先选中一个形状才可用**（§13.13），尚未加进 selector pack；
  - **尺寸四档**：`Size1` / `Size3` / `Size5` / `Size8` 有稳定 aid，是一等一的可加项，
    但一个 target 只能对一个元素，需要拆成 4 个 target，本轮未做。
- **ADR-0021 归档拆分仍未做**：`docs/memory/apps/paint.md` 已 709 行（阈值 400），
  `memory-counts` 稳定报 `memory/archive-threshold` WARN（0 error，verdict PASSED）。
  计划：把 §9–§11 整体移入 `docs/memory/archive/paint-early-probes.md`，原处留指针，
  再登记 MEMORY.md 归档索引 + LEDGER。
- **归档拆分已在本轮完成**：`apps/paint.md` = 299 行，`archive/paint-early-probes.md` = 429 行；
  `MEMORY.md` 规模表和 §7 归档索引已同步。剩余告警只剩 `archive/README.md` 的“当前为空”旧句，
  该文件不在本卡 write scope，未改。
- **两条被推翻的旧结论**已写进 §13.9 / §13.16：§11.2/§12 的「形状项没有可用 UIA pattern、
  只能走 L4 合成点击」不成立（内层 `Button` 有 `Toggle`）；§13.14 的「`1 像素` 没有 MenuItem」
  也不成立（aid 是 `Size1`，只是 name 被写成 `大小`）。
- **第四段（2026-10-18 round-3 automation）后仍开放**：
  - `paint.tool.select` 仍只放行 `rectangle`；`pencil` / `brush` / `eraser` 的 selector 已
    `observed`，但**交互通道（Invoke / Toggle / 合成点击）没有真机证据**，按「未取证不放行」
    继续 fail-closed；`fill` / `color_picker` 连非本地化锚点都没有；`ellipse` / `line` 属无 aid
    的形状项。
  - 尺寸四档虽已进适配包，但**没有任何已声明的 Tool 暴露尺寸选择**（`paint.tool.select` 的
    enum 不含尺寸），且 `大小` flyout 按钮本身无 aid（只有本地化 name + role，见 §13.16），
    所以本轮只落**声明式证据**，不声称运行期可用。
  - `docs/memory/archive/README.md` 仍写「当前为空」，与已存在的
    `docs/memory/archive/paint-early-probes.md` 冲突；该文件不在本卡 write scope，未改。

### 8. 新增长期记忆

- 无新增 `FACT` / `PITFALL` / `REJECTED`；本次只是把既有 Paint 条目从主档案迁移到 L3 归档，
  内容未改写，归档事实写入 `MEMORY.md` §7 与 `LEDGER.md`。

### 9. 给审阅者的关注点

- 重点核对 `paint.md` 1–8 固定小节与 §13.8–§13.16 是否完整保留，以及归档件是否只包含
  §9–§12 / §13.1–§13.7 的原文切片，没有在迁移中丢条目或悄悄改写。
- `docs/memory/archive/README.md` 的“当前为空”已知陈旧，属跨 scope 遗留；本卡未越界修改。
- 本卡仍未完成 handler 扩展与真机端到端，PR 是 WIP，不应按 Done 合并。
