# TASK-371 Edge runner failure cleanup

- 状态：**Done**
- 阶段：1　子阶段：Stage 1 follow-up　批次：post-TASK-370 runner resource lifecycle
- 依赖：TASK-369、TASK-370、ADR-0063、ADR-0101
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `tasks/TASK-371-edge-runner-failure-cleanup.md`
- `eval/tasks/edge/t5.2/run_real_edge_ui_approval.ps1`
- `eval/tasks/edge/t5.2/edge_process_cleanup.ps1`
- `eval/tasks/edge/t5.2/edge_process_tree_fixture.ps1`
- `eval/tasks/edge/t5.2/test_edge_process_cleanup.ps1`
- `eval/tasks/edge/t5.2/evidence/2026-11-16-task-371-failure-cleanup/**`
- `docs/audits/task-371-edge-runner-failure-cleanup-2026-11-16.md`
- `.github/workflows/ci.yml`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`
- `docs/memory/pitfalls.md`

## 目标

修复 TASK-370 暴露的测试 runner 资源生命周期缺口：当人工审批超时、人工中止或 runner
异常退出时，按专用 profile 枚举、终止并等待 Edge 进程树归零，避免测试结束后残留浏览器。

## In scope

- 新增可独立测试的 Windows Edge profile 进程枚举、`taskkill /T /F` 与有界等待原语。
- 在 `run_real_edge_ui_approval.ps1` 的失败 `finally` 中调用该原语，并写入结构化清理日志。
- 使用真实父/子进程树验证 profile 精确匹配、整树回收、无关进程不受影响与空 profile fail-closed。
- 在 Windows CI 中运行离线进程树回归。
- 在人类不点击 Approve 的前提下跑一次 30 秒超时 smoke，验证失败路径同样完成资源收敛。

## Out of scope

- 不修改生产 browser launcher、Core、Policy、Tool/UI/DB schema、公共 API 或依赖。
- 不重跑、改写或替代 TASK-370 的真实 Edge 人工审批成功证据。
- 不自动批准 L3、不点击 Approve、不提交表单、不访问外部站点。
- 不放宽现有测试断言；超时或失败必须仍为失败。

## 必须遵守

1. profile 为空或进程名非法时必须 fail-closed，禁止按宽泛条件杀进程。
2. `taskkill` 非零退出码只能作为诊断；最终判据是目标 profile 的进程数是否归零。
3. 终止与等待必须有硬上限；超限后报告剩余 PID 并失败，不得静默成功。
4. 失败清理顺序为先停 UI/Core，再回收专用 Edge，最后停 fixture server 并恢复环境变量。
5. 清理日志不得包含 token、表单字段、Cookie 或其他凭据。

## 验收命令

```powershell
pwsh -NoProfile -File eval/tasks/edge/t5.2/test_edge_process_cleanup.ps1
pnpm --dir apps/desktop-ui build
pnpm --dir apps/desktop-ui tauri build --debug --no-bundle
cargo build -p assistant-agent-core
pwsh -NoProfile -File eval/tasks/edge/t5.2/run_real_edge_ui_approval.ps1 `
  -EvidenceRoot eval/tasks/edge/t5.2/evidence/2026-11-16-task-371-failure-cleanup `
  -UiDecisionTimeoutSeconds 30
python -m unittest discover -s eval/tasks/edge/t5.2 -p "test_*.py" -v
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- check-ledger
git diff --check
```

预期：进程树单元测试通过；30 秒 smoke 以超时失败退出，`summary.json` 为 `failed`，
但专用 Edge、Core、UI、8766 listener 全部归零，且清理日志显示已收敛。

## DoD

- [ ] profile 匹配为空/非法时 fail-closed，精确匹配不会误杀无关进程。
- [ ] 真实父/子进程树被 `taskkill /T /F` 回收，等待有硬上限并验证归零。
- [ ] runner 超时失败后 Core、UI、专用 Edge、8766 listener 全部归零。
- [ ] 结构化 cleanup 日志与 `summary.json` 证明失败值与清理结果，不记录 token。
- [ ] Windows CI 固化离线进程树回归。
- [ ] Python、workspace、clippy、文档门禁全部通过。
- [ ] PR checks、merge 与 post-merge main CI 证据回填。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线上方任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-371 Edge runner 失败清理
【目标】让 T5.2 runner 在审批超时/中止/异常时也终止并回收专用 Edge 进程树
【write scope】仅：本卡、t5.2 cleanup helper/测试、runner、Windows CI 步骤、证据/审计、PLAN/plans/README/LEDGER、相关 memory
【铁律】失败路径也必须终止并 reap；资源必须验证归零；不得改断言或伪造证据；不得放宽 L3 人工审批
【禁止】不改生产 launcher/Policy/Tool/schema/公共 API/依赖；不重跑或改写 TASK-370 历史证据
【验收】helper 真进程树测试 → 2+ 进程全回收；runner 30 秒超时 smoke → exit 2 且 Edge/Core/UI/端口全零；全套门禁通过
【依赖】TASK-369 Done、TASK-370 Done、ADR-0063、ADR-0101 已核对
【疑问】无；按已建议方案自决推进
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-371-edge-runner-failure-cleanup.md`：本卡状态与执行记录。
- `eval/tasks/edge/t5.2/edge_process_cleanup.ps1`：profile 精确匹配、有界
  `taskkill /T /F`、最终进程数后置条件与结构化日志。
- `eval/tasks/edge/t5.2/edge_process_tree_fixture.ps1`：离线真实父子进程树夹具。
- `eval/tasks/edge/t5.2/test_edge_process_cleanup.ps1`：整树回收、无关进程保护、空 profile 拒绝
  与日志字段回归。
