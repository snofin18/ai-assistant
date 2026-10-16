# TASK-308　Paint 任务包选择：默认续画，显式才新建

- 状态：**Review**
- 阶段：1　子阶段：**1b**　批次：Paint 真机链路　依赖：TASK-307
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `apps/agent-core/src/main.rs`
  - `adapters/com.microsoft.paint/tasks/README.md`
  - `docs/memory/apps/paint.md`
  - 本卡 / `LEDGER.md` / 必要的进度同步文件
- **关联**：TASK-307、T3.1 / T3.2 任务包、ADR-0039（生产入口参数化）

**目标**

让 Paint 生产入口默认选择 **T3.2 续画**，只有调用方显式要求新建时
（`--document-mode new`）才选择 T3.1 新建画布。人工仍可用 `--task-package`
显式指定任意任务包，保留测试与特殊场景的逃生口。

**现象**

当前 `assistant-agent-core --production` 强制要求 `--task-package`，调用方如果仍传
T3.1，就会在已有画布上再次 `Ctrl+N`，违反“不应每次都新建画布”的产品语义。

**本卡修法**

1. `--adapter-kind paint` 时允许省略 `--task-package`：
   - `--document-mode existing`（默认）→ `t3.2.continue-existing-canvas-rectangle.json`
   - `--document-mode new` → `t3.1.new-canvas-rectangle-color-screenshot.json`
2. 显式 `--task-package <path>` 仍优先于 `--document-mode`。
3. 非 Paint 适配器没有 `--task-package` 时继续显式失败，不改变 Notepad 行为。
4. 不新增公共 trait / Tool schema / ErrorCode；只改二进制入口参数解析与装配前的路径选择。

**Out of scope（做了算漂移）**

- Planner 真实解析人类自然语言意图并自动决定 new / existing（后续卡）。
- dock / IPC 窗口摆放。
- 完整画笔 / 形状 / 颜色工具集扩展。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-agent-core
cargo test --workspace
```

**DoD**

- [ ] Paint 省略 `--task-package` 时默认选择 T3.2
- [ ] `--document-mode new` 选择 T3.1
- [ ] 显式 `--task-package` 仍覆盖文档模式
- [ ] Notepad 缺少 `--task-package` 仍显式失败
- [ ] 参数解析单测覆盖 existing / new / override / non-Paint 四类
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-308（Paint 文档模式选择）。
- **write scope**：上列入口 / 适配包 README / Paint 档案 + 本卡与台账。
- **铁律**：1（无静默失败）、9（不得静默扩大范围）。
- **禁止**：改公共 trait / schema / ErrorCode；实现画笔形状全集；自动解析自然语言意图。
- **疑问**：无。默认取 `existing`，显式 `--task-package` 是逃生口。

### 2. 实际改动文件

- `apps/agent-core/src/main.rs`：新增 `--document-mode existing|new`；Paint 省略
  `--task-package` 时按模式选择 T3.2 / T3.1；显式 `--task-package` 仍覆盖；
  Notepad 仍要求显式任务包。新增 4 个参数解析单测。
- `adapters/com.microsoft.paint/tasks/README.md`：说明生产入口默认 existing。
- `docs/memory/apps/paint.md`：§12.7 补默认续画选择。
- 本卡 / `LEDGER.md`。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core` → lib 61 passed；bin 6 passed（含 4 个新参数测试）；
  production_paint 6 passed / 2 ignored；其余 agent-core 测试全绿。
- `cargo test --workspace` → 469 passed / 0 failed。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0（仅既存 unknown-lint 警告）。
- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` → PASSED。
- `xtask hygiene / check-ledger / card-check / docscan / memory-counts` → PASSED。

### 4. DoD 逐条核对

- [x] Paint 省略 `--task-package` 默认选择 T3.2。
- [x] `--document-mode new` 选择 T3.1。
- [x] 显式 `--task-package` 覆盖文档模式。
- [x] Notepad 缺少 `--task-package` 仍显式失败。
- [x] 参数解析单测覆盖 existing / new / override / non-Paint。
- [x] 静态验收全绿；记录与台账已更新。
- [x] 无 Out of scope 文件改动。

### 5. 偏差

- none。

### 6. 更合理做法

- 只改二进制入口的参数解析，不动 `ProductionConfig` / Tool schema / 公共 trait；
  `--task-package` 保留为显式逃生口。

### 7. 遗留问题

- 真正的自然语言意图解析（“继续画” vs “新建画布”）仍归 Planner；本卡只提供显式入口。
- 完整画笔 / 形状 / 颜色工具集仍未实现。

### 8. 新增长期记忆

- `docs/memory/apps/paint.md` §12.7：生产入口默认 `existing` → T3.2。

### 9. 给审阅者的关注点

- 默认值是 `existing`，这改变了之前必须显式传任务包的行为；显式 `--task-package`
  仍保持旧行为。
- `parse_production_options` 的圈复杂度与 main.rs 行数继续增长，已进入 hygiene warning；
  后续若再加参数应拆解析模块。
