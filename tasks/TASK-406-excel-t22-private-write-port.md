# TASK-406 Excel T2.2 private scalar range write port

- 状态：**Review**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6　依赖：TASK-403、TASK-404、TASK-405、ADR-0062、ADR-0107、ADR-0108、ADR-0110

---

## write scope

- `tasks/TASK-406-excel-t22-private-write-port.md`
- `apps/agent-core/src/excel_t21_l1.rs`
- `apps/agent-core/src/excel_t21_l1_write.rs`
- `apps/agent-core/src/excel_t21_l1_write_port.rs`
- `apps/agent-core/src/excel_t21_l1_write_port_tests.rs`
- `apps/agent-core/README.md`
- `docs/automations/2026-11-25-round-3.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 agent-core 内建立 test-only、私有的 Excel T2.2 scalar range write port 合同，把
TASK-404 的 L1 anchor 与 TASK-405 的 scalar 输入门串成一次不可重试的 fake 写入事务：
写前校验目标与 pre-fingerprint，写后显式计算、双读 `.Value2`/`.Formula`、复核
shape/identity 并记录一次 post-fingerprint。真实 COM、文件、保存与生产注册继续保持未接线。

## In scope

- 新增 test-only 私有 `excel_t21_l1_write_port` 子模块及独立测试文件，避免继续扩大
  `excel_t21_l1.rs`。
- 私有无副作用 port 只接受自包含 workbook identity、显式 `RangeSelector` 与已校验
  scalar literal；不暴露 COM 指针、PID、HWND 或任意 property/method 字符串。
- 目标 range 必须与 anchor 捕获的 canonical selector 完全一致；写入必须经既有
  `ExcelL1AnchorStore::prepare_write` 完成一次性 pre-fingerprint 校验。
- port 顺序固定为 scalar 校验 → anchor prepare → scalar write → recalculate →
  `.Value2` 读回 → `.Formula` 读回 → shape/value 校验 → post-fingerprint。
- `.Value2` 与 `.Formula` 都必须完整覆盖同一 range；每个 cell 的 value type、地址与值必须
  与 scalar 的预期闭集一致，公式形态不得在写后被隐式生成。
- post-fingerprint 只能通过 `ExcelL1AnchorStore::record_post_write` 记录一次；任一写、
  重算、读回、shape、identity 或 fingerprint 失败都必须显式失败，不得伪造成功或重试。
- fake port 覆盖 null/boolean/number/string、multi-cell range、selector drift、port 失败、
  重算失败、双读 mode/dimension/value drift、post 缺失/重复、released handle 与 single-use。
- 独立 review、PR、CI、merge 与 post-merge main CI 收口；Done 状态另行 docs-only backfill。
- 同步 agent-core README、Stage 2 当前进度、PLAN/README/LEDGER 与运行日 round 证据。

## Out of scope

- 不运行真实 Excel、COM、GUI 或任何 `.xlsx` 工作簿；不操作真实文件或用户进程。
- 不实现真实 COM write/recalculate/save、磁盘影子副本、恢复、ownership journal 或 watchdog。
- 不填写 `adapters/com.microsoft.excel/tools/tools.json`、rollback recipe 或 assembly。
- 不注册 production ToolBus，不建立模型可见 `excel.range.write` Tool。
- 不修改 protocol/schema、公共 `platform/api` trait、`assistant-platform-windows` 公共面、
  ErrorCode、Policy、依赖或测试成功判据。
- 不以 fake write-port 测试冒充真实 T2.2 写入、保存、恢复或真机验收。

## 必须遵守

1. scalar 必须先通过既有输入门；anchor prepare 之前不得产生任何 port 副作用。
2. 目标 selector 必须与 anchor 捕获的 selector 一致，identity/fingerprint 必须由既有
   `ExcelL1AnchorStore` 校验。
3. 写入、重算与双读顺序固定；读回必须是同一 selector 的 complete `Value2` 与 `Formula`。
4. 双读 shape、cell coordinates、value type、canonical value 与公式前缀必须 fail-closed 校验。
5. post-fingerprint 只能在双读成功后记录一次；失败事务不得被报告为 committed。
6. production `tools.json`、ToolBus、rollback、assembly、真实文件系统保持零改动。
7. 真实 Excel/COM 未运行时不得在 DoD 或记录中写成功。

## 验收命令

```powershell
cargo test -p assistant-agent-core 'excel_t21::l1::write_port'
cargo test -p assistant-agent-core excel_t21
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-platform-windows --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
cargo run -p xtask -- replay --suite core
cargo llvm-cov --workspace --fail-under-lines 75
git diff --check
```

预期：write-port 与 Excel 专项测试全绿；workspace、文档门禁和覆盖率通过；真实 COM/文件
写入未执行且 production Excel surface 仍为空。

## DoD

- [ ] selector 与 anchor 绑定、single-use prepare、port 调用顺序均有正负测试。
- [ ] write/recalculate/readback 任一步失败均不产生 committed 结果或 post-fingerprint。
- [ ] `.Value2`/`.Formula` 的 mode、shape、coordinates、type/value 漂移全部 fail-closed。
- [ ] post-fingerprint 只在双读成功后记录一次；重复 post 与 released handle 返回既有 `ErrorCode`。
- [ ] 生产 tools/ToolBus/rollback/assembly、protocol/schema/依赖零改动。
- [ ] 专项、全量门禁与独立 review 完成；PR/CI/merge/post-merge main CI 证据落到记录区。
- [ ] 外部 automation JSONL 记录 start/gates/work/pr/ci/merge/main_ci/final 与 shell misuse。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-406 Excel T2.2 private scalar range write port
【目标】实现 test-only 私有 scalar range write port，串联 pre-fingerprint、写入、重算、Value2/Formula 双读与 post-fingerprint
【write scope】仅：TASK-406 卡、agent-core Excel L1 write/新增 write-port 模块与测试/README、2026-11-25 round 3、Stage 2、PLAN、README、LEDGER
【铁律】不可信输入先校验；无静默失败；每次写必须有 postcondition；资源有界；不伪造真实 COM/文件证据；公共契约先行
【禁止】不运行真实 Excel/COM/GUI；不读写真实 .xlsx；不改 tools.json/ToolBus/rollback/assembly/schema/公共 API/ErrorCode/依赖
【验收】write-port 专项、Excel 专项、fmt/clippy、workspace/arch/schema/codegen/文档门禁/deny/replay/coverage/diff check → 全绿
【依赖】TASK-403/404/405 Done；ADR-0062/0107/0108/0110 Accepted；LEDGER 已核对
【疑问】无；真实 range write、保存、磁盘影子副本仍留给人类在场后续卡
```

