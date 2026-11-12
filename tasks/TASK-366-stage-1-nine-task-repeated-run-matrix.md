# TASK-366 Stage 1 nine-task repeated-run acceptance matrix

- 状态：**InProgress**
- 阶段：1　子阶段：阶段级验收　批次：Stage 1 acceptance
- 依赖：TASK-105、TASK-044～046、TASK-056、TASK-364、TASK-365
- 本文件 = 卡片正文 + 执行记录（ADR-0031）。

---

## write scope

- `tasks/TASK-366-stage-1-nine-task-repeated-run-matrix.md`
- `eval/stage-1/**`
- `.github/workflows/ci.yml`
- `docs/audits/stage-1-repeated-run-matrix-2026-11-12.md`
- `PLAN.md`
- `plans/stage-1-pilots.md`
- `README.md`
- `LEDGER.md`

## 目标

把 Stage 1 的九个权威任务（T1.1～T1.3、T3.1～T3.3、T5.1～T5.3）映射到机器可读证据，
并通过严格 validator 输出 `stage1_complete`、每任务轮次与完整缺口。单次成功、缺失文件、
伪造轮次或低于阈值的成功率都不能被静默折算为阶段通过。

## In scope

- 固定九任务 app / task id / 最低成功率 / 证据路径的 manifest。
- 聚合 Notepad runs JSON、Paint ten-run summary、Edge T5.1 summary、T5.2 单次人工摘要与
  T5.3 十轮安全摘要。
- 新增 T5.3 十轮安全 runner，生成冻结、可复核的 summary。
- 新增 validator 与单元测试，覆盖缺失证据、轮次不符、成功率不足、字段冲突与正常缺口。
- 在现有 CI `check` matrix 中输出 Stage 1 验收矩阵报告并执行 validator tests。
- 完成 2026-11-12 阶段矩阵审计，明确阶段是否可关闭。

## Out of scope

- 不启动真实 Paint、Notepad、Edge 或外部站点。
- 不修改任何既有 runs / summary / CSV / fixture 证据。
- 不自动批准 T5.2 L3 提交，不伪造十轮人类确认。
- 不修改生产代码、Policy、Tool/UI/DB schema、公共 API 或第三方依赖。

## 必须遵守

1. 九个 task id 固定且唯一；每个 app 恰好三个任务。
2. 任务通过必须同时满足轮次、最低成功数、最低成功率和证据内部一致性。
3. 证据缺失或格式损坏是 validator error，不得降级为普通“未完成”。
4. 可读但不足十轮的 T5.2 证据必须令 `stage1_complete=false`。
5. 不得因为 8/9 通过就把阶段 1 标记 Done。

## 验收命令

```powershell
python eval/stage-1/run_security_ten.py
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

期望：普通 report 成功并输出 `stage1_complete=false`、`completed_tasks=8`、`missing_tasks=["t5.2"]`；
`--require-complete` 必须 exit 1；所有 validator / 文档 / workspace 门禁全绿。

## DoD

- [ ] 九任务 manifest 与严格 schema 校验落地。
- [ ] T5.3 十轮安全摘要 10/10 落盘。
- [ ] validator 正确聚合现有 8/9，并只把 T5.2 标为缺失十轮。
- [ ] 缺失证据、伪造轮次、低成功率和 schema 错误均有负向单测。
- [ ] CI 每次 PR 输出结构化 Stage 1 报告并运行 validator tests。
- [ ] 阶段审计明确 Stage 1 仍为 InProgress。
- [ ] Python、workspace、clippy、文档门禁与 `git diff --check` 全绿。
- [ ] PR、合并与 post-merge main CI 证据回填；进度文件同步。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何一行 = 漂移触发器 ⑤。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-366 Stage 1 nine-task repeated-run acceptance matrix
【目标】建立九任务严格证据矩阵，自动判定阶段可关闭性并保留 T5.2 十轮缺口
【write scope】仅：本卡、eval/stage-1/**、CI、阶段矩阵审计、PLAN/plans/README/LEDGER
【铁律】不得用单次成功冒充十轮；缺证据必须失败；不放宽阶段门槛；不伪造成功率
【禁止】真实 GUI/外网/凭据；生产代码、Policy、schema/API、依赖改动；自动 L3 批准
【验收】T5.3 10/10；validator report 8/9 且 missing=t5.2；require-complete exit 1；全门禁绿
【依赖】TASK-105、TASK-044～046、TASK-056、TASK-364、TASK-365 已核对为 Done
【疑问】无；人类已批准新建本卡并按上述范围执行
```

