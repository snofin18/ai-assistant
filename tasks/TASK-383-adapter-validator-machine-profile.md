# TASK-383 Adapter bundle validator machine profile

- 状态：Done
- 阶段：2　子阶段：2b Adapter contract implementation　批次：validator contract gate　依赖：ADR-0104、TASK-376、TASK-380、TASK-381
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031「一卡一文件」）。

---

## write scope

- `tasks/TASK-383-adapter-validator-machine-profile.md`
- `docs/adr/0105-adapter-bundle-validator-machine-profile.md`
- `docs/adr/0104-adapter-bundle-contract-and-closed-dialects.md`
- `docs/adr/README.md`
- `docs/spec/adapter-bundle.md`
- `docs/spec/tool-schema.md`
- `docs/spec/capability-matrix.md`
- `docs/spec/naming.md`
- `docs/memory/decisions.md`
- `MEMORY.md`（仅 `decisions.md` 派生规模行）
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

关闭 `DRIFT-381-1` 中的机器契约缺口，把 D2 App Map projection、D3 selector JSON 到运行时模型的
mapping、D4 postcondition/ErrorCode/capability 共享 catalog、D5 rollback anchor 联合和 D6
interrupt action/resume/priority 冻结为 validator 可直接实现、未知输入 fail-closed 的 Accepted
契约。

## In scope

- 新增 ADR-0105，只澄清 ADR-0104 的机器可执行形状，不改变其 owner、六件套目标或安全语义。
- 新增 `docs/spec/adapter-bundle.md`，定义 canonical JSON、引用、D1-D6 封闭对象与负向矩阵。
- 明确 App Map `projection_manifest`、确定性 projection 字节格式与 omission 判据。
- 明确两种 selector model 的精确字段、闭集、排序和到 `TargetDescriptor` 的 mapping。
- 明确 ToolSchema 1.0/1.1 兼容窗口、共享 catalog 引用、read evidence 与 rollback coverage mapping。
- 明确 rollback `anchor_strategy` 判别联合，以及 interrupt action/resume/priority 判别联合。
- 更新既有 tool-schema、capability-matrix、naming spec 与 ADR/decisions 索引。
- 同步 Stage 2 计划、PLAN、README 与 LEDGER。

## Out of scope

- 不实现 `adapter-validate`、Rust/Python validator 或其测试。
- 不新增/修改 `protocol/**` JSON Schema、生成类型、catalog 文件或 codegen。
- 不迁移 `adapters/**` 中任一现有 pack。
- 不改 runtime、loader、Policy、任务执行、CI、依赖或公共 Rust 接口。
- 不启动 Excel，不读取用户工作簿，不改变 ADR-0095 或 ADR-0062 已决语义。

## 必须遵守

1. 本卡只冻结机器形状；不得把“目标契约已定义”写成“validator 已实现”。
2. 未知字段、未知枚举、错误判别键、悬空引用、重复键/ID 与歧义排序一律 fail-closed。
3. Postcondition kind 字段、ErrorCode 与 capability ID 必须引用既有唯一事实源，不得在 validator
   内复制第三份枚举。
4. D2 生成不得使用模型总结或启发式语义抽取；只有作者维护的 projection manifest 能决定
   projection entry。
5. D5 不得把自然语言描述当作可执行恢复；每个 recipe anchor 必须有机器可解析的 capture、
   restore 与 post-verify。
6. D6 文本匹配必须显式标记 locale dependence，且不得成为唯一匹配条件。
7. 发现 ADR-0104 已决语义冲突时停止并记录 DRIFT，不得用澄清 ADR 静默改义。

## 验收命令

```powershell
cargo run -p xtask -- adr-index
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- check-ledger
cargo run -p xtask -- memory-counts
git diff --check
```

预期：ADR 登记表、decisions 索引与所有文档门禁全绿；D2-D6 均有封闭字段/算法/mapping；
`protocol/**`、`adapters/**`、`crates/**`、`apps/**`、CI 和依赖零差异。

## DoD

- [ ] ADR-0105 Accepted，并明确仅澄清 ADR-0104。
- [ ] D2 projection manifest、确定性字节格式和 omission 判据完整。
- [ ] D3 两种 selector model 的字段、闭集、排序及运行时 mapping 完整。
- [ ] D4 ToolSchema 兼容窗口、catalog 唯一来源、read evidence 与 ErrorCode 规则完整。
- [ ] D5 rollback coverage/anchor 判别联合与引用规则可机器实现。
- [ ] D6 interrupt action/resume/priority 与 human-only 矩阵可机器实现。
- [ ] tool-schema/capability/naming/decisions/计划状态同步。
- [ ] 文档门禁、PR checks、merge 与 post-merge main CI 回填。
- [ ] 产品代码、protocol、adapters、apps、CI 与依赖零差异。

