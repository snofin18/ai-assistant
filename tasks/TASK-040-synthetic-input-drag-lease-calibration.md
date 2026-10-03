# TASK-040　合成输入完善：拖拽（按下-移动-释放的原子性与租约独占）+ 校准流程（首次显示器组合点击已知元素验证命中）

- 状态：**InProgress（2026-10-03：真机四用例全部真实执行并通过，含「元素点击命中」；仅剩跨层目标 lease 一项 = `PL-101`，超出本卡 write scope）**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：018　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：018　**预估**：M　**难度**：M
- **write scope**：`crates/platform/windows/src/input/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

合成输入完善：拖拽（按下-移动-释放的原子性与租约独占）+ 校准流程（首次显示器组合点击已知元素验证命中）。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/platform/windows/src/input/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-040 合成输入完善：拖拽原子性与校准
【目标】在 input 层补齐拖拽 fail-closed 边界和可单测的多显示器命中校准判定
【write scope】仅 crates/platform/windows/src/input/** + 本卡记录区 + LEDGER / PARKING_LOT / memory / 轮次文件
【铁律】1 无静默失败；5 L4 最后手段；9 不扩大范围；ADR-0028；ADR-0063
【禁止】改 crates/platform/api/**、真实 GUI 操作、lease/policy/task-engine、加依赖、改卡面正文
【验收】fmt / clippy / test --workspace / xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check
【依赖】TASK-018 Done；阶段 1a GO
【疑问】自动化禁止真实 GUI；真实首次点击校准证据和跨层 lease 集成保持 WIP
```

**收口轮（2026-10-03，人类明确要求继续做真机验收）**

```text
【任务】TASK-040 收口剩余真机验收：首次显示器组合校准 + 点击已知元素命中
【目标】在 input/** 内补两个 ignored 真机用例，用真实光标移动/点击取得 expected/observed 与命中证据
【write scope】仅 crates/platform/windows/src/input/** + 本卡记录区 + LEDGER / memory
【铁律】1 无静默失败（跳过必须打印原因）；5 L4 最后手段；9 不扩大范围；ADR-0022；ADR-0063
【禁止】改 crates/platform/api/** 公共 trait / lease / policy / task-engine；加依赖；放宽 lint
【验收】fmt / clippy -D warnings / test --workspace / -p assistant-platform-windows --lib -- --ignored
        --test-threads=1 / xtask 八项门禁
【依赖】TASK-018 Done；阶段 1a GO
【疑问】跨层 lease 仍在 crates/lease / Host 装配，超出本卡 scope → 保持 `PL-101` 未闭环
```

### 2. 实际改动文件

- `crates/platform/windows/src/input/calibration.rs`（新增）
- `crates/platform/windows/src/input/mod.rs`
- `crates/platform/windows/src/input/win32.rs`
- 本卡；`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/memory/{facts,pitfalls}.md`
- `docs/automations/2026-10-02-round-2.md`、`docs/automations/2026-10-02-report.md`

**收口轮追加**

- `crates/platform/windows/src/input/acceptance.rs`：新增 `test_pointer_calibration_covers_the_real_display_set`
  与 `test_pointer_click_focuses_a_known_notepad_element` 两个 `#[ignore]` 真机用例，并修掉
  「DPI 感知每进程只能设一次」导致的夹具假跳过（见 §5 DRIFT-040-2）
- 本卡；`LEDGER.md`、`docs/memory/{facts,pitfalls}.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows`：PASS（lib 88 passed / 2 ignored；
  `handle_discipline` 7 passed；`synthetic_input_contract` 4 passed）。
- `cargo clippy -p assistant-platform-windows --all-targets -- -D warnings`：EXIT 0。
- Linux / macOS `--target` clippy：EXIT 0。
- `cargo run -p xtask -- check-comments`：0 error / 69 warning / PASSED。
- `cargo run -p xtask -- hygiene`：0 error / 102 warning / PASSED（无新增 error）。
- PR #186 / merge `5139557`；CI run `37062416829` = 11/11 SUCCESS。

**收口轮真机证据（2026-10-03，本机 200% 缩放单显示器）**