### 2. 实际改动文件

- `eval/stage-1/acceptance-matrix.json`：九任务 app / threshold / adapter / evidence 映射。
- `eval/stage-1/validate_acceptance_matrix.py`：严格结构校验、证据聚合、报告与 `--require-complete`。
- `eval/stage-1/run_security_ten.py`：T5.3 注入安全回归十轮 runner。
- `eval/stage-1/evidence/t5.3-injection-security-ten-run.json`：10/10 冻结摘要。
- `eval/stage-1/test_acceptance_matrix.py`：10 个正向 / 负向 validator 单测。
- `eval/stage-1/README.md`：矩阵与 runner 使用边界。
- `.github/workflows/ci.yml`：三平台 check matrix 增加 Stage 1 matrix tests 与报告步骤。
- `docs/audits/stage-1-repeated-run-matrix-2026-11-12.md`：当前 8/9 与 T5.2 缺口审计。
- `PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`：状态与证据同步。

### 3. 验收输出摘要

- `python eval/stage-1/run_security_ten.py`：T5.3 `10/10`，result=passed。
- `python -m unittest discover -s eval/stage-1 -p "test_*.py" -v`：
  **10 passed / 0 failed**。
- 普通 matrix report：`stage1_complete=false`、`completed_tasks=8`、`missing_tasks=["t5.2"]`。
- `--require-complete`：按预期 exit 1，并明确输出 `Stage 1 acceptance is incomplete: t5.2`。
- T5.2 可读证据为单次 human approval；validator 输出
  `insufficient_runs:1/10` 与 `insufficient_passes:1/8`。
- `cargo test --workspace`、all-targets clippy：通过；workspace 含 xtask 510/510。

### 4. DoD 逐条核对

- [x] 九任务 manifest 与严格 schema 校验落地。
- [x] T5.3 十轮安全摘要 10/10 落盘。
- [x] validator 正确聚合现有 8/9，并只把 T5.2 标为缺失十轮。
- [x] 缺失证据、伪造轮次、低成功率和 schema 错误均有负向单测。
- [x] CI 每次 PR 输出结构化 Stage 1 报告并运行 validator tests。
- [x] 阶段审计明确 Stage 1 仍为 InProgress。
- [ ] Python、workspace、clippy、文档门禁与 `git diff --check` 全绿。
- [ ] PR、合并与 post-merge main CI 证据回填；进度文件同步。

### 5. 偏差

none。未修改生产代码、Policy、Tool/UI/DB schema、公共 API 或第三方依赖。

### 6. 实施中发现的更合理做法

- 缺失或损坏证据与“证据可读但轮次不足”分级处理：前者 hard error，后者输出
  `stage1_complete=false`。这样 CI 不会因已知阶段缺口永久红灯，也不会让损坏证据静默通过。
- 负向单测发现临时矩阵路径不在仓库内时错误信息会触发 `relative_to` 异常；已改为仓库内路径
  相对显示、仓库外路径原样显示，验收判据不变。

### 7. 遗留问题

- T5.2 仍缺人类在场的十轮确认提交证据。下一步应由独立 human-present 卡提供十轮 runner
  与逐轮机器摘要；不得自动批准 L3，也不得把 TASK-364 单次成功扩写为十轮。

### 8. 新增长期记忆

无新增。

### 9. 给审阅者的关注点

- validator 是否把缺失/损坏证据和已知未完成正确区分。
- T5.2 是否仍被严格判为 1/10，且 `--require-complete` 保持 exit 1。
- CI 报告步骤是否保留 11 个 job，并持续输出 `missing_tasks`。