<!-- ───── 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ─────
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发器 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-383 Adapter bundle validator machine profile
【目标】关闭 DRIFT-381-1，把 D2-D6 冻结为可机器实现且 fail-closed 的 Accepted 契约
【write scope】仅：本卡、ADR-0105/0104 与登记、adapter-bundle/tool/capability/naming spec、decisions、计划状态/LEDGER
【铁律】契约先行；复用既有唯一事实源；未知字段/歧义/悬空引用失败；不改已决语义
【禁止】不实现 validator；不改 protocol/adapters/crates/apps/CI/依赖；不迁移 pack
【验收】ADR/spec 完整覆盖 D2-D6；文档门禁全绿；产品路径零差异
【依赖】ADR-0104、TASK-376、TASK-380、TASK-381 均已核对为 Done
【疑问】无；默认处理 = 新增澄清 ADR + 专用 spec
```

### 2. 实际改动文件

- `tasks/TASK-383-adapter-validator-machine-profile.md`
- `docs/adr/0105-adapter-bundle-validator-machine-profile.md`
- `docs/adr/0104-adapter-bundle-contract-and-closed-dialects.md`（仅增加 Clarified by）
- `docs/adr/README.md`
- `docs/spec/adapter-bundle.md`
- `docs/spec/tool-schema.md`
- `docs/spec/capability-matrix.md`
- `docs/spec/naming.md`
- `docs/memory/decisions.md`
- `MEMORY.md`（仅 decisions 派生行）
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

### 3. 验收输出摘要

- `cargo run -p xtask -- adr-index` → PASSED，94 files / 0 error / 0 warning。
- `cargo run -p xtask -- verify-schemas` → PASSED，6 schemas。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift / 0 error。
- `cargo run -p xtask -- hygiene` → PASSED，486 files / 0 error / 173 既有 warning。
- `cargo run -p xtask -- card-check` → PASSED，286 files / 0 error / 108 warning。
- `cargo run -p xtask -- docscan` → PASSED，683 files / 0 error / 175 既有 warning。
- `cargo run -p xtask -- refscan` → PASSED，1210 files / 0 error / 0 warning。
- `cargo run -p xtask -- check-ledger` → PASSED，PLAN / LEDGER 均为 2026-11-18，
  `current_stage=阶段 2`。
- `cargo run -p xtask -- memory-counts` → 首轮发现 `decisions.md` 派生值陈旧；同步
  `MEMORY.md` 318/112 → 322/113 后复跑 PASSED，0 error / 1 既有 paint warning。
- `git diff --check` → PASSED。
- PR #760：base=`main`，checks **21 success + 1 skipped**，pending 0 / failed 0，
  `MERGEABLE` / `CLEAN`；merge `2ae5a19f6f4b35decead5d060d97af849973c941`，
  post-merge main CI `44231680015` success。

### 4. DoD 逐条核对

- [x] ADR-0105 Accepted，并明确仅澄清 ADR-0104。
- [x] D2 projection manifest、确定性字节格式和 omission 判据完整。
- [x] D3 两种 selector model 的字段、闭集、排序及运行时 mapping 完整。
- [x] D4 ToolSchema 兼容窗口、catalog 唯一来源、read evidence 与 ErrorCode 规则完整。
- [x] D5 rollback coverage/anchor 判别联合与引用规则可机器实现。
- [x] D6 interrupt action/resume/priority 与 human-only 矩阵可机器实现。
- [x] tool-schema/capability/naming/decisions/计划状态同步。
- [x] 文档门禁、PR checks、merge 与 post-merge main CI 回填。
- [x] 产品代码、protocol、adapters、apps、CI 与依赖零差异。

### 5. 偏差

- 无 DRIFT。ADR-0105 只澄清 ADR-0104 的机器形状，没有改变六路径 owner、selector model、
  rollback level、interrupt 动作闭集或任何安全门禁。

### 6. 实施中发现的更合理做法

- D2 不采用模型摘要或从富 App Map 字段做启发式抽取，而是在富 App Map 内增加作者维护的
  `projection_manifest`。这样 projection 仍是派生文件，生成器只负责排序、复制、计算 token 和
  字节比较，避免伪确定性。
- D5 的 rollback post-verify 不承载 `captured_pre_content` 之类字符串占位符，改为
  `capture_equals | canonical postcondition` 判别联合，使 validator 可只靠结构验证。
- D3/D5/D6 均使用 typed discriminator 和显式 priority/order，避免 validator 依赖 JSON object
  键序、文件顺序或自然语言描述。
- closeout 复审进一步固定 D4 generated catalog 为
  `protocol/adapter-catalog/adapter-catalog-1.0.json`，并修正 interrupt `resume_after` 为
  `requires_human=true` 时条件必填、false 时禁止的矛盾表述。

### 7. 遗留问题

- 实现 `adapter-validate --report-only`、catalog generation 与完整负向测试。
- Notepad/Paint 与 Edge/Chrome 各需独立 pack 迁移卡。
- 官方 `rmcp` release 发布后，TASK-382 的临时 Git 依赖仍需切回 crates.io。

### 8. 新增长期记忆

- 新增 ADR-0105 decision：Adapter bundle validator 的机器形状固定为 projection manifest、
  typed selector/rollback/interrupt union 与共享 catalog 引用；validator 实现不得复制
  ErrorCode/capability/postcondition 枚举。

### 9. 给审阅者的关注点

- 重点检查 D2 `projection_manifest` 是否仍保持富 App Map 单一源，而不是形成第二份人工事实源。
- 重点检查 D5 的 restore/post-verify 是否真正可执行，未把自然语言或 placeholder 当机器契约。
- 重点检查 D6 的 priority 与 human-only 组合是否有歧义，尤其是三态保存、覆盖、登录和验证码。
