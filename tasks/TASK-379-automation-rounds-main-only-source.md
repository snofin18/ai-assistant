# TASK-379 automation-rounds main 唯一计数源与轮次产物硬前置

- 状态：Done
- 阶段：2　子阶段：2b Adapter contract decision　批次：治理池　依赖：ADR-0054、TASK-378　预估：S　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031「一卡一文件」）。

---

## write scope

- `tasks/TASK-379-automation-rounds-main-only-source.md`
- `xtask/src/automation_rounds.rs`
- `xtask/src/automation_rounds_tests.rs`
- `xtask/README.md`
- `docs/automations/codex-automations-operations.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

修复 `xtask automation-rounds` 从工作区枚举 `docs/automations/*-round-*.md` 的实现偏差，
改为只读取已解析的 `origin/main` 快照（缺失时仅回退到 `main`），并补 untracked、staged、
branch-only、deleted-unmerged、merged 五类测试；同时在 automation 最终审计清单中把
“本轮 round 文件已进入 main 并记录路径/PR/merge”列为自删硬前置。

## In scope

- 将轮次枚举改为一次性解析 main commit，再用该 commit 的 `git ls-tree` 和 `git show` 读取文件。
- 让 `automation-rounds` 与 human prompt reset 使用同一 main 快照，不再数工作区。
- 对缺失 main ref、git 调用失败、非法 UTF-8、non-round 文件、超限输出 fail-closed。
- 验证 merged 文件可见；untracked、staged、branch-only、deleted-unmerged 文件不可见。
- 更新 xtask README 与 automation 操作手册的最终审计顺序。
- 完成状态文档同步与 PR/CI/main CI 回填。

## Out of scope

- 不执行 `git fetch` 或任何网络/代理动作；调用方必须先更新远端快照。
- 不修改 ADR-0054/0088/0089/0099、automation charter 或 spec。
- 不修改 `protocol/**`、`adapters/**`、`crates/**`、`apps/**`、schema、Trait、产品代码或依赖。
- 不改现有轮次文件、JSONL、PR/CI 元数据或历史 LEDGER 行。
- 不新增 automation，不运行真实 GUI，不访问用户数据。

## 必须遵守

1. main 是唯一计数源；工作区当前分支、index、untracked、未合并提交一律不得参与。
2. 读取前解析唯一 commit hash，后续 `ls-tree` 与 `show` 全部使用该 hash，避免中途 ref 漂移。
3. `origin/main` 优先；仅有缺失 `origin/main` 时回退本地 `main`；两者都缺失必须报错。
4. round 文件、路径和 git 输出必须有硬上限；超限、非法 UTF-8 或命令失败不得静默跳过。
5. 测试必须用临时 git 仓库真实构造五类状态，不得只测纯函数。
6. 最终审计必须区分“round 文件已合并且记录 evidence”与“只在工作区分支/JSONL 中有记录”。

## 验收命令

```powershell
cargo test -p xtask automation_rounds
cargo test -p xtask
cargo fmt --all --check
cargo clippy -p xtask --all-targets -- -D warnings
cargo run -p xtask -- automation-rounds --run-date 2026-11-16
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
git diff --check
```

预期：五类 git 状态测试通过；真实仓库 round 1~5 仍只来自 `origin/main`；
全部门禁通过；产品路径零差异；PR checks 与 post-merge main CI 真实回填。

## DoD

- [ ] `automation-rounds` 不再读取工作区 round 文件，只读 main 快照。
- [ ] origin/main 优先与 main 回退均有测试；缺失 main fail-closed。
- [ ] merged / untracked / staged / branch-only / deleted-unmerged 五类证据齐全。
- [ ] human prompt reset 与主命令共享同一 main 快照语义。
- [ ] automation 手册把 round path、PR、merge 纳入自删硬前置。
- [ ] 产品代码、schema、ADR、spec、adapters 与依赖零差异。
- [ ] 全部门禁、PR checks、merge 与 post-merge main CI 回填。

<!-- ══ 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-379 automation-rounds main 唯一计数源与轮次产物硬前置
【目标】只从 origin/main/main 快照计数，并补五类工作区/分支状态负向测试
【write scope】仅：本卡、xtask automation_rounds 实现/测试、xtask README、automation 操作手册、计划/状态/LEDGER 允许区域
【铁律】main 唯一事实源；缺失/超限 fail-closed；资源有界；真实 git 测试；不改断言规避
【禁止】不联网 fetch；不改 ADR/spec/产品代码/依赖；不数工作区或本地未合并分支
【验收】conditional updated 2026-11-16；五类测试；真实 round 1~5；xtask 测试/clippy/fmt/文档门禁与 PR/main CI
【依赖】ADR-0054；TASK-378 Done；用户批准 TASK-379
【疑问】无
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

本卡、`xtask/src/automation_rounds.rs`、`xtask/src/automation_rounds_tests.rs`、`xtask/README.md`、
`docs/automations/codex-automations-operations.md`、`plans/stage-2-excel.md`、`PLAN.md`、`README.md`、`LEDGER.md`；均在卡内 write scope。

### 3. 验收输出摘要（命令 → 结果）

- `cargo test -p xtask automation_rounds` → 25 passed / 0 failed。
- `cargo test -p xtask` → 517 passed / 0 failed。
- `cargo fmt --all --check` → PASSED；`clippy` → exit 0，仅保留 workspace 既有 `clippy::assert_is_empty` 未知 lint 提示。
- 真实仓库 `automation-rounds --run-date 2026-11-16` → `rounds=5 / next=6`；临时未跟踪 `round-99` 探针下仍为 `5 / 6`，探针已删除。
- 临时 git 仓库五类状态：merged=1；untracked/staged/branch-only/deleted-unmerged 均保持 1；`origin/main` 缺失时回退本地 `main`，存在但不可解析为 commit 或两者皆缺均 fail-closed。
- 所有 Git 子进程用 `env_clear()` 后只恢复非 `GIT_*` 环境，设置 `GIT_TERMINAL_PROMPT=0`、`GIT_OPTIONAL_LOCKS=0`、`GIT_NO_LAZY_FETCH=1`；stdout/stderr 有界流式读取，30 秒超时会终止进程树并回收 reader。
- 大小写变体真实探针：把当前 PowerShell 子进程的 `git_dir` 指到不存在路径后，命令仍按 `-C` 读取仓库并得到 `5 / 6`。
- 独立 review 两轮阻塞项均在 follow-up 处理：promisor/lazy fetch、大小写 `GIT_*` 污染、无界 `output()`/无超时、递归子目录、skipped 自删冲突、旧硬核对与 legacy suffix 过宽；新增 nested、malformed、unknown non-round、stdout-limit、origin 优先于不同 local main 和大小写环境名测试。
- `docs/automations/` 只接受 `YYYY-MM-DD-round-N.md`、明确 fixed docs、精确 `report` / `report-run1415` / `skipped`，以及两条 ADR-0054 历史例外；`report-2`、`round-0-skipped` 与其他未知文件均 fail-closed。
- 文档门禁全部 PASSED：`hygiene` 0/171、`card-check` 0/88、`docscan` 0/175、`refscan` 0/0、`memory-counts` 0/1、`adr-index` 0/0、`check-ledger` 0/0；`git diff --check` 干净。
- PR #751 checks **21 success + 1 skipped**，failed 0 / pending 0，base=main，`MERGEABLE` / `CLEAN`；
  merge `fa87cae65b19bde6593dc0b2fd812f98ce7079b6`，post-merge main CI `44030156733` success。

### 4. DoD 逐条核对

- [x] `automation-rounds` 不再读取工作区 round 文件，只读 main 快照。
- [x] origin/main 优先与 main 回退均有测试；缺失 main fail-closed。
- [x] merged / untracked / staged / branch-only / deleted-unmerged 五类证据齐全。
- [x] human prompt reset 与主命令共享同一 main 快照语义。
- [x] automation 手册把 round path、PR、merge 纳入自删硬前置。
- [x] 产品代码、schema、ADR、spec、adapters 与依赖零差异。
- [x] 全部门禁、PR checks、merge 与 post-merge main CI 回填。

### 5. 偏差

- `DRIFT-379-1`（契约澄清）：正文写“non-round 文件 fail-closed”，但 main 中本来就有
  report / skipped / ADR-0054 历史例外文件，字面执行会让正常仓库失败。
  影响：需要把“非法文件”与“已定义的合法非 round 文件”区分。
  决策：实现采用封闭允许集合，未知文件与 `report-2`、非法 skipped 等仍 fail-closed；
  该裁决已在人类连续授权与两轮独立 review 后落地，不扩展公共接口。

### 6. 更合理做法

- 用 main commit hash 固定一次读取快照，使 `ls-tree` 与每个 `show` 不会跨越 ref 更新。
- 有界读取线程与主进程协调：任一输出流超限立即 kill/wait 两个 reader；锁冲突轮次改由 JSONL 记录明确例外。

### 7. 遗留问题

- 调用方仍必须在运行 automation 前 fetch `origin/main`；网络失败由上层自动化启动门禁报告。
- 未新增真实“Git 子进程挂起”集成测试；由有界读取、30 秒主超时、Windows 进程树终止和 5 秒清理上限兜底。

### 8. 新增长期记忆

- 无；实现事实由卡、README、手册、LEDGER 和测试保存。

### 9. 给审阅者的关注点

- 核对测试真实创建 git 仓库覆盖五类状态，且只有 `origin/main` ref 缺失时才回退本地 `main`；不可解析 commit 或两者皆缺均 fail-closed，永不回退工作区。
- 核对只枚举 `docs/automations/` 直属文件、malformed round 文件失败，并确认普通 report/skipped 文件不会被误计。
- 核对最终审计手册要求实际 round 的 path、PR、merge 三项证据，同时 skipped 分支有可满足的 JSONL 例外。
