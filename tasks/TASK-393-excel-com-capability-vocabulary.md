# TASK-393 Excel COM/spreadsheet capability vocabulary ADR

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel Adapter　批次：3a　依赖：TASK-391、TASK-392、ADR-0104、ADR-0105、ADR-0106

---

## write scope

- `tasks/TASK-393-excel-com-capability-vocabulary.md`
- `docs/adr/0107-excel-com-capability-vocabulary.md`
- `docs/adr/README.md`
- `docs/spec/capability-matrix.md`
- `protocol/capability-matrix/capability-1.0.json`
- `protocol/adapter-catalog/adapter-catalog-1.0.json`
- `adapters/com.microsoft.excel/app_map.json`
- `adapters/com.microsoft.excel/memory/app_map.v1.json`
- `adapters/com.microsoft.excel/README.md`
- `adapters/com.microsoft.excel/tests/validate_adapter_pack.py`
- `adapters/com.microsoft.excel/tests/test_validate_adapter_pack.py`
- `xtask/src/adapter_validate/tools.rs`
- `xtask/src/adapter_validate/tool_bounds.rs`
- `xtask/src/adapter_validate/mod.rs`
- `xtask/src/adapter_validate_tests.rs`
- `xtask/src/adapter_validate_excel_tests.rs`
- `docs/memory/decisions.md`
- `MEMORY.md`（仅 `decisions.md` 派生规模行）
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

以 ADR-0107 冻结 Microsoft Excel L1 COM/spreadsheet 的稳定 capability 词汇、风险下限、
禁止别名与 T2.1-T2.3 消费边界；只解除“没有 capability ID”的契约阻塞，不把空 tool surface
冒充为可运行 Excel Adapter。

## In scope

- 新增 ADR-0107，定义 `com.*` 七个 experimental capability ID 及各自允许的语义。
- 将七个 ID 写入 protocol capability catalog，并由既有 generator 更新 Adapter catalog。
- 在 capability-matrix spec 增加 Excel COM 能力族、风险/审批下限、任务卡消费矩阵与拒绝条件。
- 同步 Excel App Map、projection、README 与专属 validator：capability ADR 已解除，但
  model-visible tools 仍因 COM runner/T2.1-T2.3 未实现而保持为空。
- 增加负向测试，锁住新的 implementation blocker、legacy alias 与 broad escape 拒绝。
- 同步 ADR 登记、decisions 索引、MEMORY 派生规模、Stage 2 与 LEDGER。

## Out of scope

- 不实现 COM runner、Excel host handler、真实 Tool、ToolSchema 字段、公共 Rust API 或 Policy。
- 不实现 T2.1/T2.2/T2.3，不启动 Excel，不打开或读取工作簿，不创建 COM 对象。
- 不新增 `com.workbook_close`、`com.execute`、`com.eval`、`com.invoke`、`com.macro`、
  `com.raw_dispatch`、`office.*` 或 `spreadsheet.*` alias。
- 不授权宏、外部链接更新、Protected View 自动启用、覆盖确认、Save/Discard/Replace、
  AutoSave/OneDrive 冲突处理或任何无人值守写操作。
- 不修改 ToolSchema JSON Schema、generated Rust/TS、runtime loader、CI 或第三方依赖。

## 必须遵守

1. Capability ID 只使用已经冻结的七个 `com.*`；任何新增、删除、改名或升格 stability 都要新 ADR。
2. `com.workbook_open` Tool 的 capability 数组必须只含该一个 ID；它只能是专用、
   可识别的 owned Excel process，write/high|critical/approval/non-idempotent/L3，
   不得与其他 capability 混用、藏在 read 或 L1/L2 Tool，也不得附着或终止用户已有
   `EXCEL.EXE`。
3. 读取必须显式声明 `value2` / `formula` / `text`；UIA、键盘或 accessibility capability
   不得冒充 COM Range 读取。
4. `com.range_write` 只允许有界 scalar literal、固定 `value2`；文字字符串首非空白字符为
   `=`、`+`、`-` 或 `@` 时必须失败；公式写入、结构修改、任意 COM method dispatch
   不在本词汇授权范围内。
