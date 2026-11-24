# TASK-400 TASK-395 closeout and 2026-11-24 round evidence

- 状态：**Done**
- 阶段：2　子阶段：2b Automation evidence　批次：治理池　依赖：TASK-395、TASK-397、TASK-398、TASK-399
- 本文文件 = 卡片正文 + 执行记录（ADR-0031「一卡一文件」）

---

## write scope

- `tasks/TASK-400-task395-closeout-round-evidence.md`
- `tasks/TASK-395-excel-t21-fake-runtime-session-handlers.md`
- `docs/automations/2026-11-24-round-1.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

依据已经合入 `main` 的 TASK-397、TASK-398 与 TASK-399 证据，关闭仍停留在
Ready/WIP 的 TASK-395，并为运行日 2026-11-24 round 1 形成可审计的 docs-only 留痕；
不推进真实 Excel、COM、GUI 或生产 ToolBus 注册。

## In scope

- 核对 TASK-395 的 DoD、`DRIFT-395-1` 解除证据、TASK-398 的私有 `ToolHandler`
  包装与 TASK-399 的 test-only COM runner prototype 边界。
- 重新运行 Excel T2.1 专项测试与仓库门禁，确认既有实现仍可验证。
- 将 TASK-395 状态从 Ready 更新为 Done，并在其执行记录区追加本轮关闭证据。
- 新建 `docs/automations/2026-11-24-round-1.md`，记录 `automation-rounds`、
  选卡依据、门禁、PR、CI 与 merge 状态。
- 按 ADR-0039 / ADR-0041 仅同步 Stage 2、`PLAN.md`、`README.md` 与
  `LEDGER.md` 的允许区域。
- 在仓库外持久 JSONL 记录 start、gates、work、pr、ci、merge、lock_release
  与 self_delete 事件。

## Out of scope

- 不修改产品代码、`crates/**`、`apps/**`、`adapters/**`、`protocol/**`、ADR 或 spec。
- 不修改公共 API、ErrorCode、ToolSchema、IPC、DB schema 或依赖。
- 不启动 Excel、COM 或 GUI，不读取真实 `.xlsx`，不执行人类在场验收。
- 不填写 `adapters/com.microsoft.excel/tools/tools.json`，不注册生产 ToolBus。
- 不把 test-only COM runner prototype 写成生产 runner 已完成。
- 不重写 TASK-395 正文、历史 LEDGER 行、历史报告或既有运行日证据。

## 必须遵守

1. TASK-395 只能依据 `main` 上已合并的 TASK-397/398/399 证据关闭；不得把计划推定成完成。
2. `tools.json` 必须保持 `tools: []`；生产 assembly 与 ToolBus 注册保持零改动。
3. 公共热点文件写前逐个 guard acquire，写完立即 release；不得 `--force`。
4. 状态行只按 ADR-0083 更新；TASK-395 正文其余部分只读。
5. 所有 `exec_command` 必须显式使用 `shell: "powershell"`；仅 PowerShell 零输出或超时时
   才可按 ADR-0100 回退并留证。
6. PR 合并前必须满足 base=main、checks>=11、pending=0、failed=0、
   `MERGEABLE` 且 `mergeStateStatus=CLEAN`。

## 验收命令

```powershell
cargo test -p assistant-agent-core --lib excel_t21
cargo test -p assistant-tool-bus --test handler_contract
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

预期：Excel T2.1 与 ToolBus 错误桥专项全绿，workspace 与全部门禁通过；
产品代码、schema、API、依赖和 Excel `tools.json` 零差异。

## DoD

- [ ] TASK-395 的既有 DoD 已由 main 上真实测试与 TASK-397/398/399 merge 证据覆盖。
- [ ] TASK-395 状态与本轮 LEDGER 一致，执行记录只追加、不改写正文。
- [ ] round 1 文件依据真实命令输出与 GitHub 元数据生成。
- [ ] 公共热点文件逐个 guard acquire/release。
- [ ] 验收命令全部真实执行并记录关键输出。
- [ ] PR checks、merge 与 post-merge main CI 回填后关闭本卡。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-400 TASK-395 收口与 2026-11-24 round 1 证据
【目标】依据 main 上 TASK-397/398/399 证据关闭 TASK-395，并形成 docs-only 轮次留痕
【write scope】仅：本卡、TASK-395、2026-11-24 round 1、Stage 2、PLAN、README、LEDGER；仓库外 JSONL
【铁律】证据真实；输入先校验；公共热点先 guard；不扩 scope；状态同批；未实现能力不得写成已生效
【禁止】不改产品代码/adapters/protocol/公共 API/schema/依赖；不启动 Excel/COM/GUI；不填 tools.json；不注册生产 ToolBus
【验收】Excel T2.1 27/27、ToolBus handler 8/8、fmt、clippy、workspace、arch、schema/codegen、文档门禁、deny、diff-check 全绿
【依赖】TASK-394/397/398/399 Done；ADR-0108/0109/0110 Accepted；N_auto=0、next_round=1
【疑问】无；TASK-395 不含本轮 round 路径，按先例新建 docs-only 本卡承载收口与证据
```

### 2. 实际改动文件

- `tasks/TASK-400-task395-closeout-round-evidence.md`：创建本轮 docs-only 收口卡。
- `tasks/TASK-395-excel-t21-fake-runtime-session-handlers.md`：状态置 Done，追加关闭证据。
- `docs/automations/2026-11-24-round-1.md`：记录真实轮次、门禁、PR/CI/merge 状态。
- `plans/stage-2-excel.md`、`PLAN.md`、`README.md`、`LEDGER.md`：按 ADR-0039/0041
  同步允许的状态区域。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --lib excel_t21`：27 passed / 0 failed。
- `cargo test -p assistant-tool-bus --test handler_contract`：8 passed / 0 failed。
- `cargo fmt --all --check`：PASSED。
- `cargo clippy --all-targets -- -D warnings`：exit 0；仅有既存
  `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace`：PASSED；xtask 587 passed / 0 failed，workspace 无失败。
- `cargo test -p assistant-core arch::`：10 passed / 0 failed。
- `verify-schemas`：7 schemas / 0 error。
- `codegen --check`：0 drift / 0 error。
- `hygiene`：scanned=509，0 error / 207 warning，PASSED。
- `card-check`：scanned=303，0 error / 243 warning，PASSED。
- `docscan`：scanned=724，0 error / 175 warning，PASSED。
- `refscan`：scanned=1274，0 error / 0 warning，PASSED。
- `memory-counts`：scanned=10，0 error / 1 warning，PASSED。
- `check-ledger`：0 error / 0 warning，PASSED。
- `adr-index`：scanned=99，0 error / 0 warning，PASSED。
- `cargo deny check`：advisories / bans / licenses / sources ok。
- Excel tools machine check：`excel_tools_count=0`。
- `git diff --check`：PASSED。

### 4. DoD 逐条核对

- [x] TASK-395 的既有 DoD 已由 main 上真实测试与 TASK-397/398/399 merge 证据覆盖。
- [x] TASK-395 状态与本轮 LEDGER 一致，执行记录只追加、不改写正文。
- [x] round 1 文件依据真实命令输出与 GitHub 元数据生成；PR/CI/merge 由后续回填。
- [x] 公共热点文件逐个 guard acquire/release。
- [x] 验收命令全部真实执行并记录关键输出。
- [ ] PR checks、merge 与 post-merge main CI 待本卡合并后回填。

### 5. 偏差

- `PROCESS-TASK400-SHELL-MISUSE`：本轮已有 5 次只读/状态查询误填
  `shell="bash"`，均由项目 hook 实际重写为 Windows PowerShell 执行，无仓库写入或
  外部副作用；后续命令全部显式使用 `shell="powershell"`。

### 6. 更合理做法

- 将 TASK-395 的实现证据关闭与当前轮次报告拆到 TASK-400，避免把新 round 路径塞进
  TASK-395 的旧 write scope。
- 只把 TASK-395 置 Done，真实 COM 生产接入和人类在场 `.xlsx` 验收仍留给 ADR-0110
  后续实现卡，不把 test-only prototype 混进生产完成状态。

### 7. 遗留问题

- 真实 COM 生产接入、人类在场 `.xlsx` 单次验收、重复收敛、watchdog/ownership journal
  生产接线与生产 ToolBus 注册仍待 ADR-0110 后续卡。
- `apps/agent-core/README.md` 仍描述旧的 `DRIFT-395-1` 阻塞；该文件不在本卡 write scope，
  留待人类在场的下一张实现卡同步，不能用本轮 docs-only 收口越界修改。

### 8. 新增长期记忆

- 无。本轮新增的是状态收口和轮次证据，没有改变已验证的产品事实或否决方案。

### 9. 给审阅者的关注点

- 核对 TASK-395 只依据 main 上已合并的 TASK-397/398/399 证据关闭，未把 test-only
  wrapper 或 runner prototype 写成生产完成。
- 核对 TASK-400 只做 docs-only 收口，产品代码、schema、API、依赖和 `tools.json` 零差异。
- 核对 round 1 的 PR/CI/merge 字段在本文件提交前仍明确为 pending，没有预填未发生的结果。

### 10. Merge evidence backfill

- 实现/证据 PR #825：base=main，checks **11/11 SUCCESS**，pending 0 / failed 0，
  `MERGEABLE` / `CLEAN`。
- merge commit：`d72ed1629ee0ee1eba1fb928fd5adcfe6a226eb2`。
- post-merge main CI `45366856008`：success；Windows、Ubuntu、macOS 与其余必要 jobs 全绿。
- `PROCESS-TASK400-SHELL-MISUSE` 最终机器重算为 **18** 次，均为只读/状态查询、
  分支切换或 gate 调用；
  项目 hook 实际以 Windows PowerShell 执行；`[supersedes:2026-11-24]` 执行记录 §5 的
  中途值 5 次。无仓库意外写入或外部副作用。

### 11. Final shell-count correction

- 回填 PR #826 merge `bbf263f` / post-merge main CI `45373787116` success 后，
  最终重算发现一次只读 run-list 查询仍误填了 `shell="bash"`。
- 权威计数更新为 **18**；`[supersedes:2026-11-24]` §10 的 12 次。
- 该命令由项目 hook 实际重写为 Windows PowerShell，执行结果正常，无仓库意外写入。

### 12. Concurrent round drift

- **DRIFT-AUTO-20261124-1**
  - 现象：本轮 `.automation.lock` 仍由
    `ai-assistant-task-round-20261124-1700` 持有且年龄小于 3 小时时，工作树在
    2026-11-24 19:43:10 被切换到本地分支
    `automation/2026-11-24-round2-skipped`。该分支不是本轮创建。
  - 影响：19:30 槽位没有按活跃锁跳过，且在同一个 checkout 中切换了工作线；继续释放锁或
    self-delete 会与另一工作线互相踩写，违反单工作线互斥。
  - 证据：`git reflog` 显示 checkout；该分支无远端 ref、无 PR，工作树在观察 60 秒后仍
    clean 且没有新增提交。
  - 建议：保留本轮 `.automation.lock` 和 automation 条目供人类清理，确认 19:30 槽位已退出后，
    再释放锁并删除本轮 automation。
  - 已停工作：不再执行 self-delete，不删除锁，不清理其他 automation 目录。
