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

（待填）

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