5. 写能力不得降低 L1 恢复要求；COM 修改不得声称 Excel `Application.Undo` 可恢复。
6. `com.chart_export` 只能 create-new，禁止覆盖；Protected View、AutoSave、只读/占用、
   外链、宏、覆盖和保存冲突继续走 human-only interrupt。
7. Adapter validator 不得维护第二份 capability catalog 或接受 alias；catalog 是唯一 ID 来源。

## 验收命令

```powershell
cargo run -p xtask -- adr-index
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- adapter-validate adapters/com.microsoft.excel --report-only
python adapters/com.microsoft.excel/tests/validate_adapter_pack.py
python -m unittest discover -s adapters/com.microsoft.excel/tests -p "test_*.py"
python -m py_compile adapters/com.microsoft.excel/tests/validate_adapter_pack.py
cargo fmt --all --check
cargo test --workspace --no-fail-fast
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：ADR/catalog/spec/Excel pack 一致；catalog 只新增七个 `com.*`；tools/rollback 仍为空且
以新的 runtime/implementation blocker 失败封闭；workspace 与全部治理门禁通过；零 COM/GUI/
工作簿副作用。

## DoD

- [ ] ADR-0107 Accepted，七个 `com.*` ID、风险下限、禁止语义与重讨论条件完整。
- [ ] capability catalog、generated Adapter catalog 与 capability-matrix spec 一致。
- [ ] T2.1 的 open 是独立 write Tool，read Tool 只消费 workbook/range read；T2.2 消费 bounded
  write/save，T2.3 消费 chart create/export。
- [ ] `com.execute/eval/invoke/macro/raw_dispatch`、`office.*` 与 `spreadsheet.*` 明确拒绝。
- [ ] Excel App Map、projection、README 与专属 validator 同步到“ADR 已解除、runner 待实现”。
- [ ] 专属 Python 9 条负向测试、workspace 与全部治理门禁通过。
- [ ] PR checks、merge 与 post-merge main CI 回填。
- [ ] 零 COM/GUI/工作簿副作用，零新增依赖、公共 API、ToolSchema 或 runtime 改动。

<!-- ───── 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ─────
     以下由 Implementer 填写。改动分界线以上任何正文行 = 漂移触发器 ⑤；
     状态行仅按 ADR-0083 与产生该状态的提交同批更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-393 Excel COM/spreadsheet capability vocabulary ADR
【目标】冻结七个 com.* capability、风险下限与 T2.1-T2.3 消费边界
【write scope】仅：本卡、ADR-0107/登记、capability spec/catalog、Excel app bundle、共享 adapter validator、decisions/MEMORY、Stage 2/PLAN/README/LEDGER
【铁律】契约先行；catalog 单一事实源；未知/别名/broad escape fail-closed；写操作 L1 required
【禁止】不实现 COM runner/Tool；不改 ToolSchema/runtime/Policy/CI/依赖；不启动真实 Excel 或工作簿
【验收】ADR/catalog/spec/Excel validator 一致；workspace 与文档门禁全绿；零 COM/GUI 副作用
【依赖】TASK-391、TASK-392、ADR-0104/0105/0106 已核对
【疑问】无；按用户“继续下一步”自决推进
```

### 2. 实际改动文件

- `tasks/TASK-393-excel-com-capability-vocabulary.md`：任务卡与执行记录，in scope。
- `docs/adr/0107-excel-com-capability-vocabulary.md`：七个 `com.*` capability、风险下限、
  消费矩阵与拒绝条件，in scope。
- `docs/adr/README.md`：ADR-0107 登记与下一个可用编号 0108，in scope。
- `docs/spec/capability-matrix.md`：v0.4 Excel L1 COM capability family，in scope。
- `protocol/capability-matrix/capability-1.0.json`：新增七个 experimental/0.1.0 ID，in scope。
- `protocol/adapter-catalog/adapter-catalog-1.0.json`：`codegen` 按 byte-order 重新生成，in scope。
- `adapters/com.microsoft.excel/app_map.json`：capability ADR 已解除、runner/Tool 仍待实现，in scope。
- `adapters/com.microsoft.excel/memory/app_map.v1.json`：按投影 manifest 同步内容与 token，in scope。
- `adapters/com.microsoft.excel/README.md`：能力边界与实施 blocker 更新，in scope。
- `adapters/com.microsoft.excel/tests/validate_adapter_pack.py`：读取真实 catalog、拒绝 alias /
  broad escape，in scope。
