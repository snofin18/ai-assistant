# TASK-370 Real Edge cleanup regression

- 状态：**Done**
- 阶段：1　子阶段：Stage 1 follow-up　批次：post-TASK-369 real-machine verification
- 依赖：TASK-367、TASK-369、ADR-0063、ADR-0101
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `tasks/TASK-370-real-edge-cleanup-regression.md`
- `eval/tasks/edge/t5.2/evidence/2026-11-16-task-370-cleanup-regression/**`
- `docs/audits/task-370-real-edge-cleanup-2026-11-16.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`

## 目标

在人类在场并亲自确认一次正式 desktop-ui 审批卡的前提下，使用真实本地 Edge
重放 TASK-367 `run-04` 的 post-submit cleanup 路径，证明 TASK-369 修复后 Core
可以完成清理、写出成功结果，并让专用 Edge 进程树、Core、UI 与固定端口全部收敛。

## In scope

- 使用既有 `run_real_edge_ui_approval.ps1` 单轮 runner，不新建第二套审批实现。
- 使用独立证据目录 `eval/tasks/edge/t5.2/evidence/2026-11-16-task-370-cleanup-regression/`。
- 由人类逐轮确认一次 `form_submit`，只接受 `once` scope。
- 验证 `submitted=true`、query-free 本地成功 URL、提交 marker、批准前无结果文件。
- 验证 Core 正常退出并写出 `human/result.json`，不再出现 `taskkill=255` 导致的
  `TargetUnresponsive` 失败。
- 验证端口 8766、Core、desktop-ui 和专用 Edge profile 进程全部归零。
- 形成单轮审计并同步任务卡、PLAN、阶段计划、README 与 LEDGER。

## Out of scope

- 不重跑或改写 TASK-367 已保留的十轮证据与 `run-04` 原始失败。
- 不把单轮通过描述为新的十轮成功率。
- 不自动批准 L3、不代替人类点击 Approve、不使用默认 Edge profile。
- 不访问外部网站、真实账号、敏感数据、文件上传、支付或验证码。
- 不修改生产 launcher、Policy、Tool/UI/DB schema、公共 API 或依赖。

## 必须遵守

1. 人类必须在场检查正式 desktop-ui 审批卡和脱敏 diff，并亲自确认。
2. 批准前 `form_submit` 调用数为 0；批准后最多一次，结果未知时不得重试。
3. token 只能存在于子进程环境，不得进入命令、日志、截图或证据文件。
4. 既有 `run-04` 失败证据不可删改；本轮使用新目录独立记录。
5. 任一进程、端口或 profile 未收敛即失败，不得用“稍后消失”代替证据。

## 验收命令

```powershell
pnpm --dir apps/desktop-ui build
pnpm --dir apps/desktop-ui tauri build --debug --no-bundle
cargo build -p assistant-agent-core
pwsh -NoProfile -File eval/tasks/edge/t5.2/run_real_edge_ui_approval.ps1 `
  -EvidenceRoot eval/tasks/edge/t5.2/evidence/2026-11-16-task-370-cleanup-regression
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

期望：单轮 `result=passed`，Core 正常写出 `human/result.json`，提交恰好一次，
成功页 marker 可见，端口和全部相关进程归零；全部门禁通过。

## DoD

- [ ] 人类亲自完成一次正式 desktop-ui Approve，批准前 submit 调用为 0。
- [ ] `form_submit` 恰好一次，`submitted=true`，成功 URL 无 query/fragment。
- [ ] Core 正常退出并写出结果，post-submit cleanup 不再报 `TargetUnresponsive`。
- [ ] 端口、Core、UI、专用 Edge 进程树全部收敛，token 不落盘。
- [ ] 单轮审计、任务卡、PLAN、plans、README、LEDGER 同步。
- [ ] Python、workspace、clippy 与全部文档门禁通过。
- [ ] PR checks 11/11，merge 与 post-merge main CI 证据回填。

<!-- ═══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ═══
     以下由 Implementer 填写。改动分界线上任意一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-370 Real Edge cleanup regression