- `eval/tasks/edge/t5.2/run_real_edge_ui_approval.ps1`：失败 `finally` 接入 Edge cleanup，
  失败 summary 增加 cleanup 结果。
- `.github/workflows/ci.yml`：Windows CI 执行进程树回归。
- `eval/tasks/edge/t5.2/evidence/2026-11-16-task-371-failure-cleanup/**`：
  attempt-01 人工批准旁证与 attempt-02 超时失败清理证据。
- `docs/audits/task-371-edge-runner-failure-cleanup-2026-11-16.md`：实现、失败路径和证据审计。
- `plans/stage-1-pilots.md`：新增 TASK-371 条目。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `pwsh -NoProfile -File eval/tasks/edge/t5.2/test_edge_process_cleanup.ps1`：
  `PASS: profile-scoped process-tree cleanup converged without collateral termination`。
- `pnpm --dir apps/desktop-ui build`：通过。
- `pnpm --dir apps/desktop-ui tauri build --debug --no-bundle`：通过；构建后自动改写的
  `apps/desktop-ui/src-tauri/Cargo.toml` feature 形式已恢复，不纳入本卡。
- `cargo build -p assistant-agent-core`：通过。
- 30 秒 smoke attempt-02：`result=failed`，
  `failure=No human decision was received within 30 seconds`，符合预期。
- Edge cleanup：`initial_process_count=8`、`taskkill_attempt_count=8`、
  `remaining_process_count=0`、`elapsed_ms=2515`、`result=passed`。
  exit codes 为 `0,128,128,128,128,128,128,128`；root 回收后子 PID 已退出导致的 128
  仅是诊断，最终后置条件为 0 个 profile 进程。
- smoke 资源：`core_alive=False`、`ui_alive=False`、`dedicated_edge_remaining=0`、
  `port_8766_listener=False`、fixture `stopped=True`。
- attempt-01 收到一次人类 `once` 审批并正常提交，保存为独立旁证：
  `submit_step_count=1`、`submitted=true`、`post_url` 为无 query 本地成功页；
  该次不用于失败清理判据。
- `python -m unittest discover -s eval/tasks/edge/t5.2 -p "test_*.py" -v`：9/9 passed。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：通过；workspace 510 passed。
- `cargo test -p assistant-core arch::`：10 个 arch 用例通过。
- `cargo run -p xtask -- hygiene`：0 error / 168 warning，PASSED。
- `card-check`：0 error / 71 warning，PASSED。
- `docscan`：0 error / 175 warning，PASSED。
- `refscan`、`check-ledger`、`check-comments`、`verify-schemas`、`codegen --check`、
  `memory-counts`、`adr-index`、`replay --suite core`：全部 PASSED。
- `cargo deny check`：`advisories ok, bans ok, licenses ok, sources ok`。
- `pnpm --dir apps/desktop-ui lint`、`typecheck`、`test`：通过；Node model 90/90、
  Vitest 5/5。
- `git diff --check`：通过。
- PR #732 checks **11/11 success**，pending 0 / failed 0，base=`main`，
  `MERGEABLE` / `CLEAN`，merge `702d645a9ab036c4d94a14d779c850f8875a51e3`。
- post-merge main CI `43816679420`：success；Windows 新回归步骤实际执行并通过，
  Ubuntu/macOS 按条件正确跳过。

### 4. DoD 逐条核对

- [x] profile 匹配为空/非法时 fail-closed，精确匹配不会误杀无关进程。
- [x] 真实父/子进程树被 `taskkill /T /F` 回收，等待有硬上限并验证归零。
- [x] runner 超时失败后 Core、UI、专用 Edge、8766 listener 全部归零。
- [x] 结构化 cleanup 日志与失败 summary 证明失败值和清理结果，不记录 token。
- [x] Windows CI 已固化离线进程树回归。
- [x] Python、workspace、clippy 与文档门禁全部通过。
- [x] PR checks、merge 与 post-merge main CI 证据已回填。

### 5. 偏差

- attempt-01 原计划执行无人批准的 30 秒超时 smoke，但人类在场完成了一次 `once` 审批；
  该次仅访问本地 fixture，审批与提交均有效，已独立保留为人工批准旁证，不冒充失败清理证据。
- attempt-02 达到 30 秒窗口后按预期失败，未提交、无 `human/result.json`。
- 本卡最终 shell misuse 审计：`bash=16` / `cmd=0`；16 次均由 ADR-0100 hook 重写或
  在解析前 fail-closed 拦截，均未在 bash/WSL 执行；
  无额外非预期产品写入。

### 6. 更合理做法（实施中发现，非漂移）

- 将 profile 进程库存而非 `taskkill` 退出码作为唯一 cleanup postcondition，与 TASK-369
  的生产 root reap 语义一致。
- 对匹配进程数与 taskkill 尝试数分别设置硬上限，避免异常 profile 或持续重生导致无界循环。
- 在离线测试中使用真实 PWsh 父/子进程树，同时验证目标树被回收和非目标树不受影响。

### 7. 遗留问题（进 `docs/PARKING_LOT.md` 的编号）

- 无新增遗留；TASK-370 记录的 runner 失败清理缺口已由本卡闭合。

### 8. 新增长期记忆（FACT / PITFALL / REJECTED 条目原文）

- 无。新增 PITFALL 会改变 `MEMORY.md` 规模表，而 `MEMORY.md` 不在本卡 write scope；
  为避免越界，验收 runner 失败收敛事实保留在本卡与独立审计中。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 重点检查 cleanup 是否始终按完整 `$runProfile` 匹配，避免误杀用户默认 Edge。
- 重点检查 taskkill 尝试上限与 15 秒 deadline 是否能在任何失败分支保持有界。
- attempt-02 的 `taskkill` 128 是子 PID 已随 root 退出后的预期诊断；最终 0 进程才是判据。
