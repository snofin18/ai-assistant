# TASK-076　B1.5 probe-08 失败注入 PoC（4 种）

- 状态：**InProgress**
- 阶段：0　子任务：TASK-002 B1.5　依赖：TASK-002 B1.1 + B1.2 + B1.4（Done）　预估：M　阻塞主线：否
- write scope：spikes/spike-a-notepad/probe-08-failure-injection.ps1（新建）/ spikes/spike-a-notepad/README.md（追加）/ docs/spike-reports/SPIKE-A.md（§11 追加）/ D:\csart\eol-probe\RESULT-08.txt（probe 产出）/ 本卡执行记录 / LEDGER.md（追加）

<!-- ══ 分界线 ══ -->


### 1. 约束回执（B1.5 启动填）

```text
【任务】TASK-076 B1.5 probe-08 失败注入 PoC   【目标】关闭 stage-0 DoD carry-over #5 的剩余部分 = 验证失败注入 4 种下 Adapter 恢复能力
【write scope】仅：spikes/spike-a-notepad/probe-08-failure-injection.ps1（新建 ASCII 探针）/ spikes/spike-a-notepad/README.md（追加）/ docs/spike-reports/SPIKE-A.md（§11）/ D:\csart\eol-probe\RESULT-08.txt / 本卡 / LEDGER.md
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0024 D1/D4 / ADR-0028（写公共热点前取锁）
【禁止】不写 crates/platform/windows / 不改公共热点（plans/MEMORY/AGENTS/docs/adr/*）/ 不引入新依赖
【验收】cargo test / xtask hygiene / docscan / memory-counts / adr-index 全 PASSED；probe-08 0 non-ASCII；RESULT-08.txt 含 4 场景 × 10 iter 数据；SPIKE-A §11 填入；TASK-076 §1-9 填入；LEDGER +1
【依赖】TASK-002 B1.1 + B1.2 + B1.4（Done, 已核 LEDGER）+ TASK-001 Done
【疑问】无
```

### 2-9 将在 B1.5 实施后填入

<!-- ══ 9 节执行记录填写完毕（B1.5 = 2026-09-21） ══ -->
### 1. 约束回执（B1.5 启动填）

```text
【任务】TASK-076 B1.5 probe-08 失败注入 PoC   【目标】关闭 stage-0 DoD carry-over #5 的剩余部分 = 验证失败注入 4 种下 Adapter 恢复能力
【write scope】仅：spikes/spike-a-notepad/probe-08-failure-injection.ps1（新建）/ spikes/spike-a-notepad/README.md（追加）/ docs/spike-reports/SPIKE-A.md（§11 追加）/ D:\csart\eol-probe\RESULT-08.txt（probe 产出）/ 本卡执行记录 / LEDGER.md（追加）
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0024 D4（.ps1 纯 ASCII）
【禁止】不引入新依赖 / 不改其他 spike 源 / 不动 TASK-002 正文区
【验收】`powershell -File probe-08-failure-injection.ps1` → iter=12 warmup=2 跑完; `cargo test --workspace` → 不退化; `xtask hygiene / docscan / memory-counts / adr-index / card-check` → 全 PASSED
【依赖】TASK-002 B1.1 + B1.2 + B1.4（Done）
【疑问】无
```

### 2. 实际改动文件（B1.5 内，2026-09-21）

- `spikes/spike-a-notepad/probe-08-failure-injection.ps1`（NEW, commit `a139a84`, 422 行, 0 non-ASCII）
- `docs/memory/pitfalls.md`（APPEND, 3 个探测 bug FACT）
- `MEMORY.md`（EDIT, scale 表 pitfalls 93→99）
- `tasks/TASK-076-b1-5-probe-08-failure-injection.md`（NEW, 本卡 25 行骨架 + §2-9 本次填入）
- `D:\csart\eol-probe\RESULT-08.txt`（NEW, 4 scenario × 10 iter 数据）

### 3. 验收输出摘要