### 2. 实际改动

- 新增 test-only 私有 `excel_t21_l1_write_port` 事务与独立测试文件；`ExcelL1ValueSnapshot`
  增加 canonical address、rows、columns，使写前 selector 与 anchor 捕获范围双向绑定。
- 写事务顺序固定为 scalar parse → selector/anchor 校验 → single-use `prepare_write` →
  fake scalar write → recalculate → `.Value2` read → `.Formula` read → shape/value 校验 →
  post-fingerprint。任一写后失败都保留已消费 anchor，不记录 post，也不能在同一 anchor
  上重试成功。
- A1 写后地址同时校验 worksheet、canonical address、range 边界、rows/columns 与 cell 数；
  NamedRange 必须返回 anchor 捕获的完整地址。`Sheet2!A1:B2`、address/dimension drift、
  mode drift、formula prefix、partial/duplicate/out-of-bounds cell 均 fail-closed。
- FakePort 记录实际收到的 selector 与 scalar literal，并由写后状态生成双读；覆盖写入、
  重算、Value2、Formula、fingerprint 的真实 `Err`，验证 Value2 失败时不继续调用 Formula。
- `excel_t21_l1.rs` 只接线 test-only 子模块，并把 canonical address helper 放到现有
  write-port 文件，使 L1 866 行、write-port 744 行、测试 808 行均低于 900 行硬门禁。
- production `tools.json`、ToolBus/rollback/assembly、protocol/schema、`platform/api`
  与依赖零接线或零变更。

