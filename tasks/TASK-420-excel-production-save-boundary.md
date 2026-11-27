# TASK-420 Excel T2.2 production save and crash-recovery boundary

- 状态：**Review**
- 阶段：Stage 2 Excel T2.2 / production boundary decision

---

## write scope

- `tasks/TASK-420-excel-production-save-boundary.md`
- `docs/audits/stage-2-excel-t22-production-save-boundary-2026-11-27.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在开放 production Excel save caller、ToolBus 或 `tools.json` 之前，基于 ADR-0108 /
ADR-0110 / ADR-0111 与既有真实 COM、L1 transaction 和 failure-path 证据，明确下一阶段的
保存调用与 Host 崩溃恢复边界，并给出可直接拆卡的 ADR/spec 前置和实现顺序。

## In scope

- 只读核对 ADR、spec、TASK-407/409、TASK-415、TASK-418、TASK-419 和 production assembly 现状。
- 记录同一进程 save caller 与独立 helper process 的取舍。
- 给出安全、可验证的下一阶段顺序和明确的 NO-GO 边界。
- 同步 PLAN、README、LEDGER。

## Out of scope

- 不修改 ADR/spec、产品代码、公共 API、schema、ErrorCode、依赖或 Windows runner。
- 不实现 production save caller、helper process、tool assembly、ToolBus 或 `tools.json`。
- 不启动真实 Excel/COM/GUI，不执行用户工作簿保存或恢复。
- 不把本审计当作 production Tool 已开放的批准。

## 必须遵守

1. 现有真实 COM 证据与 private test 证据必须分开描述。
2. Host 整体崩溃恢复需求未获 ADR 明确前，fail-closed，不开放 production surface。
3. 推荐方案必须说明 helper process 的启动、认证、发现、回收和崩溃注入验收。
4. 不把 docs-only 决策冒充实现完成。

## 验收

```powershell
cargo run -p xtask -- check-ledger
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
git diff --check
```

预期：审计明确当前 NO-GO、推荐方案和后续拆卡顺序；production surface 零改动。

## DoD

- [ ] 审计列出已满足的 private/real-COM 证据与实际缺口。
- [ ] 审计给出同一进程 caller 与 helper process 的决策矩阵。
- [ ] 审计给出推荐方案、ADR/spec 前置和实现顺序。
- [ ] 明确 production save/ToolBus/tools.json 仍不得开放。
- [ ] 文档门禁、PR/CI/merge/main CI 证据完成。

---

## 执行记录

### 1. 约束回执

```text
【任务】TASK-420 Excel T2.2 production save and crash-recovery boundary
【目标】在 production save caller 前明确 Host 崩溃恢复与 helper process 边界
【write scope】仅本卡、审计报告、PLAN/README/LEDGER
【铁律】contract-first；证据分层；未决需求 fail-closed；不伪造 production 完成
【禁止】不改 ADR/spec/产品代码/API/schema/依赖；不实现 ToolBus/helper；不启动真实 Excel/COM/GUI
【验收】审计 + check-ledger/hygiene/docscan/card-check/memory-counts/adr-index + diff check
【依赖】TASK-407/409/415/418/419；ADR-0108/0110/0111
【疑问】Host 崩溃恢复是否列为 production 必需能力；本卡默认按 PLAN 的独立 helper process 缺口暂定 NO-GO
```

### 2. 实际修改

- 新增 production save / crash-recovery 边界审计，记录现有私有与真实 COM 证据、三项方案对比、
  NO-GO 边界和后续 ADR/spec 前置。
- 未修改 ADR/spec、产品代码、公共 API、schema、依赖、ToolBus 或 `tools.json`。

### 3. 验收记录

- 审计报告已落盘；待运行 `check-ledger`、`hygiene`、`docscan`、`card-check`、
  `memory-counts`、`adr-index` 与 `git diff --check`。

### 4. DoD 核对

待完成后回填。

### 5. 偏差

无。

### 6. 新增长期记忆

无。

### 7. 遗留问题与下一步

- 需要在新的 ADR/spec 中决定 Host 崩溃是否为 production 必需能力；在本卡审计结论下，
  helper process 是推荐但尚未批准的长期方案。

### 8. 外部证据

- 待提交后回填 PR / CI / merge / main CI。

### 9. 给审阅者的关注点

- 核对审计没有把 test-only real COM 证据当成 production caller。
- 核对 Host 崩溃恢复的决定没有被默认假设成已批准。
- 核对 helper process 的认证、发现和回收边界足够具体，可直接拆实现卡。
