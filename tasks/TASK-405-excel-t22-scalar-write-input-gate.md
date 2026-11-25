# TASK-405 Excel T2.2 scalar write input gate

- 状态：**Review**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6　依赖：TASK-403、TASK-404、ADR-0062、ADR-0107、ADR-0108

---

## write scope

- `tasks/TASK-405-excel-t22-scalar-write-input-gate.md`
- `apps/agent-core/src/excel_t21_l1.rs`
- `apps/agent-core/src/excel_t21_l1_write.rs`
- `apps/agent-core/src/excel_t21_l1_write_tests.rs`
- `apps/agent-core/README.md`
- `docs/automations/2026-11-25-round-2.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 agent-core 内建立 test-only、私有的 Excel T2.2 scalar literal 输入门，只接收有界
JSON scalar，拒绝公式形态字符串，并只允许在既有 L1 anchor 的 pre-write fingerprint
上一次性准备写入；生产 Tool、COM、文件写入与 ToolBus 继续保持未接线。

## In scope

- 新增 test-only 私有 `excel_t21_l1_write` 子模块，消费 TASK-404 已建立的
  `ExcelL1AnchorStore` 与 `ExcelL1WorkbookIdentity`。
- scalar 闭集固定为 JSON `null`、boolean、number 与 string；array/object 整体失败。
- number 从原始 JSON 词法复用既有 `CanonicalDecimal` 规范化，不先经过 `serde_json::Value`
  的 f64 路径，也不接受未规范化文本。
- string 在跳过空白、控制字符、BOM 与零宽格式字符后，首个有效字符为
  `=`、`+`、`-`、`@` 即失败；其余文本原样保留。
- scalar 的紧凑 JSON 表示必须不超过既有 256 KiB range payload 上限；超限拒绝，不截断。
- 准备写入必须先校验 scalar，再调用既有 `ExcelL1AnchorStore::prepare_write`；非法输入
  不得消耗 anchor 的一次写入准备状态。
- 增加 scalar 正负例、公式前缀、类型闭集、payload 上限、identity/fingerprint 漂移、
  unknown/released handle 与 single-use 测试。
- 独立 review、PR、CI、merge 与 post-merge main CI 收口；Done 状态另行 docs-only backfill。
- 同步 agent-core README、Stage 2、PLAN/README/LEDGER 与运行日 round 证据。

## Out of scope

- 不运行真实 Excel、COM、GUI 或任何 `.xlsx` 工作簿；不操作真实文件或用户进程。
- 不实现真实 range write、重计算、`.Value2/.Formula` 双读、保存、恢复或磁盘影子副本。
- 不填写 `adapters/com.microsoft.excel/tools/tools.json`、rollback recipe 或 assembly。
- 不注册 production ToolBus，不建立模型可见 `excel.range.write` Tool。
- 不修改 protocol/schema、公共 `platform/api` trait、ErrorCode、Policy、依赖或测试成功判据。
- 不以 scalar parser 或 fake anchor 测试冒充 T2.2 真机写入验收。

## 必须遵守

1. scalar 输入必须来自单一 JSON 值；未知类型与超界整体失败，不得降级为字符串。
2. number 必须经 `CanonicalDecimal`，不得引入浮点近似。
3. 公式形态字符串在写入前失败，不能依赖 Excel 写入后补救。
4. scalar 校验失败时 anchor 状态不得改变，尤其不得把 `write_prepared` 置为 true。
5. 只有同一 owned workbook identity 与 captured pre-fingerprint 才能准备写入。
6. 生产 `tools.json`、ToolBus、rollback、assembly 与真实文件系统保持零改动。
7. 真实 Excel/COM 未运行时不得在 DoD 或记录中写成功。

## 验收命令

```powershell
cargo test -p assistant-agent-core 'excel_t21::l1::write'
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

预期：scalar 与 L1 专项测试全绿；workspace、文档门禁和覆盖率通过；真实 COM/文件写入
未执行且 production Excel surface 仍为空。

## DoD

