# TASK-306　Paint 模态弹层检测与 `paint.document.new` fail-closed

- 状态：**Review**
- 阶段：1　子阶段：**1b**　批次：治理池　依赖：TASK-303、TASK-304、TASK-256
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `crates/platform/windows/src/win32.rs`
  - `crates/platform/windows/src/window/mod.rs`
  - `crates/tool-bus/src/error.rs`
  - `crates/tool-bus/tests/coverage_contract.rs`
  - `apps/agent-core/src/paint_handlers.rs`
  - `apps/agent-core/src/paint_handler_actions.rs`
  - `apps/agent-core/tests/production_paint.rs`
  - 本卡 / `LEDGER.md` / 必要的进度同步文件
- **关联**：`DRIFT-305-1`、`DRIFT-306-1`、`docs/memory/apps/paint.md` §12、ADR-0022（Win32 身份）、ADR-0063（有界状态）、ADR-0084（Paint handler）、ADR-0092（dock / 窗口几何）

**目标**

当 Paint 已经存在未保存画布时，`Ctrl+N` 会弹出 WinUI 三态保存框。该弹层不是 UIA
元素，但它是 Paint 线程上的 `Microsoft.UI.Content.PopupWindowSiteBridge` 顶层窗口。
本卡让平台层把这种目标线程弹层纳入 `WindowState.occluded` 的保守判定，并让
`paint.document.new` 在发送任何进一步合成输入前 fail-closed。

**现象（2026-10-16 真机，一手）**

- 人类保留未保存的红色矩形时复跑 T3.1，Paint 弹出：
  `是否要保存? / "无标题"中有未保存的更改。 / 保存 / 不保存 / 取消`。
- `PrintWindow(PW_RENDERFULLCONTENT)` 能抓到该框，但 UIA ControlView 与 RawView
  **都看不到它**；`EnumThreadWindows` 能看到同一线程上的顶层窗口
  `class=Microsoft.UI.Content.PopupWindowSiteBridge`。
- 本轮 `visual_assert` 仍然 Satisfied：`changed_ratio_within` 只要求变化比例不超过
  `0.08`，两张完全相同的图也会成功；旧的红矩形因此掩盖了“本轮没有真正写出新内容”。
  这正是 `state_changed` Falsified 的真实原因之一，不能把该断言当作抖动删除。

**必须遵守**

- 铁律 1：任何 blocked / 失败都必须带可读原因与 `ErrorCode`，不得静默继续。
- 铁律 4：`paint.document.new` 的写入必须后置可验证；本卡不删除
  `state_changed`，只阻止把弹层当成功路径。
- 铁律 5：Windows 细节留在 `crates/platform/windows`；`apps/agent-core` 只消费
  `WindowProvider` trait 的 `window_state`。
- 不改 `WindowProvider` trait、`ToolBus` schema、协议 schema 或 `ErrorCode` 枚举。
  现有的 `ErrorCode::UserInteraction` 已能表达“需要人类处理弹层”，本卡只补
  `ToolBusError::Mcp` 到该既有类别的映射。
- 不新增第三方依赖；不新增 crate；不引入 `unsafe` 到 `apps/agent-core`。
- 弹层检测必须是有界的一次枚举，不缓存、不无限轮询。

**Out of scope（做了算漂移）**

- 实现“已有画布继续画”的 Planner / task package 选择逻辑（后续卡）。
- 实现全部画笔、形状、颜色工具（后续卡）。
- 修改 `visual_assert` / `state_changed` 的语义或删除 `state_changed`。
- 自动选择保存、不保存或取消；本卡只 fail-closed，不替人类决定。
- 把 `TaskStatus` 从 `Failed` 改成 `NeedsHuman` 的运行时协议变更。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-platform-windows
cargo test -p assistant-tool-bus
cargo test -p assistant-agent-core --test production_paint
```

**DoD**

- [ ] 平台层能只读检测 Paint 线程的 `Microsoft.UI.Content.PopupWindowSiteBridge`
- [ ] `paint.document.new` 在弹层已存在时于 `Ctrl+N` 前 fail-closed
- [ ] `paint.document.new` 在 `Ctrl+N` 触发弹层后于 `Alt+L` / 后续输入前 fail-closed
- [ ] 失败映射为既有 `ErrorCode::UserInteraction`，消息可读且不自动重试
- [ ] fake 平台新增负向用例，断言弹层存在时不发任何输入
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 的文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-306（Paint 模态弹层 fail-closed）。
- **write scope**：上列平台 / tool-bus / Paint handler / fake 测试文件 + 本卡与台账。
- **铁律**：1（无静默失败）、4（写后置条件）、5（平台细节不进入 core）。
- **禁止**：改 `WindowProvider` trait / schema / `ErrorCode` 枚举；实现画笔与形状；替人类处理保存框。
- **疑问**：无。已知限制：现有 runtime 对普通工具错误落 `TaskStatus::Failed`，不是
  `NeedsHuman`；本卡使用既有 `UserInteraction` 错误类别表达“需要人类处理”，
  `NeedsHuman` 落状态留给后续运行时卡。

### 2. 实际改动文件

- `crates/platform/windows/src/win32.rs`：新增目标线程弹层枚举与类名判定
  （`Microsoft.UI.Content.PopupWindowSiteBridge` / `#32770`），附纯函数单测。
- `crates/platform/windows/src/window/mod.rs`：`window_state.occluded` 纳入阻塞弹层。
- `crates/tool-bus/src/error.rs`：新增 `MCP_USER_INTERACTION = -32010` 到既有
  `ErrorCode::UserInteraction` 的映射。