```text
cargo test -p assistant-platform-windows --lib -- --ignored --nocapture --test-threads=1
  pointer_calibration: samples=3 displays=1 max_error=0 px total_error=0 px tolerance=2 px
  test_pointer_calibration_covers_the_real_display_set ... ok
  pointer_click: element=editor center=(1642, 759) cursor=(1642, 759) focused=true
  notepad_probe: 窗口清理完成（无残留） = true
  test_pointer_click_focuses_a_known_notepad_element ... ok
  pointer_move: display=\.\DISPLAY1 scale=2 target=(1066, 666) cursor=(1066, 666) error=(0, 0) px
  test_pointer_move_lands_on_the_requested_physical_point ... ok
  notepad: is_ime_open = false / disk content = "中文abcstart\n" / 窗口清理完成（无残留） = true
  test_unicode_text_and_ctrl_s_round_trip_through_real_notepad ... ok
  test result: ok. 4 passed; 0 failed; 0 ignored
```

- 校准：每台显示器 3 个内点（1/4、1/2、3/4 宽 × 1/3 高），真实移动 + `GetCursorPos` 回读，
  经 `calibrate_pointer_samples` 判定 —— 本机 `max_error = 0 px`（容差 2 px）。
- 命中：按适配包主选择器（`RichEditD2DPT` + role `Document`）解析真实记事本编辑器，
  读矩形 `(504, 148, 2780, 1370)`、点其中心 `(1642, 759)`，真实单击后该元素
  `HasKeyboardFocus == true`；光标确实落在 `(1642, 759)`。
- `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace`
  全部 EXIT 0；`-p assistant-platform-windows` lib 88 passed / 4 ignored。

**合并与 CI 证据（回填）**

- PR **#194**（`codex/task-040-real-machine-acceptance` → `main`）：CI run `37102249646` = **11/11 SUCCESS**
  （`check` windows / ubuntu / macos、`cargo deny` ×2、doc consistency、desktop-ui checks、
  desktop-ui tauri (windows)、commitlint、gate negative verification #6、xtask deferred inventory）；
  合并前 `mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`、`baseRefName=main`。
- 收口提交 `4f15cc1`，合并提交 **`c86ea63`**（`state=MERGED`）。
- 口径声明：CI **不跑** `#[ignore]` 真机用例 —— 本卡 §3 的真机结论只来自本机原始输出，
  不把它伪装成 CI 门禁（这也是 `PL-106` 想结构化解决的事）。
- 回填走独立分支 `codex/task-040-merge-backfill-2`（原 `codex/task-040-merge-backfill` 是
  PR #186 那轮的旧回填分支）：`LEDGER.md` 只追加一行，不改写已有行。

### 4. DoD 逐条核对

- [x] 新增纯校准判定：3..64 个样本，按显示器 ordinal 去重，任一轴误差超限即 `VerifyFailed`。
- [x] 拖拽拒绝缺 drop 点与零距离 drop 点；拖拽仍编成单个 `SendInput` 批次。
- [x] 部分插入且前缀可能留下左键按下时，按 `SendInput` 返回的插入数条件式补发一次 `LeftUp`。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] xtask 门禁全 PASSED。
- [x] 真实“首次显示器组合点击已知元素”命中证据（2026-10-03）：两个 ignored 真机用例真实执行并
      通过 —— 多显示器组合校准 `max_error = 0 px`；点击编辑器元素中心后 `HasKeyboardFocus = true`
      （见 §3 收口轮证据）。
- [ ] 跨层目标租约独占：归 `crates/lease` / Host 装配，不在本卡 write scope —— 仍未执行，
      保持 `PL-101`；**本卡因此不标 Done**（见 §5 / §7）。

### 5. 偏差

**DRIFT-040-1（真实 GUI 校准与 lease 集成超出自动化边界）**

- 现象：卡片标题包含“首次显示器组合点击已知元素验证命中”和“租约独占”。前者要在真实
  桌面点击已知元素；后者要跨到 `crates/lease` 或 Host 装配，均超出
  `crates/platform/windows/src/input/**`。
- 影响：本轮交付的是可机器验证的校准判定、拖拽参数边界和部分批次恢复；不能声称真实
  点击校准已完成，也不能声称目标 lease 已接入。
- 建议：真实校准作为人工验收项执行；目标 lease 归 TASK-025 / 后续装配卡，并登记 `PL-101`。
- 已停工作：未启动或操作任何真实应用，未改 `crates/platform/api/**` / `crates/lease/**`。

**DRIFT-040-2（真机夹具「假跳过还报 ok」，本轮发现并修复）**

