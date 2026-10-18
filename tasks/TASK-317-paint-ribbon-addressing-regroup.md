# TASK-317　Paint 工具栏寻址重构：按总分类拆工具 + 组内有界路径（ADR-0096 落地）

- 状态：**InProgress**
- 阶段：1　子阶段：**1b**　批次：Paint 真机链路　依赖：TASK-309
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `docs/adr/0096-paint-ribbon-addressing-by-group-with-bounded-path.md`
  - `docs/adr/README.md`（登记表追加一行 + 下一个可用编号）
  - `adapters/com.microsoft.paint/tools/tools.json`
  - `adapters/com.microsoft.paint/selectors/targets.json`
  - `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`
  - `apps/agent-core/src/paint_handlers.rs`、`paint_handler_actions.rs`、`paint_handler_selection.rs`
  - `apps/agent-core/tests/production_paint.rs`
  - `LEDGER.md`、`PLAN.md` 当前状态块、`plans/stage-1-pilots.md`、`README.md` 三处
  - 本卡
- **关联**：ADR-0096、ADR-0095、TASK-309、`docs/memory/apps/paint.md` §13.6 / §13.8 / §13.17 / §13.18

**目标**

把 Paint 的工具寻址从「一份扁平 enum」改为 ADR-0096 的**按总分类拆工具 + 组内有界路径**，解决同名歧义、漏项与三级寻址。

**现象**

`paint.tool.select` 的 enum 把工具组与形状组的子项混在一起，缺「文本」「放大镜」；§13.6 记录 5 个同名子项跨 3 个 flyout 出现，扁平名字无法唯一定位；`图像 → 翻转 → 水平翻转` 这类三级路径无从表达。handler 侧除 `rectangle` 外全部硬拒（`paint_handlers.rs:264-272`）。

**本卡修法**

按 ADR-0096 D1–D6：一个总分类一个工具 → 组内 `path`（1..=3，逐元素命中本组词汇表，非法即 `ToolInvalidArgs`）→ 词汇表按组声明 → 通道按组声明 → 读回不得用会粘住的 `ToggleState` → 迁移 tools.json / selector pack / handler。

**Out of scope（做了算漂移）**

- 新增 ErrorCode 或改 envelope / audit-event。
- 「选择」组与 Copilot 组进 schema。
- 真机端到端跑通（TASK-309 剩余段）。
- 视觉兜底通道（L5）改造。

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

- [ ] ADR-0096 落档 Accepted 并登记进 `docs/adr/README.md`
- [ ] schema 按总分类拆工具；`paint.tool.select` 取值收敛为「工具」组子项
- [ ] 组内 `path` 有界（1..=3）且逐元素校验；非法输入 fail-closed
- [ ] 同名子项跨组不再歧义（正反测试）
- [ ] 读回不使用会粘住的 `ToggleState`；不可靠时显式标注
- [ ] 适配包校验器与 tools.json / selector pack 对齐且 PASSED
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-317（Paint 工具栏寻址重构）。
- **write scope**：ADR-0096 登记、Paint tools / selectors / validator / handler / production 测试与状态文件。
- **铁律**：1（无静默失败）、3（策略唯一放行点）、9（不扩范围）、10（先 ADR 后 schema）。
- **禁止**：新增 ErrorCode / envelope / audit-event；选择组与 Copilot 进 schema；真机端到端；L5 视觉兜底。
- **验收**：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test -p assistant-agent-core`、`cargo test --workspace`、
  `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py`、
  `cargo run -p xtask -- adr-index`。
- **依赖**：ADR-0096 Accepted（PR #473，merge `fb690c7`）；TASK-309 提供 §13.8–§13.18 真机探针证据。
- **疑问**：无；图像组 / 选择组 / Copilot 的证据不足，按 ADR 只落已取证范围并保留 WIP。

### 2. 实际改动文件

- `adapters/com.microsoft.paint/tools/tools.json`：`paint.tool.select` 改为「工具」组局部
  `path`（长度 1）；新增 `paint.shape.select`（23 个形状名）与 `paint.brush.select`
  （9 个笔型名）。三者输出均明确 `ui_state_readback=false`，不把粘住的 `ToggleState`
  冒充读回。
- `adapters/com.microsoft.paint/selectors/targets.json`：新增已取证的工具按钮、23 个形状项、
  9 个画笔 `MenuItem` target；全部使用实测本地化 name + UIA role，并显式
  `allow_fallback_entry=true`。target 总数 21 → 55。
- `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`：新增 ADR-0096 钉桩 —— 每个
  已取证组内 target 必须保持 `observed`、`exact_name_and_role`、正确 name / role 与
  fallback-only；缺项或改错即失败。
- `adapters/com.microsoft.paint/rollback/recipes.json`：工具 / 形状 / 画笔三组各自声明 L2
  补偿 recipe；画笔 recipe 明确工具轴与画笔轴正交。
- `adapters/com.microsoft.paint/tasks/t3.1...json` / `t3.2...json`：矩形选择从
  `paint.tool.select(tool=rectangle)` 迁到 `paint.shape.select(path=["矩形"])`。
- `apps/agent-core/src/paint_handler_support.rs`：新增组内词汇表、target 映射与
  `required_selection_path`；非法枚举 / 空路径 / 超深路径一律 transform 为既有
  `ToolInvalidArgs`。
- `apps/agent-core/src/paint_handlers.rs`：拆出工具 / 形状 / 画笔三条选择路径；形状仍走
  `click_element_center`，画笔先 `expand` 再选 `MenuItem`，工具组走实测的 `toggle`。
- `apps/agent-core/src/paint_handler_actions.rs`、`paint_handler_map.rs`、
  `paint_handler_selection.rs`、`paint_registry.rs`、`task_package_render.rs`、
  `production_paint.rs`：接线、本地选择状态、路径 postcondition 和测试断言同步。
- `adapters/com.microsoft.paint/app_map.json`、`memory/app_map.v1.json`：命令与记忆路由
  改为工具 / 形状 / 画笔三组语义。
- 本卡、`LEDGER.md`。

### 3. 验收输出摘要

- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` →
  `PASSED: app_id=com.microsoft.paint targets=55 tools=12 recipes=8 probe_status=required`。
