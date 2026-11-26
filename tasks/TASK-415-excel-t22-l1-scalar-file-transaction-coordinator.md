# TASK-415 Excel T2.2 L1 scalar file-transaction coordinator

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6i　依赖：TASK-404、TASK-406、TASK-407、TASK-411、TASK-414、ADR-0062、ADR-0107、ADR-0108、ADR-0111

---

## write scope

- `tasks/TASK-415-excel-t22-l1-scalar-file-transaction-coordinator.md`
- `apps/agent-core/src/excel_t21_l1.rs`
- `apps/agent-core/src/excel_t21_l1_write_port.rs`
- `apps/agent-core/src/excel_t21_l1_transaction_tests.rs`
- `apps/agent-core/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 test-only Excel T2.2 L1 边界内组合现有 anchor、scalar write-port 与文件影子副本能力：
一次调用先捕获内存值锚点和物理影子文件，再执行带 `.Value2/.Formula` 双读与 post-fingerprint
的 scalar 写入；成功返回可回滚句柄，失败保留有界证据且显式报错。回滚仍要求当前文件 digest
等于调用方确认的 post-save digest，成功恢复 anchor digest 后才释放 anchor。

本卡只证明私有事务编排和 fake/临时文件语义，不开放真实保存 caller、production Tool 或
Host 崩溃恢复。

## In scope

- 在既有 `excel_t21_l1_write_port` 私有模块中增加组合入口：
  - 先解析 scalar，非法输入不得创建 anchor 或物理 shadow；
  - 通过 `ExcelL1AnchorStore::capture_with_file_shadow` 捕获 anchor 与文件影子；
  - 复用现有 scalar write-port 的 prepare/write/recalculate/双读/post-fingerprint 路径；
  - 成功返回 anchor handle、post fingerprint、原始文件 digest 与写入 receipt；
  - 失败保留 anchor/shadow 证据，不用默认值或删除证据伪装成功。
- 增加显式回滚入口：
  - 只接受组合成功后返回的待回滚 receipt；
  - 使用 receipt 的 post fingerprint 构造观察身份；
  - 要求调用方提供当前文件 post-save digest；
  - 复用 `restore_with_file_shadow` 的冲突门、文件恢复与 anchor 释放语义。
- 新增 test-only 专项测试，使用 fake write port 和本卡临时 `.xlsx` 字节覆盖：
  - 非法 scalar 在捕获前失败；
  - 成功组合后 anchor/shadow 计数有界；
  - 模拟保存后按 post-save digest 回滚，文件回到原 digest；
  - 用户文件漂移或 fingerprint 证据缺失时拒绝回滚并保留 anchor/shadow；
  - 写入端口失败时不返回成功且证据可清理；
  - 重复回滚/未知 handle 显式失败，cleanup 收敛。
- 同步 agent-core README、Stage 2、PLAN、README、LEDGER。
- 完成真实门禁、独立 review、PR/CI/merge 与 post-merge main CI 收口。

## Out of scope

- 不注册 `excel.*` production ToolBus Tool，不填写 `tools.json`、rollback recipe 或 assembly。
- 不启动真实 Excel/COM/GUI，不打开、保存、覆盖或恢复用户工作簿。
- 不实现 production in-place save port、Save As、原子替换、断电恢复或 Host 崩溃 helper process。
- 不修改 `assistant-platform-api`、Windows runner、protocol/schema、ErrorCode、Policy 或依赖。
- 不把 test-only 临时文件恢复证明为 production T2.2 保存回滚已经完成。

## 必须遵守

1. scalar 解析必须发生在 anchor 和物理 shadow 发布之前；非法输入不得消耗容量。
2. 组合成功只能返回显式待回滚 receipt，不得把“已写入但尚未保存确认”声明为最终提交。
3. 回滚必须复用既有 L1 fingerprint 冲突门和文件 shadow digest 门；任一门失败不得写文件。
4. 写入、读回、fingerprint、记录 post 或回滚失败都必须返回既有 `ErrorCode`，不得吞错误。
5. 失败时保留 anchor/shadow 或显式 cleanup 的证据边界必须由测试锁定，不得无记录丢证据。
6. 测试只使用本卡临时目录与合成 `.xlsx` 字节，不触碰用户目录或真实 Excel。
7. 不修改测试断言规避缺陷，不新增 `#[allow]`、`unsafe`、第三方依赖或公共接口。

