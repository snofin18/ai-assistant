# TASK-411 Excel T2.2 platform-private ownership journal

- 状态：**Review**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6f　依赖：TASK-401、TASK-402、TASK-403、TASK-409、TASK-410、ADR-0108、ADR-0110、ADR-0111、ADR-0112

---

## write scope

- `tasks/TASK-411-excel-t22-platform-private-ownership-journal.md`
- `crates/platform/windows/src/excel_ownership_journal.rs`
- `crates/platform/windows/src/excel_ownership_journal_json_tests.rs`
- `crates/platform/windows/src/excel_com_runner.rs`
- `crates/platform/windows/src/excel_com_runner_state.rs`
- `crates/platform/windows/src/excel_com_runner_windows.rs`
- `crates/platform/windows/src/cdp/mod.rs`
- `crates/platform/windows/src/excel_com_runner_tests.rs`
- `crates/platform/windows/src/excel_com_runner_journal_tests.rs`
- `crates/platform/windows/src/excel_com_runner_repeat_tests.rs`
- `crates/platform/windows/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 `assistant-platform-windows` 内实现 platform-private、durable、bounded 的 Excel ownership journal，
并在 runner 的危险清理路径前发布、清理成功后删除；本卡不实现 watchdog、process-tree termination
或 production Tool registration。

## In scope

- 新增 journal schema version 1，复用具名 `ExcelOwnedProcessIdentity`，禁止裸 PID ownership。
- 固定长度、字段数、UTF-8/JSON 转义、路径与 phase 命名边界。
- 使用唯一 `.tmp` 路径和 `create_new + write_all + sync_all + rename` 原子 publish。
- 在 workbook close、application quit、worker shutdown 前发布 cleanup journal。
- journal 写入失败必须 fail-closed；close/quit 失败时保留 journal，成功后删除。
- fake tests 覆盖 IO 失败、目标已存在、stale identity、文件名冲突、清理失败保留和成功删除。
- 修正 ADR-0108 D9 cleanup 错误映射：应用对话/用户拒绝保持 `UserInteraction`，结构失败为 `Fatal`。
- 同步 crate README、Stage 2、PLAN、README 与 LEDGER；无新增长期记忆时在记录区明确说明。

## Out of scope

- 不实现独立 watchdog、Job Object、进程树终止、bounded reap 或 Host 崩溃恢复。
- 不解决 `CoCreateInstance` 与 owned-process 建立之间的 creation race。
- 不新增 production task-end caller、ToolBus Tool、`tools.json`、rollback recipe 或 assembly。
- 不修改 `assistant-platform-api`、protocol schema、ErrorCode 或依赖。
- 不启动真实 Excel、不读取或写入真实工作簿、不进行 GUI 操作。

## 必须遵守

1. journal 只允许仓库外、绝对、受控根目录；测试使用独立临时根并通过字段注入，不得写用户目录。
2. journal 文件名必须唯一标识 coordinator pid、runner、session、phase；不同 phase 不得覆盖。
3. Windows `rename` 不得被当作通用 replace；目标已存在必须显式失败并清理临时文件。
4. 无完整 owned identity、journal publish 失败或 cleanup 未成功时不得返回成功。
5. 失败清理必须保留可审计 journal；不得静默删除或在同一 journal 上重试。
6. 不修改测试断言规避实现缺陷；不新增 `#[allow]`、`unsafe` 或第三方依赖。

## 验收命令

```powershell
cargo fmt --all --check
cargo test -p assistant-platform-windows
cargo clippy --all-targets -- -D warnings
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings
cargo clippy --target aarch64-apple-darwin -p assistant-platform-windows --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- docscan
cargo deny check
git diff --check
```

预期：专项 fake tests 覆盖本卡 DoD；上述门禁全绿，除仓库既有 warning/skipped check 外无失败。

## DoD

- [ ] journal schema v1 有固定字段、版本和硬长度上限，并复用完整 owned identity。
- [ ] publish 使用唯一 phase 文件名与 `.tmp + create_new + write_all + sync_all + rename`。
- [ ] IO 失败、目标已存在和文件名冲突均 fail-closed，临时文件不静默残留。
- [ ] stale identity 被显式拒绝，裸 PID 不是所有权证据。
- [ ] cleanup 前发布 journal；cleanup 成功后删除，失败时保留。
- [ ] `UserInteraction` cleanup 映射保持 `UserInteraction`，结构失败映射 `Fatal`，测试断言公共结果。
- [ ] 不新增公共 API、schema、ErrorCode、依赖、ToolBus/tools.json/rollback/assembly。
- [ ] 未启动真实 Excel/GUI；专项 fake tests 与全量门禁通过。
- [ ] 独立 review no P0-P2，PR/CI/merge/main CI 证据落入记录区。

