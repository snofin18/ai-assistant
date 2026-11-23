# TASK-392 Excel Adapter safety shell pack

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel Adapter　批次：3
- 依赖：TASK-372、TASK-375、TASK-383、TASK-384、TASK-387、TASK-388、TASK-391、
  ADR-0104、ADR-0105、ADR-0106

---

## write scope

- `tasks/TASK-392-excel-adapter-safety-pack.md`
- `plans/stage-2-excel.md`
- `adapters/com.microsoft.excel/**`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

建立第一个可被通用 validator 完整读取的 `com.microsoft.excel` Adapter safety shell pack，
冻结 Excel 身份、COM/UIA 通道边界、workbook+worksheet 身份原则、数据读取口径、
Protected View / AutoSave / 文件冲突中断与禁止属性。

## In scope

- 新建 `adapters/com.microsoft.excel/`，采用 ADR-0104 的七键 bundle envelope。
- 编写富 App Map、确定性 core projection、UIA 壳层 selector、空 tool/rollback 声明及
  Excel interrupt pack。
- Selector 只覆盖 Excel 主窗口与模态/UIA 安全边界，不声明 cell/row/column/grid selector。
- Interrupt 覆盖 Protected View、AutoSave/OneDrive 冲突、只读/占用、覆盖确认、更新链接与
  宏警告，并保持所有危险决定为人工动作。
- 记录 workbook/worksheet 身份、命名区域优先、`.Value2` / `.Formula` / `.Text` 固定读法、
  COM 程序化修改不进入 Excel undo 栈，以及 `DisplayAlerts=false` 永久禁止。
- 增加专属 Python validator，锁定 bundle、projection 字节、selector 边界、空 tool/rollback
  契约与 interrupt 人工矩阵。

## Out of scope

- 不新增 capability ID、ToolSchema 字段、公共 API、crate 或依赖。
- 不创建 Excel COM 对象，不启动/附着真实 Excel GUI，不打开、读取、写入或保存工作簿。
- 不实现 COM runner、selector runtime、中断消费者、L1 影子副本或任何 T2.1~T2.3 工作流。
- 当前 capability catalog 没有 Excel COM/spreadsheet 能力，本卡不得用 UIA capability 冒充 COM；
  `tools.json` 与 `recipes.json` 必须保持显式空数组，后续由独立 capability ADR 解锁。
- 不修改 `protocol/**`、`docs/spec/**`、`docs/adr/**`、Policy、ToolBus、UI 或 CI。

## 必须遵守

1. 所有未观测 selector 明确标记 `probe_status=provisional|required`，不得写成 observed。
2. 可见文本只能作 locale-dependent fallback，不得成为唯一或 primary selector。
3. Protected View 的“启用编辑”、覆盖、更新链接和宏警告必须 `ask_human` / requires human。
4. 不得出现 `DisplayAlerts=false`、自动选择 Save/Discard/Replace/Accept 或固定 sleep 处理。
5. tools 为空时必须同时保证 rollback 为空，且 README/App Map 明确说明这是 capability 阻塞，
   不是已完成 Excel COM tool surface。
6. 不把 data-only validator 通过冒充真实 Excel、Protected View 或 AutoSave 已完成验收。

## 验收命令