- `python -c "import json,pathlib; ..."` 遍历适配包全部 JSON → `json-ok`。
- `cargo test -p assistant-agent-core` → 63 lib tests + 6 bin tests + integration suites
  全绿；Paint production `6 passed / 2 ignored`（两条真机用例仍按原标记忽略）。
- 新增 2 个 handler 单测：组内词汇表 / 三级上限通过路径，以及跨组词、超深、空路径拒绝。
- 完整 `cargo fmt --check` / clippy / workspace / xtask 输出见主 PR；merge hash 待回填。

### 4. DoD 逐条核对

- [x] ADR-0096 落档 Accepted 并登记进 `docs/adr/README.md`（PR #473）。
- [ ] schema 按总分类拆工具：已落工具 / 形状 / 画笔三组；图像组仍缺实测 selector，保持 WIP。
- [x] 三组 `path` 均有界（1..=1）且逐元素命中本组词汇表；非法输入 fail-closed。
- [x] 同名交叉组不再进入同一枚举：形状 / 画笔 / 工具各自持有独立词汇表。
- [x] 读回不使用粘住的 `ToggleState`；输出显式 `ui_state_readback=false`。
- [ ] 适配包校验器与 tools / selector 已对齐并通过；图像组工具尚未声明。
- [ ] 静态验收全绿（至少 `assistant-agent-core` 已绿）；workspace / xtask 待本轮最终提交前复跑。
- [x] 未修改 Out of scope 文件；未新增 ErrorCode、envelope、audit-event 或依赖。

### 5. 偏差

- 无越权偏差。实现刻意拆成三组已取证范围：`paint.image.transform` 在 ADR 中是新工具，
  但当前只有 `图像 → 翻转 → 水平翻转 / 垂直翻转` 的层级描述，没有 ControlView 的 name /
  role / pattern 原始证据；没有把推断写进 selector pack。

### 6. 更合理做法

- 形状 / 画笔子项可以在未来由探针输出直接生成 target 表，但目前仓库没有稳定生成器；
  手写 55 个 target 的重复度较高。建议下一张治理卡在确保证据来源可追溯的前提下引入
  adapter-pack 代码生成或表驱动校验，减少本轮这类手工 JSON 漂移。

### 7. 遗留问题

- **TASK-317 保持 `InProgress`**：图像组工具、真实 GUI 的按钮通道验证仍未完成。
- `paint.tool.select` 的 `toggle`、`paint.brush.select` 的 `expand + invoke` 目前由 ADR-0096
  §13.8–§13.10 的只读探针 pattern 支撑，尚无真机端到端结果；不得冒充已验证。
- 选择组 / Copilot 按卡面 Out of scope 未进 schema。
- 本 PR 的 merge hash、CI run 与 round 记录走后续 docs-only 回填 PR。

### 8. 新增长期记忆

- 无新增 `FACT` / `PITFALL` / `REJECTED`；ADR-0096 本身已记录 Paint 工具栏树模型与
  `ToggleState` 粘滞事实。若真实 GUI 验证推翻某组通道，再按规则追加。

### 9. 给审阅者的关注点

- 重点核对 `selectors/targets.json` 的 23 个形状名 / role 是否与
  `docs/memory/apps/paint.md` §13.9 逐项一致，9 个画笔名 / role 是否与 §13.10 一致。
- 重点核对 `required_selection_path` 的 fail-closed 边界：空数组、跨组词、超深数组。
- `paint.tool.select` 只覆盖工具组；`paint.canvas.draw_rectangle` 的既有 `expected_tool`
  契约未改，矩形选择由 `paint.shape.select` 负责。
