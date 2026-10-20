# TASK-046　T3.3：绘制 → 用户点撤销 → **像素级验证回到快照**

- 状态：**Done**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：045　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：045　**预估**：M　**难度**：M
- **write scope**：同上
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

T3.3：绘制 → 用户点撤销 → **像素级验证回到快照**。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

同上

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
【任务】TASK-046 T3.3：绘制 -> 撤销 -> 像素级回到快照
【目标】挂载并验证 Paint `paint.canvas.rollback_last_write`，确认一次画布动作可由 L0 `Ctrl+Z` 回到 pre snapshot
【write scope】adapters/com.microsoft.paint/tasks/**、eval/tasks/paint/**；
              apps/agent-core/src/paint_handlers.rs、paint_handler_map.rs、paint_handler_support.rs、
              paint_registry.rs、task_package_render.rs、notepad_registry.rs（仅子集测试）；
              apps/agent-core/tests/production_paint.rs、tests/task_package*.rs、tests/support/paint_real_run*.rs；
              收尾时按 AGENTS.md 更新本卡记录区、LEDGER、状态文件与 paint 应用记忆
【铁律】无静默失败；写操作必须有 postcondition；Ctrl+Z 必须显式 Adapter 声明且前台匹配；
        不改公共 schema/trait/ErrorCode；不把撤销失败报成成功
【禁止】新增第三方依赖；改 platform/api trait；放宽 900 行/歧义/视觉门禁；
        用未持久化像素冒充锚点；在 Paint 不在前台时发送 Ctrl+Z
【验收】format/clippy/workspace；adapter validator；task package tests；
        T3.3 fake 正负例；真实 ignored 单次与十次；xtask 全套
【依赖】TASK-045 Done（313af12）；套件已有 rollback schema / app_map / recipes；已核对 LEDGER
【疑问】TASK-046 正文仍是占位；默认按现有 schema + app_map 自裁决，记录 DRIFT-046-1
```

### 2. 实际改动文件

- `adapters/com.microsoft.paint/tasks/t3.3.open-png-draw-rollback.json`
- `adapters/com.microsoft.paint/tasks/README.md`
- `eval/tasks/paint/t3.3/**`
- `apps/agent-core/src/paint_handlers.rs`
- `apps/agent-core/src/paint_handler_actions.rs`
- `apps/agent-core/src/paint_handler_map.rs`
- `apps/agent-core/src/paint_handler_support.rs`
- `apps/agent-core/src/paint_registry.rs`
- `apps/agent-core/src/task_package_render.rs`
- `apps/agent-core/src/notepad_registry.rs`（仅 Paint mounted-tool 子集断言）
- `apps/agent-core/tests/production_paint.rs`
- `apps/agent-core/tests/task_package.rs`
- `apps/agent-core/tests/support/paint_adapter_copy.rs`
- `apps/agent-core/tests/support/paint_real_run_fake.rs`
- `apps/agent-core/tests/support/paint_real_run_t3_3.rs`
- `crates/platform/windows/src/win32.rs`（`DRIFT-046-2` 批准的 popup 判定修复）
- `MEMORY.md`（`DRIFT-046-3` 批准的 Paint 档案行数与路由同步）

### 3. 验收输出摘要

```text
python eval/tasks/paint/t3.3/validate.py
-> paint_t3_3_static_validation_ok ... tools=11 cases=8

cargo test -p assistant-agent-core --test task_package
-> 20 passed / 0 failed

cargo test -p assistant-agent-core --test production_paint test_paint_t3_3 -- --nocapture
-> 2 passed / 0 failed / 1 ignored（fake 正例 + no-op undo fail-closed）

cargo test -p assistant-platform-windows win32::tests
-> 11 passed / 0 failed（含 stale flyout / visible flyout / invisible modal 三态）

run-ten.ps1 -Runs 1（修复后单次真机）
-> 1 passed / 0 failed；exit 0；64.953 s

run-ten.ps1 -AllowClosePaint（正式十次真机）
-> 10 passed / 10 runs；success_rate=1.0；平均 60.801 s
-> expected_undo_steps=1 与像素回滚断言每轮均通过
```

十次原始日志在 `target/paint-t3-3-ten-run/run-01.log` 至 `run-10.log`；
冻结摘要为 `eval/tasks/paint/t3.3/ten-run-summary.json`。

最终门禁：

```text
cargo fmt --all --check
-> exit 0

cargo clippy --all-targets -- -D warnings
-> exit 0（仅仓库既有 unknown lint `clippy::assert_is_empty` 警告）

cargo test --workspace
-> exit 0；所有非 ignored 测试与 doctest 通过

xtask verify-schemas / codegen --check / hygiene / memory-counts /
adr-index / refscan / docscan / card-check / check-ledger / check-comments
-> 全部 PASSED

cargo deny check
-> advisories / bans / licenses / sources 全部 ok

cargo run -p xtask -- replay --suite core
-> 2 steps / 5 changes / 0 mismatch / PASSED
```

### 4. DoD 逐条核对

- [x] card 标题声明的能力可被 fake 正负例、task-package contract 与真实 Paint 十次用例覆盖
- [x] `cargo fmt --all --check` 0 diff
- [x] `cargo clippy --all-targets -- -D warnings` exit 0
- [x] `cargo test --workspace` 全绿
- [x] `xtask verify-schemas / codegen --check / hygiene / memory-counts / adr-index /
      refscan / docscan / card-check / check-ledger / check-comments` 全部 PASSED
- [x] `cargo deny check` 与 `xtask replay --suite core` PASSED
- [x] `LEDGER.md` 已追加；`docs/memory/apps/paint.md` 与 `MEMORY.md` 路由/规模已同步

### 5. 偏差

`DRIFT-046-1`

- **触发器**：#3（公共工具已声明但 handler/assertion 未挂载）、#5（write scope 写“同上”且
  正文步骤仍是占位）、#9（测试/实现需要复用现有 Paint runtime 分层）。
- **现象**：TASK-046 卡面未给出实现步骤或完整 write scope；但 `tools.json` 已经声明
  `paint.canvas.rollback_last_write`，`app_map.json` 已声明 Paint L0 undo 能力为
  `Ctrl+Z`、前台校验与 pixel snapshot fallback。
- **影响**：不扩权到 Agent Core 的 Paint registry/handler/assertion/test 文件，就无法把
  已声明工具真正挂载并验证；只改 adapter task/eval 会得到 `MissingAssertion` 或 unknown tool。
- **自裁决**：按现有 schema 和 ADR/architecture §9 实施；仅新增该工具的私有 handler、
  assertion table 与测试接线，不修改工具 schema、platform trait 或 ErrorCode。所有写动作
  先校验 Paint 前台；`Ctrl+Z` 后以 pre snapshot + after capture 的 visual observation
  做强验证，失败显式 `VerifyFailed`。

`DRIFT-046-2`

- **触发器**：#5（write scope 未列平台层）。
- **现象**：真机 undo 前得到 `foreground=true, occluded=true`。诊断确认 Paint 会长期保留
  已关闭 flyout 的 `Microsoft.UI.Content.PopupWindowSiteBridge` 句柄；旧
  `has_blocking_popup()` 只按类名命中，误把隐藏 flyout 当成未保存模态框。
- **影响**：不放宽 undo 前台 / 无遮挡门禁就无法完成 T3.3；如果简单删除
  `PopupWindowSiteBridge` 检测又会破坏 TASK-306 的未保存模态框 fail-closed。
- **自裁决**：把 `crates/platform/windows/src/win32.rs` 纳入本次最小必要范围，不改公共
  trait / schema / ErrorCode。判据改为“popup 当前可见，或主窗口已禁用时的不可见模态
  popup”。新增三态单测，并用正式十轮真机验证不再误报。

`DRIFT-046-3`

- **触发器**：#5（`MEMORY.md` 属 Orchestrator 维护面，不在 Implementer write scope）。
- **现象**：`docs/memory/apps/paint.md` 记录 TASK-046 事实后从 449 行增至 479 行；
  `cargo run -p xtask -- memory-counts` 报 `MEMORY.md` §1 规模表仍是 449。
- **影响**：不更新派生计数，记忆规模门禁失败；更新它又超出普通 Implementer 写权限。
- **自裁决**：只更新 `MEMORY.md` 中 `apps/paint.md` 行数 `449 -> 479` 和同一路由描述，
  不改变 L0 结构、其它规模值或任何项目决策。`memory-counts` 复跑 0 error；现存
  `archive-threshold` warning 是改动前已存在的 449 > 400 状态，留给独立归档卡处理。

`DRIFT-046-4`

- **触发器**：#5（流程偏差，晚于第一次 lock 释放才更新 `LEDGER.md` 最终文案）。
- **现象**：第一次热点文件批次结束后已释放 `LEDGER.md` 锁；随后补执行记录时直接更新了
  同一行，之后才重新取锁。
- **影响**：本次窗口内没有检测到其它持有者，也没有覆盖他人内容；但“先写后锁”违反
  ADR-0028，不能因为结果无冲突就当作合规。
- **自裁决**：立即重新 `guard acquire LEDGER.md`，把偏差写进本卡与台账；后续任何热点
  文件写入前必须重新取锁。当前只保留这一行说明，不尝试回滚或重写已提交给他人的条目。

### 6. 更合理做法

- tool 的公开契约已存在，本次没有新增第二套 undo 机制；继续复用
  `paint.canvas.rollback_last_write`、既有 visual observation 与 task-package assertion。
- pre snapshot 必须来自 capture 步骤的真实 `pre_snapshot_blob_id`；禁止拿撤销后的当前画面
  冒充 reference。
- undo 前先对已解析的 Paint 窗口执行有界置前并回读；仍不满足 foreground + unoccluded
  才返回 `UserInteraction`，避免控制台瞬时遮挡导致假失败，同时保留发键的最终前台校验。
- `production_paint.rs` 因新增 T3.3 真机入口触及 900 行硬上限后，把与具体用例无关的
  adapter copy helper 拆到 `tests/support/paint_adapter_copy.rs`；主文件降到 899 行，
  没有放宽 hygiene 阈值。

### 7. 遗留问题

- `paint.canvas.rollback_last_write` 目前限定单次任务显式声明的 `expected_undo_steps`
  `1..=64`；T3.3 标准拖拽在 Paint 11.2605.81.0 上实测为 1 步，其他 Paint 动作的粒度仍需
  各自用像素 snapshot 取证。
- 撤销后像素匹配容差仍为 `mean_abs_diff <= 2`、`confidence >= 0.9`；其他 DPI / 多屏 /
  主题下未单独做十次。
- 生产入口默认仍是 T3.2；T3.3 通过显式 task package / ignored 真机 runner 执行。

### 8. 新增长期记忆

- Paint 11.2605.81.0 对“一次矩形 drag”执行一次 `Ctrl+Z` 即回到 pre snapshot；T3.3 真机
  `10/10`。
- `PopupWindowSiteBridge` 句柄会在 flyout 关闭后残留；平台判定必须结合
  `IsWindowVisible` 与 owner disabled 状态，不能只看类名。
- `PopupWindowSiteBridge` 生命周期与 Paint undo 粒度已同步进
  `docs/memory/apps/paint.md` §4 / §6 / §8 / §13.24 与 `MEMORY.md` 路由。

### 9. 给审阅者的关注点

1. `has_blocking_popup()` 的新三态逻辑是否同时覆盖“隐藏 stale flyout 不阻塞”和“不可见
   模态 popup + disabled owner 仍阻塞”。
2. visual observation 是否确实读取 `pre_snapshot_blob_id`，并由 `value_equals` +
   `mean_abs_diff_within` 双断言约束 undo 步数和像素结果。
3. 十轮结果是否每轮都带 `1 passed / 0 failed`，且 no-op undo 负例仍显式失败。
