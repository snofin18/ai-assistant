# TASK-305　画布像素写入的后置条件：不该要求 UIA 指纹变化（DRIFT-305-1）

- 状态：**Blocked**（等待人类裁决断言口径）
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：TASK-303、TASK-304
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：TASK-303（真机 harness 注入实测值）、TASK-304（PMv2，截图恢复整窗）　**预估**：S　**难度**：S
- **write scope**：`apps/agent-core/src/task_package_render.rs`、`adapters/com.microsoft.paint/tools/tools.json`
- **关联**：`DRIFT-305-1`、`docs/memory/apps/paint.md` §7 / §12.5、ADR-0074（视觉断言）、ADR-0077

**目标**

修掉 T3.1 真机链路**最后一条**失败，让「画布像素写入」的后置条件与语义一致。

**现象（2026-10-16 真机，一手）**

`cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_1_runs_on_real_paint`
跑到**最后一步** `draw_rectangle`，四条运行时断言里：

| index | 断言 | 结果 |
|---:|---|---|
| 0 | `value_equals selected_tool` | ✅ Satisfied |
| 1 | `value_equals active_layer_id` | ✅ Satisfied |
| 2 | **`visual_assert`**（`pixels` / `changed_ratio_within`，delta 12、ratio 0.08、confidence 0.85） | ✅ **Satisfied —— 像素证据这次真的成立了**（TASK-304 的 PMv2 修复之功） |
| 3 | `state_changed`（`within_ms: 2000`，要求指纹变化） | ❌ **Falsified**：`before == after == sha256:6400a471…` |

**根因（契约问题，不是实现问题）**

`apps/agent-core/src/task_package_render.rs` 的 `paint_canvas_assertions("paint.canvas.draw_rectangle")`
在视觉断言之后**又加了一条** `state_changed{within_ms: STATE_CHANGED_WINDOW_MS(2000)}`。
而画布是**自绘表面**：画一笔是**纯像素写入**，UIA 树的规范文本（control type / automation id / class）
**不变** → 指纹前后一致 → 该断言**永远为假**。同一断言在
`adapters/com.microsoft.paint/tools/tools.json` 里也声明了一份（`fingerprint_scope: "canvas"`、`within_ms: 3000`）。

**拟议修法（需人类裁决）**

**A（推荐）**：删掉画布写入步骤的那条 `state_changed`，**以 `visual_assert` 作为唯一写后置条件** ——
它正是为「像素级写入」设计的信号（ADR-0074 / ADR-0077），且本轮**已经在真机通过**。
同时删掉 `tools.json` 里同义的 `state_changed{fingerprint_scope:"canvas"}`，避免两份声明互相矛盾。

**B**：保留 `state_changed` 但改 `fingerprint_scope` 为「像素摘要」——等同于把 A 的语义换一种表达，
需要给 `state_changed` 增加像素作用域（改公共 spec，成本更高）。

**C**：把该断言放宽为「允许不变」——**不推荐**：那等于承认「写操作无法验证」，
违反铁律 4（每个写操作必须有 postcondition）。

**Out of scope（做了算漂移）**

