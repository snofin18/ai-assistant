# TASK-369 Browser cleanup reap semantics

- 状态：**Done**
- 阶段：1　子阶段：Stage 1 follow-up　批次：production cleanup repair
- 依赖：TASK-367、ADR-0063、`docs/audits/stage-1-t5.2-ten-run-2026-11-12.md`
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `tasks/TASK-369-browser-cleanup-reap-semantics.md`
- `crates/platform/windows/src/browser_launcher.rs`
- `crates/platform/windows/src/browser_launcher_tests.rs`
- `crates/platform/windows/README.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`

## 目标

修正浏览器 launcher 的清理语义：当 root 子进程已经被有界 `wait()` reap 时，
已经完成的 `taskkill` 非零退出码只作为诊断信息，不得再把一次已达成资源回收目标的关闭误报为
`TargetUnresponsive`；`taskkill` 启动失败和 root 未 reap 路径仍必须显式失败。

## In scope

- 在 `terminate_child_tree` 内提取窄分类 helper，区分“`taskkill` 未能启动”与
  “`taskkill` 已返回，但 root 已由 bounded wait reap”。
- 为 TASK-367 `run-04` 的真实 `taskkill=255` 形态补单元回归，证明 root reap 后分类为成功。
- 为 `taskkill` 进程启动失败补负向回归，证明不会静默吞错。
- 保留现有真实父子进程树测试，继续验证整棵测试进程树消失且 root 被 reap。
- 同步 launcher 不变量说明、任务卡、PLAN、阶段计划、README 与 LEDGER。

## Out of scope

- 不修改浏览器启动参数、专用 profile 校验、CDP endpoint 解析、fallback kill 或 `Drop` 清理路径。
- 不新增 Job Object、进程快照、公共 API、`ErrorCode`、依赖、schema、Policy 或 `unsafe`。
- 不修改 TASK-367 十轮证据、失败轮、Stage 1 矩阵或历史审计结论。
- 不把“taskkill 非零退出码一律成功”扩展到 `taskkill` 未能启动或 root 未 reap 的路径。

## 必须遵守

1. ADR-0063：终止进程树后必须 `wait()` reap；返回成功前 root 必须处于已 reap 状态。
2. `taskkill` 启动失败仍返回显式 `Fatal`，不得因后续等待偶然成功而掩盖清理命令未执行。
3. root 未在 bounded wait 内退出时保留 fallback kill 与 `TargetUnresponsive`。
4. 不修改既有测试判据来规避失败；新增测试必须覆盖真实失败形态和负向路径。
5. TASK-367 的 `9/10` 与 `run-04` 证据保持不可改写。

## 验收命令

```powershell
cargo fmt --all --check
cargo test -p assistant-platform-windows browser_launcher::tests::test_root_reap_accepts_nonzero_taskkill_exit
cargo test -p assistant-platform-windows
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- check-ledger
git diff --check
```

期望：`taskkill=255` + root reaped 分类为成功；启动错误仍显式失败；真实测试进程树消失；
目标 crate、workspace、clippy 与文档门禁全绿；PR checks 11/11，merge 后 main CI success。

## DoD

- [ ] root 已 reap 后，`taskkill` 非零退出码不再把 cleanup 误报为 `TargetUnresponsive`。
- [ ] `taskkill` 启动失败与 root 未 reap 路径继续 fail-closed。
- [ ] 新增正向与负向单测，并保留真实父子进程树回收测试。
- [ ] 目标 crate、workspace、clippy 与全部文档门禁通过。
- [ ] PR checks 11/11，merge 与 post-merge main CI 证据回填。
- [ ] PLAN、plans、README、LEDGER、任务卡与 launcher 不变量同步。

<!-- ═══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ═══
     以下由 Implementer 填写。改动分界线上任意一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-369 Browser cleanup reap semantics
