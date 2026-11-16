# TASK-376 Adapter 六件套公共 ADR/spec 落地

- 状态：Blocked
- 阶段：2　子阶段：2b Adapter contract decision　批次：1 gate　依赖：TASK-372、TASK-373、TASK-374、TASK-375

---

## write scope

- `tasks/TASK-376-adapter-contract-adr-spec-landing.md`
- `docs/adr/0104-*.md`
- `docs/adr/README.md`
- `docs/spec/tool-schema.md`
- `docs/spec/capability-matrix.md`
- `docs/spec/naming.md`
- `docs/memory/decisions.md`
- `docs/memory/facts.md`
- `docs/memory/pitfalls.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

把 TASK-374 的六项 Adapter 契约决策包转成经人类明确接受的新 ADR-0104 与对应 spec 文本，
冻结 D1-D6、兼容边界和迁移判据；本卡只落公共契约，不实现 validator、Adapter 或 COM runner。

## In scope

- 根据人类对 D1-D6 的逐项裁决，创建 ADR-0104 并更新 ADR 登记表与 decisions 索引。
- 将 `adapter.toml` 六槽 bundle、App Map owner、selector model、tool envelope、
  rollback 覆盖和 interrupt 闭集写入选定 spec 章节。
- 明确 schema 兼容窗口、未知版本/未知字段/悬空引用的 fail-closed 规则与后续迁移顺序。
- 为每项决策给出可机器验证的负向判据，供后续只读 validator 卡实现。
- 同步本卡记录、Stage 2 计划、PLAN、README 与 LEDGER。

## Out of scope

- 不修改 `protocol/**`、`adapters/**`、`crates/**`、`apps/**` 或任何 Rust/TS 产品代码。
- 不新增依赖、crate、模块、validator、Tool、Target、rollback recipe、interrupt variant 或 Excel pack。
- 不以决策草案中的“建议”代替人类对 D1-D6 的明确接受，不自定义第七种默认答案。
- 不运行真实 Excel GUI，不启动可写 Excel，不打开用户工作簿。
- 不把本卡完成解释为 Adapter 六件套已经在运行时生效。

## 必须遵守

1. 必须先取得六项人类裁决；任何一项未答或答案互相冲突时保持 Blocked。
2. ADR/spec 是唯一可改的公共契约落点；本卡不得先改代码再补文档。
3. 未知版本、未知字段、跨 bundle root 路径、悬空引用和未声明动作继续 fail-closed。
4. `none_readonly` 只有人类明确批准后才可进入 spec；未批准前不得把现有私有归一化写成规范。
5. 新增 ADR 前核对登记表下一可用号；若 0104 已占用，按登记表重新取号并记录原因。
6. 文档修改完成前不得把 Stage 2 标记 Done，也不得宣称迁移已执行。

## 验收命令

```powershell
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- adr-index
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：人类裁决覆盖 D1-D6；ADR/spec 内字段名、版本和负向判据一致；全部文档门禁通过；
`protocol/**`、`adapters/**`、`crates/**`、`apps/**` 与产品代码零差异。

## DoD

- [ ] 六项人类裁决均有明确答案、决策者和裁决日期。
- [ ] ADR-0104（或重新分配的合法编号）与 spec 文本一致，并被 `adr-index` 登记。
- [ ] 每项 D1-D6 均有 owner、兼容窗口、fail-closed 负向判据和后续迁移步骤。
- [ ] 未批准项保留显式阻塞，不把草案建议写成 Accepted 事实。
- [ ] `protocol/**`、`adapters/**`、`crates/**`、`apps/**` 与产品代码零差异。
- [ ] 文档门禁全绿，PR 与 post-merge main CI 证据回填。

<!-- ══ 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-376 Adapter 六件套公共 ADR/spec 落地
【目标】把 TASK-374 六项决策包转成经人类接受的 ADR/spec；未获裁决前保持 Blocked
【write scope】仅：本卡；裁决后限本卡列出的 ADR/spec/memory/状态文件
【铁律】契约先行；公共接口变化只允许人类裁决；未知输入 fail-closed；不得静默扩大范围
【禁止】不代答 D1-D6；不修改 protocol/adapters/产品代码；不新增依赖或 validator
【验收】人类六项裁决齐全后，ADR/spec 与文档门禁全绿；当前阻塞阶段只验证卡与状态一致性
【依赖】TASK-372/373/374/375 已 Done；TASK-374 明确下一步是人类闸门
【疑问】D1-D6 六项均待人类选择，默认处理 = 不编码、保持 Blocked
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-376-adapter-contract-adr-spec-landing.md`：创建候选卡并记录人类闸门。
- `plans/stage-2-excel.md`：登记 TASK-376 为 Blocked gate。
- `PLAN.md`：当前状态块记录下一步阻塞点。
- `LEDGER.md`：追加 TASK-376 Blocked 事件。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `cargo run -p xtask -- automation-rounds --run-date 2026-11-16` → PASSED；
  `next_round=1 / n_auto=0 / reset_source=daily-0800 / session_source=automation`。
- Eligible open PR 查询 → `[]`，open PR 0，可 eligible PR 0。
- `docs/audits/stage-2-adapter-contract-adr-decision-2026-11-16.md` → 状态为
  Decision Draft，待人类裁决；D1-D6 六项问题均未回答。
- `cargo run -p xtask -- verify-schemas` → PASSED（5 schemas / 0 error）。
- `cargo run -p xtask -- codegen --check` → PASSED（0 drift / 0 error）。
- `cargo run -p xtask -- hygiene` → PASSED（0 error / 168 条既有 warning）。
- `cargo run -p xtask -- card-check` → PASSED（0 error / 88 条既有 warning）。
- `cargo run -p xtask -- docscan` → PASSED（0 error / 175 条既有 warning）。
- `cargo run -p xtask -- refscan` → PASSED（0 error / 0 warning）。
- `cargo run -p xtask -- memory-counts` → PASSED（0 error / 1 条既有 paint warning）。
- `cargo run -p xtask -- adr-index` → PASSED（0 error / 0 warning）。
- `cargo run -p xtask -- check-ledger` → PASSED（0 error / 0 warning）。
- `git diff --check` → PASSED。
- 本卡未修改 `docs/adr/**`、`docs/spec/**`、`protocol/**`、`adapters/**`、
  产品代码或依赖；实现门禁留待闸门解除后运行。

### 4. DoD 逐条核对

- [ ] 六项人类裁决：未取得。
- [ ] ADR/spec 落地：按契约先行要求停止，未创建 Accepted ADR。
- [ ] 兼容窗口与负向判据：候选材料已存在，验收文本未冻结。
- [x] 未批准项保持显式 Blocked，未把建议写成事实。
- [x] 当前仅新增任务卡与状态文档，产品路径零差异。
- [ ] PR/main CI：本卡尚未实现，待闸门解除后回填。

### 5. 偏差

```text
DRIFT-376-1
触发器：④ 改动 ADR 已决事项/公共契约前须裁决；⑧ spec 与代码现状存在已知冲突
现象：TASK-374 已交付六项决策草案，但 D1-D6 仍标注“待人类裁决”，
      Stage 2 计划也明确下一步是人类批准并落地公共 ADR/spec。
影响：继续编码会把草案建议冒充 Accepted 契约，违反契约先行与公共接口升级规则。
我的建议：人类逐项回答 D1-D6 后，将本卡改为 Ready；再由单个实现会话只改 ADR/spec。
已停止的工作：未创建 ADR、未修改 spec/protocol/adapters、未实现 validator 或产品代码。
需要人类裁决：D1-D6 六项全部。
```

### 6. 更合理做法（实施中发现，非漂移）

- 把公共契约裁决与后续 validator/Adapter 实现分成独立卡，避免“先写约束检查器再倒推规范”。
- 在人类答案落盘前只保存候选卡和阻塞理由，使下一轮 automation 可直接按卡号恢复。

### 7. 遗留问题

- D1 bundle 是否六槽全必填。
- D2 App Map 是否接受“富源 + 确定性 core projection”。
- D3 是否接受两种显式 selector model。
- D4 是否追加 `none_readonly` 并采用 Adapter tool envelope。
- D5 是否要求所有 write 有 rollback 覆盖行。
- D6 是否接受 interrupt 五动作闭集并淘汰 task-id delegate。

### 8. 新增长期记忆

- 无。阻塞事实已由 TASK-374 决策审计与 Stage 2 计划记录，不重复添加。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 确认本卡只是候选/Blocked 卡，不能据此宣称公共 Adapter 契约已 Accepted。
- 确认 `none_readonly`、bundle projection authority 与 rollback 全覆盖没有被默认采用。
- 确认后续实现卡仍保持 ADR/spec 与产品代码分 PR，不以 validator 行为反向定义契约。