- `crates/tool-bus/tests/coverage_contract.rs`：映射负向/正向契约测试。
- `apps/agent-core/src/paint_handler_actions.rs`：新增 `window_state` 与
  20 × 50 ms 有界 `wait_for_blocking_popup`。
- `apps/agent-core/src/paint_handlers.rs`：`paint.document.new` 在 `Ctrl+N` 前与后
  fail-closed，统一返回 `UserInteraction`。
- `apps/agent-core/tests/production_paint.rs`：fake 增加 `modal_popup` 状态与
  `support/paint_modal_popup.rs` 模块挂载。
- `apps/agent-core/tests/support/paint_modal_popup.rs`：新增弹层存在时不发任何输入的负向用例。
- `adapters/com.microsoft.paint/interrupts/interrupts.json`：补 WinUI 弹层的中断声明。
- `docs/memory/apps/paint.md`：§12.6 记录通道事实与 `visual_assert` 假阳性。
- `docs/memory/pitfalls.md`：新增“变化比例上界不等于写入发生”的坑。
- `tasks/TASK-305-canvas-write-postcondition.md`：追加 A 方案撤回更正。
- `LEDGER.md` / `MEMORY.md`：台账与规模表同步。

### 3. 验收输出摘要

- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0（仅既存 `clippy::assert_is_empty`
  unknown-lint warning；非本卡引入）。
- `cargo test -p assistant-platform-windows` → `111 passed / 0 failed / 4 ignored`。
- `cargo test -p assistant-tool-bus` → `80 passed / 0 failed`。
- `cargo test -p assistant-agent-core --test production_paint` → `5 passed / 0 failed / 1 ignored`，
  含新增 `paint_modal_popup::test_paint_document_new_fails_closed_when_modal_popup_is_present`。
- `cargo test --workspace` → `469 passed / 0 failed / 0 ignored`。
- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` → PASSED
  `targets=17 tools=10 recipes=6`。
- `cargo run -p xtask -- hygiene / check-ledger / card-check / docscan / memory-counts` → 全部 PASSED。
- 真机只读探针（生成物在 `target/paint_bind`，不入库）：保存框打开时
  `window_state minimized=false foreground=false occluded=true`；`EnumThreadWindows`
  同线程窗口为 `class=Microsoft.UI.Content.PopupWindowSiteBridge`。
- **真机 T3.1 复跑（保存框取消后，人类重新打开干净 Paint）**：
  `cargo test -p assistant-agent-core --test production_paint -- --ignored --nocapture test_paint_t3_1_runs_on_real_paint`
  → `1 passed / 0 failed / 9.90 s`，`TaskStatus::Completed`。新 Paint 前台时预检没有误报弹层，
  T3.1 全链路继续完成。

### 4. DoD 逐条核对

- [x] 平台层只读检测 Paint 线程弹层。
- [x] `Ctrl+N` 前弹层存在 → 不发送任何输入，直接 fail-closed。
- [x] `Ctrl+N` 后 20 × 50 ms 有界窗口内发现弹层 → 不再发 `Alt+L` / 工具 / 拖拽。
- [x] 失败映射到既有 `ErrorCode::UserInteraction`，消息可读且不可自动重试。
- [x] fake 负向用例断言 `revision == 1` 且 `pointer_actions == 0`。
- [x] 静态验收全绿；本卡 / LEDGER / 长期记忆已更新。
- [x] 未实现画笔、形状或保存框自动选择；无 Out of scope 代码改动。

### 5. 偏差

- **`DRIFT-306-1`（已记录、未扩大本卡范围）**：现有 runtime 对普通工具错误只落
  `TaskStatus::Failed`，没有把 `ErrorCode::UserInteraction` 转成 `TaskStatus::NeedsHuman`
  的路径。本卡使用既有错误类别表达“需要人类处理”，不修改运行时状态机；若要真正落
  `NeedsHuman`，需另立运行时契约卡。

### 6. 更合理做法

- 弹层检测放在 `crates/platform/windows`，handler 只消费 `WindowState`；这样
  `apps/agent-core` 不需要 `unsafe` 或直接依赖 `windows` crate。
- 复用既有 `ErrorCode::UserInteraction`，不新增 schema / ErrorCode 枚举；
  tool-bus 只补一个内部 MCP code 到该既有类别的映射。

### 7. 遗留问题

- 保存框仍由人类处理；本卡不自动选择保存 / 不保存 / 取消。
- `state_changed` 暂不删除；TASK-305 的 A 方案已撤回。
- 后续卡必须实现“已有画布续画”与全画笔/形状/颜色工具；本卡只处理弹层 fail-closed。

### 8. 新增长期记忆

- `docs/memory/apps/paint.md` §12.6：WinUI 保存框不是 UIA 元素；
  `EnumThreadWindows` / `PopupWindowSiteBridge` 是实测通道。
- `docs/memory/pitfalls.md` 2026-10-16：`changed_ratio_within` 是上界，相同图也会成功，
  不能当作“写入发生”的唯一证据。

### 9. 给审阅者的关注点

- `window_state.occluded` 现在把目标线程的任意 `PopupWindowSiteBridge` 视为阻塞；
  这会把形状 flyout 等其它 WinUI 弹层也判为 blocked。对 `paint.document.new` 这是保守正确行为，
  但审阅时需确认不会误伤正常流程。
- `MCP_USER_INTERACTION = -32010` 是 tool-bus 内部映射；未改协议 schema / ErrorCode 枚举。
- 真机负向验证已补：保存框打开时 `occluded=true`；取消并重新打开干净 Paint 后 T3.1
  `1 passed / 9.90 s`，说明干净前台路径不会被预检挡住。