【目标】修正 root 已 reap 后 taskkill 非零退出码误报
【write scope】仅：本卡、browser_launcher.rs/tests、Windows crate README、PLAN/plans/README/LEDGER
【铁律】无静默失败；资源必须终止并 reap；不放宽测试判据；不扩公共接口；不伪造证据
【禁止】改启动/profile/CDP/fallback/Drop；改 TASK-367 证据；加 Job Object/依赖/ErrorCode/unsafe
【验收】TASK-367 taskkill=255 回归 + 启动错误负向测试 + 真实进程树测试 + workspace/门禁全绿
【依赖】TASK-367 Done；ADR-0063；run-04 审计已核对
【疑问】无；沿用 TASK-367 残余风险建议并新建 TASK-369
```

### 2. 实际改动文件

- `crates/platform/windows/src/browser_launcher.rs`：提取
  `classify_taskkill_after_root_reap`；root 已 reap 时忽略已返回的非零 `taskkill` 状态，
  但保留 spawn error 的 `Fatal` 映射；root 未 reap 路径不变。
- `crates/platform/windows/src/browser_launcher_tests.rs`：新增 `taskkill=255` 正回归与
  `taskkill` 启动失败负回归；原真实父子进程树回收测试保持不变。
- `crates/platform/windows/README.md`：说明 root reap 是资源后置条件，已返回的
  `taskkill` 非零状态只作诊断；未启动或 root 未 reap 仍显式失败。
- `PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`：Done、PR #727、
  merge `7e82274759b483e193f35c44c80002359a1b5558` 与 post-merge main CI
  `43791998265` 已回填。
- 本卡：状态、CI 证据与最终 shell misuse 审计已回填。

### 3. 验收输出摘要

- TASK-367 现场形态回归：`test_root_reap_accepts_nonzero_taskkill_exit` 通过。
- 负向回归：`test_root_reap_preserves_taskkill_spawn_error` 通过，错误码为 `Fatal`。
- `cargo test -p assistant-platform-windows`：140 passed / 0 failed / 5 ignored；
  真实 `test_process_tree_termination_reaps_root_and_child` 通过。
- `cargo clippy --all-targets -- -D warnings`：exit 0；仅有仓库既有
  `clippy::assert_is_empty` unknown-lint 警告。
- `cargo test --workspace`：exit 0；`xtask` 510 passed / 0 failed，全部 doctest 通过。
- `cargo fmt --all --check`、`git diff --check`：通过。
- PR #727 checks **11/11 success**，pending 0 / failed 0，base=`main`，
  `MERGEABLE` / `CLEAN`；merge `7e82274759b483e193f35c44c80002359a1b5558`。
- post-merge main CI `43791998265`：success；三平台 check job、cargo deny、desktop-ui、
  matrix、hygiene、schema/codegen、replay 与 check-ledger 均通过。

### 4. DoD 逐条核对

- [x] root 已 reap 后，`taskkill` 非零退出码不再把 cleanup 误报为 `TargetUnresponsive`。
- [x] `taskkill` 启动失败与 root 未 reap 路径继续 fail-closed。
- [x] 新增正向与负向单测，并保留真实父子进程树回收测试。
- [x] 目标 crate、workspace、clippy 与全部文档门禁通过。
- [x] PR checks 11/11，merge 与 post-merge main CI 证据回填。
- [x] PLAN、plans、README、LEDGER、任务卡与 launcher 不变量同步。

### 5. 偏差

产品行为无漂移。TASK-369 会话最终 shell misuse 审计为 `bash=14` / `cmd=0`：
前期 2 次为只读调查，后续 12 次为 closeout/PR 审计期间的只读命令、一次被 hook 拒绝的命令，
以及一次由 API 生成 remote closeout commit 的已授权写入命令。
所有命令均由 ADR-0100 PreToolUse hook 重写为 PowerShell 或 fail-closed 拦截，未在 WSL/bash 执行，
未产生额外仓库写入；后续命令继续显式使用 PowerShell。

### 6. 实施中发现的更合理做法

- 把“root 已 reap 后如何解释 taskkill 状态”提取成纯分类 helper，让资源后置条件与
  spawn error 保持两个显式分支；真实父子进程树测试继续验证系统行为，不被 mock 取代。

### 7. 遗留问题

无。

### 8. 新增长期记忆

无新增 `docs/memory/**` 条目；该判定已写入 `assistant-platform-windows` 的 launcher
不变量说明，属局部实现语义，不扩展为跨项目规则。

### 9. 给审阅者的关注点

- root 已 reap 是本路径的唯一成功前提；`taskkill` 已返回后非零状态只作诊断。
- `taskkill` 启动失败仍映射 `Fatal`，root 未 reap 仍走 fallback 与 `TargetUnresponsive`。
- 新测试是否同时覆盖 TASK-367 的真实 `255` 形态和无 spawn 的负向路径。