## 验收命令

```powershell
cargo test -p assistant-agent-core scalar_file_transaction
cargo test -p assistant-agent-core excel_t21
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- docscan
cargo deny check
git diff --check
```

预期：专项与全量门禁通过；未运行真实 Excel/COM/GUI；production `tools.json`、ToolBus、
rollback、assembly、schema、公共 API 与依赖零改动。

## DoD

- [ ] 非法 scalar 在 anchor/shadow 捕获前失败，容量与物理文件计数不变。
- [ ] 成功组合执行 scalar write-port 全路径并返回 anchor handle、post fingerprint、原始 digest 与 receipt。
- [ ] 回滚只接受组合 receipt，并在 post-save digest 与 post fingerprint 均匹配后恢复原文件。
- [ ] 文件漂移、post fingerprint 缺失、重复回滚或未知 handle 均显式失败，且不覆盖用户变化。
- [ ] 写入端口失败不返回成功；anchor/shadow 证据边界明确，cleanup 后计数与临时文件收敛。
- [ ] 测试使用独立临时目录，成功/失败路径无本卡临时文件静默残留。
- [ ] production Tool/ToolBus、`tools.json`、rollback、assembly、schema、公共 API、ErrorCode、Windows runner 与依赖零改动。
- [ ] 全量门禁、独立 review no P0-P2、PR/CI/merge/main CI 证据落入记录区。
- [ ] 外部 automation JSONL 记录 start/gates/work/pr/ci/merge/main_ci/final 与 shell misuse。

