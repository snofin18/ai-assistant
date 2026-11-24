# TASK-398 Excel T2.1 ToolBus handler wrappers

- 状态：**Review**
- 阶段：2　子阶段：Stage 2 Excel T2.1　批次：4d　依赖：TASK-395、TASK-397、ADR-0108、ADR-0109

---

## write scope

- `tasks/TASK-398-excel-t21-toolbus-handler-wrappers.md`
- `apps/agent-core/src/excel_t21.rs`
- `apps/agent-core/src/excel_t21_tool_handlers.rs`
- `apps/agent-core/src/excel_t21_handlers.rs`
- `apps/agent-core/src/excel_t21_model.rs`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

把 TASK-395 的四个 Excel T2.1 纯逻辑 handler 包装为私有 `ToolHandler`，通过真实
in-process ToolBus 证明成功载荷、不可信来源和 typed `HandlerFailure` failure envelope
均无损；仍不注册生产 ToolBus、不启动 Excel、不修改 Adapter tool 声明。

## In scope

- 新增 test-only `excel_t21_tool_handlers.rs`，提供四个私有 `ToolHandler` 包装。
- `ExcelT21Error` 统一映射为 `ToolBusError::HandlerFailure { code, message }`。
- wrapper 的 ToolBus session id 必须从既有 registry 派生，并由同一方法产出 `ToolBusConfig`。
- 成功输出映射为 `ToolOutput`：app-derived 内容必须带 `app_content` source，并声明
  workbook/range 的既有 payload 上限。
- 修正 range payload 超限应映射 `ToolInvalidArgs`、workbook payload 超限应映射
  `UserInteraction` 的既有错误矩阵偏差。
- 用 fake runtime 经真实 `ToolBus::start` / `call_tool` 验证 open/read 成功和 typed failure。
- 覆盖全部 `ErrorCode::ALL` 的 `ExcelT21Error` → `HandlerFailure` 映射。
- 同步 Stage 2、PLAN、README、LEDGER 与任务记录。

## Out of scope

- 不把四个 handler 注册到生产 ToolBus，不修改 production assembly。
- 不修改或填写 `adapters/com.microsoft.excel/tools/tools.json` 或 rollback recipes。
- 不实现或启动 Windows COM runner，不调用 Excel，不读取真实 `.xlsx`。
- 不修改 protocol/schema、ToolSchema、ErrorCode、ToolBus 公共 API、ToolHandler trait、
  第三方依赖或 CI。
- 不执行人类在场验收，不把 test-only wrapper 写成已可生产使用。

## 必须遵守

1. `ToolHandler::call` 只调用既有 pure handler，不新增第二套业务逻辑。
2. 所有 `ExcelT21Error::code()` 必须原样进入 `HandlerFailure.code`，不得降级为
   `Transient`、`Fatal` 或 MCP 数字码。
3. application-derived payload 必须 `untrusted=true`，source 固定为
   `com.microsoft.excel` 的 `app_content`。
4. wrapper 必须在 envelope 前再次执行 payload byte bound；超限失败，不依赖截断。
5. `tools.json` 保持 `tools: []`，production assembly 不引用新模块。
6. 测试不得启动 Excel/COM/GUI，不得读取真实 `.xlsx`。

## 验收命令

```powershell
cargo test -p assistant-agent-core --lib excel_t21
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo deny check
git diff --check
```

## DoD

- [ ] 四个 pure handler 均由私有 `ToolHandler` 包装。
- [ ] `ExcelT21Error` 的 13 类 `ErrorCode` 全部原样映射为 `HandlerFailure`。
- [ ] 真实 in-process ToolBus 的 open/read 成功 envelope 保留数据与 `untrusted` source。
- [ ] 真实 in-process ToolBus 的 typed failure envelope 保留 `VerifyFailed` code 与 message。
- [ ] envelope 前 payload bound 二次校验存在且有测试。
- [ ] `tools.json`、production assembly、protocol、公共 API、依赖零改动。
- [ ] 文档门禁、PR、CI、merge 与 post-merge main CI 回填。

<!-- ───────── 分界线：以上是卡片正文，Orchestrator 所有，Implementer 只读 ─────────
     以下由 Implementer 填写。状态行仅按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-398 Excel T2.1 ToolBus handler wrappers
