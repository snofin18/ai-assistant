# TASK-381 Adapter 六件套通用 validator 实施前契约审计

- 状态：Review
- 阶段：2　子阶段：2b Adapter contract implementation　批次：1 validator gate　依赖：ADR-0104、TASK-376、TASK-380
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031「一卡一文件」）。

---

## write scope

- `tasks/TASK-381-adapter-bundle-validator-readiness.md`
- `docs/audits/stage-2-adapter-bundle-validator-readiness-2026-11-17.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `LEDGER.md`
- `docs/automations/2026-11-17-round-1.md`

## 目标

核对 ADR-0104 的 D1-D6 是否已经细化到可以用封闭字段和确定性算法实现通用只读 Adapter
bundle validator；只有所有实现输入都来自 Accepted 契约时才允许进入产品代码实现。

## In scope

- 逐项比较 ADR-0104、三份 spec、四套现有 pack 和 TASK-380 生成类型。
- 为 D1-D6 标注 `ready` / `blocked`，列出缺失字段、算法、闭集或 owner。
- 把不可直接实现的契约缺口记录为 `DRIFT-381-1`，并给出后续 ADR/spec 卡建议。
- 同步本卡记录、Stage 2 计划、PLAN 与 LEDGER；不写 `MEMORY.md` 长期记忆正文。

## Out of scope

- 不实现 `adapter-validate`、Rust/Python validator 或任何产品代码。
- 不修改 `protocol/**`、`adapters/**`、`crates/**`、`apps/**`、CI 或依赖。
- 不自行定义 D2 projection 生成算法、D5 recipe 字段联合或 D6 interrupt 参数形状。
- 不迁移现有 pack，不把只读审计结果冒充 validator 已交付。
- 不启动真实 Excel，不读取真实业务工作簿。

## 必须遵守

1. ADR-0104 是上位 Accepted 契约；缺口必须显式记录，不得用实现猜测补齐。
2. 现有 pack 只作为形状证据，不得倒推为新的公共契约。
3. fail-closed 判据必须落实到字段、引用、版本、路径或确定性字节比较。
4. 发现不可实现点后停止该部分编码，并转做报告/证据收口。
5. 不把“能力矩阵目标已批准”写成“validator 已实现”或“现有 pack 已通过”。

## 验收命令

```powershell
cargo run -p xtask -- adr-index
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- check-ledger
git diff --check
```

预期：审计逐项覆盖 D1-D6；每个 blocked 项都有缺失契约、影响和建议落点；全部文档门禁通过；
产品代码、protocol、adapters 与 CI 零差异。

## DoD

- [ ] D1-D6 均有可实现性结论和 Accepted 契约引用。
- [ ] blocked 项列出缺失字段/算法/闭集及不实现理由。
- [ ] `DRIFT-381-1` 明确停止工作与后续 ADR/spec 建议。
- [ ] 产品代码、protocol、adapters、apps 与 CI 零差异。
- [ ] 文档门禁、PR checks、merge 与 post-merge main CI 回填。

<!-- ══ 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-381 Adapter 六件套 validator 实施前契约审计
【目标】核对 ADR-0104 的 D1-D6 是否已细化到可直接实现通用只读 validator
【write scope】仅：本卡、审计报告、Stage 2 计划/PLAN/LEDGER、本轮 automation 证据
【铁律】契约先行；矛盾必须 DRIFT；未知输入 fail-closed；不得把审计冒充实现
【禁止】不改 protocol/adapters/crates/apps/CI；不自行定义未批准的数据形状；不启动真实 Excel
【验收】文档门禁全绿；D1-D6 可实现性、缺口和建议逐项落盘
【依赖】ADR-0104、TASK-376、TASK-380 已 Done
【疑问】D2/D5/D6 的完整实现形状未冻结；默认处理 = 记录 DRIFT，不写产品代码
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-381-adapter-bundle-validator-readiness.md`：新增本卡并记录只读审计结论。
- `docs/audits/stage-2-adapter-bundle-validator-readiness-2026-11-17.md`：D1-D6 可实现性、
  缺口、停止边界和后续顺序。
- `plans/stage-2-excel.md`：增加 TASK-381 preflight 行并更新当前进度。
- `PLAN.md`：只更新「当前状态」块中的日期、当前阶段、当前任务卡与阻塞项。
- `LEDGER.md`：追加 TASK-381 Review 事件。
- `docs/automations/2026-11-17-round-1.md`：本轮 automation 证据与 JSONL 路径。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `cargo run -p xtask -- adr-index` → PASSED，93 files / 0 error / 0 warning。
- `cargo run -p xtask -- hygiene` → PASSED，486 files / 0 error / 173 warning（全部既有提示；
  新增文档没有 error）。
- `cargo run -p xtask -- card-check` → PASSED，284 files / 0 error / 90 warning（全部既有或
  规则模板类提示；本卡未产生 error）。
- `cargo run -p xtask -- docscan` → PASSED，673 files / 0 error / 175 warning。
- `cargo run -p xtask -- refscan` → PASSED，1200 files / 0 error / 0 warning。
- `cargo run -p xtask -- check-ledger` → PASSED，PLAN / LEDGER 日期均为 2026-11-17，
  current_stage=阶段 2。
- `cargo run -p xtask -- memory-counts` → PASSED，0 error / 1 条既有 paint warning。
- `git diff --check` → PASSED。
- PR / CI / merge / post-merge main CI：待创建后回填。

### 4. DoD 逐条核对

- [x] D1-D6 均有可实现性结论和 Accepted 契约引用。
- [x] blocked 项列出缺失字段/算法/闭集及不实现理由。
- [x] `DRIFT-381-1` 明确停止工作与后续 ADR/spec 建议。
- [x] 产品代码、protocol、adapters、apps 与 CI 零差异。
- [ ] 文档门禁、PR checks、merge 与 post-merge main CI 回填。

### 5. 偏差

```text
DRIFT-381-1
触发器：⑧ 发现 spec 与实现前提不足；⑨ 实现通用 validator 需要新增未批准的字段方言
现象：ADR-0104 冻结了 D1-D6 的 owner、拒绝类别和高层字段，但没有完整冻结
      App Map projection 的确定性生成算法、rollback coverage 的完整对象联合、
      interrupt action 的参数形状，以及 selector/postcondition 的完整闭集。
影响：直接实现 validator 只能猜测 Accepted 语义，或把现有四套 pack 的临时形状
      倒推成公共契约；任一选择都会违反“契约先行”和“不得先实现再补文档”。
我的建议：先以独立 ADR/spec 卡冻结可机器实现的字段联合、闭集、确定性排序/序列化
          与 omission report，再由独立 validator 卡实现；现有 pack 保持只读不改。
已停止的工作：未修改 protocol/adapters/crates/apps/CI，未创建 validator 命令。
需要人类裁决：批准后续 contract-clarification ADR/spec 的范围与字段细节。
```

### 6. 实施中发现的更合理做法（非漂移，已直接落地 + 理由）

- 把“validator 实现卡”拆成“契约冻结审计”与“validator 实现”两步。当前 ADR 已足以阻止
  错误方向，但不足以生成一个不猜测字段的 validator；先审计能避免把四套旧 pack 的事实倒推成
  新契约。
- 独立 review 后收窄 D3/D4/D5：ADR-0095、ADR-0062、架构 §7.4 与 `crates/verify` 已冻结相关
  运行时语义；真实缺口是这些既有语义到 binder JSON 的确定性映射与可复用接入，而不是语义本身
  尚不存在。

### 7. 遗留问题（进 `docs/PARKING_LOT.md` 的编号）

- 不新增 PARKING_LOT：该缺口是当前 Stage 2 的直接 gate，已落 `DRIFT-381-1` 与审计报告，
  而不是旁路想法。
- 通用只读 validator 仍未实现；后续必须先完成 ADR/spec clarification。

### 8. 新增长期记忆（FACT / PITFALL / REJECTED 条目原文，若无写“无”）

- 无。审计报告与任务卡已足以保存本轮结论，未修改 `MEMORY.md` 或 `docs/memory/*`。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 本卡只证明 validator 实施前提不足，不代表 D1-D6 决策被撤销。
- 后续 contract-clarification 不得改变 ADR-0104 已批准的 owner、兼容窗口与六项目标。
- 在字段联合和闭集冻结前，不应用 validator 或 pack 现状反向重写 ADR。