### 3. 门禁输出摘要

- `cargo test -p assistant-agent-core 'excel_t21::l1::write_port'` → **12 passed**。
- `cargo test -p assistant-agent-core excel_t21` → **82 passed / 1 ignored**。
- `cargo test --workspace` → PASSED；assistant-agent-core lib 183 passed / 1 ignored，
  xtask 587 passed，其余 crate/doctest 全绿。
- `cargo fmt --all --check`、workspace Clippy、Linux-target platform Clippy → PASSED；
  仅仓库既有 `clippy::assert_is_empty` unknown-lint warning。
- assistant-core arch、schema/codegen、hygiene/card-check/docscan/refscan/memory-counts/
  adr-index/check-ledger/check-comments 全绿；hygiene 0 error。
- `cargo deny check` → advisories / bans / licenses / sources ok。
- core replay → 2 steps / 0 mismatch。
- `cargo llvm-cov --workspace --fail-under-lines 75` → **78.00%** line coverage。
- `git diff --check` → PASSED。

### 4. DoD 核对

- [x] selector 与 anchor 绑定、single-use prepare、port 调用顺序均有正负测试。
- [x] write/recalculate/readback 任一步失败均不产生 committed 结果或 post-fingerprint。
- [x] `.Value2`/`.Formula` 的 mode、shape、coordinates、type/value 漂移全部 fail-closed。
- [x] post-fingerprint 只在双读成功后记录一次；重复 post 与 released handle 返回既有 `ErrorCode`。
- [x] 生产 tools/ToolBus/rollback/assembly、protocol/schema/依赖零改动。
- [ ] PR/CI/merge/post-merge main CI 证据待 implementation PR 收口。
- [ ] 外部 automation JSONL 的 PR/CI/merge/main_ci/final 事件待收口。

### 5. 偏差

- 独立 review 首轮发现 1 P1 + 2 P2：readback 未绑定 worksheet/dimension、读取错误路径
  覆盖不足、fake 未使用实际写入参数。全部修复后同一 reviewer 复审 **No findings
  (P0-P2)**。
- 复审 P3 指出 out-of-bounds 用例先命中 partial count；已改为 4 个唯一 cell 且其中一个
  越界，精确定位坐标校验。
- 首次 hygiene 发现 L1 909 行、write-port 测试 1000 行超过 900 行硬门禁；已移动
  address helper 与 fake support，当前分别为 866 / 808 行，hygiene 恢复 0 error。
- 真实 Excel/COM/GUI、真实 range write、保存、磁盘影子副本与恢复均未执行。

### 6. 新增长期记忆

无。本卡只新增 fake write-port 合同，没有新增真实 Excel/COM/文件恢复事实。

### 7. 遗留与下一步

- 下一卡需接真实 COM range write/recalculate port、`.Value2/.Formula` 双读与 save
  preflight；全部真机证据通过前 production Tool surface 保持为空。
- 真实磁盘影子副本、恢复、ownership journal 与 watchdog 仍由后续 T2.2 卡完成。
- 当前 fake 的 Formula 值闭集是合同模型；真实 Excel 对 boolean/number/string/blank 的
  `.Formula` 返回形状仍需人类在场真机校准。

### 8. 外部证据

- automation JSONL：
  `C:\Users\fexfe\.codex\automations-logs\2026-11-25\ai-assistant-task-round-20261126-0300.jsonl`
- round evidence：`docs/automations/2026-11-25-round-3.md`。
- implementation PR / CI / merge hash / post-merge main CI：待提交后回填。
- 独立 review：首轮 1 P1 + 1 P2 + 1 P3；复审 no P0-P2。

### 9. 给审阅者的关注点

- 最高风险是 fake port 与真实 Excel 属性的语义差异；本卡只冻结事务与 double-read 合同，
  不证明真实 COM 行为。
- post-fingerprint 被要求在语义双读后获取一次；后续真实 runner 必须保证该顺序不会因
  UI 抖动或异步计算而误判。
- 任何 port 失败都会消费 anchor 的 single-use 写前准备；后续实现需要明确重新 capture
  anchor 的恢复策略，不应放宽 single-use。
