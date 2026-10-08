# TASK-106　Paint 运行时装配：T3.1 垂直切片（生产 handler + Plan 来源参数化）

- 状态：**Done**
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
【目标】把 Paint T3.1 的 7 个工具接进生产装配根，使任务包在 fake 平台可执行到 Completed
【write scope】apps/agent-core/src/**、apps/agent-core/tests/**；本卡与状态同步文件
【铁律】无静默失败；策略引擎唯一放行点；契约先行；不静默扩大范围；热点文件先 guard
【禁止】新增 crate / 依赖；操作真实 GUI；接 T3.1 之外的 Paint 工具
【验收】fmt / clippy / cargo test --workspace / production_root* 回归 / xtask 门禁
【依赖】ADR-0084 Accepted（已合并 `471ddce`）；后续人类选择 ADR-0085 的 `element_bounds` 方案
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

第三步（2026-10-08，人类选择后者后的实现）：

- `docs/adr/0085-element-bounds-read-path.md`、`docs/adr/README.md`、
  `docs/memory/decisions.md`：登记并记录 `ElementBounds` / `element_bounds` 决策。
- `crates/platform/api/**`：新增 `ElementBounds` 与 `UiAutomationProvider::element_bounds`，并导出公共类型。
- `crates/platform/windows/**`、`crates/replay/**`、`apps/agent-core/tests/support/**`：
  同步 Windows / unsupported / replay / fake provider 实现与负向测试。
- `apps/agent-core/src/paint_handlers.rs`、`paint_handler_support.rs`、`paint_handler_map.rs`、
  `paint_registry.rs`：7 个 Paint handler、目标加载、声明子集注册与 reserved host operation。
- `apps/agent-core/src/production.rs`、`production_resume.rs`、`production_policy.rs`：
  `AdapterKind` 分支、Paint registry、挂载集精确断言、已记录工具级 approval 的消费。
- `apps/agent-core/src/task_package_render.rs`、`runtime_binding.rs`：
  Paint assertion table、`pre/post_snapshot_blob_id` 别名与 L3 `point_of_no_return`。
- `apps/agent-core/src/main.rs`：`--adapter-kind notepad|paint` 与默认 adapter root。
- `apps/agent-core/tests/production_paint.rs`：T3.1 fake 正向 + 缺工具 / 缺 target / 缺 blob sink 负向。
- `docs/memory/facts.md`、`docs/memory/apps/paint.md`、`MEMORY.md`：事实与应用档案同步。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --lib notepad_targets::` → **3 passed / 0 failed**
  （含新增 `test_second_adapter_loads_with_its_own_required_targets`）。
- `cargo clippy -p assistant-agent-core --all-targets -- -D warnings` → EXIT 0。
- Notepad 回归：`--test production_root` **7 passed**、`production_root_t1_2` **6 passed**、
  `production_root_t1_3` **1 passed**、`production_root_uia` **1 passed / 8 ignored** → 全绿。
- 第二步复跑：`cargo clippy -p assistant-agent-core --all-targets -- -D warnings` EXIT 0；
  `cargo test -p assistant-agent-core --lib notepad_registry::` **7 passed**；Notepad 回归
  `production_root` 7 / `production_root_t1_2` 6 / `production_root_t1_3` 1 → 全绿。

- `cargo test -p assistant-agent-core --test production_paint` → **4 passed / 0 failed**
  （T3.1 `Plan -> Completed`；缺工具 / 缺 target / 缺 blob sink 三条 fail-closed）。
- `cargo test -p assistant-agent-core --test production_root --test production_root_t1_2 --test production_root_t1_3`
  → **14 passed / 0 failed**。
- `cargo test -p assistant-agent-core --lib notepad_registry::` → **8 passed / 0 failed**。
- `cargo test -p assistant-replay --test replay` → **16 passed / 0 failed**。
- `cargo fmt --all --check` → EXIT 0；`cargo clippy --all-targets -- -D warnings` → EXIT 0；
  `cargo test --workspace` → 全绿（xtask **469 passed**，agent-core lib **56 passed**，Paint 专项 4 passed）。
- xtask：`hygiene` 0E/119W、`memory-counts` PASSED、`adr-index` PASSED、`refscan` PASSED、
  `docscan` PASSED、`card-check` PASSED、`check-ledger` PASSED、`check-comments` PASSED、
  `verify-schemas` PASSED、`codegen --check` PASSED、`check-migrations` PASSED。
- PR #284：CI 全绿、`base=main`、`mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`；
  merge hash **`f5bdfa6`**。

### 4. DoD 逐条核对

- [x] T3.1 任务包经装配可渲染为 `Plan` 并在 fake 平台执行到 `Completed`。
- [x] Paint 缺工具 / 缺必需 target / 未注入 blob sink 均显式失败；专项负向测试覆盖。
- [x] 既有 Notepad `production_root*` 测试全绿（参数化零回归）。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] xtask 门禁全部 PASSED。
- [x] LEDGER 追加事件；`docs/memory/facts.md` / `apps/paint.md` / `decisions.md` 已同步。

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

**DRIFT-106-2**（触发器 ④ + ⑧）

- **现象**：ADR-0084 D5 与实现前勘察说 `paint.canvas.resolve_point` 的 canvas bounds 从
  `snapshot_tree` 的 canvas 节点读取。但现行 `UiAutomationProvider::snapshot_tree` 的公共
  返回类型 `TreeSnapshot` 只暴露 `window` / `fingerprint` / `node_count`，没有树节点、
  canvas 句柄或 bounds。`ResolvedElement` 也只有本地句柄、父句柄与 role；`ResolvedWindow`
  同样不含屏幕矩形。当前平台 trait 没有 `bounds` 方法。
- **影响**：handler 在不新增平台能力、不伪造 `canvas_bounds_px`、不改用可见文本兜底的
  前提下，无法计算屏幕坐标。用任务输入或常量冒充 bounds 会违反铁律 1/2；改
  `crates/platform/api/**` 触发 ADR-0084 D8 和漂移触发器 ③④，必须先有 ADR。
- **建议**：另立 ADR/卡扩展平台读取路径（候选是 `snapshot_tree` 返回结构化节点、
  新增 element bounds 方法，或让平台 capture/coordinate provider 显式提供 target bounds）；
  人类裁决后再实现 `paint.canvas.resolve_point`。不得在本卡直接改公共 trait。
- **已停工作**：`paint_handlers.rs`、`build_paint_registry`、`production.rs` 适配器分支、
  T3.1 fake 正向/负向测试均未编写；已提交的 target 与注册循环参数化不受影响。
- **闭环（2026-10-08 人类选择后者）**：新增 ADR-0085，采用专用
  `UiAutomationProvider::element_bounds`；`TreeSnapshot` 保持不变。Paint handler 已改用该只读
  bounds 路径，fake 正向与负向测试全绿。

### 6. 更合理做法

- 用**专用只读 bounds 方法**而不是扩 `TreeSnapshot`：前者只把 Windows 已有的
  `CurrentBoundingRectangle` 路径公开，后者会把节点身份、回放格式、序列化和树裁剪一次性
  拉进公共契约。
- 把 Paint 的 7 个工具声明做成 registry 子集而不是改 `tools.json`：未实现的 3 个工具继续
  留在适配包声明中，但生产挂载集精确限制为 T3.1 子集。
- 任务包中的 `$pre_snapshot_blob_id` 在 renderer / binding 层显式做别名，避免把任务包错误
  扩散到 handler 或伪造一个输入值。

### 7. 遗留问题

- Paint selector、工具/颜色/图层回读、drag 命中误差与视觉容差仍待真机校准；
  属 `DRIFT-044-1` / `PL-113`，本卡只提供可运行对象。
- Paint 其余 3 个工具（`paint.canvas.rollback_last_write` / `paint.file.open` /
  `paint.file.save_as`）仍留后续卡。
- `paint.document.new` 的 `document_generation` 是 Host 内动作代次；真机若需要应用原生
  generation，需另立契约。

### 8. 新增长期记忆

- `docs/memory/facts.md`：记录 ADR-0085 的 `element_bounds` 语义、Windows/replay fail-closed 路径。
- `docs/memory/apps/paint.md`：记录 TASK-106 的 7 handler、物理 bounds 路径与 provisional selector。
- `docs/memory/decisions.md`：记录 ADR-0085 决策全文。

### 9. 给审阅者的关注点

1. `ElementBounds` 的坐标语义固定为全局虚拟屏物理像素；请重点审阅 Paint 点换算与
   `pointer_action` 的 `NormalizedPoint` 转换是否一致。
2. Paint 工具/颜色/图层的真实 UIA read-back 尚未校准，当前 fake 通过不代表真机 selector 已通过；
   真机十次运行仍是独立验收卡。
3. 工具级 approval 消费现在覆盖所有非低风险写步骤；请确认它没有削弱 policy，而是只消费
   已显式记录、有限 TTL / uses 的人工 grant。