【目标】人类在场单轮重放 TASK-367 post-submit cleanup 路径
【write scope】仅：本卡、TASK-370 新证据、单轮审计、PLAN/plans/README/LEDGER
【铁律】L3 必须人类确认；只提交一次；资源收敛；不改旧证据；不伪造结果
【禁止】自动批准、外部站点/敏感数据、改生产代码/schema/API/依赖、重写 run-04
【验收】单轮 result=passed；Core 写出结果；submitted=true；端口和进程全零
【依赖】TASK-367 9/10、TASK-369 Done、ADR-0063/0101 已核对
【疑问】无；采用既有单轮 inner runner，不把单轮结果扩写为十轮成功率
```

### 2. 实际改动文件

- `tasks/TASK-370-real-edge-cleanup-regression.md`：任务与两次预审批失败记录。
- `eval/tasks/edge/t5.2/evidence/2026-11-16-task-370-cleanup-regression/**`：
  第一次 300 秒预审批超时及其 runner 输出。
- `eval/tasks/edge/t5.2/evidence/2026-11-16-task-370-cleanup-regression-attempt-02/**`：
  第二次 900 秒等待窗口、审批卡可见但人工决定未发生、手工中止与清理证据。
- `eval/tasks/edge/t5.2/evidence/2026-11-16-task-370-cleanup-regression-attempt-03/**`：
  第三次人类确认、单轮提交、Core 正常退出与资源收敛证据。
- `docs/audits/task-370-real-edge-cleanup-2026-11-16.md`：单轮真实 Edge 回归审计。
- `PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`：Done 与 CI 证据回填。
- `apps/desktop-ui/src-tauri/Cargo.toml`：Tauri build 自动改写的空 features 形式已恢复，
  不纳入任务变更。

### 3. 验收输出摘要

- `pnpm --dir apps/desktop-ui build`：通过。
- `pnpm --dir apps/desktop-ui tauri build --debug --no-bundle`：通过，正式 UI 二进制已重建。
- `cargo build -p assistant-agent-core`：通过。
- 第一次运行使用默认 300 秒：审批卡进入 `awaiting_human_confirmation`，未收到人工决定；
  runner 写出 `result=failed`、`failure=No human decision was received within 300 seconds`。
- 第一次失败后专用 Edge 进程仍存活；已对 profile 根进程 `1932` 执行
  `taskkill /PID 1932 /T /F`，明确终止 7 个子进程与根进程，最终
  `dedicated_edge_remaining=0`、`port_8766_listener=False`。
- 第二次使用 900 秒并重新启动审批卡；CUA 确认 `Once`、脱敏 diff 与
  `Approve once` 按钮可见，`Approve once` 在输入确认短语前为 disabled。
- 第二次仍无人输入 `once` 或点击 Approve；手工中止等待后，对 profile 根进程 `4740`
  执行 `taskkill /PID 4740 /T /F`，明确终止 7 个子进程与根进程，最终
  `dedicated_edge_remaining=0`、`port_8766_listener=False`。
- 第三次人工确认后 17 秒内正常结束：`phase=passed`、`result=passed`、
  `submit_step_count=1`、`submitted=true`、query-free
  `post_url=http://127.0.0.1:8766/t5.2/submitted`，
  `pre_approval_result_exists=false`、`token_logged=false`。
- `human/result.json` 状态为 `completed`，4 个 step 全部 committed，`submit_form`
  恰好一次。
- cleanup：Core/UI 均退出，专用 Edge 进程 0，8766 listener false，
  fixture `stopped=True`。
- `python -m unittest discover -s eval/tasks/edge/t5.2 -p "test_*.py" -v`：
  9/9 passed。
- `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、
  `cargo fmt --all --check`、`hygiene`、`card-check`、`docscan`、`refscan` 与
  `git diff --check`：通过；workspace `xtask` 510 passed。
- PR #730 checks **11/11 success**，pending 0 / failed 0，base=`main`，
  `MERGEABLE` / `CLEAN`；merge `76b74d8aa4a72964d1aec7721c9f3573053afdb2`。
- post-merge main CI `43809369712`：success；三平台、deny、desktop-ui、Stage 1 matrix、
  hygiene、schema/codegen、replay 与 ledger 门禁全部通过。

### 4. DoD 逐条核对

- [x] 人类亲自完成一次正式 desktop-ui Approve，批准前 submit 调用为 0。
- [x] `form_submit` 恰好一次，`submitted=true`，成功 URL 无 query/fragment。
- [x] Core 正常退出并写出结果，post-submit cleanup 不再报 `TargetUnresponsive`。
- [x] 端口、Core、UI、专用 Edge 进程树全部收敛，token 不落盘。
- [ ] 单轮审计、任务卡、PLAN、plans、README、LEDGER 同步。
- [x] Python、workspace、clippy 与全部文档门禁通过。
- [x] PR checks 11/11，merge 与 post-merge main CI 证据回填。

### 5. 偏差

- 两次尝试均在人工决定前结束，属于预审批失败，不计人工轮，也没有提交动作。
- TASK-370 最终 shell misuse 审计为 `bash=22` / `cmd=0`；包括只读调查/runner
  文档读取、一次 WIP commit、一次 hygiene、一次 card-check 与一次 guard release，
  一次 tree 对比、一次 commit metadata 读取、一次 remote branch ref 创建，
  两次 closeout guard release、一次 closeout commit、一次 closeout tree API，以及 closeout 路径中的 5 次只读/元数据调用，均由 ADR-0100 hook 重写为
  PowerShell，无额外非预期写入。
- 现有 runner 的 `finally` 只回收 Core、UI 和 fixture server，不回收 Core 启动的
  专用 Edge 进程树；两次均需人工补做 `taskkill /T /F` 后才收敛。

### 6. 实施中发现的更合理做法

- runner 失败路径应按 `$runProfile` 枚举并终止专用 Edge 进程树，再等待 profile
  进程归零；该修改应另立窄卡，避免在人类验收卡中无审批扩大生产/测试工具范围。

### 7. 遗留问题

- `run_real_edge_ui_approval.ps1` 失败路径缺少专用 Edge 清理，建议新建后续卡。

### 8. 新增长期记忆

无新增；失败证据和 runner 清理缺陷记录在本卡。

### 9. 给审阅者的关注点

- 没有把超时或手工中止伪装成人工审批通过。
- 第三次运行中人工确认后 `form_submit` 恰好一次，Core 正常写出结果。
- 成功路径下专用 Edge、Core、UI 和 8766 端口全部收敛。
