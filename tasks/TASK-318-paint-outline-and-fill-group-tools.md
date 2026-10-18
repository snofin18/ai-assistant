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

（待填）

### 3. 验收输出摘要

（待填）

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
