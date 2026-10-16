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
- 本卡。

### 3. 验收输出摘要

- `powershell -NoProfile -File eval/tasks/paint/probe-toolset.ps1`（只读，未点击）→
  `seen=91 emitted=91 truncated=False elapsed_ms=177`；窗口
  `name="无标题 - 画图" class=MSPaintApp proc=mspaint`。原始表见 `docs/memory/apps/paint.md` §13.8。
- 探针**一次就修正了两条既有记录**：§11.1 的 `ToggleButton` / `SplitButton` 是 XAML 类名，
  UIA ControlType 实测为 `Button` / `RadioButton`；§13.7 的调色板色名与真机 UIA name 不符，已改。
- 待补：handler / selector 验收、真机端到端、归档拆分。

### 4. DoD 逐条核对

（待填）

### 5. 偏差

（待填）

### 6. 更合理做法

（待填）

### 7. 遗留问题

（待填）

### 8. 新增长期记忆

（待填）

### 9. 给审阅者的关注点

（待填）
