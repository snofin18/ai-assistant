# TASK-307　Paint 已有画布续画与“真实像素变化”下界

- 状态：**Done**
- 阶段：1　子阶段：**1b**　批次：Paint 真机链路　依赖：TASK-306
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `adapters/com.microsoft.paint/tasks/t3.2.continue-existing-canvas-rectangle.json`（新增）
  - `adapters/com.microsoft.paint/tasks/README.md`
  - `apps/agent-core/src/paint_handlers.rs`
  - `apps/agent-core/tests/production_paint.rs`
  - `apps/agent-core/tests/support/paint_continue_existing.rs`（新增）
  - `apps/agent-core/tests/support/paint_real_run.rs`
  - `docs/memory/apps/paint.md`
  - 本卡 / `LEDGER.md` / 必要的进度同步文件
- **关联**：TASK-306、TASK-305、ADR-0074（视觉断言）、ADR-0077、ADR-0084、`docs/memory/apps/paint.md` §12.6

**目标**

让 Paint 真机链路具备“在已有画布上继续画”的可执行任务包，并让 `draw_rectangle`
在像素没有真实变化时显式失败，避免 `visual_assert` 的 `changed_ratio_within`
上界在旧内容上误过。

**为什么不能只靠 `visual_assert`**

`visual_assert` 的 `changed_ratio_within` 判定是 `changed_ratio <= max_changed_ratio`，
两张完全相同的图也会 `Satisfied`（`crates/verify/tests/visual_postcondition_contract.rs`）。
已有画布上如果旧内容已经满足视觉区域，本轮 no-op 可能仍然通过视觉断言。

**本卡修法**

1. 新增不调用 `paint.document.new` 的 T3.2 任务包：直接在当前文档上选工具 / 颜色 / 图层、
   截取 pre snapshot、解析坐标、拖拽、截取 post snapshot。
2. `paint.canvas.draw_rectangle` 在 drag 后比较 `before_capture.blob_id()` 与
   `after_capture.blob_id()`；两者相同即返回 `VerifyFailed`，证明本轮没有真实像素变化。
   这是 content-address 级别的下界检查，不新增公共 schema / `visual_assert` op。
3. fake 平台增加“续画”正向测试；真机 harness 增加 T3.2 ignored 用例，验证不触发
   `Ctrl+N` / 保存框。

**Out of scope（做了算漂移）**

- 新增 `visual_assert` 下界 op 或修改 `crates/verify` 公共契约。
- Planner 根据当前文档自动选择 T3.1 / T3.2；本卡只交付可执行任务包与 harness。
- 完整画笔 / 形状 / 颜色工具集扩展。
- dock / IPC 窗口摆放。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-agent-core --test production_paint
cargo test --workspace
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_2_continues_existing_canvas
```

**DoD**

- [ ] T3.2 任务包不包含 `paint.document.new`
- [ ] `draw_rectangle` 对 `before == after` 的内容地址显式 `VerifyFailed`
- [ ] fake 续画正向测试通过
- [ ] 真机 T3.2 在已有未保存画布上通过，且不出现保存框
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-307（已有画布续画 + 真实像素变化下界）。
- **write scope**：上列适配包 / handler / fake / 真机 harness / Paint 档案 + 本卡与台账。
- **铁律**：1（无静默失败）、4（写后置条件）、5（平台细节不进入 core）。
- **禁止**：改 `visual_assert` 公共契约；自动选择保存 / 丢弃；实现完整画笔形状工具集。
- **疑问**：无。T3.2 的 Planner 自动选择留给后续卡；本卡交付可执行任务包与真机证据。

### 2. 实际改动文件

- `adapters/com.microsoft.paint/tasks/t3.2.continue-existing-canvas-rectangle.json`（新增）：
  9 步，不包含 `paint.document.new`；直接在当前文档上选工具 / 颜色 / 图层、截 pre、画矩形、截 post。
- `adapters/com.microsoft.paint/tasks/README.md`：补充 T3.2 说明与 content-address 下界。
- `apps/agent-core/src/paint_handlers.rs`：`draw_rectangle` 在 drag 后比较 before / after capture
  content address；相同即 `VerifyFailed`。
- `apps/agent-core/tests/production_paint.rs`：fake capture 按画布 revision 变化；新增 T3.2 配置与模块挂载。
- `apps/agent-core/tests/support/paint_continue_existing.rs`（新增）：fake 续画正向测试，断言不出现
  `new_document`、最终 `Completed`、画布 revision 递增。
- `apps/agent-core/tests/support/paint_real_run.rs`：新增 ignored
  `test_paint_t3_2_continues_existing_canvas`。
- `docs/memory/apps/paint.md`：记录 T3.2 真机证据与下界判据。
- 本卡 / `LEDGER.md` / 必要进度同步文件。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --test production_paint` → `6 passed / 0 failed / 2 ignored`。
- 真机 T3.2（人类置前台，已有未保存画布）：
  `cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_2_continues_existing_canvas`
  → `1 passed / 0 failed / 8.72 s`、`TaskStatus::Completed`；没有 `Ctrl+N`，没有保存框。
- 合并：PR #433 / merge `a075f1f`；pull_request CI 22 checks 全绿（21 success + 1 skipping）。
- `cargo test --workspace` → `469 passed / 0 failed`。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0（仅既存 unknown-lint 警告）。
- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` → PASSED。
- `xtask hygiene / check-ledger / card-check / docscan / memory-counts` → PASSED。

### 4. DoD 逐条核对

- [x] T3.2 任务包不包含 `paint.document.new`；9 步。
- [x] `draw_rectangle` 对 `before == after` content address 显式失败。
- [x] fake 续画正向测试通过。
- [x] 真机 T3.2 在已有未保存画布上通过，且没有保存框。
- [x] 静态验收全绿；本卡 / LEDGER / Paint 档案已更新。
- [x] 未修改 Out of scope 的公共 schema / 完整画笔形状工具集。

### 5. 偏差

- none。Planner 自动在 T3.1 / T3.2 之间选择不在本卡范围，已在 §7 留作后续卡。

### 6. 更合理做法

- 用 content-addressed capture 的 blob id 做“下界”检查，而不是新增
  `visual_assert` op 或修改 `crates/verify` 公共契约；实现最小，故障面清晰。
- fake capture 以 `canvas_revision` 变化制造不同的前后图，使续画测试也能验证真实变化路径。

### 7. 遗留问题

- Planner 仍不会自动根据当前文档选择 T3.1 / T3.2；本卡只交付可执行 T3.2 和真机证据。
- 完整画笔 / 形状 / 颜色工具集仍未实现，后续单独开卡。
- 当前 Paint 里有两个未保存矩形（T3.1 + T3.2）；保存或丢弃仍由人类决定。

### 8. 新增长期记忆

- `docs/memory/apps/paint.md`：T3.2 续画不使用 `Ctrl+N`；content-address 下界可发现 no-op。

### 9. 给审阅者的关注点

- content-address 检查的是整窗截图，不是裁剪后的画布；它证明“有像素变化”，不证明变化恰好在目标区域。
  目标区域/颜色仍由 `visual_assert` 负责。
- T3.2 真机通过依赖已有画布；harness 不抢焦点、不创建文档。