```powershell
cargo run -p xtask -- adapter-validate adapters/com.microsoft.excel --report-only
python adapters/com.microsoft.excel/tests/validate_adapter_pack.py
python -m py_compile adapters/com.microsoft.excel/tests/validate_adapter_pack.py
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
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

预期：通用与专属 validator 均通过；空 tool/rollback 不被误当成 COM 能力已实现；
workspace 与文档门禁全绿；零真实 GUI/COM/工作簿副作用。

## DoD

- [ ] Excel bundle 七键、六文件、app_id 与确定性 projection 全部通过通用 validator。
- [ ] App Map 覆盖 workbook+worksheet 身份、命名区域优先与三种读取口径。
- [ ] Selector 只声明 shell/UIA 边界，不出现 grid/cell selector，未观测状态如实标记。
- [ ] Interrupt pack 覆盖六类人工/安全边界，危险动作不自动继续。
- [ ] tools/rollback 为空且 capability ADR 阻塞被 README、App Map 与专属测试明确锁定。
- [ ] 专属 Python validator、workspace 与全部门禁通过；零 COM/GUI/工作簿副作用。
- [ ] PR checks、merge 与 post-merge main CI 回填。

<!-- ─── 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ───
     以下由 Implementer 填写。改动分界线上方任何一行 = 漂移触发器 ⑤。
-->

## 执行记录

### 1. 约束回执

【任务】TASK-392 Excel Adapter safety shell pack
【目标】建立 `com.microsoft.excel` bundle v1 的身份、UIA 壳层 selector、App Map 与
Protected View/AutoSave/文件冲突中断契约。
【write scope】仅：本卡、`plans/stage-2-excel.md`、`adapters/com.microsoft.excel/**`、
`PLAN.md`、`README.md`、`LEDGER.md`。
【铁律】契约先行；不可信输入先校验；无静默失败；资源生命周期显式；不得静默扩围。
【禁止】不改 capability catalog/ToolSchema/公共 API；不创建 COM 对象、不启真实 GUI、
不读写工作簿；不把声明数据冒充运行验证。
【验收】通用与专属 validator、schema/codegen、workspace、fmt 与全部治理门禁。
【依赖】TASK-372/375/383/384/387/388/391 与 ADR-0104/0105/0106。
【疑问】无；capability 不足时按契约保持 tools/rollback 为空。

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `tasks/TASK-392-excel-adapter-safety-pack.md`：状态与执行记录，in scope。
- `plans/stage-2-excel.md`：批次 3 派发为 TASK-392，in scope。
- `adapters/com.microsoft.excel/adapter.toml`：身份、COM/UIA 边界与 bundle envelope，in scope。
- `adapters/com.microsoft.excel/app_map.json`：Excel 知识源与 projection manifest，in scope。
- `adapters/com.microsoft.excel/memory/app_map.v1.json`：确定性 projection，in scope。
- `adapters/com.microsoft.excel/selectors/targets.json`：仅 shell/modal UIA target，in scope。
- `adapters/com.microsoft.excel/tools/tools.json`：显式空 tool surface，in scope。
- `adapters/com.microsoft.excel/rollback/recipes.json`：无 write tool 时显式空 coverage，in scope。
- `adapters/com.microsoft.excel/interrupts/interrupts.json`：六类人工安全边界，in scope。
- `adapters/com.microsoft.excel/README.md`：capability 阻塞、安全边界与验收说明，in scope。
- `adapters/com.microsoft.excel/tests/validate_adapter_pack.py`：专属数据契约 validator，in scope。
- `adapters/com.microsoft.excel/tests/test_validate_adapter_pack.py`：8 条安全变异负向回归，in scope。
- `LEDGER.md`：追加 Review 事件，in scope。
- `PLAN.md` / `README.md`：Done 回填当前状态、当前阶段与最近进展，in scope。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- `cargo run -p xtask -- adapter-validate adapters/com.microsoft.excel --report-only` →
  PASSED，7 files / 0 error / 0 warning。
- `python adapters/com.microsoft.excel/tests/validate_adapter_pack.py` →
  PASSED，targets=2 / tools=0 / recipes=0 / interrupts=6 / probe_status=provisional。
- `python -m py_compile adapters/com.microsoft.excel/tests/validate_adapter_pack.py
  adapters/com.microsoft.excel/tests/test_validate_adapter_pack.py` → PASSED。
- `python -m unittest -v test_validate_adapter_pack.py` → **8 tests OK**，selector scope/class、
  interrupt class/provisional、process reuse、GUI claim、interrupt default 与禁用属性变异均被拒绝。
- `cargo run -p xtask -- verify-schemas` → PASSED，7 schemas。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift。
- `cargo fmt --all --check` → PASSED。
- `cargo test --workspace --no-fail-fast` → exit 0；xtask 584 passed，0 failed。
- `cargo run -p xtask -- hygiene` → PASSED，0 error。
- `cargo run -p xtask -- card-check` → PASSED，0 error。
- `cargo run -p xtask -- docscan` → PASSED，0 error。
- `cargo run -p xtask -- refscan` → PASSED，0 error / 0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED，0 error。
- `cargo run -p xtask -- check-ledger` → PASSED。
- `git diff --check` → PASSED。

### 4. DoD 逐条核对

- [x] Excel bundle 七键、六文件、app_id 与确定性 projection 均通过通用 validator。
- [x] App Map 覆盖 workbook+worksheet 身份、命名区域优先与 `value2/formula/text` 口径。
- [x] Selector 只声明 `XLMAIN` 与 `#32770` shell/modal 边界，不含 cell/grid/row/column；
  probe 状态如实为 `provisional/required`。
- [x] Interrupt pack 覆盖 Protected View、AutoSave/OneDrive、只读/占用、覆盖、更新链接、
  宏警告，六项全部 `ask_human` 且 requires human。
- [x] tools/rollback 均为空；README、App Map 和专属测试明确锁定 capability ADR 阻塞。
- [x] 专属 validator、workspace 与全部门禁通过；未创建 COM/GUI/工作簿副作用。
- [x] PR checks、merge 与 post-merge main CI：PR #799 checks 11/11 success，pending 0 / failed 0，
  `MERGEABLE` / `CLEAN`；merge `92024e8f221b535c982c97822da996ed51b7d74a`；
  post-merge main CI `45101893667` success。

### 5. 偏差

- 无功能或契约 DRIFT。
- `capability catalog` 当前无 Excel COM/spreadsheet ID，因此本卡按既定边界保持 0 tools /
  0 recipes；不新增 capability，也不以 UIA capability 冒充 COM。
- `PROCESS-TASK392-SHELL-MISUSE`：实现、审阅与 closeout 期间累计 **46** 次 `shell="bash"` 误填
  （当前实现 38 次 + 首轮独立 review 5 次 + 复核 1 次 + closeout 2 次）；均未写仓库，
  PowerShell 正常时已切回。

### 6. 更合理做法（实施中发现，非漂移）

- 把“空 tool surface”做成专属测试断言，而不是只写在 README；这样后续若有人提前塞入
  未获 ADR 的 COM Tool，数据门禁会直接失败。
- Interrupt match 全部标注 provisional；validator 同步强制这一证据状态，避免把未观测的
  Excel 对话框标题冒充真机事实。
- 独立 review 的 P2/P3 已闭合：validator 现在精确锁定 `XLMAIN/#32770` selector 与 interrupt
  shape、App Map 安全布尔/deny-list，并实际读取 capability catalog 证明空 surface 的阻塞来源；
  新增 8 条内存变异负向测试。

### 7. 遗留问题

- Excel COM/spreadsheet capability vocabulary 需独立 ADR，之后才能声明 workbook/range 工具
  与对应 rollback recipes。
- `XLMAIN`、Protected View、AutoSave/OneDrive、只读/占用、覆盖、链接和宏警告仍需要真实
  Excel 场景观测与人工验收。
- `#32770` 必须在未来 runtime 中绑定本轮拥有的 Excel 进程身份；仅 class 匹配不可形成可靠 target。
- 本卡不实现 COM runner、T2.1/T2.2/T2.3 或 L1 影子副本。

### 8. 新增长期记忆

- 无。Excel 版本、undo 边界、Protected View/AutoSave 风险和 COM ownership 已存在于
  `docs/memory/apps/excel.md`；本卡只消费这些事实。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

1. 重点确认 0 tools / 0 recipes 不仅写死为空，而且 validator 实际读取 capability catalog。
2. 重点确认精确 `XLMAIN/#32770`、App Map danger deny-list 与 8 条负向变异测试没有漏检。
3. 重点确认六类 interrupt 均不会自动 Enable Editing、Save、Discard、Replace、Update 或
   Accept，且未观测 match 全为 provisional。
