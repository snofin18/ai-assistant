# TASK-319　Paint 形状笔宽（大小）按组拆工具（ADR-0098 落地）

- 状态：**InProgress**
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

待本段实现完成后回填。

### 3. 验收输出摘要

待回填。

### 4. DoD 逐条核对

待回填。

### 5. 偏差

待回填。

### 6. 更合理做法

待回填。

### 7. 遗留问题

待回填。

### 8. 新增长期记忆

待回填。

### 9. 给审阅者的关注点

待回填。
