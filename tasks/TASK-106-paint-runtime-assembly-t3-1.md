# TASK-106　Paint 运行时装配：T3.1 垂直切片（生产 handler + Plan 来源参数化）

- 状态：**InProgress**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：043、044、**ADR-0084 Accepted**　预估：L　难度：L
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：043、044、**ADR-0084 Accepted**　**预估**：L　**难度**：L
- **write scope**：`apps/agent-core/src/**`、`apps/agent-core/tests/**`、本卡与状态同步文件
- **关联**：ADR-0084、ADR-0058、ADR-0043、ADR-0067、ADR-0076、ADR-0077、ADR-0079、
  `adapters/com.microsoft.paint/**`、`eval/tasks/paint/**`、`docs/spec/runtime-execution.md`

**目标**

把 Paint 的 T3.1 工具子集接进生产装配根，使「新建画布 → 选矩形工具与颜色 → 解析画布坐标 →
画矩形 → 捕获像素」成为**可运行的 `Plan`**（fake 平台可执行到 `Completed`），
为 `DRIFT-044-1` / `PL-113` 的真机十次验收提供运行对象。

**write scope**（本卡独有部分）

`apps/agent-core/src/**`、`apps/agent-core/tests/**`；按卡面步骤同步记录与必要记忆 / 台账。

**In scope**

1. `production.rs` 由 Notepad 专用改为**适配器参数化**（ADR-0084 D1）：target 目录、handler 集、
   任务包路径由装配输入给出；Notepad 既有行为不变。
2. 新增 Paint target 目录加载（输入 `adapters/com.microsoft.paint/selectors/targets.json`）。
3. 新增 Paint handler 模块，实现 T3.1 的 **7 个工具**：`paint.document.new` / `paint.tool.select` /
   `paint.color.select_foreground` / `paint.layer.select` / `paint.canvas.resolve_point` /
   `paint.canvas.draw_rectangle` / `paint.canvas.capture_pixels`；注册集与声明子集**精确一致**。
   （2026-10-08 人类裁决：`paint.layer.select` 由 Out of scope 移入 In scope —— T3.1 任务包
   `select_layer` 步骤依赖它；ADR-0084 D4 的"6 个"按勘误处理，见登记表。）
4. fake 平台上的正向测试（T3.1 任务包 → `Plan` → `Completed`）与 fail-closed 负向测试。

**Out of scope**

- Paint 其余 3 个工具（`paint.canvas.rollback_last_write` / `paint.file.open` /
  `paint.file.save_as`）—— 另立后续卡。
- 真机十次运行与坐标误差测量（`DRIFT-044-1` / `PL-113`）—— 独立验收卡。
- 改 `crates/**` 公共接口 / schema / ErrorCode；新增 crate 或第三方依赖；操作真实 GUI。

**步骤**

1. 读 ADR-0084 + ADR-0058 + 现有 `production.rs` / `notepad_handlers.rs` / `notepad_targets.rs` 的接线形状。
2. 先做参数化重构，跑既有 `production_root*` 回归，确认 Notepad 行为不变。
3. 加 Paint target 目录 + handler 模块，按 tools.json 形状注册 6 个工具。
4. fake 平台正向 / 负向测试。
5. 跑全部验收命令；不合格 → DRIFT。
6. 同步 `docs/memory/apps/paint.md` / `facts` / `pitfalls`（如有新事实）。

**DoD**

