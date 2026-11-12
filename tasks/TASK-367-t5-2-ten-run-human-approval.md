# TASK-367 T5.2 ten-run human approval

- 状态：**Review**
- 阶段：1　子阶段：阶段级验收　批次：Stage 1 acceptance
- 依赖：TASK-364、TASK-366、ADR-0101、ADR-0102、ADR-0103
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `tasks/TASK-367-t5-2-ten-run-human-approval.md`
- `eval/tasks/edge/t5.2/run_ten_real_edge_ui_approvals.ps1`
- `eval/tasks/edge/t5.2/README.md`
- `eval/tasks/edge/t5.2/evidence/2026-11-12-task-367-ten-run/**`
- `eval/stage-1/**`
- `docs/audits/stage-1-t5.2-ten-run-2026-11-12.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`

## 目标

在人类连续在场并逐轮亲自确认最多十次的前提下，使用正式 desktop-ui、真实本地 Edge、专用
profile 与本地 T5.2 合成表单完成十轮 L3 approval/submit 验收，形成可被 Stage 1 矩阵严格消费
的十轮聚合摘要，并把 T5.2 从单次证据升级为十轮证据。

## In scope

- 新增十轮外层 runner，逐轮调用既有 `run_real_edge_ui_approval.ps1`。
- 每轮使用独立证据目录和专用 Edge profile，并按任务包固定端口顺序复用本地 8766。
- 聚合 `requested_runs`、`passed_runs`、`success_rate`、`result` 与逐轮 `rounds[]`。
- 每轮复核 `submit_form` 恰好一次、`submitted=true`、本地 query-free success URL、
  `submitted-marker`、人工决定前 result 不存在、token 不落盘与进程/端口收敛。
- 十轮完成后把 T5.2 matrix adapter 从单次人工摘要切到十轮 `round_summary`。
- 十轮通过门槛后验证 Stage 1 矩阵变为 9/9，并完成审计与状态同步。

## Out of scope

- 不修改生产代码、Policy、Tool/UI/DB schema、公共 API、依赖或既有证据。
- 不自动批准 L3，不由 agent 输入不可逆确认短语、点击 Approve 或代替人类操作。
- 不访问外部站点、真实账号、敏感数据、文件上传、支付、验证码或跨源流程。
- 不把失败轮、技术重试轮或未发生的确认伪装成成功十轮。
- 不因达到 8/10 而跳过具体失败轮审计或放宽矩阵判据。

## 必须遵守

1. 人类必须逐轮亲自检查正式 desktop-ui 审批卡、输入确认短语并点击 Approve。
2. 人工决定前 `form_submit` 调用数为 0；批准后最多一次，结果未知时立即停止且不重试。
3. token 只能存在于子进程环境，不得进入命令行、日志、截图、错误消息或证据文件。
4. 任一轮失败必须保留原始日志并显式记录，不得覆盖、删除或改写失败证据。
5. 至少 8/10 轮通过且每轮内部 postcondition 全满足，才允许更新 T5.2 矩阵证据。
6. 只有矩阵 9/9 且 `--require-complete` exit 0，才允许讨论 Stage 1 关闭。

## 验收命令

```powershell
pnpm --dir apps/desktop-ui build
pnpm --dir apps/desktop-ui tauri build --debug --no-bundle
cargo build -p assistant-agent-core
pwsh -NoProfile -File eval/tasks/edge/t5.2/run_ten_real_edge_ui_approvals.ps1 `
  -EvidenceRoot eval/tasks/edge/t5.2/evidence/2026-11-12-task-367-ten-run
python -m unittest discover -s eval/stage-1 -p "test_*.py" -v
python eval/stage-1/validate_acceptance_matrix.py
python eval/stage-1/validate_acceptance_matrix.py --require-complete
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
git diff --check
```

期望：十轮聚合摘要至少 8/10，且每轮均满足一次性提交、成功 marker、token 与资源收敛；
matrix report 输出 `stage1_complete=true`、`completed_tasks=9`、`missing_tasks=[]`；
`--require-complete` exit 0；全部测试与文档门禁通过。

## DoD

- [ ] 十轮 outer runner、README 与聚合 schema 落地。
- [ ] 人类亲自完成十轮确认；通过轮次严格记录，未通过轮次不掩盖。
- [ ] 每轮一次提交、成功 marker、query-free URL、token 与资源收敛证据可复核。
- [ ] T5.2 matrix 证据切换到十轮摘要，validator 与测试同步到 9/9。
- [ ] Stage 1 审计、PLAN、plans、README、LEDGER 与卡记录同步。
- [ ] Python、workspace、clippy、文档门禁与 `git diff --check` 全绿。
- [ ] PR checks、merge 与 post-merge main CI 证据回填。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-367 T5.2 ten-run human approval
【目标】由人类逐轮确认最多十次，形成正式 desktop-ui 的 T5.2 十轮验收证据并完成矩阵 9/9
【write scope】仅：本卡、T5.2 十轮 runner/README/新证据、eval/stage-1、阶段审计、PLAN/plans/README/LEDGER
【铁律】无静默失败；L3 必须人类确认；点击后结果未知不重试；失败证据不可改写；不放宽阶段门槛
【禁止】生产代码/Policy/schema/API/依赖改动；外部站点或敏感数据；agent 代替人类批准
【验收】至少 8/10 十轮证据；matrix 9/9 且 require-complete exit 0；全部测试与文档门禁绿
【依赖】TASK-364、TASK-366、ADR-0101/0102/0103 已核对
【疑问】无；人类已批准新建本卡并确认可在场逐轮确认最多十次
```

