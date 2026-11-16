# TASK-373 Adapter 契约就绪审计

- 状态：**Done**
- 阶段：2　子阶段：Stage 2b Adapter contract readiness　批次：1
- 依赖：TASK-372、ADR-0022、ADR-0028、ADR-0031、ADR-0041、ADR-0058、ADR-0062、ADR-0083

---

## write scope

- `tasks/TASK-373-adapter-contract-readiness-audit.md`
- `plans/stage-2-excel.md`
- `docs/audits/stage-2-adapter-contract-readiness-2026-11-16.md`
- `docs/memory/facts.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在不修改公共契约、Adapter 数据包或产品代码的前提下，审计现有 Notepad、Paint、Edge、Chrome
四套 Adapter 对 `adapter.toml`、App Map、selector、tool、rollback、interrupts 六件套的声明
与消费现状，形成可直接支撑下一份 ADR 的范围、缺口和决策输入。

## In scope

- 盘点四套现有 Adapter 的六件套文件、结构版本、必填字段和跨文件引用。
- 追踪仓库现有 loader / validator / tests 实际消费了哪些六件套内容，区分“文件存在”与“契约生效”。
- 检查 App Map 双文件、selector 主通道、tool 可逆性声明、rollback 恢复路径、interrupt 人工边界。
- 将未统一字段、缺失文件、命名漂移、owner 重疊和未验证行为写成 ADR-ready 缺口矩阵。
- 形成 Stage 2 下一卡的窄范围建议；本卡不实施该建议。
- 同步本卡、Stage 2 计划、PLAN/README/LEDGER；如产生新的长期硬事实，追加 `docs/memory/facts.md`。

## Out of scope

- 不修改 `docs/adr/*`、`docs/spec/*`、`protocol/**`、`adapters/**` 或任何 Rust/TS 产品代码。
- 不新增、删除或重命名公共字段、Tool、Target、rollback recipe、interrupt 或 schema。
- 不创建 Excel Adapter pack，不实现 COM apartment/lifecycle，不运行真实 Excel GUI。
- 不运行会打开用户工作簿、修改 Adapter 数据或触发外部网络副作用的命令。
- 不以审计结论替代 ADR 或人类对公共接口的裁决。

## 必须遵守

1. 只把当前文件、解析器和测试的真实行为写成事实；不用计划文字推断“已生效”。
2. 每个缺口必须给出文件路径、字段或行为证据，并明确是缺失、未消费、含义冲突还是尚未验证。
3. 不修改现有检查来适配审计结果；只读运行现有门禁。
4. 发现需要改 schema/API/ADR 的事项时只记录 ADR decision input，不在本卡实施。
5. 四套 Adapter 必须使用同一矩阵，不以 Edge/Chrome 的共享 CDP 结构跳过 rollback/interrupt 检查。

## 验收命令

```powershell
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：全部现有门禁通过；审计矩阵覆盖 4 个 Adapter × 6 件套；零产品接口改动。

## DoD

- [ ] Notepad、Paint、Edge、Chrome 的六件套存在性与解析状态逐项取证。
- [ ] 至少检查 loader / validator / tests 对六件套的真实消费路径。
- [ ] 输出统一缺口矩阵，区分 owner、版本、引用、恢复、interrupt 和 schema 问题。
- [ ] 给出下一份 ADR 的边界建议与后续窄卡拆分，不在本卡修改公共契约。
- [ ] 零 `adapters/**`、`docs/spec/**`、`docs/adr/**`、`protocol/**` 或产品代码差异。
- [ ] 文档/仓库门禁全绿，PR 与 post-merge main CI 证据回填。

<!-- ══ 分界线：以上为卡片正文，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线上方任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。
-->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-373 Adapter 契约就绪审计
【目标】在不改公共契约的前提下，审计现有 Adapter 六件套声明并形成可直接支撑下一份 ADR 的缺口矩阵
【write scope】仅：本卡、plans/stage-2-excel.md、Stage 2 就绪审计、PLAN/README/LEDGER；如产生新事实再追加 docs/memory/facts.md
【铁律】契约先行；禁止静默扩大范围；不可信输入先校验；无静默失败；资源必须回收；不得顺手重构
【禁止】不修改 docs/adr、docs/spec、现有 Adapter/schema、Rust 产品代码、依赖或 unsafe；不领取需要真实 GUI/公共接口裁决的批次
【验收】verify-schemas / codegen / hygiene / card-check / docscan / refscan / memory-counts / check-ledger / git diff --check 全绿；审计覆盖 4×6 件套
【依赖】TASK-372 Done、Stage 2 InProgress、LEDGER 已验证；N_auto=0、next_round=1、无 eligible open PR
【疑问】无；若发现必须改 ADR/spec/schema，只留 decision input，不实施
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-373-adapter-contract-readiness-audit.md`：本卡状态、约束与九节执行记录。
- `plans/stage-2-excel.md`：仅新增 TASK-373 审计行并更新当前进度块。
- `docs/audits/stage-2-adapter-contract-readiness-2026-11-16.md`：四 Adapter × 六件套矩阵、消费路径与
  ADR decision inputs。
- `PLAN.md`：当前状态块四行同步。
- `README.md`：状态行、当前阶段、最近进展三处同步。
- `LEDGER.md`：追加 TASK-373 Review 事件。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` →
  `PASSED: app_id=com.microsoft.paint targets=78 tools=15 recipes=8 probe_status=required`。
- `python adapters/browser.edge/tests/validate_adapter_packs.py` →
  `PACK OK browser.edge` / `PACK OK browser.chrome`。
- 只读 Rust 消费搜索 →
  `NO_RUST_ADAPTER_MANIFEST_CONSUMER` / `NO_RUST_RICH_APP_MAP_CONSUMER` /
  `NO_RUST_ROLLBACK_PACK_CONSUMER` / `NO_RUST_INTERRUPT_PACK_CONSUMER`。
- `cargo run -p xtask -- verify-schemas` → PASSED，5 schemas，0 error。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift / 0 error。
- `cargo run -p xtask -- card-check` → PASSED，0 error；本卡修正分界线后无本卡新增 error。
- `cargo run -p xtask -- docscan` → PASSED，0 error。
- `cargo run -p xtask -- hygiene` → PASSED，0 error，168 条既有 warning。
- `cargo run -p xtask -- refscan` → PASSED，0 error / 0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED，0 error / 1 条既有 paint 归档 warning。
- `git diff --check` → PASSED，无 whitespace error。
- PR #736 checks **11/11 success**，pending 0 / failed 0，base=`main`，`MERGEABLE` / `CLEAN`；
  merge `2fab59ff9de42ac7cd6b272c17ce7bb50095e511`。
- post-merge main CI `43839388347` success，10 success + 1 skipped（merge push 的 commitlint），
  failed 0。

### 4. DoD 逐条核对

- [x] Notepad、Paint、Edge、Chrome 的六件套存在性与解析状态逐项取证。
- [x] 检查 target/tool/App Map loader 和两个 Python pack validator 的真实消费路径。
- [x] 输出统一缺口矩阵，区分 owner、版本、引用、恢复、interrupt 与 schema 问题。
- [x] 给出下一份 ADR 的六项决策输入与五步拆卡顺序，未修改公共契约。
- [x] 零 `adapters/**`、`docs/spec/**`、`docs/adr/**`、`protocol/**` 或产品代码差异。
- [x] PR #736、11/11 checks、merge hash 与 post-merge main CI `43839388347` 已回填。

### 5. 偏差

- none。审计发现公共契约缺口后停在 decision input，没有实施 ADR/spec/schema 修改。

### 6. 更合理做法（实施中发现，非漂移）

- 先按“文件存在、可解析、被运行时消费、被门禁验证”四层拆分，避免把 Paint/Edge 局部 validator
  误当成四套包的统一契约。该拆法直接把下一步收敛为 ADR/spec，而不是继续复制 Excel 数据包。

### 7. 遗留问题

- Adapter bundle envelope、App Map owner、selector AST、tool postcondition 消费、rollback recipe
  条件和 interrupt grammar 均需 ADR/spec 裁决。
- Edge/Chrome 缺 core App Map v1 projection 与 rollback pack；Paint 有 5 个写工具无 recipe，
  其中 L3 与 L2 需要按统一规则区分。
- Notepad 无独立 pack validator；Paint/Edge validator 未接入 `xtask` 或 CI。

### 8. 新增长期记忆

- 无。结论与既有 `docs/memory/pitfalls.md` 2026-09-28 / 2026-10-01 的“声明存在不等于运行时支持”
  条目一致；本卡用当前四包证据补充审计，不新增重复 PITFALL。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 重点核对“文件存在但无 Rust consumer”的四项结论，不能被局部 Python validator 抵消。
- 重点核对 rollback 缺口统计：L3 可不承诺恢复，但 L1/L2 的声明与 recipe 覆盖必须由新 ADR 统一。
- 重点核对后续顺序：先 ADR/spec，再通用 validator，再收敛现有包，最后才创建 Excel Adapter pack。
