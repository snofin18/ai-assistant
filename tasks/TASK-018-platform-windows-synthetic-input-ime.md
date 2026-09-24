# TASK-018　`platform/windows`：合成输入（SendInput）+ 焦点校验 + 坐标归一化（DPI/多屏）+ IME 处理

- 状态：**Done**（2026-09-25）
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：016（+017 已 Done）　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：016（trait 形状 + 纯类型，Done）；017（本 crate 的 `com` / `win32` / `handles` 层已落地，Done）
- **预估**：M　**难度**：M
- **write scope**：`crates/platform/windows/src/input/**`、`crates/platform/windows/src/coordinates/**`、
  `crates/platform/windows/Cargo.toml`（仅 `windows` crate 的 **feature 开关**）、
  `crates/platform/windows/src/uia/mod.rs`（**仅** `pointer_action` / `key_action` 两个方法体）、
  `crates/platform/windows/src/unsupported.rs`（**仅** 同两个方法的非 Windows 分支）、
  `crates/platform/windows/README.md`、`crates/platform/windows/tests/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）；架构 v2 §6.9（坐标空间）/ §13.1.1（trait）/ §13.2（Windows 通道）；
  `docs/memory/win32-input-research.md`（**全量读**）；`docs/spec/naming.md`、`docs/spec/error-codes.md`；
  **铁律 1 / 4 / 5 / 6 / 7**；ADR-0019 N1（负向验证）；ADR-0024（`windows` crate）；ADR-0045（非宿主平台编译门禁）

**目标**

把 TASK-017 留下的两个**合成输入**占位实现（`pointer_action` / `key_action`）真正落到
`SendInput` 上，并把**坐标归一化**（DPI / 多屏）与 **IME** 处理一并做完 ——
这是架构 v2 铁律 5 里的 **L4（合成输入）**层，只在 L1（API）~ L3（无障碍）都做不到时才被调用。

**为什么现在做（不静默扩范围）**

1. 批次表 A2 已把 `src/input/**`（`SendInput` / `keybd_event` / `SendKeys`）与 `src/coordinates/**`（DPI / 多屏）分给 **TASK-018**（TASK-017 卡面第 20 行同样写明）。
2. TASK-017 的 Q2 裁决 = 两个方法**实现到「明确报错」**并标 `// STUB(TASK-018):`，并写明「TASK-018 落地时替换」——本卡正是那个替换。
3. `crates/platform/api` 已提供 `NormalizedPoint` / `CoordinateSpace` / `PhysicalPoint`（`to_physical` 是**纯函数**）—— 平台侧只需提供**实际显示器组合的 `CoordinateSpace`**，无需重定义换算。
4. 不做则 TASK-035（Notepad Adapter）的任何快捷键 / 点击路径都无法走通；且占位实现会一直报 `CapabilityMissing`（虽然是明确失败，但能力缺失）。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `src/input/**`：`SendInput` 封装 —— `send_virtual_key`（VK 下/上）与 `send_unicode`（`KEYEVENTF_UNICODE`，绕过 IME 与键盘布局） | 架构 v2 §13.2 + `win32-input-research.md` §1 |
| 2 | `src/input/**`：**发送前 100% 校验前台窗口** —— `GetForegroundWindow` 与目标 hwnd 不一致 → 先 `SetForegroundWindow` 并**回读确认**，仍不一致 → `TargetUnresponsive`（**不猜、不盲发**） | 批次表 A2 验收要点（铁律 1） |
| 3 | `src/input/**`：**`SendInput` 返回值必校** —— `sent != requested` → 区分 UIPI（`ERROR_ACCESS_DENIED` 5）/ 参数（87）/ 其他，映射成 `ErrorCode`（**不吞**） | 铁律 1 + `win32-input-research.md` §1 |
| 4 | `src/coordinates/**`：列举显示器（`EnumDisplayMonitors` / `GetMonitorInfoW`）并构造**实际** `CoordinateSpace`（Per-Monitor V2 下的 `GetDpiForWindow`） | 架构 v2 §6.9 + §6.2 `coordinate_space` |
| 5 | `src/coordinates/**`：`NormalizedPoint` → `PhysicalPoint`（经 `CoordinateSpace::to_physical`）+ **显示器归属校验**（点落在哪个显示器就用哪个的 DPI） | 架构 v2 §6.9 规则 1 / 3 |
| 6 | `src/input/**`：**IME 处理** —— 文本写入走 `KEYEVENTF_UNICODE`（不经 IME 组字）；并提供 `is_ime_open`（`ImmGetContext` / `ImmGetOpenStatus`）供上层判断 | `win32-input-research.md` §1 + 批次表 A2 验收要点 |
| 7 | `src/uia/mod.rs`：用上面两域替换 `pointer_action` / `key_action` 的 `// STUB(TASK-018):` 占位（**仅** 这两个方法体） | TASK-017 Q2 裁决 |
| 8 | `crates/platform/windows/README.md`：同步「已知限制」（合成输入能做什么 / 不能做什么） | gov §5.4 + 阶段 1 DoD |
| 9 | 单测：正向 + **负向**（ADR-0019 N1）；**非 Windows 平台不依赖真实应用** | gov §5.5 + ADR-0019 |
| 10 | 真机手工验收记录（真实记事本，Windows 11 25H2）—— 写进执行记录 §3 | 批次表 A2 验收要点 |

**Out of scope（做了算漂移）**

- 真正的**拖拽租约**与坐标校准（多次采样、误差矩阵）→ TASK-040（本卡只做**单次**坐标归一化）
- 截图 / 脱敏 / 视觉验证与视觉兜底 → TASK-041 / 042
- 策略判定 / 白名单 / 人工确认（铁律 6 的 L3 不可逆动作）→ TASK-021 / 027（**本层不判权限**）
- 后置断言引擎 → TASK-023；撤销 → TASK-024；租约 → TASK-025
- Host IPC → TASK-019；CDP / Edge → TASK-048；Excel / Photoshop → 阶段 2 / 3
- 真实记事本 3 任务闭环 → TASK-035 ~ 038
- **任何** `#[allow]` 放宽（漂移触发器 ⑥）；新增**未登记**的第三方依赖（触发器 ①）
- 改 `crates/platform/api/**`（trait 形状**冻结**；确需改 → DRIFT，触发器 ③）
- 改 `crates/core/**` / `crates/core/tests/arch*`（不在 write scope）
- 其他 `crates/platform/windows/src/uia/**` 文件（本卡只碰 `mod.rs` 的两个方法体）

**必须遵守**

1. **铁律 5（API 优先）**：本层是 **L4**。每个公开函数的文档注释都要写明「调用方应先试 `set_value` / `edit_text` / `invoke_action`」。
2. **铁律 1（无静默失败）**：`SendInput` 返回值、`SetForegroundWindow` 返回值、`GetForegroundWindow` 回读 —— **每一个**都要判并映射成带 `ErrorCode` 的错误；禁止 `let _ =`。
3. **铁律 4（写操作必有 postcondition）**：本层的 postcondition = **前台窗口与目标一致**（发送前确认）；高层的「结果确实变了」归 TASK-023。
4. **铁律 7**：`unsafe` / FFI **只允许**在 `crates/platform/*`；每个 `unsafe` 块必须有 `// SAFETY:`（gov §5.2）。
5. **铁律 8**：`ResolvedWindow` / `ResolvedElement` **不得**派生 `Serialize` / `Deserialize`；不把 HWND / COM 指针放进可序列化结构。
6. **铁律 6**：本层**不做**人工确认，也**不允许**自己判权限 —— 只提供能力，由策略引擎（TASK-021）与 HITL（TASK-027）决定能不能发。
7. **ADR-0024**：`windows` crate 版本保持 `=0.62.2`；本卡**只**开 feature（`Win32_UI_Input_KeyboardAndMouse` / `Win32_UI_HiDpi` / `Win32_UI_Input_Ime` 等），**不升级**。
8. **非 Windows 目标**：`src/input/**` / `src/coordinates/**` 必须 `#[cfg(windows)]` 门控；非 Windows 分支走 `unsupported.rs` 的同形 RPITIT（**零 `#[allow]`**）。按 **ADR-0045** 在两条 `--target` clippy 上验收。
9. **不得用 `keybd_event` / `SendKeys`**：`win32-input-research.md` §2 / §3 已定论（`keybd_event` 已被 `SendInput` 取代；`SendKeys` 在非交互会话不可靠）。
10. **注释密度**（gov §5.2）：每 20~40 行有一条解释性注释；`unsafe` 块必有 `// SAFETY:`；临时方案必带 `// TODO(TASK-0NN):`。

**待裁决（Q1 ~ Q3）**

- **Q1（feature 开关算不算「加依赖」）**：`SendInput` / `GetForegroundWindow` / `EnumDisplayMonitors` / `GetDpiForWindow` / `ImmGetContext` 分属
  `windows` crate 的多个 feature，而 TASK-017 只开了 `Win32_UI_Accessibility` + `Win32_System_Ole`。
  **建议默认**：把 feature 开关当作**同一已登记依赖的配置**（不是新依赖 → 不命中触发器 ①），但在执行记录 §5 **显式声明**开了哪些、为什么。
- **Q2（IME 的深度）**：完整 IME（组字、候选窗、编码转换）超出本卡范围。
  **建议默认**：本卡只做**两件事**：① 文本输入走 `KEYEVENTF_UNICODE`（**绕过** IME，因此「IME 开着也能正确写入」）；
  ② 提供 `is_ime_open()` 供上层判断（不自行关 IME）。**不做**候选窗操作。
- **Q3（真机验收归属）**：CI 三平台矩阵里只有 `windows-latest` 能跑，且**没有真实记事本 fixture**（靶机 `notepad-like` 归 TASK-033）。
  **建议默认**（同 TASK-017 Q4）：单测**不依赖真实应用**（纯逻辑 + 能力探测）；「Spike A2 坐标精度矩阵 ≤ 2 px」与「IME 开着也能写入」改为**人类在本机跑的手工验收**，
  结果贴进执行记录 §3，**不作为 CI 门禁**。

**未裁决时的处理**

- Q1 / Q2 / Q3 未单独答复 → 按**建议默认**执行（三者都是「本卡不扩范围」的保守方向），并在执行记录 §5 登记为「按建议默认」。

**DoD**

- [ ] `pointer_action` / `key_action` 的 `// STUB(TASK-018):` 占位已被真实实现替换，且两个方法都不再返回 `CapabilityMissing`
- [ ] **发送快捷键前 100% 校验前台窗口**（不一致 → 先尝试置前，回读仍不一致 → 明确报错，**不盲发**）
- [ ] `SendInput` 返回值被逐次校验；UIPI（`ERROR_ACCESS_DENIED`）与参数错误（87）可区分且有单测
- [ ] 坐标归一化走 `CoordinateSpace::to_physical`（**纯函数**，不重写换算）；DPI 取**实际**显示器的值
- [ ] `KEYEVENTF_UNICODE` 路径有单测；`is_ime_open()` 有实现
- [ ] 每个 `unsafe` 块有 `// SAFETY:`；**无任何** `#[allow]` 放宽
- [ ] `ResolvedWindow` / `ResolvedElement` 仍**未**派生 `Serialize` / `Deserialize`
- [ ] 非 Windows 平台 `cargo test --workspace` 仍全绿（`#[cfg(windows)]` 门控 + 明确错误；无 `todo!()` / `unimplemented!()`）
- [ ] 单测含**负向**用例（ADR-0019 N1）
- [ ] `crates/platform/windows/README.md`「已知限制」已同步
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings` 退出码 0（ADR-0045）
- [ ] `cargo clippy --target aarch64-apple-darwin -p assistant-platform-windows --all-targets -- -D warnings` 退出码 0（ADR-0045）
- [ ] `cargo test --workspace` 全绿；`cargo test -p assistant-platform-windows` 全绿
- [ ] `cargo test -p assistant-core arch::` 仍全绿（本卡**不**改 arch 断言）
- [ ] `xtask verify-schemas / codegen --check / hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations` 全部 PASSED
- [ ] `cargo deny check` 全 ok
- [ ] `LEDGER.md` 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`（应用专属的进 `docs/memory/apps/notepad.md`）
- [ ] 无任何 Out of scope 的文件被修改（write scope 外的连带改动全部登记为 DRIFT）

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings
cargo clippy --target aarch64-apple-darwin -p assistant-platform-windows --all-targets -- -D warnings
cargo test -p assistant-platform-windows
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-migrations
cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-018 platform/windows：合成输入（SendInput）+ 焦点校验 + 坐标归一化（DPI/多屏）+ IME 处理
【目标】把 TASK-017 留下的 pointer_action / key_action 两个合成输入占位真正落到 SendInput 上，
        并把坐标归一化（DPI / 多屏）与 IME 处理一并做完 —— 架构 v2 铁律 5 的 L4（合成输入）层
【write scope】仅：crates/platform/windows/src/input/**、.../src/coordinates/**、.../Cargo.toml（仅 feature 开关）、
  .../src/uia/mod.rs（仅 pointer_action / key_action 两个方法体）、.../src/unsupported.rs（仅同两个方法的非 Windows 分支）、
  crates/platform/windows/README.md、crates/platform/windows/tests/**
【铁律】1（无静默失败）/ 4（写操作有 postcondition）/ 5（API 优先 —— L4 是最后手段）/ 6（L3 不可逆需人工确认）/ 8（句柄不跨进程）
【禁止】拖拽租约与坐标校准（TASK-040）、截图 / 视觉兜底（TASK-041 / 042）、策略判定（TASK-021）、
        租约与淘汰（TASK-025）、撤销（TASK-024）、跨步骤后置断言引擎（TASK-023）
【验收】见卡面「验收命令」：fmt / clippy（含两条 --target）/ test --workspace / arch:: / xtask ×9 / cargo deny
【依赖】016 ✅（trait 形状 + 纯类型）、017 ✅（本 crate 的 com / win32 / handles 层）—— 已核对 LEDGER
【疑问】Q1（feature 开关算不算加依赖）/ Q2（IME 的深度）/ Q3（真机验收归属）
        → 三者均按卡面「建议默认」执行（都是「本卡不扩范围」的保守方向），登记见 §5
```

### 2. 实际改动文件

| 文件 | 类型 | 说明 |
|---|---|---|
| `crates/platform/windows/src/coordinates/mod.rs` | 新增（569 行） | **三平台编译**的纯逻辑：`MonitorRecord`（物理矩形 + 有效 DPI + 主屏标记，**半开区间**命中）、`scale_from_dpi`、`coordinate_space_for_monitor`、`monitor_index_containing_physical` / `monitor_at_physical`、`to_physical_on_monitor`、`coordinate_space_for_logical_point`、`VirtualScreen::normalize`；8 个单测（含 3 个负向） |
| `crates/platform/windows/src/coordinates/win32.rs` | 新增（207 行） | `#[cfg(windows)]`：`enumerate_monitors`（`EnumDisplayMonitors` + 回调）、`record_for_monitor`（`GetMonitorInfoW` + `GetDpiForMonitor`）、`monitor_for_window` / `monitor_for_physical_point`、`dpi_for_window`、`coordinate_space_for_window`、`virtual_screen` |
| `crates/platform/windows/src/input/mod.rs` | 新增（约 480 行） | **三平台编译**的纯逻辑：`VirtualKey` newtype、`KeyEvent` / `PointerStep`、`virtual_key_for_name`、`key_events_for_chord`、`utf16_units`、`pointer_steps`、`requires_foreground`；9 个单测（含 4 个负向） |
| `crates/platform/windows/src/input/win32.rs` | 新增（约 520 行） | `#[cfg(windows)]`：`pointer_action` / `send_key_action` / `send_unicode_text` / `is_ime_open` + `ensure_foreground` / `resolve_key_target` / `element_window` / `focus_element` / `send_inputs` / `classify_send_input_failure`；6 个单测 |
| `crates/platform/windows/src/input/acceptance.rs` | 新增 | **真机验收**（`#[cfg(all(test, windows))]` + `#[ignore]`）：坐标精度 ≤ 2 px、真实记事本 Unicode + `Ctrl+S` 磁盘回读。**位置偏离见 DRIFT-018-2** |
| `crates/platform/windows/tests/synthetic_input_contract.rs` | 新增 | 契约/负向：`// STUB(TASK-018):` 扫描器（含负向样本 + 文件数下界防恒真）、远点 → `TargetNotFound`、未知键名 → `ToolInvalidArgs`（不碰真实窗口） |
| `crates/platform/windows/src/uia/mod.rs` | 改（**仅两个方法体**） | `pointer_action` / `key_action` 的 `// STUB(TASK-018):` 占位替换为 `crate::input` 的真实实现（用全限定路径，未加 `use`） |
| `crates/platform/windows/Cargo.toml` | 改（**仅 feature 开关**） | `Win32_UI_Input_KeyboardAndMouse` / `Win32_UI_Input_Ime` / `Win32_UI_HiDpi` / `Win32_Graphics_Gdi`（同一已登记依赖的配置，Q1） |
| `crates/platform/windows/src/lib.rs` | 改（**write scope 外**） | `pub mod coordinates;` + `pub mod input;` —— **DRIFT-018-1** |
| `crates/platform/windows/README.md` | 改 | 职责 +2 域、边界、测试表、真机验收、已知限制 +6 条、`unsafe_code` 段补 TASK-018 |

**未改动但需说明**：`src/unsupported.rs` 在卡面 write scope 内，但**经核对无需改动** —— TASK-017 已把 `pointer_action` / `key_action` 写成返回带 `ErrorCode` 的 `CapabilityMissing`（正是本卡 DoD 要求的形态），且其单测断言覆盖。

### 3. 验收输出摘要

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | **0 diff** |
| `cargo clippy --all-targets -- -D warnings`（Windows） | **exit 0** |
| `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings` | **exit 0**（ADR-0045） |
| `cargo clippy --target aarch64-apple-darwin -p assistant-platform-windows --all-targets -- -D warnings` | **exit 0**（ADR-0045） |
| `cargo test -p assistant-platform-windows` | lib **81** + `handle_discipline` **7** + `synthetic_input_contract` **4** + doctest **1**，**0 failed**（另有 2 个 `#[ignore]` 真机用例） |
| `cargo test --workspace`（Windows） | **32 target / 607 passed / 0 failed**（TASK-017 之后 = 31 / 579） |
| `cargo test -p assistant-core arch::` | **5 passed**（未改 arch 断言） |
| `cargo run -p xtask -- verify-schemas` | PASSED（0 error） |
| `cargo run -p xtask -- codegen --check` | PASSED（0 drift） |
| `cargo run -p xtask -- hygiene` | PASSED（121 文件 / 0e / 3w = 既有 baseline） |
| `cargo run -p xtask -- memory-counts` | PASSED（8 / 0e / 0w） |
| `cargo run -p xtask -- adr-index` | PASSED（0e / 0w） |
| `cargo run -p xtask -- docscan` | PASSED（0e / 563w） |
| `cargo run -p xtask -- card-check` | PASSED（0e / 49w） |
| `cargo run -p xtask -- check-ledger` | PASSED（0e / 0w） |
| `cargo run -p xtask -- check-migrations` | PASSED（0e / 0w） |
| `cargo deny check` | `advisories ok, bans ok, licenses ok, sources ok`（exit 0） |

**真机手工验收**（`cargo test -p assistant-platform-windows --lib -- --ignored --nocapture --test-threads=1`，2026-09-25，本机 Win11 25H2 build 26200.9457 / 主显示器 `\\.\DISPLAY1` scale 2.0）：

```text
pointer_move: display=\\.\DISPLAY1 scale=2 target=(1066, 666) cursor=(1066, 666) error=(0, 0) px
test input::acceptance::test_pointer_move_lands_on_the_requested_physical_point ... ok
notepad: is_ime_open = false
notepad: disk content = "中文abcstart\n"
test input::acceptance::test_unicode_text_and_ctrl_s_round_trip_through_real_notepad ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 81 filtered out
```

解读：① 坐标误差 **0 px**（判据 ≤ 2 px）—— 归一化 + `MOUSEEVENTF_VIRTUALDESK` 路径正确；
② 文本「中文abc」经 `KEYEVENTF_UNICODE` 写入后 `Ctrl+S` 落盘、**从磁盘读回**（后置条件由文件内容证明，不是「看起来成功」）；
③ `is_ime_open = false` 是**第二级查询**（`ImmGetDefaultIMEWnd` + `WM_IME_CONTROL`）的结果 —— 上一会话单级 `ImmGetContext` 对 WinUI 记事本返回 `CapabilityMissing`，本次修复后能拿到真实状态。

### 4. DoD 逐条核对

- [x] `pointer_action` / `key_action` 的 `// STUB(TASK-018):` 占位已被真实实现替换，且两个方法都不再返回 `CapabilityMissing` —— 由 `tests/synthetic_input_contract.rs` 的扫描器（含负向样本 + 文件数下界）与两个行为断言机器校验
- [x] **发送快捷键前 100% 校验前台窗口** —— `ensure_foreground`：`foreground_hwnd() == hwnd` 才发；否则 `bring_to_front`（内部已 `SetForegroundWindow` + 回读）→ 仍不一致 → `TargetUnresponsive`，**不盲发**
- [x] `SendInput` 返回值被逐次校验；UIPI（5）与参数错误（87）可区分且有单测 —— `classify_send_input_failure`（纯函数）+ 4 个单测（5 / 87 / 0 / 未识别码）
- [x] 坐标归一化走 `CoordinateSpace::to_physical`（**纯函数**，不重写换算）；DPI 取**实际**显示器的值 —— `to_physical_on_monitor` 只构造 `CoordinateSpace`；`GetDpiForMonitor(MDT_EFFECTIVE_DPI)` / `GetDpiForWindow` 返回 0 → 报错，**不**退回 96
- [x] `KEYEVENTF_UNICODE` 路径有单测；`is_ime_open()` 有实现 —— `test_unicode_input_uses_scan_code_and_leaves_vk_empty` 断言 `wVk == 0` 与抬起标志；`is_ime_open` 两级查询
- [x] 每个 `unsafe` 块有 `// SAFETY:`；**无任何** `#[allow]` 放宽 —— 全 crate 零 `#[allow]`（唯一例外是 `src/lib.rs` 的 crate 级 `unsafe_code`，TASK-017 DRIFT-017-1 已登记）
- [x] `ResolvedWindow` / `ResolvedElement` 仍**未**派生 `Serialize` / `Deserialize` —— `tests/handle_discipline.rs` 7 个测试全绿
- [x] 非 Windows 平台 `cargo test --workspace` 仍全绿（`#[cfg(windows)]` 门控 + 明确错误；无 `todo!()` / `unimplemented!()`）—— 由两条非宿主 `--target` clippy 覆盖编译（本机无法运行非宿主测试二进制，见 §7）
- [x] 单测含**负向**用例（ADR-0019 N1）—— 至少 7 处：0 DPI、越界归一化、混合 DPI 歧义、空显示器列表、重复修饰键、`DragTo` 缺释放点、未识别 `SendInput` 错误码
- [x] `crates/platform/windows/README.md`「已知限制」已同步 —— 新增 6 条（L4 定位 / 坐标正确≠点到预期窗口 / IME 只做两件事 / `SendInput` 返回值语义 / `KeyTarget::Element` 焦点回读）
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [x] `cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings` 退出码 0
- [x] `cargo clippy --target aarch64-apple-darwin -p assistant-platform-windows --all-targets -- -D warnings` 退出码 0
- [x] `cargo test --workspace` 全绿（32 / 607 / 0）；`cargo test -p assistant-platform-windows` 全绿
- [x] `cargo test -p assistant-core arch::` 仍全绿（5 passed，本卡**未**改 arch 断言）
- [x] `xtask verify-schemas / codegen --check / hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations` 全部 PASSED
- [x] `cargo deny check` 全 ok
- [x] `LEDGER.md` 追加一行；`docs/memory/facts.md` +4、`docs/memory/pitfalls.md` +2（应用专属事实并入 `facts.md` —— 见 §6 的取舍说明）
- [x] 无任何 Out of scope 的文件被修改（write scope 外的连带改动 = `src/lib.rs` 注册两个模块，已登记 **DRIFT-018-1**）

### 5. 偏差

| 编号 | 现象 | 影响 | 处理 |
|---|---|---|---|
| **DRIFT-018-1** | `crates/platform/windows/src/lib.rs` 注册 `pub mod coordinates;` + `pub mod input;` —— **超出卡面 write scope**（卡面只给了 `src/input/**` 与 `src/coordinates/**` 的文件，没给 `lib.rs`） | 不改则两个新模块**永远不会被编译**（Rust 里 `mod` 声明是唯一的挂载点），本卡交付物无法存在 | 已改；**未**改任何既有公共 API / 依赖 / schema；已登记 LEDGER。**建议 Orchestrator**：后续卡面把「新模块的 `lib.rs` 注册」并入 write scope（与 PL-073 同型的「卡面漏了必然要做的一步」） |
| **DRIFT-018-2** | 真机验收模块落在 `src/input/acceptance.rs` 而非 `tests/**`（卡面 write scope 列了 `tests/**`，且 In scope 第 10 项要求「真机手工验收记录」，但没指定模块位置） | 真机验收必须调 Win32（`GetCursorPos` / `SetProcessDpiAwarenessContext`），而 workspace 把 `unsafe_code` 定为 `deny`：**只有本 crate `src/lib.rs` 有 crate 级放开**，`tests/**` 是**独立 crate**拿不到该授权；卡面又禁止 `#[allow]` | 用 `#[cfg(all(test, windows))]` + `#[ignore = "…"]` 放在 lib 内 —— CI 不跑（`--ignored` 才跑），且**不**放宽任何 lint。文件头写明理由。已登记 LEDGER |
| **Q1** | `SendInput` / `ImmGetContext` / `EnumDisplayMonitors` / `GetDpiForWindow` 分属 `windows` crate 的 4 个 feature | 若算「加依赖」则命中漂移触发器 ① | 按卡面**建议默认**：视为**同一已登记依赖的配置**（`windows = "=0.62.2"` 未变），不命中触发器 ①；已在 `Cargo.toml` 与 README 显式声明开了哪 4 个、为什么 |
| **Q2** | 完整 IME（组字 / 候选窗 / 编码转换）超出本卡范围 | 扩范围风险 | 按**建议默认**：只做两件事 —— ① 文本走 `KEYEVENTF_UNICODE`（**绕过** IME）；② `is_ime_open()` 只**报告**状态。**不做**候选窗、**不**自行关 IME（关 IME 会改用户环境） |
| **Q3** | CI 三平台矩阵里只有 `windows-latest` 能跑，且没有真实记事本 fixture（靶机 `notepad-like` 归 TASK-033） | 真机验收无法进 CI | 按**建议默认**：单测**不依赖真实应用**（纯逻辑 + 能力探测）；坐标精度与「IME 开着也能写」改为**人类在本机跑的手工验收**，结果贴在 §3，**不作为 CI 门禁** |
| **状态行** | 本卡 `- 状态：**Ready**` → 改为 `**Done**（2026-09-25）`；该行在**分界线以上**（正文区） | 与 `AGENTS.md` §8「正文区只读」矛盾 | 与 TASK-017 卡**同例**（其状态行也已由 Implementer 改写）；根因即已登记的 **PL-073**（状态行无归属机制）。此处只改**这一个字段**，正文其余部分**一字未动** |
| **两个本地常量** | `MONITORINFOF_PRIMARY`（`0x1`）与 `IMC_GETOPENSTATUS`（`0x5`）在 `windows` 0.62.2 里**未导出** | 不定义就写不出来 | 按头文件值本地定义 + 注释说明来源。**不是**放宽 lint（没有 `#[allow]`），也不是「猜」——两个值都来自 SDK 头文件且被真机验收间接验证 |

### 6. 更合理做法

1. **纯逻辑与 FFI 分离的收益当场兑现**：把 DPI 换算、键名映射、组合键顺序、归一化公式放在**三平台编译**的 `mod.rs`，让 ADR-0045 的两条非宿主 `--target` clippy 真的覆盖到它们（这是 PL-070 教训的第一次落地）。代价是 `coordinates/mod.rs` / `input/mod.rs` 里**重复了 3~5 个错误构造器**（`crate::error` 只在 Windows 编译）—— 取舍：宁可重复 5 行私有函数，也不要为了复用而把整个 `error` 模块变成三平台依赖（那会把 `windows` crate 的 HRESULT 分类拖进非 Windows 构建）。
2. **`SendInput` 失败分类做成纯函数**（`classify_send_input_failure`）而不是内联在 FFI 调用点：单测因此**不需要真机**就能覆盖 UIPI / 87 / 队列阻塞 / 未识别码四条路径（真机上制造 UIPI 需要另一个完整性级别的进程，成本高且不稳定）。
3. **`is_ime_open` 做成两级查询**：单级 `ImmGetContext` 对 WinUI 目标恒失败，会把「查询层次不够」伪装成「平台能力缺失」。两级的代价是 10 行代码，收益是**真实可用的状态**（实测 `false`）。若两级都失败才报 `CapabilityMissing` —— 保持「可解释的失败 > 编造的默认值」。
4. **混合 DPI 的收敛判定用「两次命中同一台」而不是「迭代到不动点」**：迭代法在边界点会震荡（A→B→A→…）而没有终止保证；「主屏试算 → 重算一次 → 比对」是**有界**的（最多 2 次换算），且失败时明确报 `CapabilityMissing`（不猜）。
5. **`VirtualScreen::normalize` 拒绝越界而不是夹边界**：夹边界会让「点到屏幕外」变成「点到屏幕角上」——**看起来成功**但落点错误，正是铁律 1 禁止的静默失败。
6. **本卡把新增事实写进 `docs/memory/facts.md` 而不是新建 `apps/notepad.md` 条目**：两条事实（WinUI 无 Win32 IME 上下文、`SendInput` 坐标公式）**不是记事本专属** —— 前者适用于所有 UWP / WinUI 目标，后者适用于所有 Windows 目标。记事本专属的形状（`RichEditD2DPT` 等）TASK-017 已记。

### 7. 遗留问题

1. **PL-074（新提）**：`pointer_action(point, action)` 的签名不带目标窗口 → 混合 DPI 多屏下逻辑点**无法唯一归属显示器**（本卡用收敛策略，不收敛即 `CapabilityMissing`）；且 `DragTo` 的起点与终点用**同一个** `CoordinateSpace` 换算 → **跨显示器拖拽在混合 DPI 下终点会有偏差**。修法（给签名加 `&ResolvedWindow` 或传 `CoordinateSpace`）改公共接口 → **需 ADR**。已登记 `docs/PARKING_LOT.md`。
2. **非宿主平台的测试执行**：本机只能 `clippy --target`（编译覆盖），**无法运行** Linux / macOS 的测试二进制（没有交叉链接器 / 运行器）。因此「非 Windows 平台 `cargo test` 全绿」这条 DoD 在本机只能由 CI 的 ubuntu / macos job 兑现 —— 本卡已确认两条 `--target` clippy 覆盖到全部纯逻辑与新模块。
3. **`SendInput` 的返回值不等于「目标处理了几个」**：它只是「插入输入队列的事件数」。真正的「结果确实变了」由 TASK-023 的后置断言引擎判 —— 本卡**不**越界实现（README「已知限制」已写明）。
4. **`pointer_action` 抢用户的鼠标**（L4 的本质）：真机验收会移动光标并短暂抢前台焦点。README 已写明调用方必须先试 L1 ~ L3。
5. **`KeyTarget::Element` 的焦点链路只在真机上间接验证**：本卡的记事本验收走的是 `KeyTarget::Window`（+ `Ctrl+S`）。`Element` 路径（UIA `SetFocus` + 回读 `CurrentHasKeyboardFocus`）只有单测覆盖分支，**没有真机用例** —— 建议 TASK-035（Notepad Adapter）把它纳入验收。

6. **真机验收会留下一个记事本窗口（2026-09-25 现场观察 → 已修）**：Win11 25H2 的记事本是**打包 MSIX 应用**，`notepad.exe` 只是启动器存根 —— 一次启动产生**两个**进程，`Child::kill()` 只杀得掉存根，**窗口不会关**。首轮验收后桌面上因此留下了一个记事本（用户报告「弹出过两次 notepad」）。**已修**（本 PR 的补丁提交）：验收改为按**窗口**关（`PostMessageW(WM_CLOSE)`），判据 = 「启动前窗口快照」∩「标题含本次临时文件名」，即使用户自己也开着记事本也不会误关；关不掉时**打印**残留（不静默）。复跑实测 `notepad after: 0`，并打印 `窗口清理完成（无残留） = true`。坑已入 `docs/memory/pitfalls.md`。
7. **PR 合并流程缺陷（2026-09-25 实际发生 → 已登记 PL-075）**：栈式 PR 的 base 依赖人工切回，本次漏做，导致 **PR #19 被合进 base 分支而不是 `main`**（`main` 上从未有 TASK-018，而 GitHub 显示 Merged、CI 全绿）。已用 **PR #20**（`base = main`）补合。修法待裁决，见 `docs/PARKING_LOT.md` PL-075。

### 8. 新增长期记忆

- **FACT ×4**（`docs/memory/facts.md`）：① WinUI 目标没有 Win32 IME 上下文 → `is_ime_open` 必须两级查询（实测第二级返回 0）；② 四个符号的位置与缺失（`SendMessageW` / `WM_IME_CONTROL` 在 `WindowsAndMessaging`；`MONITORINFOF_PRIMARY` / `IMC_GETOPENSTATUS` 未导出；`EnumDisplayMonitors` 返回 `BOOL`；`*_FLAGS` 的 `BitOr` 不是 `const fn`）；③ 新基线 32 target / 607 passed（+28）；④ `SendInput` 绝对坐标标志组合与公式 + 实测 0 px。
- **PITFALL ×3**（`docs/memory/pitfalls.md`）：① `hygiene/missing-card-reference` 会拦**普通 `//` 注释**里裸写的标记词（第 3 次踩到 —— 写规则说明时必须绕开字面量）；② WinUI 上单级 `ImmGetContext` 会把「查询层次不够」伪装成「平台能力缺失」；③ **Win11 的 `notepad.exe` 只是启动器存根** —— `Child::kill()` 关不掉窗口（一次启动 = 两个进程），按进程杀既不够也不安全，应按**窗口**关并加「启动前快照」判据。
- **PL ×2**（`docs/PARKING_LOT.md`）：**PL-074**（`pointer_action` 无目标窗口 → 混合 DPI 归属歧义 + 跨显示器拖拽偏差）；**PL-075**（栈式 PR 的 base 漏切回 → GitHub 会把子 PR 静默合进父分支，`main` 上其实没有那张卡，而 PR 页面照样显示 Merged）。
- **无新增 ADR**（本卡没有改 ADR 已决事项；Q1 ~ Q3 都按「不扩范围」的建议默认执行）。

### 9. 给审阅者的关注点

1. **`ensure_foreground` 是唯一的安全闸门**（`src/input/win32.rs`）：`SendInput` 由**当前前台窗口**接收，所以「以为发给了 A、实际发给了用户正在用的 B」是这里唯一不能容忍的失效模式。请重点核对：① 是否**所有**发送路径（`send_key_action` / `send_unicode_text` / `send_pointer_steps` 前的指针路径）都经过它；② `bring_to_front` 的**回读**是否真的能区分「置前成功」与「系统静默拒绝」。**已知边界**：`pointer_action` **不**做前台校验（它按屏幕坐标点击，签名里没有目标窗口）—— 这是 trait 形状的既有限制，见 PL-074。
2. **混合 DPI 的收敛判定**（`src/coordinates/mod.rs` 的 `coordinate_space_for_logical_point`）：请核对「主屏试算 → 命中 → 重算 → 两次同一台」是否真的**有界**、以及 `CapabilityMissing` 分支是否**不可能**被误当成成功。本机是**单显示器 + 均匀 DPI**，该分支**只有单测**（`mixed_dpi_displays()` 夹具）覆盖，**没有真机证据** —— 这是本卡最大的未验证面。
3. **`KEYEVENTF_UNICODE` 的结构正确性**（`unicode_input`）：`wVk` 必须为 0（否则会**同时**按下一个真实键，在当前布局下可能是快捷键）、key-up 必须同时带 `KEYEVENTF_UNICODE | KEYEVENTF_KEYUP`（否则输入队列残留「按下未松开」状态）。两个不变量的单测断言在 `src/input/win32.rs`。
