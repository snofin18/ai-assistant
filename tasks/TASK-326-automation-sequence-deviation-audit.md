# TASK-326　2026-10-20 自动化自删顺序偏差审计

- 状态：**Done**
- 阶段：1　子阶段：跨阶段治理　批次：治理池　依赖：TASK-325　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- **write scope**：
  - `tasks/TASK-326-automation-sequence-deviation-audit.md`
  - `docs/audits/automation-sequence-deviation-2026-10-20.md`
  - `docs/automations/2026-10-20-report.md`（只追加事后更正）
  - `plans/stage-1-pilots.md`（仅新增任务行与当前进度）
  - `PLAN.md`（仅当前状态块）
  - `README.md`（仅状态行、`## 当前阶段`、`## 最近进展`）
  - `LEDGER.md`（只追加）
- **关联**：ADR-0031、ADR-0039、ADR-0041、ADR-0054、ADR-0083、ADR-0089

**目标**

根据外部 automation JSONL 与 GitHub merge 时间，审计 2026-10-20 五轮 automation
中“先自删、后落地最终证据校正”的顺序偏差，并在仓库内形成可追溯的更正记录。

**In scope**

- 核对 5 个 automation 的触发、PR、CI、merge、lock release 与 self-delete 时间。
- 确认所有 automation 均已执行、自删且无 open PR / 残留锁。
- 找出 self-delete 之后仍有仓库留痕落地的轮次。
- 追加运行日报告更正，新增独立审计报告。
- 在 `LEDGER.md` 只追加更正事件。
- 仅同步计划、`PLAN.md`、`README.md` 允许写入的当前状态区域。

**Out of scope（做了算漂移）**

- 任何产品代码、测试、schema、公共接口、ADR、spec 或依赖改动。
- 修改历史 LEDGER 行、历史 round 正文或历史任务卡正文。
- 重新运行 automation、重新发送通知或删除外部 JSONL 日志。
- 将 Stage 1c 标记完成或执行真实浏览器登录。

**必须遵守**

- 只依据外部 JSONL 与 GitHub merge 事实，不凭聊天印象。
- 历史记录只追加；更正使用 `[supersedes:日期]` 语义。
- 公共热点文件写前取 guard，写完立即释放；不得使用 `--force`。
- 计划文件只新增任务行与当前进度，不改既有条目正文（ADR-0041）。
- 每条 `exec_command` 必须显式声明 `shell: "powershell"`。

**验收命令**

```powershell
git diff --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- check-ledger
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
```

**完成定义（DoD）**

- [ ] 两次 self-delete 后落地的证据校正均有精确时间线与 merge hash。
- [ ] 运行日报告以追加方式更正“偏差与遗留已收口”的旧结论。
- [ ] LEDGER 追加 TASK-326 事件，不改历史行。
- [ ] 5 个 automation 的最终残留状态有证据。
- [ ] 文档门禁全部通过。
- [ ] 无产品代码、测试、schema、ADR、spec 或依赖改动。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-326 2026-10-20 自动化自删顺序偏差审计
【目标】把两次 self-delete 后落地证据校正的事实写入仓库审计与运行日更正
【write scope】仅：本卡、审计报告、运行日报告追加、计划/PLAN/README/LEDGER
【铁律】证据不伪造；历史只追加；热点文件取 guard；N_auto>5 只做 docs-only；禁用产品代码
【禁止】修改历史行、产品代码、测试、schema、ADR、spec、依赖、真实 GUI
【验收】git diff --check；xtask hygiene/card-check/docscan/check-ledger/refscan/memory-counts/adr-index
【依赖】TASK-325 Done；五轮 automation 外部日志与 PR #500~#511 已存在
【疑问】无
```

### 2. 实际改动文件

- `tasks/TASK-326-automation-sequence-deviation-audit.md`
- `docs/audits/automation-sequence-deviation-2026-10-20.md`
- `docs/automations/2026-10-20-report.md`（只追加事后复核）
- `plans/stage-1-pilots.md`（新增 TASK-326 行与当前进度）
- `PLAN.md`（当前状态块）
- `README.md`（允许三处）
- `LEDGER.md`（只追加）

### 3. 验收输出摘要

- 外部 JSONL：5/5 存在且每行均可解析；无格式损坏。
- automation 目录：只剩 `.run-jitter-salt`，5 个任务均已自删。
- `.automation.lock`：不存在。
- open PR：0。
- `main`：`b8e4eb730f920c7057ce249618bfaae50b9319e4`，工作树干净。
- `git diff --check` → exit 0。
- `git diff --name-only` → 仅 5 个已跟踪文档文件；2 个审计文件为新增未跟踪文件。
- `cargo run -p xtask -- hygiene` → PASSED；438 files，0 error，136 existing warnings。
- `cargo run -p xtask -- card-check` → PASSED；220 files，0 error，35 existing warnings。
- `cargo run -p xtask -- docscan` → PASSED；528 files，0 error，229 existing warnings。
- `cargo run -p xtask -- check-ledger` → PASSED；`plan_date=2026-10-21`、
  `ledger_last_date=2026-10-21`、`current_stage=阶段 1`。
- `cargo run -p xtask -- refscan` → PASSED；995 files，0 error，0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED；9 files，0 error，1 existing archive-threshold warning。
- `cargo run -p xtask -- adr-index` → PASSED；87 files，0 error，0 warning。

### 4. DoD 逐条核对

- [x] 两次 self-delete 后落地的证据校正均有精确时间线与 merge hash。
- [x] 运行日报告以追加方式更正旧结论。
- [x] 5 个 automation 的最终残留状态有证据。
- [x] 未修改历史 LEDGER 行、round 正文或既有任务卡正文。
- [x] 文档门禁 hygiene / card-check / docscan / check-ledger / refscan / memory-counts / adr-index 全部通过。
- [x] 无产品代码、测试、schema、ADR、spec 或依赖改动。

### 5. 偏差

- 本轮审计没有新增产品偏差。
- 被审计事实中的 `DRIFT-AUTO-20261020-1` 与 `DRIFT-AUTO-20261020-2` 见审计报告 §4。
- 本轮有 5 条只读门禁调用误用 `shell: "bash"`，均在 bash 中因缺少 `cargo` 而失败；
  随后全部以显式 PowerShell 重跑，未修改仓库内容。

### 6. 更合理做法

最终收尾顺序必须拆成两段硬门：

1. 所有主 PR、回填 PR、更正 PR 已 merged；
2. 再做 final verification，随后 lock release，最后 self-delete。

任何 self-delete 后仍需要写仓库内容的场景，都应视为 automation 未完成，而不是继续补一个“事后更正”来伪装收口。

### 7. 遗留问题

- Stage 1c 仍需真实专用浏览器 profile 与人工登录/安全验收。
- 本卡只修正 automation 证据链，不改变 Stage 1 产品范围。

### 8. 新增长期记忆

- 无新增通用规则；本卡把现有 ADR-0054 的违反事实与修复建议落到运行日审计。

### 9. 给审阅者的关注点

1. 不要修改外部 JSONL 或历史 round 正文。
2. 核对更正使用追加，而不是重写旧结论。
3. 确认后两轮没有 self-delete 后落地事件。
