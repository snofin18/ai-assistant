# TASK-353　form_fill approval readback evidence contract

- 状态：**Done**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：344、351、352　预估：S　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

**目标**

关闭 `DRIFT-352-1`：冻结 `form_fill` 审批专用 readback evidence 的内部通道、严格形状、一次性生命周期和模型不可见边界；不实现 handler、preview 或 UI。

**write scope**

- `docs/adr/0103-browser-form-fill-approval-readback-evidence.md`
- `docs/adr/README.md`
- `docs/spec/browser-form-confirmation.md`
- `docs/memory/decisions.md`
- `docs/memory/facts.md`
- `tasks/TASK-353-form-fill-approval-readback-contract.md`
- `tasks/TASK-352-browser-form-approval-preview.md`：仅恢复状态与记录后续
- `plans/stage-1-pilots.md`、`PLAN.md`、`README.md`、`LEDGER.md`、`MEMORY.md`：仅状态与进度同步

**In scope**

1. 接受 ADR-0103，固定 `evidence.extra` 命名空间和 JSON 形状。
2. 明确所有字段必须属于同一 form，且证据只在 readback 成功后出现。
3. 明确 Runtime 严格解析、一次性消费、与 `form_read` before 精确匹配。
4. 明确证据不得进入模型、日志、审计、DB、任务输出或 replay。
5. 明确 preview 的最小 redactor 规则，原始值不得进入 `DiffPreview::Fields`。
6. 将 TASK-352 恢复为 Ready，供后续实现。

**Out of scope**

- 不修改 `browser_form_fill` / `browser_task_approval` / runtime 产品代码。
- 不新增 Tool schema、protocol 字段、DB schema、ErrorCode 或依赖。
- 不实现 UI 事件、once grant、`form_submit` 或真实浏览器验收。

**必须遵守**

- evidence 只作审批内部证据，不得成为通用模型数据通道。
- 缺失、重复、畸形或漂移一律 fail-closed。
- redactor 未实现时不得生成可批准 preview。

**DoD**

- [ ] ADR-0103 Accepted，并登记 `docs/adr/README.md`。
- [ ] spec 升到 0.3，记录 evidence 形状、生命周期与拒绝方案。
- [ ] TASK-352 恢复 Ready 并引用 ADR-0103。
- [ ] 文档门禁与 `git diff --check` 全绿。
- [ ] LEDGER、facts、PLAN、plans、README 与 MEMORY 同步。

**验收命令**

```powershell
git diff --check
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-comments
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-353 form_fill approval readback evidence contract
【目标】冻结 evidence.extra 审批 readback 通道并恢复 TASK-352
【write scope】ADR-0103、spec、decisions/facts、TASK-352/353 与状态同步文件
【铁律】不可信输入先校验；Policy/HITL 唯一放行；契约先行；不得静默扩大范围
【禁止】产品代码、Tool/protocol/DB schema、UI、once grant、真实浏览器或依赖
【验收】adr-index + refscan + docscan + card-check + check-ledger + hygiene + memory-counts 全绿
【依赖】TASK-344/351 Done；TASK-352 因 DRIFT-352-1 Blocked
【疑问】无；采用既有 Evidence.extra，不新增公共 API
```

### 2. 实际改动文件

- `docs/adr/0103-browser-form-fill-approval-readback-evidence.md`
- `docs/adr/README.md`
- `docs/spec/browser-form-confirmation.md`
- `docs/memory/decisions.md`
- `docs/memory/facts.md`
- `tasks/TASK-352-browser-form-approval-preview.md`
- `tasks/TASK-353-form-fill-approval-readback-contract.md`
- `PLAN.md`、`plans/stage-1-pilots.md`、`README.md`、`LEDGER.md`、`MEMORY.md`

### 3. 验收输出摘要

- `git diff --check` → exit 0。
- `cargo run -p xtask -- adr-index` → PASSED，92 files，0E/0W。
- `cargo run -p xtask -- refscan` → PASSED，1126 files，0E/0W。
- `cargo run -p xtask -- docscan` → PASSED，616 files，0E/175W（既有 warning）。
- `cargo run -p xtask -- card-check` → PASSED，255 files，0E/35W（既有 warning）。
- `cargo run -p xtask -- check-ledger` → PASSED，0E/0W。
- `cargo run -p xtask -- hygiene` → PASSED，477 files，0E/154W（既有 warning）。
- `cargo run -p xtask -- memory-counts` → 首跑按机器提示修正 `facts=292/239`、`decisions=314/111` 后 PASSED。
- `cargo run -p xtask -- check-comments` → PASSED，477 files，0E/75W（既有 warning）。

### 4. DoD 逐条核对

- [x] ADR-0103 Accepted，并登记 `docs/adr/README.md`。
- [x] spec 升到 0.3，记录 evidence 形状、生命周期与拒绝方案。
- [x] TASK-352 恢复 Ready 并引用 ADR-0103。
- [x] 文档门禁与 `git diff --check` 全绿。
- [x] LEDGER、facts、PLAN、plans、README 与 MEMORY 同步。

### 5. 偏差

```text
契约偏差：无；ADR-0103 只使用既有 Evidence.extra 通道。

过程偏差：首次追加 `docs/memory/decisions.md` 时漏取 ADR-0028 guard。发现后已补取并立即释放
该文件锁，期间无并行写入迹象；后续热点文件均按 guard acquire → write → release 执行。
本偏差不影响产品代码、schema 或契约结论，但按治理规则如实记录。
```

### 6. 更合理做法

- 用现有 evidence 通道承载审批内部 readback，避免扩大模型可见 ToolOutput 或跨 crate 公共 API。

### 7. 遗留问题

- TASK-352 恢复 Ready 后实现 handler evidence、preview redactor 与 runtime 一次性消费。

### 8. 新增长期记忆

- `FACT`：`form_fill` 审批 readback 可通过 `ToolEnvelope.evidence.extra` 的固定命名空间传递；证据一次性消费且不得进入模型、日志、审计、DB、任务输出或 replay。

### 9. 给审阅者的关注点

- 核对 evidence 是否严格非模型可见，且所有字段归属同一 form。