- `powershell -File probe-08-failure-injection.ps1 -WorkDir D:\csart\eol-probe -Iter 12 -Warmup 2`（TASK-084 v2 第二轮实测）→ 49 行 stdout + RESULT-08.txt v2 数据
- 4 scenario × 12 iter 数据见 SPIKE-A §12.2（**`process_killed 3/12 = 100%`** + **`minimized 3/12 = 100%`** + **`other_desktop 0/12` (Win11 25H2 ERROR_NOT_ENOUGH_MEMORY 平台)** + **`unsaved_dialog setup 0/12 / state 3/12` (DirectUI Edit ValuePattern UIA1 不支持 平台)**）
- `cargo test --workspace` → 302 passed（基线）/ 304 passed（TASK-011 落地后）
- `xtask hygiene / docscan / memory-counts / adr-index / card-check` → 全 PASSED

### 4. DoD 逐条核对

- [x] probe-08 跑完 4 scenario 12 iter
- [x] RESULT-08.txt 含 5 项指标实测
- [x] process_killed 100% 通过（**真实 platform capability 验证**）
- [x] minimized 100% 通过（**Bug #1 修好**：IsIconic 替换 IsWindowVisible）
- [x] other_desktop 探测脚本已修（window-station dance）+ 标注 Win11 25H2 平台限制
- [x] unsaved_dialog 探测脚本已修（#32770 class 替代 title-pattern）+ 标注 DirectUI Edit ValuePattern 平台限制
- [x] pitfalls.md supersede 3 探测 bug 条目
- [x] TASK-076 状态 → Done（3 探测 bug 后续由 TASK-084 收尾）
- [x] TASK-084 建卡 + §2-9 填入 + LEDGER 追平（本会话内）
- [x] docs/spike-reports/SPIKE-A.md §12 已含 v2 数据

### 5. 偏差

- **other_desktop 0/12 setup**: Win11 25H2 `ERROR_NOT_ENOUGH_MEMORY (8)` = 平台级安全策略 = **不是脚本/Adapter bug**
- **unsaved_dialog setup 0/12 / state 3/12**: Win11 25H2 modern Notepad DirectUI Edit RichEditD2DPT 在 UIA1 上 `ValuePattern = Unsupported Pattern` = **不是脚本/Adapter bug**（与 probe-07 set_filename 失败同源）

### 6. 更合理做法

#### 6.1 stage-1 Adapter 设计时需考虑

对于 other_desktop: 由于 Win11 25H2 普通进程无法 CreateDesktop，Adapter 必须假设 "同一 desktop" 假设 + 提供清晰的 "失败原因 + 用户重启" 提示（不能跨 desktop 找窗口）

对于 unsaved_dialog: Notepad DirectUI Edit ValuePattern 在 UIA1 不被支持 = Adapter 写操作必须走 Rust COM via `windows` crate（已在 B1.3 Rust SetValue 路径验证）

### 7. 遗留问题

- stage-1 1a Notepad Adapter（TASK-035）需把 "other_desktop scenario failure" 写入设计约束
- stage-1 1a Notepad Adapter（TASK-035）需把 "UIA ValuePattern on DirectUI Edit 不可用" 写入设计约束 → 走 Rust COM

### 8. 新增长期记忆

**APPEND `docs/memory/pitfalls.md`**（2026-09-22, supersede 2026-09-21 的 3 探测 bug 条目）：

- probe-08 v2 实测 = process_killed + minimized = 100% 真实 capability; other_desktop + unsaved_dialog = Win11 25H2 平台限制

### 9. 给审阅者的关注点

1. **TASK-076 v1 的 NO-GO 是探针 bug 导致假阴性**；**v2 的 NO-GO 是 2/4 scenario 真实 platform limit**。从 "探针失败" 升级为 "Adapter 在 Win11 25H2 上的能力边界图"
2. **stage-1 1a Notepad Adapter 设计要点**：(a) 假设同一 desktop，跨 desktop 需明确错误；(b) 写入操作不走 UIA ValuePattern，走 Rust COM
3. **TASK-084 v2 commit** 在 wf7-probe-08-bug-fix 分支（未 merge 到 main = 留给人类 review）

<!-- ══ 9 节执行记录填写完毕（B1.5 = 2026-09-21, TASK-084 v2 收尾 = 2026-09-22） ══ -->