<!-- 分界线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-411 Excel T2.2 platform-private ownership journal
【目标】实现有界、原子发布、可保留/完成清理的私有 ownership journal，并修正 cleanup 错误映射
【write scope】仅本卡、Windows platform journal/runner/state/tests/README、Stage 2、PLAN、README、LEDGER
【铁律】无静默失败；完整 owned identity；清理成功后删除 active journal；不扩公共 API/schema/依赖；热门文档先取 guard
【禁止】不实现 watchdog/termination/production caller/tools.json；不启动真实 Excel/GUI；不改 ErrorCode/schema/依赖
【验收】专项 fake tests + workspace fmt/clippy/test + xtask gates + deny + diff-check
【依赖】TASK-401/402/403/409/410 Done；ADR-0108 D8/D9、ADR-0110/0111/0112 已核对；N_auto=0
【疑问】无；watchdog 与 verified termination 留给 TASK-412
```

### 2. 实际改动

- 新增 `crates/platform/windows/src/excel_ownership_journal.rs`：schema v1、完整 owned identity、
  runner/session/phase 唯一路径、8 个活动文件硬上限、test-only 私有 root guard 与原子 no-replace publish。
- 新增 `crates/platform/windows/src/excel_com_runner_state.rs`：把 runner 状态机移出主文件并接入 journal；
  Opening/Closing 在 close 成功后按 session 清理，Close/Quit 失败保留 Cleanup，结构失败失败关闭。
- 调整 `excel_com_runner.rs` / `excel_com_runner_windows.rs`：仅在 Windows production 解析
  `LOCALAPPDATA\ai-assistant\excel-ownership-journal`，测试使用独立临时子目录；公共方法面不变。
- 新增 journal 单元、JSON、runner 顺序/多 session/失败保留测试，并拆分超过硬行数上限的测试文件。
- `cdp/mod.rs` 仅增加 `#[cfg(test)] pub(crate)` 的 JSON parser 测试可见性，不进入 production API。
- 同步 crate README、Stage 2、PLAN、README、LEDGER；未新增长期 memory。

### 3. 门禁输出摘要

- `cargo test -p assistant-platform-windows`：**182 passed / 0 failed / 8 ignored**。
- `cargo test --workspace`：全部 workspace tests 通过；workspace doctests 全部通过。
- `cargo fmt --all --check`、`git diff --check`：PASSED。
- host / `x86_64-unknown-linux-gnu` / `aarch64-apple-darwin` Clippy：exit 0；仅仓库既有
  `clippy::assert_is_empty` 未知 lint warning。
- `verify-schemas`、`codegen --check`、arch、`hygiene`、`card-check`、`docscan`、
  `memory-counts`、`adr-index`、`check-ledger`、`check-comments`：PASSED。
- `cargo deny check`：advisories / bans / licenses / sources 全部 ok。
- `cargo llvm-cov --fail-under-lines 75`：PASSED，line coverage **77.99%**。
- 清理了旧测试模式遗留在 `%TEMP%` 根目录的 64 个 `excel-ownership-*.json`；新实现测试后
  `%TEMP%` 中 journal 文件与 `assistant-task411-*` 目录均为 0。

### 4. DoD 核对

- [x] journal schema v1 有固定字段、版本和硬长度上限，并复用完整 owned identity。
- [x] publish 使用唯一 phase 文件名与 `.tmp + create_new + write_all + sync_all + MoveFileExW` 无 replace 发布。
- [x] IO 失败、目标已存在和临时文件冲突均 fail-closed，临时文件不静默残留。
- [x] stale identity 被显式拒绝；裸 PID 不是所有权证据。
- [x] cleanup 前发布 journal；session 成功后删除 opening/closing，整体成功后删除 cleanup，失败时保留。
- [x] `UserInteraction` cleanup 保持 `UserInteraction`，结构失败映射 `Fatal`，测试断言公共结果。
- [x] 未新增公共 API、schema、ErrorCode、依赖、ToolBus/tools.json/rollback/assembly。
- [x] 未启动真实 Excel/GUI；专项 fake tests 与全量门禁通过。
- [x] 独立 review 最终 no P0-P2；仍保留 3 个 test-only P3（见第 9 节）。
- [ ] PR/CI/merge/main CI 证据待回填。

### 5. 偏差

- 首轮 review 发现 1 P1 + 2 P2：Windows rename TOCTOU、phase 级 journal 去重跨 session 复用、
  默认测试 root 泄漏。均已修复并复审关闭。
- 为满足 600/900 行硬门禁，把 runner state 与三类测试拆分到同卡私有模块；未改变公共边界。
- 工具偏差：本 round 误填 `shell: "bash"` 共 **24** 次（含 2 次被 PreToolUse 拒绝的
  README/stage 查询）；其余命令均显式 PowerShell。最终计数与失败项写入外部 JSONL。

### 6. 新增长期记忆

- 无。TASK-411 的稳定事实由任务卡、crate README 与 `LEDGER.md` 承载；未改
  `MEMORY.md` / `docs/memory/*`，避免未授权改写 L0。

### 7. 遗留与下一步

- TASK-412 实现 watchdog contract、creation race、`OpenProcess` 错误分类、verified
  process-tree termination、三层 deadline/reap 与结构化 cleanup report。
- 仍禁止 production `tools.json` / ToolBus / rollback / assembly registration。
- test-only P3：`TestJournalRoot` Drop 清理失败静默；direct test-only Quit 不自行发布 journal；
  journal root 清理失败的上层观测仍留给后续集成卡。

### 8. 外部证据

- 本卡、`crate README`、Stage 2、`PLAN.md`、`README.md`、`LEDGER.md`。
- 仓库外运行证据：`C:\Users\fexfe\.codex\automations-logs\2026-11-26\ai-assistant-task-round-20261126-1700.jsonl`。
- implementation PR / CI / merge / post-merge main CI：待回填。

### 9. 给审阅者的关注点

- 重点核对 Windows `MoveFileExW` 未带 `REPLACE_EXISTING`，以及唯一路径生成是否保持有界。
- 重点核对 close/quit 失败时 journal 保留、成功时仅删除对应 session 的 opening/closing。
- 重点核对 `UserInteraction` 与结构失败 `Fatal` 的公共错误映射，以及 production public API 零变化。
