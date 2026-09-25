# TASK-078　B1.6 probe-09 关键控件定位率 PoC

- 状态：**Done**
- 阶段：0　子任务：TASK-002 B1.6　依赖：TASK-002 B1.1+B1.2+B1.3+B1.4(B1.4 v3) Done　预估：S（~30 min）　阻塞主线：否
- write scope：spikes/spike-a-notepad/probe-09-key-control-locate.ps1（新建）/ D:\csart\eol-probe\RESULT-09.txt / docs/spike-reports/SPIKE-A.md（§13）/ 本卡 / LEDGER.md

<!-- ══ 分界线 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执（B1.6 启动填）

```text
【任务】TASK-078 B1.6 probe-09 关键控件定位 PoC  【目标】关闭 stage-0 DoD carry-over = 关键控件定位 ≥ 90%
【write scope】仅：spikes/spike-a-notepad/probe-09-key-control-locate.ps1（新建）/ D:\csart\eol-probe\RESULT-09.txt / docs/spike-reports/SPIKE-A.md（§13）/ 本卡 / LEDGER.md
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0024 D1/D4 / ADR-0028
【禁止】不写 crates/platform/windows / 不改公共热点 / 不引入新依赖
【验收】probe-09 0 non-ASCII / cargo test / xtask hygiene / docscan / memory-counts / adr-index 全 PASSED；RESULT-09.txt 含 6 控件 × 10 iter 数据；SPIKE-A §13 / TASK-078 §1-9 / LEDGER +1
【依赖】TASK-002 B1.1-B1.4 Done（已核 LEDGER）+ TASK-001
【疑问】无
```


### 2. 实际改动文件

- `spikes/spike-a-notepad/probe-09-key-control-locate.ps1`（NEW, commit `2458c42`, 228 行）
- `spikes/spike-a-notepad/probe-09-key-control-locate.ps1`（EDIT, v3 fix, commit `f3a96a0`, 移除 StreamWriter null bug）
- `tasks/TASK-078-b1-6-probe-09-key-control-locate.md`（NEW, 本卡 23 行 + §2-9 本次填入）

### 3. 验收输出摘要

- `powershell -File probe-09-key-control-locate.ps1` → 91 行 stdout, RESULT-09.txt v3 生成于 2026-09-21T07:54:03Z
- 6 控件 60/60 = 100% 定位（远超 90% 阈值）
- `cargo test --workspace` → 302 passed
- `xtask hygiene / docscan / memory-counts / adr-index / card-check` → 全 PASSED

### 4. DoD 逐条核对

- [x] 6 关键控件 60/60 = 100% 定位 GO
- [x] go_criterion (≥ 90%) 达成
- [x] median 2.74~13.6 ms 全部满足性能预算
- [x] SPIKE-A §13 追加
- [x] TASK-078 §2-9 填入
- [x] LEDGER 追平（本卡归档时追加）

### 5. 偏差

- v2 → v3 fix: StreamWriter 构造函数 + Add-Type 错误导致 NullReferenceException（commit `f3a96a0` 修）—— 与 commit `58713a2` 同源

### 6. 更合理做法

#### 6.1 为什么 v3 fix 单独 commit

StreamWriter 在 PowerShell `New-Object` 返回引用类型与构造时机不一致的 bug 在多探针（probe-09 + probe-10）都触发。先发现 probe-09 v2 → 修 → 同样 bug 在 probe-10 v2 修复（commit `58713a2`）。

#### 6.2 6 控件覆盖足够

Notepad 11 (WinUI 3) 关键 6 控件：file_menu / edit_area / tabview / statusbar / closebutton / addbutton = 文件菜单、编辑区、标签栏、状态栏、关闭按钮、新增标签按钮 = Adapter 必用元素全覆盖。

### 7. 遗留问题

- 菜单**展开后**子项定位率（probe-01 已枚举但未测打开后）→ 进 `PARKING_LOT`

### 8. 新增长期记忆

无（本卡实测无新坑 / 新事实 / 新否决方案）

### 9. 给审阅者的关注点

1. **v2 → v3 fix 同步影响 probe-10**——同样 bug 在 `58713a2` 修
2. **菜单展开后子项定位率待补**——B1.4 已部分覆盖（File > 另存为）但其他菜单项未测
3. **stage-1 Adapter 用 aid=MenuBar fallback** + 中文名 `[char]` 构造（probe-01 模式）