- [ ] T3.1 任务包经装配可渲染为 `Plan` 并在 fake 平台执行到 `Completed`
- [ ] Paint 注册集缺任一 T3.1 工具 / target 缺必需目标 / 未注入 blob sink → 显式失败带 `ErrorCode`
- [ ] 既有 Notepad `production_root*` 测试全绿（参数化零回归）
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md` 或 `apps/paint.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core production_root::
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-106 Paint 运行时装配：T3.1 垂直切片
【目标】把 Paint T3.1 的 6 个工具接进生产装配根，使任务包在 fake 平台可执行到 Completed
【write scope】apps/agent-core/src/**、apps/agent-core/tests/**；本卡与状态同步文件
【铁律】无静默失败；策略引擎唯一放行点；契约先行；不静默扩大范围；热点文件先 guard
【禁止】改 crates/** 公共接口 / schema / ErrorCode；新增 crate / 依赖；操作真实 GUI；接 T3.1 之外的 Paint 工具
【验收】fmt / clippy / cargo test --workspace / production_root* 回归 / xtask 门禁
【依赖】ADR-0084 Accepted（已合并 `471ddce`）
【疑问】无
```

### 2. 实际改动文件

第一步（2026-10-08，参数化 target 目录加载）：

- `apps/agent-core/src/notepad_targets.rs`：`REQUIRED_TARGETS` 改为参数化的
  `load(path, required_targets)`，新增 `NOTEPAD_REQUIRED_TARGETS` 常量，并加「第二适配器
  按自身必需集加载 Paint 目录」的用例。
- `apps/agent-core/src/production.rs`：调用点显式传入 `NOTEPAD_REQUIRED_TARGETS`（Notepad 行为不变）。

第二步（2026-10-08，registry 注册循环参数化）：

- `apps/agent-core/src/notepad_registry.rs`：抽出共享的
  `register_declared_tools(declared, expected_tool_names, handlers)`；`load_and_validate_declarations`
  与 `missing_production_tools` 改为接受 expected 工具名参数；`build_notepad_registry` 显式传
  `EXPECTED_TOOL_NAMES`（Notepad 行为不变）。这样第二个适配器复用同一条注册循环，不复制实现。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --lib notepad_targets::` → **3 passed / 0 failed**
  （含新增 `test_second_adapter_loads_with_its_own_required_targets`）。
- `cargo clippy -p assistant-agent-core --all-targets -- -D warnings` → EXIT 0。
- Notepad 回归：`--test production_root` **7 passed**、`production_root_t1_2` **6 passed**、
  `production_root_t1_3` **1 passed**、`production_root_uia` **1 passed / 8 ignored** → 全绿。
- 第二步复跑：`cargo clippy -p assistant-agent-core --all-targets -- -D warnings` EXIT 0；
  `cargo test -p assistant-agent-core --lib notepad_registry::` **7 passed**；Notepad 回归
  `production_root` 7 / `production_root_t1_2` 6 / `production_root_t1_3` 1 → 全绿。

### 4. DoD 逐条核对

### 5. 偏差

**DRIFT-106-1**（漂移触发器 ⑤ + ⑧）

- **现象**：本卡正文与 ADR-0084 D4 把 T3.1 工具子集写成 **6 个**，并把
  `paint.layer.select` 明确列入 Out of scope（"Paint 其余 4 个工具"）。但
  `adapters/com.microsoft.paint/tasks/t3.1.new-canvas-rectangle-color-screenshot.json`
  的第 5 步 `select_layer` 调用的正是 **`paint.layer.select`**，且后续
  `paint.canvas.draw_rectangle` 的 `expected_layer_id` 依赖它的输出。
- **影响**：按卡面只实现 6 个工具，T3.1 的 `Plan` 会引用未注册工具 → 执行不到
  `Completed`，卡的 DoD 不可达；实现 7 个则违反卡面 Out of scope（触发器 ⑤）。
- **建议**：把 `paint.layer.select` 从 Out of scope 移入 In scope（T3.1 子集 = **7 个**工具）；
  ADR-0084 D4 的"6 个"按勘误处理（ADR 只增不改，在本卡记录 + 后续 ADR/登记表标注）。
- **已停工作**：未开始 Paint handler 实现与 registry 的工具名绑定；已提交的
  `a7e2b25`（target 目录参数化）与工具数量无关，不受影响。
- **闭环（2026-10-08 人类裁决）**：按建议第 1 条 —— `paint.layer.select` 移入 In scope，
  T3.1 子集 = 7 个工具；卡面 In/Out scope 已按裁决更正，ADR-0084 D4 的"6 个"在登记表标注勘误。

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