- `adapters/com.microsoft.excel/tests/test_validate_adapter_pack.py`：新增 blocker 负向回归，
  精确 catalog/alias/broad-escape 回归，总数 12，in scope。
- `xtask/src/adapter_validate/tools.rs`：把 `com.workbook_open` 的 write/L3/approval/
  non-idempotent 与四类 Excel write 下限委托给共享 bounds，in scope。
- `xtask/src/adapter_validate/tool_bounds.rs`：七个 `com.*` 的 Tool 元数据投影与风险下限，
  in scope。
- `xtask/src/adapter_validate/mod.rs`：注册 bounds 与 Excel 专属测试模块，in scope。
- `xtask/src/adapter_validate_tests.rs`：保留通用 Adapter fixture 并暴露测试 fixture，
  in scope。
- `xtask/src/adapter_validate_excel_tests.rs`：open read/mixed 与四类 write capability
  伪装 read 的失败回归，in scope。
- `docs/memory/decisions.md`：ADR-0107 decision，in scope。
- `MEMORY.md`：仅更新 `decisions.md` 派生规模 330 / 115，in scope。
- `plans/stage-2-excel.md`：新增 TASK-393 批次行与 Review 当前进度，in scope。
- `LEDGER.md`：追加 TASK-393 Review 事件，in scope。

### 3. 验收输出摘要

- `cargo run -p xtask -- adr-index` → PASSED，96 files / 0 error / 0 warning。
- `cargo run -p xtask -- verify-schemas` → PASSED，7 schemas。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift / 0 error。
- `cargo run -p xtask -- check-comments` → PASSED，502 files / 0 error / 75 既有 warning。
- `cargo run -p xtask -- adapter-validate adapters/com.microsoft.excel --report-only` → PASSED，
  7 files / 0 error / 0 warning。
- `python adapters/com.microsoft.excel/tests/validate_adapter_pack.py` → PASSED，
  targets=2 / tools=0 / recipes=0 / interrupts=6 / probe=provisional。
- `python -m unittest discover -s adapters/com.microsoft.excel/tests -p "test_*.py" -v` → **12 tests OK**。
- `python -m py_compile ...`（cache 置于仓库外）→ PASSED。
- `cargo fmt --all --check` → PASSED。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅有仓库既有 unknown lint
  `clippy::assert_is_empty` warning。
- `cargo test --workspace --no-fail-fast` → exit 0；xtask 587 passed，全部 test target 0 failed。
- `cargo deny check` → advisories / bans / licenses / sources 全 OK；仅既有 duplicate 与
  unused-allow warning。
- `cargo run -p xtask -- hygiene` → PASSED，502 files / 0 error / 194 既有 warning。
- `cargo run -p xtask -- card-check` → PASSED，296 files / 0 error / 198 warning。
- `cargo run -p xtask -- docscan` → PASSED，708 files / 0 error / 175 既有 warning。
- `cargo run -p xtask -- refscan` → PASSED，1251 files / 0 error / 0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED，0 error / 1 既有 paint archive warning。
- `cargo run -p xtask -- check-ledger` → PASSED；PLAN / LEDGER 均为 2026-11-23。
- `git diff --check` → PASSED。
- PR checks、merge 与 post-merge main CI：PR #802 checks 11/11 success，pending 0 / failed 0，
  `MERGEABLE` / `CLEAN`；merge `c23e76b8b64d5aea7959808f02808e5ec49a8bc4`；
  post-merge main CI `45115059165` success。

### 4. DoD 逐条核对

- [x] ADR-0107 Accepted，七个 `com.*` ID、风险下限、禁止语义与重讨论条件完整。
- [x] capability catalog、generated Adapter catalog 与 capability-matrix spec 一致。
- [x] T2.1 的 open 是独立 write Tool，read Tool 只消费 workbook/range read；T2.2 消费 bounded
  write/save，T2.3 消费 chart create/export。
