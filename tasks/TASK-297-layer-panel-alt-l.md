# TASK-297　图层面板：改用 `Alt+L` + 有界轮询，去掉歧义 toggle 锚点

- 状态：Done
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：TASK-256、TASK-296（调色板修复）、ADR-0087/0094
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-256（T3.1 真机链路）　**预估**：S　**难度**：S
- **write scope**：`apps/agent-core/src/paint_handler_actions.rs`、`apps/agent-core/src/paint_handlers.rs`、`apps/agent-core/src/paint_handler_map.rs`、`apps/agent-core/tests/production_paint.rs`、`adapters/com.microsoft.paint/selectors/targets.json`
- **关联**：`docs/memory/apps/paint.md` §11.4 / §12.4、`eval/tasks/paint/t3.1/probe-evidence.json`（`layers_panel`）、TASK-294/296

**目标**

修掉 T3.1 真机的下一处 blocker：`paint.layer.select` 报
`TargetAmbiguous: RoleAndParent: parent candidate 'layers-toggle-container-name-fallback' matched 4 elements`。

**为什么这样修**

仓库里已有一手证据（`probe-evidence.json` 的 `layers_panel`）：
**`Alt+L` 能开合图层面板，且「不需要解析本地化的 toggle 按钮」**。而现实现恰好在 `layersList`
缺失时去解析那个「层」父候选 —— 只读探针实测它命中 **4** 个元素（`TargetAmbiguous`，ADR-0044 拒绝猜）。

**In scope**

1. `resolve_layer_list`：先解析 `layersList`（aid）；**缺失才**发 `Alt+L`，再**有界轮询**（20 × 50 ms）重解；
   仍缺失 → 显式失败。`Alt+L` 是**开关**，因此状态感知 —— 面板已展开时一个键都不发。
2. 去掉「用完把面板收回去」那一步（它同样依赖那个歧义锚点；面板保持展开是无害终态）。
3. 形状库与图层共用一处 `poll_for_element`（避免两套有界轮询行为漂移）。
4. 适配包移除 `layers_toggle` 目标（无人引用 + 歧义陷阱），fake 同步。

**Out of scope（做了算漂移）**

- 改 `crates/**`（TASK-296 已落）；`paint.tool.select` 后置条件、DPI 感知口径（另卡）。
- 调色板 / 颜色路径。

**必须遵守**

- 铁律 1：`layersList` 在 1 s 内不出现 → 显式失败，不猜、不无限等。
- `Alt+L` 只在 `layersList` 缺失时发（开关语义），不得盲发。
- 不改任何测试断言来"让它过"。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_paint
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
python eval/tasks/paint/t3.1/validate.py
cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_1_runs_on_real_paint   # 真机，需人类置前台
```

**DoD**

- [ ] `resolve_layer_list` 状态感知 + 有界轮询；不再引用歧义 toggle 锚点
- [ ] 适配包 / fake 同步（`layers_toggle` 移除）
- [ ] 上述静态验收全绿（真机 E2E 见 §7）
- [ ] `LEDGER.md` 追加一行；9 节记录填写
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-297（图层路径去歧义）。
- **write scope**：上列 handler / fake / 适配包选择器 + 本卡。
- **铁律**：1（有界等待 + 显式失败）、5（L2 快捷键优先于解析不可靠控件）、9（不静默扩大范围）。
- **禁止**：改 `crates/**`、盲发 `Alt+L`、动其它工具路径。
- **疑问**：无。

### 2. 实际改动文件

- `apps/agent-core/src/paint_handler_actions.rs`（`resolve_layer_list` 重写 + `poll_for_element` 抽取 + 形状库复用）
- `apps/agent-core/src/paint_handlers.rs`（`select_layer_output` 传 `task_id`、去掉收回逻辑与 `TARGET_LAYERS_TOGGLE`）
- `apps/agent-core/src/paint_handler_map.rs`（`LayerSelectHandler` 传 `call.task_id()`）
- `apps/agent-core/tests/production_paint.rs`（`layersList` 仅在展开时存在、`Alt+L` 改回开关语义、删 `layers_toggle` 基建）
- `adapters/com.microsoft.paint/selectors/targets.json`（移除 `layers_toggle`）
- `LEDGER.md`、本卡

### 3. 验收输出摘要

- `cargo fmt --all --check`：exit 0。
- `cargo clippy --all-targets -- -D warnings`：exit 0。
- `cargo test -p assistant-agent-core --test production_paint`：**4 passed / 0 failed / 1 ignored**。
- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py`：
  `PASSED: app_id=com.microsoft.paint targets=17 tools=10 recipes=6`（原 18）。
- `python eval/tasks/paint/t3.1/validate.py`：`paint_t3_1_static_validation_ok cases=10`。
- **真机 E2E**：见 §7（本轮未跑成 —— 未等到人类把 Paint 置前台）。

### 4. DoD 逐条核对

- [x] `resolve_layer_list` 状态感知 + 有界轮询；不再引用歧义 toggle 锚点
- [x] 适配包 / fake 同步（`layers_toggle` 移除，`layersList` 折叠时缺失）
- [x] 静态验收全绿（真机 E2E 见 §7，仍未跑）
- [x] `LEDGER.md` 追加一行
- [x] 无 Out of scope 文件被修改

### 5. 偏差

none（本卡是 TASK-256 调色板 blocker 之后**同一真机链路**上的下一处校准，非漂移）。

### 6. 更合理做法

- 形状库（异步物化）与图层面板（`Alt+L` 展开）共用 `poll_for_element`：一处有界轮询，两处行为不会漂移。
- `Alt+L` 状态感知（只在 `layersList` 缺失时发）—— 对「`Alt+L` 是开关」这一实测语义天然安全，
  不会再出现「面板本来开着 → 盲发一次把它关掉」的旧故障形态。
- 删掉「用完收回面板」：面板保持展开是无害终态，而收回要再盲发一次开关键（且依赖歧义锚点）。

### 7. 遗留问题

- **真机 T3.1 端到端仍未跑通**：本轮提示后未等到人类把 Paint 置前台，用例按设计 30 s 超时失败
  （未发送任何输入）。需人类在场时重跑。
- `paint.tool.select` 仍无后置条件（推荐顺序第 2 步）。
- 进程未声明 DPI 感知、`scale` 报 1.0，与 ADR-0092 细则 3 冲突（推荐顺序第 1 步）。

### 8. 新增长期记忆

- 本卡不改 `docs/memory/*`；`Alt+L` 是开关 + 图层面板不需要解析本地化 toggle 这两条**既有**证据
  已在 `docs/memory/apps/paint.md` §11.4 与 `probe-evidence.json` 中，本次只是照它落地。

### 9. 给审阅者的关注点

- `Alt+L` 只在 `layersList` 缺失时发 —— 请确认这是「状态感知」而不是「盲发开关」（旧实现正是盲发）。
- 适配包 target 数 18 → 17（移除 `layers_toggle`）；`layers_panel` 仍被 `app_map.json` /
  `paint_registry.rs` 引用，故保留。

### 2026-10-20 TASK-324 收口补充

- PR #412 已合并到 `main`，merge commit = `f81f792c83461c34f9247b4a60132f5284cc1c73`。
- GitHub CI rollup = 11 checks / 11 success / pending 0；后续 TASK-303 真机 E2E 已证明图层面板路径生效。
- TASK-324 仅依据既有真实证据把状态从 `Review` 更新为 `Done`，未改正文或产品代码。
