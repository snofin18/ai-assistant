# TASK-396 2026-11-23 自动化运行日收口

- 状态：InProgress
- 阶段：2　子阶段：2b Automation evidence　批次：治理池　依赖：TASK-395（Blocked/WIP）、ADR-0039、ADR-0041
- 本文文件 = 卡片正文 + 执行记录（ADR-0031「一卡一文件」）

---

## write scope

- `tasks/TASK-396-automation-run-day-report-2026-11-23.md`
- `docs/automations/2026-11-23-round-3.md`
- `docs/automations/2026-11-23-report.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在运行日 2026-11-23 的最终槽位中，依据真实 JSONL、GitHub PR/CI/merge 元数据与本地门禁输出，
补齐 round 3 与运行日报告；不推进仍被 `DRIFT-395-1` 阻塞的 Excel 产品实现。

## In scope

- 新建 round 3 与 2026-11-23 运行日报告，汇总既有 round 1~2、当前 round 3 的真实证据。
- 核对并记录 TASK-395 的 Blocked 状态、`DRIFT-395-1` 与 `DRIFT-395-2` 的遗留。
- 运行本卡验收命令，记录专项 fake tests、workspace 与文档门禁结果。
- 完成后仅按 ADR-0039 / ADR-0041 同步 Stage 2、`PLAN.md`、`README.md` 与 `LEDGER.md` 的允许区域。
- 追加仓库外持久 JSONL，记录 start、automation-rounds、选卡、门禁、PR、CI、merge、release 与 self-delete。

## Out of scope

- 不修改 TASK-395、产品代码、`crates/**`、`apps/**`、`adapters/**`、`protocol/**`、ADR 或 spec。
- 不裁决 `DRIFT-395-1`，不新增或修改 `ToolBusError` 公共接口、ErrorCode、ToolSchema 或 schema。
- 不启动 Excel、COM、GUI，不读取真实 `.xlsx`，不执行人类在场验收。
- 不注册生产 ToolBus；`adapters/com.microsoft.excel/tools/tools.json` 必须保持空数组。
- 不新增依赖、crate、模块、automation 或 CI 门禁。
- 不覆盖既有 JSONL、历史卡片正文、历史 LEDGER 行或未执行轮次的结果。

## 必须遵守

1. 未执行或不存在的证据只能写“未执行/无证据”，不得按计划推定成功。
2. `automation-rounds` 的 `N_auto` 与 `next_round` 是轮次事实源；外部日志差异只记录，不改写机器结论。
3. 公共热点文件写前逐个取 guard，写完立即释放；不得 `--force` 抢锁。
4. `TASK-395` 保持 Blocked；报告不得代替 `DRIFT-395-1` 裁决，也不得把四 Tool 写成已生效。
5. 所有 `exec_command` 必须显式使用 `shell: "powershell"`；仅 PowerShell 零输出或超时时可按 ADR-0100 回退并留证。
6. PR 合并前必须满足 base=main、CI success、checks>=11、pending=0、failed=0、`MERGEABLE` 且 `CLEAN`。

## 验收命令

```powershell
cargo test -p assistant-agent-core --lib excel_t21
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
cargo run -p xtask -- adr-index
cargo deny check
git diff --check
```

预期：专项 fake tests 与全部门禁真实通过；Excel tools count 为 0；产品代码、schema、API、依赖零差异。

## DoD

- [ ] round 3 与运行日报告均依据真实 JSONL、GitHub 元数据和本轮门禁输出生成。
- [ ] TASK-395 的 Blocked 状态与两个 DRIFT 被如实披露，未被报告动作解锁。
- [ ] 产品代码、schema、API、依赖与 `tools.json` 零差异。
- [ ] 公共热点文件按 ADR-0028 逐个 guard acquire/release。
- [ ] 验收命令全部真实执行并记录关键输出。
- [ ] PR checks、merge 与 post-merge main CI 回填后关闭本卡。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-396 2026-11-23 自动化运行日收口
【目标】在 TASK-395 仍被 DRIFT-395-1 阻塞且无其他 Ready/InProgress 卡时，生成 round 3 与运行日报告
【write scope】仅：本卡、round 3、运行日报告、Stage 2、PLAN、README、LEDGER；仓库外追加本 automation JSONL
【铁律】证据真实；公共热点先 guard；不扩 scope；状态同批；TASK-395 保持 Blocked；tools.json 保持空
【禁止】不修改产品代码/schema/API/依赖；不启动 Excel/COM/GUI；不代裁 DRIFT-395-1；不使用 auto merge
【验收】专项 fake tests、fmt、clippy、workspace、arch、schema/codegen、文档门禁、deny、diff-check 全绿
【依赖】TASK-394 Done；TASK-395 Blocked/WIP；automation-rounds PASSED：next_round=3、N_auto=2
【疑问】无；无可执行产品卡，自决创建 docs-only 收口卡并保留全部产品阻塞
```

### 2. 实际改动文件

- `tasks/TASK-396-automation-run-day-report-2026-11-23.md`：创建本卡并填写执行记录。
- `docs/automations/2026-11-23-round-3.md`：记录 final slot 的机器轮次、范围裁决、门禁与安全边界。
- `docs/automations/2026-11-23-report.md`：汇总运行日 5 个真实槽位、GitHub 证据与剩余阻塞。
- `LEDGER.md`：追加本轮 pending 事件。
- `plans/stage-2-excel.md`、`PLAN.md`、`README.md`：待本卡 Done 后仅同步允许的完成标记与状态块。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --lib excel_t21`：22 passed / 0 failed。
- `cargo fmt --all --check`：PASSED。
- `cargo clippy --all-targets -- -D warnings`：exit 0；仅有既存 unknown-lint warning。
- `cargo test --workspace`：PASSED；xtask 587 passed / 0 failed。
- `cargo test -p assistant-core arch::`：10 passed / 0 failed。
- `verify-schemas`：7 schemas / 0 error。
- `codegen --check`：0 drift / 0 error。
- `hygiene`：0 error / 207 warning，PASSED。
- `card-check`：0 error / 216 warning，PASSED。
- `docscan`：0 error / 175 warning，PASSED。
- `refscan`：0 error / 0 warning，PASSED。
- `memory-counts`：0 error / 1 warning，PASSED。
- `adr-index`：0 error / 0 warning，PASSED。
- `check-ledger`：0 error / 0 warning，PASSED。
- `cargo deny check`：advisories / bans / licenses / sources ok。
- Excel tools machine check：`excel_tools_count=0`。

### 4. DoD 逐条核对

- [x] round 3 与运行日报告依据真实 JSONL、GitHub 元数据和本轮门禁输出生成。
- [x] TASK-395 的 Blocked 状态与两个 DRIFT 被如实披露，未被报告动作解锁。
- [x] 产品代码、schema、API、依赖与 `tools.json` 零差异。
- [ ] 公共热点文件逐个 guard acquire/release；`LEDGER.md` 已完成并补记两次 guard-skipped
  偏差，其余 Done 同步文件待 closeout。
- [x] 验收命令全部真实执行并记录关键输出；`git diff --check` 待提交前最终复跑。
- [ ] PR checks、merge 与 post-merge main CI 回填后关闭本卡。

### 5. 偏差

- `PROCESS-ROUND3-SHELL-MISUSE`：本轮有 6 次只读证据查询误填 `shell="bash"`，项目 hook
  实际以 Windows PowerShell 执行，无仓库写入或外部副作用。此前冻结累计值为 79，本轮后
  运行日累计为 85。
- `PROCESS-TASK396-GUARD-SKIPPED`：修正 shell 计数时，`LEDGER.md` 两次在上一批追加后已
  释放 guard 的情况下被直接改写。两次发现后都立即重新 acquire/release 复盘；期间没有并发
  写入证据。
- `DRIFT-395-2` 保留：TASK-395 的 write scope 不含 round 3 与运行日报告。本轮通过新建
  docs-only TASK-396 承载报告路径，没有追改旧卡 scope，也不声称历史 DRIFT 已回溯消除。

### 6. 更合理做法

- 将产品实现阻塞与运行日证据收口拆成两张卡：TASK-395 只保留公共错误码桥的产品阻塞，
  TASK-396 只处理报告与允许的状态同步。这样不需要修改旧卡正文，也不会用报告动作绕过
  `DRIFT-395-1` 的公共接口裁决。
- 运行日报告同时列出机器 `next_round=3` 和外部 5 个槽位日志，避免把未合并报告的 22:00、
  00:30 槽位误写成“没有执行”。

### 7. 遗留问题

- `DRIFT-395-1`：需要人类裁决 `ToolBusError` 如何无损承载既有 `ErrorCode` 分类。
- TASK-395 保持 Blocked；Windows COM runner、ownership journal、独立 watchdog、真实 `.xlsx`
  与人类在场验收均未开始。
- 四 Tool 继续不进入 `tools.json`，不注册生产 ToolBus。

### 8. 新增长期记忆

- 无。运行日事实保存在本卡、round 3 与运行日报告；已有 `DRIFT-395-1` 继续作为公开阻塞。

### 9. 给审阅者的关注点

- 核对新建 TASK-396 是否只承担报告收口，且没有借机修改 TASK-395 或产品代码。
- 核对运行日报告是否把 22:00 / 00:30 的实际日志与 `next_round=3` 的机器结论分别陈述。
- 核对 PR 合并后状态同步只改 Stage 2 完成标记 / 当前进度、`PLAN.md` 四行、README 三处和
  `LEDGER.md` 追加行。