### 2. 实际改动文件

- `eval/tasks/edge/t5.2/run_ten_real_edge_ui_approvals.ps1`：十轮外层 runner、
  失败轮证据聚合、有界端口收敛检查和 `-FinalizeOnly`。
- `eval/tasks/edge/t5.2/README.md`：正式 Tauri 构建与十轮人工验收说明。
- `eval/tasks/edge/t5.2/evidence/2026-11-12-task-367-ten-run/**`：十轮 human
  approval 证据、两次点击前失败证据和 9/10 聚合摘要。
- `eval/stage-1/acceptance-matrix.json`、`validate_acceptance_matrix.py`、
  `test_acceptance_matrix.py`：T5.2 切换到十轮 `round_summary`，测试更新为阶段 9/9。
- `docs/audits/stage-1-t5.2-ten-run-2026-11-12.md`：十轮方法、失败轮、资源与安全审计。
- 本卡：状态与执行记录。

### 3. 验收输出摘要

- 十轮聚合：`requested_runs=10`、`passed_runs=9`、`failed_runs=1`、
  `success_rate=0.9`、`result=passed`。
- 唯一失败轮 `run-04`：服务端恰有一条成功 submit request；随后 Core 在
  Edge cleanup 阶段因 `taskkill` 返回 255（但 root reaped）而失败。
- 两次点击前失败保留在 `preapproval-failures/`：一次固定端口不匹配，一次
  Tauri dev URL 未加载正式前端；均未形成审批卡、未计为人工轮。
- 九次成功轮均满足 `submit_step_count=1`、`submitted=true`、query-free
  `post_url`、`submitted-marker`、批准前 result 不存在、token 不落盘。
- `python -m unittest discover -s eval/stage-1 -p "test_*.py" -v`：11/11 passed。
- `python eval/stage-1/validate_acceptance_matrix.py`：`stage1_complete=true`、
  `completed_tasks=9`、`missing_tasks=[]`。
- `python eval/stage-1/validate_acceptance_matrix.py --require-complete`：exit 0。
- `pnpm --dir apps/desktop-ui build`、Tauri `--debug --no-bundle`、Core build：通过。
- `pnpm --dir apps/desktop-ui test`：model 90/90、DOM 5/5 passed。
- `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test --workspace`：通过；workspace 含 xtask 510/510。
- `hygiene`、`memory-counts`、`adr-index`、`refscan`、`docscan`、
  `card-check`、`git diff --check`：exit 0。

### 4. DoD 逐条核对

- [x] 十轮 outer runner、README 与聚合 schema 落地。
- [x] 人类亲自完成十轮确认；9 轮通过，1 轮失败被显式保留。
- [x] 每轮一次提交、成功 marker、query-free URL、token 与资源收敛证据可复核。
- [x] T5.2 matrix 证据切换到十轮摘要，validator 与测试同步到 9/9。
- [ ] Stage 1 审计、PLAN、plans、README、LEDGER 与卡记录同步（PR 收口阶段完成）。
- [x] Python、workspace、clippy、文档门禁与 `git diff --check` 全绿。
- [ ] PR checks、merge 与 post-merge main CI 证据回填。

### 5. 偏差

- 固定 8766 任务包与首版外层端口分配不一致，已改回顺序复用固定端口；失败尝试
  `run-01-attempt-01` 保留。
- 直接 Cargo 构建启动的是 Tauri dev URL，审批卡未渲染；已改用
  `pnpm --dir apps/desktop-ui tauri build --debug --no-bundle`，失败尝试
  `run-01-attempt-02` 保留。
- `run-04` 在提交后遇到真实 cleanup 缺陷，未重试该轮，按失败计入 10 轮并保留原始
  Core/server 日志。
- 未修改生产代码、Policy、schema/API、依赖或旧证据。

### 6. 实施中发现的更合理做法

- 聚合器只对已经完成人工提交但单轮后置校验失败的轮次记录 `passed=false`，并要求
  server log 恰有一条成功提交请求；点击前失败不能冒充人工轮。
- 子进程 shutdown 日志可能瞬时落后于实际收敛，因此最终判据采用有界端口复查，
  同时保留原始 `shutdown.log` 不改写。

### 7. 遗留问题

- `crates/platform/windows/src/browser_launcher.rs::terminate_child_tree` 在 root
  已 reap 时仍把非零 `taskkill` 状态映射为 `TargetUnresponsive`；建议另立生产修复卡。
- PR checks、merge、post-merge main CI 与 Done 状态回填尚未完成。

### 8. 新增长期记忆

无新增；本卡范围内未修改 `docs/memory/**`。

### 9. 给审阅者的关注点

- 9/10 是否诚实保留 `run-04` 失败，而不是重试后掩盖。
- 两次点击前失败是否正确排除在人工十轮之外。
- T5.2 `round_summary` 适配器和 Stage 1 9/9 是否严格由证据驱动。