【目标】把四个 Excel T2.1 pure handler 包装为私有 ToolHandler，经真实 in-process ToolBus 验证成功载荷与 typed failure envelope
【write scope】仅：本卡、excel_t21 新私有模块与模块声明、Stage 2/PLAN/README/LEDGER
【铁律】无静默失败；输入不可信；错误码不丢失；资源有界；公共接口零变更；不把 test-only 包装冒充生产注册
【禁止】不改 tools.json/rollback/COM runner；不注册生产 ToolBus；不改 protocol/schema/ErrorCode/依赖；不启 Excel/GUI；不读真实 .xlsx
【验收】专项 ToolBus 合同测试、workspace tests、fmt、clippy、schema/codegen、文档/LEDGER 门禁、deny、diff-check 全绿
【依赖】TASK-395 Ready WIP、TASK-397 Done、ADR-0108/0109 Accepted；main dc699b9
【疑问】无；四个既有模块已在 900 行硬限，因此包装代码放独立 test-only 模块
```

### 2. 实际改动文件

- `apps/agent-core/src/excel_t21.rs`：新增 test-only `tool_handlers` 模块声明。
- `apps/agent-core/src/excel_t21_tool_handlers.rs`：四个私有 `ToolHandler` wrapper、registry 派生
  session identity、唯一 `ToolBusConfig`、typed `HandlerFailure` 映射、source 与 payload bound，
  以及 in-process ToolBus 合同测试。
- `apps/agent-core/src/excel_t21_handlers.rs`：公开 crate 内 session registry 读取入口；
  range payload 超限修正为 `ToolInvalidArgs`，workbook read 保持 `UserInteraction`。
- `apps/agent-core/src/excel_t21_model.rs`：`ExcelT21Error::Display` 仅保留原始 message；
  registry session id 允许 crate 内 wrapper 派生。
- 本卡、Stage 2、PLAN、README、LEDGER。
- `adapters/com.microsoft.excel/tools/tools.json`、production assembly、protocol、公共 API 与
  依赖未修改。

### 3. 验收输出摘要

- `cargo test -p assistant-agent-core --lib excel_t21` → **27 passed / 0 failed**。
- `cargo test --workspace` → PASSED；agent-core lib `128 passed`、xtask `587 passed`，其余
  workspace tests / doctests 全绿。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅保留仓库既有
  `clippy::assert_is_empty` unknown-lint warning。
- `cargo fmt --all --check` → PASSED。
- `verify-schemas` → 7/7 parsed；`codegen --check` → 0 drift。
- `cargo test -p assistant-core arch::` → 10 passed / 0 failed。
- `cargo deny check` → advisories / bans / licenses / sources ok。
- `git diff --check` → PASSED。
- 独立 review 首轮 P1 + 2 P2 已修复，复核结论 **no findings**。

### 4. DoD 逐条核对

- [x] 四个 pure handler 均由私有 `ToolHandler` 包装。
- [x] `ExcelT21Error` 的 13 类 `ErrorCode` 全部原样映射为 `HandlerFailure`。
- [x] 真实 in-process ToolBus 的 open/read 成功 envelope 保留数据与 `untrusted` source。
- [x] 真实 in-process ToolBus 的 typed failure envelope 保留 `VerifyFailed` code 与 exact message。
- [x] envelope 前 payload bound 二次校验存在，range 超限为 `ToolInvalidArgs` 并有专项测试。
- [x] `tools.json`、production assembly、protocol、公共 API、依赖零改动。
- [ ] 文档门禁、PR、CI、merge 与 post-merge main CI 回填。

### 5. 偏差

- 独立 review P1：wrapper 的 session id 与 registry 可任意错配。已改为从 session registry
  派生，并由同一实例产出唯一 `ToolBusConfig`；测试使用该 config，新增跨 task 拒绝。
- 独立 review P2：range payload 超限曾被映射为 `UserInteraction`。已在 pure handler 与 wrapper
  均按 ADR-0108 修正为 `ToolInvalidArgs`；workbook read 保持 `UserInteraction`。
- 独立 review P2：`ExcelT21Error::Display` 曾添加 code 前缀。现仅输出原始 message，测试改为
  exact equality。
- `PROCESS-TASK398-SHELL-MISUSE`：主会话 51 次、独立 review 12 次只读/build/CI 命令误填
  `shell="bash"`，合计 **63** 次，无 `powerspace`；项目 hook 实际以 PowerShell 执行，
  无意外仓库写入或外部副作用。

### 6. 更合理做法

- wrapper 只委托既有 pure handler，不复制业务逻辑；session identity 从 registry 派生，避免
  两个字符串各自维护。
- error mapping 继续复用 ADR-0109 的 typed `HandlerFailure`，不建立第二套错误桥。
- 测试同时覆盖四个成功路径、13-code 映射、source provenance、payload bound、跨 task 拒绝与
  `VerifyFailed` envelope。

### 7. 遗留问题

- `tool_bus_config()` 的配对仍是装配约定；后续 production assembly 必须使用同一方法，不得
  手工传入另一个 session id。
- production ToolBus 注册、Windows COM runner、ownership journal、独立 watchdog、真实
  `.xlsx` 与人类在场验收仍属后续卡片。
- `tools.json` 必须继续为空，直到 COM runner 与真机收敛门禁通过。

### 8. 新增长期记忆

无。

### 9. 给审阅者的关注点

- 确认 wrapper 的 ToolBus session id 来自 registry，且 `tool_bus_config()` 使用同一值。
- 确认 range/workbook payload 超限分别保持 `ToolInvalidArgs` / `UserInteraction`。
- 确认错误 envelope message 不再带 code 前缀，且 source 固定为
  `app_content/com.microsoft.excel/excel_workbook`。