- 现象：本模块多个 `#[ignore]` 真机用例都先调 `SetProcessDpiAwarenessContext(Per-Monitor V2)`。
  该 API **每进程只能生效一次**，于是第一个用例设成功之后，后面的用例拿到
  `ERROR_ACCESS_DENIED (0x80070005)` → 打印 SKIP 后 `return` → 测试仍报 **ok**。
  实测证据：修复前 4 个用例里 2 个走了 SKIP 分支（`test_pointer_move_...` 与新的点击用例），
  真正执行的只有 2 个 —— 也就是说旧的两条坐标类验收在本进程顺序下**可能一次都没跑**。
- 影响：SKIP 与 PASS 在报告里无法区分（与铁律 1 的「无静默失败」冲突），也解释了为什么
  此前「真机验收通过」的结论偏弱。
- 处置：把 DPI 声明改成**进程内一次性确认 + 缓存**（`OnceLock`），并在设置失败时用
  `GetThreadDpiAwarenessContext` + `GetAwarenessFromDpiAwarenessContext` **回读**当前档位：
  已是 Per-Monitor Aware → 视为成功；是别的档位 → 显式报错。修复后 4 个用例全部真实执行。
- 未处理（留给后续）：SKIP 仍然以 `ok` 结束（Rust test 框架没有「跳过」状态）；如需机器区分
  PASS / SKIP，得改成「输出结构化 JSON 供 CI 解析」，那超出本卡 scope → 登记 **PL-106**。

**DRIFT-040-1 现状（部分闭环）**：真机校准与命中证据已补齐（本卡 §3）；剩余部分只剩
「跨层目标租约独占」，它要动 `crates/lease` / Host 装配 —— 不属于本卡 write scope，
继续留在 `PL-101`。

### 6. 更合理做法

把“校准”拆成纯判定层与人工取证层：纯判定层在 CI 中稳定覆盖坐标系、显示器和误差阈值；
真实点击只采集 expected/observed 样本，由人工或受控真机验收喂给同一个纯函数。这样既保留
可回放性，又不让无人值守流程去操作用户桌面。

### 7. 遗留问题

- **PL-101**：跨层目标 lease 集成仍待后续卡（真实首次校准样本已由本轮补齐，见 §3）。
- **PL-106**（新提）：真机用例的 SKIP 与 PASS 在测试报告里无法区分；建议后续卡把验收结果
  输出成结构化记录供 CI 解析，避免「假跳过还报 ok」再次发生（本轮的 DPI 修复只消掉了这一条
  具体诱因，没有消掉这个报告形态）。
- `PL-074` 的混合 DPI 无目标窗口问题仍在；本卡没有改公共 `pointer_action` 签名。

### 8. 新增长期记忆

- FACT：input 层新增有界 `calibrate_pointer_samples`，3..64 个样本、按显示器 ordinal 计数，
  任一轴误差超限即 `VerifyFailed`。
- PITFALL：`SendInput` 部分插入时不能因为看到过 `LeftDown` 就盲发 `LeftUp`；必须先按
  返回的插入数判断前缀是否可能仍处于按下状态，否则可能误释放用户正在按的鼠标键。
- FACT（收口轮）：真机 `assistant-platform-windows` ignored 全套（串行）4 passed —— 多屏校准
  `max_error = 0 px`（本机 200% 缩放）、点击编辑器矩形中心后该元素 `HasKeyboardFocus = true`、
  `pointer_action(Move)` 误差 0 px、真实记事本 Unicode + Ctrl+S 落盘回读一致且无窗口残留。
- PITFALL（收口轮）：`SetProcessDpiAwarenessContext` **每进程只能设一次**；多真机用例共用同一
  测试进程时，后来的用例会拿到 `ERROR_ACCESS_DENIED`，若照旧 `return` 就会被报告成 `ok`
  （假跳过）。修法是进程内 `OnceLock` 缓存 + `GetThreadDpiAwarenessContext` /
  `GetAwarenessFromDpiAwarenessContext` 回读确认档位。

### 9. 给审阅者的关注点

- 审阅 `pointer_requires_release_recovery` 的“已插入前缀”判据是否足够保守。
- 审阅校准的 per-axis max 判定和 64 样本上限是否符合后续 Paint 坐标需求。
- 本 WIP **仍不应标 Done**：跨层目标 lease 未接入（`PL-101`，超出本卡 scope）；真实校准与
  点击命中证据已补齐，请核对 §3 的原始输出。
- 请重点看 **DRIFT-040-2**：真机夹具此前有「跳过还报 ok」的形态，本轮修掉了 DPI 这一条诱因；
  若你认为还需要把 SKIP/PASS 结构化（`PL-106`），请指示是否另开卡。