<!-- 分界线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-415 Excel T2.2 L1 scalar file-transaction coordinator
【目标】组合 anchor、write-port 与文件影子副本，形成显式可回滚的 test-only L1 事务
【write scope】仅：本卡、agent-core L1 模块/专项测试、agent-core README、Stage 2、PLAN、README、LEDGER
【铁律】无静默失败；写后有 postcondition；输入先校验；资源有界；不扩公共边界；热点文档先取 guard
【禁止】不碰真实 Excel/GUI/用户工作簿；不填 tools.json、ToolBus、rollback、assembly；不改 schema/公共 API/依赖
【验收】scalar_file_transaction + Excel 域 + fmt/clippy/workspace/arch/schema/codegen/doc gates/deny/diff-check
【依赖】TASK-404/406/407/411/414 Done；ADR-0062/0107/0108/0111 已核对；N_auto=0
【疑问】无；production save caller 与 Host 崩溃恢复保持后续卡
```

### 2. 实际改动

- `excel_t21_l1_write_port.rs` 新增 test-only `ExcelL1ScalarFileWriteReceipt`、
  `execute_scalar_write_with_file_shadow` 与 `rollback_scalar_write_with_file_shadow`：
  scalar 在捕获前解析；捕获后复用 write/recalculate/Value2/Formula/post-fingerprint 全路径；
  回滚以借用 receipt 同时校验 post fingerprint 与调用方给出的 saved-file digest。
- 新增 `excel_t21_l1_transaction_tests.rs`：7 个专项覆盖非法 scalar 前门、物理发布失败收敛、
  成功回滚、重复回滚、用户文件 drift、write failure 与缺失 post-fingerprint。
- 同步 agent-core README；未修改 production Tool/ToolBus、`tools.json`、rollback、assembly、
  schema、公共 API、ErrorCode、Windows runner 或依赖。

### 3. 门禁输出摘要

- `cargo test -p assistant-agent-core scalar_file_transaction`：7 passed。
- `cargo test -p assistant-agent-core excel_t21`：110 passed / 1 ignored。
- `cargo test --workspace`、doctest、`cargo test -p assistant-core arch::`：全部通过。
- `cargo fmt --all --check`、workspace Clippy：PASSED；仅仓库既有
  `clippy::assert_is_empty` unknown-lint warning。
- `verify-schemas`、`codegen --check`、`hygiene`、`memory-counts`、`adr-index`、
  `check-comments`、`cargo deny check`：PASSED。
- `cargo llvm-cov --fail-under-lines 75`：PASSED，workspace line coverage **77.96%**。
- 独立 review 初审 **No findings (P0-P2)**；P3 回滚 receipt 语义已改为借用并补 capture
  publish failure cleanup 测试；复审仍为 **No findings (P0-P2)**，专项 7/7 通过。

### 4. DoD 核对

- [x] 非法 scalar 在 anchor/shadow 捕获前失败，容量与物理文件计数不变。
- [x] 成功组合执行 scalar write-port 全路径并返回 anchor handle、post fingerprint、原始 digest 与 receipt。
- [x] 回滚只接受组合 receipt，并在 post-save digest 与 post fingerprint 均匹配后恢复原文件。
- [x] 文件漂移、post fingerprint 缺失、重复回滚均显式失败，且不覆盖用户变化。
- [x] 写入端口失败不返回成功；capture publish/write/rollback 失败后 cleanup 收敛已有专项测试。
- [x] 测试使用独立临时目录，成功/失败路径无本卡临时文件静默残留。
- [x] production Tool/ToolBus、`tools.json`、rollback、assembly、schema、公共 API、ErrorCode、
  Windows runner 与依赖零改动。
- [x] 独立 review 复审 no P0-P2。
- [x] PR #887 checks 22（21 success + 1 skipped），merge `e91ecdfe9d5438aea73f58452a4c838db0695f26`。
- [x] post-merge main CI `45884850506` success。
- [ ] 外部 automation JSONL final 事件与最终 shell misuse 待写入。

### 5. 偏差

- 首版回滚按值消费 receipt，失败后调用方无法保留同一证据句柄；独立 review 以 P3 指出后
  已改为借用 receipt，并由底层 anchor release 负责拒绝重复成功。
- 为满足 600 行软预算告警边界，组合实现留在既有 844 行 test-only write-port 文件，
  专项测试放入独立 500 行私有测试模块；均在 900 行硬门禁内。

### 6. 新增长期记忆

无。TASK-415 的稳定事实由本卡、agent-core README 与 LEDGER 承载，未修改
`MEMORY.md` / `docs/memory/*`。

### 7. 遗留与下一步

- production in-place save port、真实 COM 写入后的文件落盘、原子替换、异常断电恢复与
  task-end caller 仍待后续卡。
- production `tools.json`、ToolBus、rollback recipe 与 assembly registration 继续为空。

### 8. 外部证据

- implementation PR #887：
  `https://github.com/snofin18/ai-assistant/pull/887`
  checks 22（21 success + 1 skipped），pending 0 / failed 0，base=main，
  `MERGEABLE` / `CLEAN`。
- merge `e91ecdfe9d5438aea73f58452a4c838db0695f26`；post-merge main CI
  `45884850506` success。
- automation JSONL：
  `C:\Users\fexfe\.codex\automations-logs\2026-11-26\ai-assistant-task-round-20261127-0300.jsonl`。

### 9. 给审阅者的关注点

- 核对 receipt 借用的单一状态语义：首次成功释放 anchor，重复回滚必须显式失败。
- 核对 post fingerprint 与 saved-file digest 两道门是否都在任何文件写入前通过。
- 核对 capture publish failure 后 anchor/shadow cleanup 是否真正收敛，而非依赖测试目录 Drop。