- [ ] JSON scalar 闭集、原始 number 词法精度与 256 KiB payload 边界有正负测试。
- [ ] 公式形态字符串在指定前缀、前导空白、控制字符与零宽字符下均失败。
- [ ] scalar 校验失败不消耗 anchor 状态；成功准备只接受 captured pre-fingerprint 且 single-use。
- [ ] released/unknown handle、identity/fingerprint 漂移均返回既有 `ErrorCode`。
- [ ] 生产 tools/ToolBus/rollback/assembly、protocol/schema/依赖零改动。
- [ ] 专项、全量门禁与独立 review 完成；PR/CI/merge/post-merge main CI 证据落到记录区。
- [ ] 外部 automation JSONL 记录 start/gates/work/pr/ci/merge/main_ci/final 与 shell misuse。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-405 Excel T2.2 scalar write input gate
【目标】实现 test-only 私有 scalar literal 输入门与一次性 L1 写前准备
【write scope】仅：本卡、agent-core L1 入口/新增 write 模块与测试/README、round 2、Stage 2、PLAN、README、LEDGER
【铁律】不可信输入先校验；无静默失败；写前 postcondition；资源有界；证据不得伪造；公共契约先行
【禁止】不运行真实 Excel/GUI；不读写真实 .xlsx；不改 tools.json/ToolBus/rollback/assembly/schema/公共 API/依赖
【验收】scalar/L1 专项、fmt/clippy、workspace/arch/schema/codegen/文档门禁/deny/replay/coverage/diff check → 全绿
【依赖】TASK-403 / TASK-404 Done；ADR-0062/0107/0108 Accepted；LEDGER 已核对
【疑问】无；公开 T2.2 Tool schema 未冻结，本卡只做私有、test-only 输入门
```

### 2. 实际改动

- 新增 test-only 私有 `excel_t21_l1_write` 子模块，直接从原始 JSON scalar 文本解析，
  不先经过 `serde_json::Value` 的 f64 路径；array/object 在解析与序列化前拒绝。
- number 严格按 JSON number 语法校验后复用既有 `CanonicalDecimal`，保留高位整数和长小数
  词法精度；非法 leading zero / exponent / trailing token 返回 `ToolInvalidArgs`。
- string 从 JSON 解码后，在跳过空白、控制字符及 Unicode Default_Ignorable 集合后检查
  `=`、`+`、`-`、`@`；隐藏前缀后接普通文本仍允许。
- scalar 的规范 JSON 表示超过既有 256 KiB payload 上限时整体失败；恰好上限可接受，
  超一字节拒绝。
- 合法 scalar 才调用既有 `ExcelL1AnchorStore::prepare_write`；scalar 校验失败不消耗
  anchor 状态。fingerprint / workbook identity 漂移与 released/unknown handle 均显式失败。
- `excel_t21_l1.rs` 只增加 test-only 子模块入口；生产 assembly、ToolBus、`tools.json`、
  rollback、protocol/schema、`platform/api` 与依赖零接线或零变更。

### 3. 门禁输出摘要

- `cargo test -p assistant-agent-core 'excel_t21::l1::write'` → **7 passed / 0 failed**。
- `cargo test -p assistant-agent-core excel_t21` → **70 passed / 0 failed / 1 ignored**。
- `cargo fmt --all --check`、workspace Clippy、workspace tests → PASSED；仅仓库既有
  unknown-lint warning。
- `cargo test -p assistant-core arch::` → 10 passed。
- schema/codegen/hygiene/card-check/docscan/refscan/memory-counts/adr-index/check-ledger/
  check-comments 最终回填后均 0 error；hygiene 233 warning 均为既有或软阈值告警。
- `cargo deny check` → advisories / bans / licenses / sources ok，保留既有 warning。
- core replay → 2 steps / 0 mismatch；coverage → **77.95%**。

### 4. DoD 核对

- [x] JSON scalar 闭集、原始 number 词法精度与 256 KiB payload 边界有正负测试。
- [x] 公式形态字符串在指定前缀、前导空白、控制字符与零宽字符下均失败。
- [x] scalar 校验失败不消耗 anchor 状态；成功准备只接受 captured pre-fingerprint 且 single-use。
- [x] released/unknown handle、identity/fingerprint 漂移均返回既有 `ErrorCode`。
- [x] 生产 tools/ToolBus/rollback/assembly、protocol/schema/依赖零改动。
- [ ] PR/CI/merge/post-merge main CI 证据待实现 PR 完成后回填。
- [ ] 外部 automation JSONL 的最终 PR/CI/merge/main_ci/final 待合并后追加。

### 5. 偏差

- 独立 review 首轮发现 2 P1 + 3 P2：number 精度丢失、控制/零宽公式前缀绕过、
  array/object 先序列化、专项 filter 空跑、边界测试不足。全部修复。
- 第二轮发现 Unicode Cf / Default_Ignorable 枚举不全；第三轮发现 U+E0000..E0FFF
  尾部范围未覆盖；已按 Default_Ignorable 区间 fail-closed 并补回归。
- 最终 review：**no P0/P1/P2 findings**。
- `excel_t21_l1_write.rs` 新增 3 个 soft cyclomatic warning，仍低于 hard gate；
  所有文件长度与函数长度均未新增 hard error。
- 本卡只完成 scalar 输入门与写前准备，不执行真实 range write、重算、
  `.Value2/.Formula` 双读、保存、恢复或真实 COM/GUI。

### 6. 新增长期记忆

无。本卡没有新增真实 Excel/COM/文件恢复事实。

### 7. 遗留与下一步

- 下一卡需接真实写入 port、`.Value2/.Formula` 双读、重算与 save preflight；
  全部真机证据通过前 production Tool surface 保持为空。
- Unicode Default_Ignorable 采用保守区间覆盖；真实 Excel 是否忽略全部这些前缀仍需
  人类在场真机校准，但输入门当前按 fail-closed 处理。
- 真实文件影子副本、恢复、ownership journal 与 watchdog 仍由后续 T2.2 卡完成。

### 8. 外部证据

- automation JSONL：
  `C:\Users\fexfe\.codex\automations-logs\2026-11-25\ai-assistant-task-round-20261126-0030.jsonl`
- round evidence：`docs/automations/2026-11-25-round-2.md`。
- 上一轮修正 PR #864 已 merge `a527759eaa46b9c1f2f525326c43694c72c0ce85`；
  post-merge main CI `45656044930` success。
- TASK-405 implementation PR / CI / merge / main CI 待回填。

### 9. 给审阅者的关注点

- 最高风险是原始 JSON number 词法是否在所有未来调用入口保持不丢失；不得把本入口退回到
  `serde_json::Value` f64 路径。
- Default_Ignorable 采用保守 Unicode 区间；需确认过宽拒绝符合 T2.2 fail-closed 方向。
- 256 KiB 边界按规范 JSON 表示计算；未来 Tool schema 若改变 scalar/bounds 需先更新契约。
