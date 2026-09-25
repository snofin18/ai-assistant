# TASK-082　补漏：4 个 fix commits 的 LEDGER 行追平

- 状态：**Done**
- 阶段：0　子任务：事实源追平（无业务行为变化）　依赖：none　预估：XS（~10 min）　阻塞主线：否
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。
- 阶段级信息见 `plans/stage-0-spikes.md`。

**write scope**：本卡 + LEDGER.md（追加 4 行）

**背景**

2026-09-22 用户会话指出：上一轮 6 commits 中 4 个 fix commits（`f0cb279` wf3 card-check finding 输出 / `01f8bf5` patch-e7-warnings / `4b046bc` wf2 TASK-074 撞号 / `a61d6bf` wf4 新建 080/081）的 LEDGER 行没补。事实源失同步。

**根因**：上一轮 6 commits 全在 worktree 分支上没 merge，main HEAD 仍为 `f3a96a0`；补 LEDGER 行 = 在事实源追平这些 commits 的存在性，与是否 merge 无关（commit hash 字段是稳定锚点）。

**补漏清单（按 commit 时间序）**

| commit | 任务 | 摘要 |
|---|---|---|
| `f0cb279` | wf3 card-check finding 输出 | `xtask/src/card_check.rs` run() 末尾补 `for f in &findings { f.render() }`，与 docscan/hygiene 对齐 |
| `01f8bf5` | patch-e7-warnings | `spikes/spike-a-notepad/src/bin/uia_dep_proof.rs` E7 5 个 camelCase warning + 1 unused 修完，E7 PASS 2ms |
| `4b046bc` | wf2 F-1 撞号 | TASK-074-f1 → TASK-083-f1 卡号 + 4 处引用更新（MEMORY.md / TASK-073 / audit doc / LEDGER.md）+ card-check 现在能找到 finding |
| `a61d6bf` | wf4 新建 080/081 | `tasks/TASK-080-spi.md` + `tasks/TASK-081-setvalue.md` Ready 占位卡（XTASK 池 080/081） |

**注意**：`a61d6bf` 含 cherry-pick 自 `01f8bf5` + `4b046bc`，所以本卡 LEDGER 追加 4 行（不去重 = LEDGER 是事件流不是去重流）

**验收命令**
```powershell
git log --oneline -1 LEDGER.md   # 确认本卡 commit
grep -c '^| 2026-09-22' LEDGER.md # 验证 4 行已追加
```

**DoD**
- [ ] LEDGER.md 追加 4 行（一行一 commit hash）
- [ ] 本卡 §2-9 填入
- [ ] xtask hygiene / card-check 全 PASSED

<!-- ══ 分界线 ══ -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-082 补漏 LEDGER 行
【write scope】仅：LEDGER.md / tasks/TASK-082-*.md
【铁律】AGENTS.md §3 / ADR-0028 / ADR-0031
【禁止】不改其他文件 / 不动公共热点正文 / 不重置漂移
【验收】git log -1 LEDGER.md 含新 commit + grep 验证 4 行追加 + xtask 全绿
【依赖】none（commit hash 已是稳定锚点）
【疑问】❶ cherry-pick 重复是否去重？答：不 = LEDGER 是事件流。
```

### 2. 实际改动文件

- `tasks/TASK-082-ledger-catchup.md`（NEW，本卡）
- `LEDGER.md`（APPEND 4 行）

### 3. 验收输出摘要

见下方 commit 信息（git log -1 LEDGER.md）

### 4. DoD 逐条核对

- [x] LEDGER.md 追加 4 行
- [x] 本卡 §2-9 填入
- [x] xtask 全绿

### 5. 偏差

none

### 6. 更合理做法

**事实源 vs merge**：补漏 = 补记录，不动 main HEAD。merge 是后续 review 流程（本会话内不做）。

### 7. 遗留

- 6 个 worktree commits 是否合并 = 留给人类 review
- TASK-011 ErrorCode 完整 Rust 枚举 = 留给下会话
- TASK-012/013/014/015 stage-1 续 = 留给下会话

### 8. 新增长期记忆

无

### 9. 给审阅者

1. LEDGER 现在 = 事件流 = 包含历史 cherry-pick 重复 = 审计可追溯；commit hash 字段仍是稳定锚点
2. 本会话内合并后 commits → 留给下个会话去开 master fast-forward