- 改 `crates/verify` 的断言引擎本身（A 不需要）。
- 改其它工具（`paint.tool.select` 等）的断言。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_paint
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_1_runs_on_real_paint   # 真机，需人类置前台
```

**DoD**

- [ ] 人类裁决 A / B / C
- [ ] 按裁决改断言口径（渲染表 + 适配包保持一致）
- [ ] 静态验收全绿 + 真机 T3.1 跑通（`TaskStatus::Completed`）
- [ ] `LEDGER.md` 追加一行；9 节记录填写
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-305（画布写入后置条件口径）。
- **write scope**：上列两个文件 + 本卡 / 台账。
- **铁律**：4（每个写操作必须有可验证 postcondition）、1（不得用"允许不变"掩盖不可验证）。
- **禁止**：动 `crates/verify` 断言引擎；动其它工具的断言。
- **疑问**：**需要人类裁决 A / B / C**（见正文「拟议修法」）→ 本卡停在 **Blocked**。

### 2. 实际改动文件

- 无（本卡只记录 DRIFT 与拟议修法，等裁决后再改）。

### 3. 验收输出摘要

- 真机输出（2026-10-16，人类在场）：`draw_rectangle` 的 4 条断言中 0/1/2 **Satisfied**（**含 `visual_assert`**）、
  仅 index 3 的 `state_changed{within_ms:2000}` Falsified（`before == after == sha256:6400a471…`）。
- 链路其余步骤（`new_document` / `select_rectangle_tool` / `select_foreground_color` / `select_layer` /
  `resolve_start` / `resolve_end`）本轮**全部通过**。

### 4. DoD 逐条核对

- [ ] 人类裁决 A / B / C —— **未裁决，故本卡未开工**
- [ ] 断言口径修改 —— 未做
- [ ] 真机 T3.1 跑通 —— 未做（只差这一条断言）
- [x] `LEDGER.md` 追加一行
- [x] 无 Out of scope 文件被修改

### 5. 偏差

**`DRIFT-305-1`（漂移触发器 ⑧：spec / 断言与实测矛盾）**

- **现象**：`task_package_render.rs` 为 `paint.canvas.draw_rectangle` 渲染的 `state_changed{within_ms:2000}`
  在真机上**必然为假** —— 画布是自绘表面，像素写入不改变 UIA 树，指纹前后一致（一手 SHA-256 相同）。
- **影响**：T3.1 真机链路永远无法 `Completed`；而真正表达"画布被改动"的那条 `visual_assert`
  **已经通过**（TASK-304 的 PMv2 修复让像素证据首次成立）。
- **已停工作**：没有改 `task_package_render.rs` 或 `tools.json`（改断言口径属于契约变更，需先裁决）。
- **建议**：按正文 **A** —— 画布写入以 `visual_assert` 为唯一后置条件，两份声明（渲染表 + 适配包）同步删除
  同义的 `state_changed`。

### 6. 更合理做法

- 「UIA 指纹变化」对**控件树写入**（改文本 / 勾选 / 切工具）是好信号，对**自绘像素表面**不适用；
  后者天然属于视觉断言（ADR-0074 / ADR-0077）。两种写入本就该用两种后置条件。
- 本轮之所以能一眼分清，是因为 TASK-304 先把截图修好了：`visual_assert` 从"不可评估"变成"真的比对像素"，
  失败因此精确定位到那一条多余的指纹断言上。

### 7. 遗留问题

- 等裁决期间 T3.1 真机链路停在 `draw_rectangle` 的最后一条断言（其余全通）。
- 裁决后落地改动很小（删一条断言 + 适配包同步），预计一次真机复跑即可收口。

### 8. 新增长期记忆

- 见收尾（落地后拟记 `docs/memory/pitfalls.md`：「自绘表面写操作不能用 UIA 指纹做后置条件」）。

### 9. 给审阅者的关注点

- 请确认 A 方案**不**削弱铁律 4：`visual_assert` 是**像素级**后置条件（delta/ratio/confidence 三档阈值），
  比"指纹变了"更强地证明了"画布确实被写入了目标内容"。
- 若选 B（给 `state_changed` 加像素作用域），需要同时改公共 spec/schema，成本与风险都更高。

---

**2026-10-16 晚 · 更正（同一断言第二次真机复跑）**

本卡 §5 写的「该断言对画布写入**必然为假**」**不准确，已作废**。人类重启 Paint 后复跑，
**整条 T3.1 真机链路 `ok`（1 passed / 9.13 s）** —— 即同一条 `state_changed{within_ms:2000}` 这次
**通过了**（指纹确实变了）。

因此准确表述是：**「窗口 / 画布 UIA 指纹变化」对自绘表面的像素写入是「状态相关、不可靠」的判据**
（同一份代码：一次 Falsified、一次 Satisfied），**不是"恒为假"**。

这**仍然支持原建议 A**（改用像素级 `visual_assert` 作为写后置条件），但**理由要改成"消除抖动"**：

1. 抖动的直接后果是**同一 commit 时红时绿** —— 这是不可接受的验收信号（比"恒为假"更隐蔽）。
2. `visual_assert`（`pixel_delta_threshold 12` / `max_changed_ratio 0.08` / `confidence_min 0.85`）
   在**两次真机运行中都是 Satisfied**，是稳定且语义正确的判据。

**附带事实（与上一轮的保存对话框有关）**：本次复跑（人类**重启 Paint**后）**没有再弹保存对话框** ——
与「上一轮那次对话框来自**上一轮遗留的未保存矩形** + 本轮 `Ctrl+N`」的判断一致。画布证据：2× 整窗
截图中画布上有红色矩形，状态栏 `40 × 40像素`（= (10,10)→(50,50)）与 `100%` 缩放。

---

**2026-10-16 晚 · 第二次更正：A 方案撤回（TASK-306 真机只读探针）**

**A（删除 `state_changed`）作废，不得执行。** TASK-306 的真机取证把上一段“抖动”解释纠正为
**未保存保存框导致的 no-op**：

1. `PrintWindow` 抓到了保存框，文案为
   `是否要保存? / "无标题"中有未保存的更改。 / 保存 / 不保存 / 取消`；
2. 该框不在 UIA ControlView / RawView 中，但它是 Paint 线程上的
   `Microsoft.UI.Content.PopupWindowSiteBridge` 顶层窗口；`EnumThreadWindows` 可见；
3. `WindowState.occluded` 在弹层存在时为真；T3.1 真机 harness 未在 `Ctrl+N` 前处理这个前置条件，
   因此旧红色矩形仍在画布上；
4. `visual_assert` 的 `changed_ratio_within` 是**上界**（`changed_ratio <= 0.08`），
   `crates/verify/tests/visual_postcondition_contract.rs` 明确证明**两张完全相同的图也 Satisfied**；
   所以它不能证明“本轮真的写入了新内容”；
5. 唯一在这次 no-op 中报警的是 `state_changed`（`before == after`）。删掉它会让
   “保存框挡住绘制、旧矩形冒充新结果”的路径变成绿色通过。

**修正后的结论**：`state_changed` 不是“抖动”，至少在保留未保存画布的那次复跑中它检测到了真实问题。
后续若要处理自绘表面的写入证明，应先修 TASK-306 的弹层 fail-closed，再评估是否需要
“最小像素变化”这类**下界**断言；不得用删掉唯一变化信号的方式收口。