- [x] `com.execute/eval/invoke/macro/raw_dispatch`、`office.*` 与 `spreadsheet.*` 明确拒绝。
- [x] Excel App Map、projection、README 与专属 validator 同步到“ADR 已解除、runner 待实现”。
- [x] 专属 Python 12 条 catalog/safety 回归、workspace 与全部治理门禁通过。
- [x] PR checks、merge 与 post-merge main CI 回填：PR #802 checks 11/11 success，
  merge `c23e76b8b64d5aea7959808f02808e5ec49a8bc4`，main CI `45115059165` success。
- [x] 零 COM/GUI/工作簿副作用，零新增依赖、公共 API、ToolSchema 或 runtime 改动。

### 5. 偏差

- 无功能或契约 DRIFT。
- `PROCESS-TASK393-SHELL-MISUSE`：最终累计 **80** 次 `shell="bash"` 误填（主实现 72 +
  独立 review 8）；主会话 72 次中 69 次是只读/build 查询，1 次 `git commit` 写入本 PR
  预期提交、1 次 `gh pr create` 创建 PR #802、1 次 `git switch -c` 建立 closeout 分支；
  无意外文件或外部副作用。
- 首次 `memory-counts` 按预估写成 116 非空行，机器实测为 115；已按工具输出修正为 330 / 115。
- 独立 review 发现两项 P1：open capability 可藏在 read Tool，且 range write 的
  formula/value2 语义冲突。已收紧 ADR/spec，并在共享 validator 中强制 open 的
  write/L3/high/approval/non-idempotent 下限；write 固定 `value2`，公式形态字符串失败。
- 独立 review 的 P3“TASK-392 历史行仍写等待 capability ADR”不改历史条目正文；
  ADR-0041 禁止改写已完成任务行，TASK-393 的新当前进度与批次行已显式 supersede 该描述。
- 第二轮 independent review 发现其他四个 Excel write capability 仍可伪装 read、
  App Map value semantics 未同步写模式；现已增加共享 write-floor 校验、四类负向循环测试，
  并在 App Map/projection/README 写明 `value2` scalar literal 与 Formula 后置证明。
- 首轮抽取后 `tools.rs` / `adapter_validate_tests.rs` 超过 900 行硬门禁；已按职责拆成
  `tool_bounds.rs` 与 `adapter_validate_excel_tests.rs`，最终 hygiene 0 error。

### 6. 更合理做法

- 把旧 blocker `spreadsheet_and_com_capability_ids_require_adr` 改成 implementation blocker，
  避免 catalog 新增 ID 后把“词汇已批准”误读成“Tool 已实现”。
- Excel 专属 validator 只检查 `com.*` 存在和 forbidden denylist，不复制七个 ID 的第二份
  catalog；具体 envelope 子集由后续 T2.1-T2.3 使用共享 generated catalog 校验。
- 七个 capability 的精确集合只放在 Excel 专属 contract test 中锁定；生产 validator 仍只消费
  protocol generated catalog，避免形成 runtime 的第二份枚举。
- `com.workbook_open` 使用共享 validator 的能力下限规则而不是 App 专用特判散落在 handler；
  T2.1 可以据此机器拒绝把进程启动藏在 read Tool 中。
- 四类 Excel write capability 共用一张小型 validator floor 表；range/save/chart create
  必须 L1，chart export 允许 L1/L2/L3 但所有写能力都不得 low 或免审批。

### 7. 遗留问题

- T2.1：独立 owned-workbook open Tool、read-only workbook/range read、命名区域汇总与真实
  `.xlsx` 只读验收。
- T2.2：bounded value write、重算、`.Value2` 校验、in-place save 与 L1 recovery。
- T2.3：chart create、PNG create-new export、不覆盖与真机证据。
- 真实 Protected View / AutoSave / 只读/占用 / 外链 / 宏 interrupt 仍需人类在场验收。

### 8. 新增长期记忆

- 新增 ADR-0107 decision：Excel L1 COM 使用七个 experimental `com.*` capability，禁止
  broad dispatch、alias 与 UIA 冒充；capability ADR 只解除静态目录阻塞。

### 9. 给审阅者的关注点

- 重点检查 capability 粒度是否既能表达 T2.1-T2.3，又没有引入通用 COM escape hatch。
- 重点检查 `workbook_open`、write/save/chart 的审批与 L1 下限是否足够保守。
- 重点检查 Excel pack 没有把“catalog 已有 ID”误写成“tool 已实现”或“真机已验证”。
