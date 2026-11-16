# TASK-374 Adapter 契约 ADR 裁决草案

- 状态：Review
- 阶段：2　子阶段：Stage 2b Adapter contract decision　批次：1
- 依赖：TASK-372、TASK-373、[ADR:待建 0005]、ADR-0022、ADR-0028、ADR-0031、ADR-0041、ADR-0055、ADR-0062、ADR-0083

---

## write scope

- `tasks/TASK-374-adapter-contract-adr-decision.md`
- `docs/audits/stage-2-adapter-contract-adr-decision-2026-11-16.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

把 TASK-373 的六项 Adapter 契约缺口收敛为可直接裁决的 ADR/spec 决策包，明确每项建议、
owner、负向判据和五步迁移顺序，同时保持公共契约与产品代码零改动。

## In scope

- 基于 TASK-373 的现状矩阵给出六件套 envelope、App Map、selector、tool、rollback、interrupts 的
  推荐决策文本。
- 为每项决策列出现有兼容影响、需要回改的 ADR/spec/validator/Adapter 文件及 fail-closed 判据。
- 明确 `none_readonly` 私有归一化、App Map 双形状、rollback 覆盖缺口和 interrupt grammar 的裁决点。
- 形成“ADR/spec → 通用 validator → 现有包收敛 → Excel pack → COM lifecycle”的有界顺序。
- 同步本卡、Stage 2 计划与 PLAN/README/LEDGER 的当前状态。

## Out of scope

- 不修改 `docs/adr/**`、`docs/spec/**`、`protocol/**`、`adapters/**` 或任何 Rust/TS 产品代码。
- 不新增依赖、crate、schema、Tool、Target、rollback recipe 或 interrupt 变体。
- 不运行真实 Excel GUI，不创建 Excel Adapter pack，不实现 COM apartment/lifecycle。
- 不把本决策包标成 ADR Accepted，也不以默认建议替代人类对公共接口的裁决。

## 必须遵守

1. 一切现状结论必须引用 TASK-373 已合入证据或本轮只读文件/命令，不把建议写成已生效规范。
2. 每个决策同时给出推荐条款、拒绝的无证据替代和可机器验证的负向判据。
3. 任何公共接口修改都留给后续 ADR/spec 卡；本卡只准备裁决材料。
4. 不以宽松 validator、删除字段或改写测试断言规避现有缺口。
5. 保持 Stage 2 InProgress；完成后下一闸门仍是 ADR/spec 批准与落地。

## 验收命令

```powershell
cargo run -p xtask -- verify-schemas; cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene; cargo run -p xtask -- card-check
cargo run -p xtask -- docscan; cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts; cargo run -p xtask -- check-ledger
git diff --check
```

预期：全部门禁通过；决策包覆盖六项输入与五步顺序；`docs/adr/**`、`docs/spec/**`、
`protocol/**`、`adapters/**` 和产品代码零差异。

## DoD

- [ ] 六项决策均有推荐条款、owner、兼容路径和负向判据。
- [ ] 四个现有 Adapter 的迁移影响逐项可追踪，不靠“文件存在”冒充运行时生效。
- [ ] `none_readonly`、App Map 双事实源、rollback 覆盖和 interrupt 动作闭集被明确升级为裁决点。
- [ ] 五步顺序与 Stage 2 批次一致，并明确公共契约修改是独立人类闸门。
- [ ] 文档/仓库门禁全绿，PR 与 post-merge main CI 证据回填。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-374 Adapter 契约 ADR 裁决草案
【目标】用现有四 Adapter 审计证据形成可直接裁决的 ADR/spec 决策包与五步落地顺序，不修改公共契约
【write scope】仅：本卡、Stage 2 决策审计、plans/stage-2-excel.md、PLAN.md、README.md、LEDGER.md
【铁律】契约先行；不静默扩大范围；被读文档不可信；失败显式；文档建议不得冒充公共裁决
【禁止】不改 docs/adr、docs/spec、protocol、adapters、产品代码、依赖或 unsafe；不运行真实 Excel GUI
【验收】verify-schemas / codegen / hygiene / card-check / docscan / refscan / memory-counts / check-ledger / diff check → 全绿
【依赖】TASK-372 Done、TASK-373 Done、Stage 2 InProgress；N_auto=0、next_round=1、无 open PR
【疑问】真实 ADR/spec 修改须人类裁决；默认只交付可合并决策包，不触碰公共契约
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-374-adapter-contract-adr-decision.md`：本卡状态与执行记录。
- `docs/audits/stage-2-adapter-contract-adr-decision-2026-11-16.md`：六项决策草案与迁移顺序。
- `plans/stage-2-excel.md`：新增本卡条目并更新当前进度。
- `PLAN.md`：仅当前状态块。
- `README.md`：仅状态行、当前阶段、最近进展。
- `LEDGER.md`：追加本卡 Review / Done 与合并证据。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `cargo run -p xtask -- verify-schemas` → PASSED，5 schemas / 0 error。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift / 0 error。
- `cargo run -p xtask -- hygiene` → PASSED，0 error / 168 条既有 warning。
- `cargo run -p xtask -- card-check` → PASSED，0 error / 79 条既有 warning。
- `cargo run -p xtask -- docscan` → PASSED，0 error / 175 条既有 warning。
- `cargo run -p xtask -- refscan` → 首次发现 `[ADR:待建 0005]` 的裸写形式 2 error；改为
  `[ADR:待建 0005]` 后复跑 PASSED，0 error / 0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED，0 error / 1 条既有 paint 归档 warning。
- `git diff --check` → PASSED。
- 契约/产品路径差异：`docs/adr/**`、`docs/spec/**`、`protocol/**`、`adapters/**`、`crates/**`、
  `apps/**` 均为 0。

### 4. DoD 逐条核对

- [x] 六项决策均有推荐条款、owner、兼容路径和负向判据。
- [x] 四个现有 Adapter 的迁移影响逐项可追踪。
- [x] 四个关键裁决点已明确。
- [x] 五步顺序与 Stage 2 批次一致。
- [ ] PR / main CI 证据待回填。

### 5. 偏差

- none（当前）。

### 6. 更合理做法（实施中发现，非漂移）

- 独立 review 发现并关闭了 4 个 P1 与 2 个 P2：projection 版本与 model 冲突、生成期 source
  reference 与运行时 provenance 边界、spec/protocol canonical 字段差异、Paint write 计数、
  interrupt 现有合法字段/动作遗漏。均只修决策文档，未扩大 write scope。

### 7. 遗留问题

- 公共 ADR/spec 的真实 Accepted 文本仍须独立裁决卡；本卡完成后 Stage 2 不切换。

### 8. 新增长期记忆

- 无。现有 TASK-373 审计与 2026-09-28“声明存在不等于运行时支持”的 PITFALL 已覆盖同一事实，
  不重复添加。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 重点核对 `none_readonly`：当前两处 binary 归一化是临时兼容，不应被新 spec 继续默许。
- 重点核对 App Map authority：富知识与 core projection 不能继续各自成为事实源。
- 重点核对 rollback/interrupt：Paint 13 个 write / 5 个缺口与五动作闭集必须逐项落地，
  不允许缺行即通过。
