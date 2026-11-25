# TASK-403 Excel T2.1 platform runtime private mapping

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 Excel T2.1　批次：4i　依赖：TASK-401、TASK-402、ADR-0108、ADR-0111、ADR-0112

---

## write scope

- `tasks/TASK-403-excel-t21-platform-runtime-mapping.md`
- `apps/agent-core/src/excel_t21.rs`
- `apps/agent-core/src/excel_t21_platform_runtime.rs`
- `apps/agent-core/src/excel_t21_platform_runtime_tests.rs`
- `apps/agent-core/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 agent-core 内实现 `ExcelT21Runtime` 到 `assistant-platform-windows::excel_com_runner`
的私有映射，验证重复 open/read/close 时 domain handle、platform handle 与 worker
receiver 有界收敛，同时保持生产 `tools.json`、ToolBus、rollback 与 assembly 为空。

## In scope

- 新增 test-only 私有 `PlatformExcelT21Runtime`，把平台自包含类型映射为现有领域模型。
- 惰性启动并只创建一个 `ExcelComRunner`；同一 runtime 最多保留一个 open workbook。
- 建立 domain `WorkbookHandle` 到平台 `ExcelWorkbookHandle` 的有界映射；close 成功后释放。
- 映射 open metadata、workbook snapshot、range selector、read mode、cell value type、
  named-range scope、calculation 与 typed platform error。
- 对 platform 返回的 path/read-only/mode/range/cell shape 做二次校验，错误显式失败。
- 增加 fake platform port 的往返、错误保真、越序、重复 close、shutdown 与
  open/read/close 循环收敛测试。
- 增加 Windows-only ignored 真实验收入口；本轮不执行、不操作用户工作簿。
- 独立 review、PR、CI、merge 与 post-merge main CI 收口；Done 状态另行 docs-only backfill。
- 同步 agent-core README、Stage 2 批次与当前进度、PLAN/README/LEDGER。

## Out of scope

- 不把 `ExcelT21Runtime`、handler 或 mapping 注册到生产 ToolBus/assembly。
- 不填写 `adapters/com.microsoft.excel/tools/tools.json`、rollback recipe 或模型可见 Tool。
- 不执行真实 GUI、真实 COM 或任何 `.xlsx` 工作簿；ignored 入口仅供后续人类在场运行。
- 不实现 ownership journal、独立 watchdog、process-tree kill 的跨进程协议或 UI。
- 不修改 protocol/schema、公共 `platform/api` trait、`assistant-platform-windows` 公共面、
  ErrorCode、Policy、依赖或测试成功判据。
- 不以 fake 往返或 ignored 用例编译成功冒充真实工作簿验收。

## 必须遵守

1. COM 对象不得进入 agent-core；domain adapter 只持有可跨线程的平台 handle/命令通道。
2. domain handle 只允许映射到同一 runtime 注册的 platform handle；未知、重复或越序调用显式失败。
3. process identity 必须使用 PID、start ticks、executable path 三项完整映射。
4. workbook path/read-only、read mode、range address/dimensions/cell set 在映射前后均校验。
5. close 成功后移除 mapping；失败时不得伪造关闭或悄悄丢弃仍在使用的 handle。
6. shutdown 必须显式收口 runner worker；Drop 只能作为不可报告错误的最后兜底。
7. 生产 `tools.json`、ToolBus、rollback、assembly 保持空/零注册。
8. 真实 COM 未运行的 DoD 项不得写成功；可完成范围只声称 fake/contract 验证。

## 验收命令

```powershell
cargo test -p assistant-agent-core excel_t21_platform_runtime
cargo test -p assistant-agent-core excel_t21
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo clippy --target x86_64-unknown-linux-gnu -p assistant-agent-core --all-targets -- -D warnings
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
git diff --check
```

预期：fake/契约测试全绿；生产 Excel Tool 仍为空；真实 COM ignored 入口编译但本轮不执行。

## DoD

- [ ] domain adapter 完全映射 platform open/read/close/shutdown，不携带 COM 类型。
- [ ] workbook handle 有界且 close 后释放；重复 open/close 循环无 mapping/receiver 增长。
- [ ] open/read/read range/fingerprint/ownership 的成功与 typed failure 路径有测试。
- [ ] platform path/read-only/mode/range/cell shape 二次校验有正负测试。
- [ ] 生产 tools/ToolBus/rollback/assembly、protocol/schema/依赖零改动。
- [ ] 专项、全量门禁与独立 review 完成；PR/CI/merge/post-merge main CI 证据落到记录区。
- [ ] 外部 automation JSONL 记录 start/gates/work/pr/ci/merge/main_ci/final 与 shell misuse。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-403 Excel T2.1 platform runtime private mapping
【目标】把 ExcelT21Runtime 私有映射到 platform runner，并验证 handle/receiver 收敛
【write scope】仅：本卡、agent-core Excel mapping/test 模块与 README、Stage 2、PLAN、README、LEDGER
【铁律】COM 不跨线程；公共接口先契约；无静默失败；不可信结果再校验；不伪造真机证据
【禁止】不填 tools.json、不注册 ToolBus/rollback/assembly、不改 schema/公共平台 API/依赖；本轮不运行真实 COM
【验收】专项、fmt/clippy/Linux platform target/workspace/arch/schema/codegen/文档门禁/deny/replay/coverage
【依赖】TASK-401 / TASK-402 Done；ADR-0108/0111/0112 Accepted；已核对 LEDGER
【疑问】无；真实重复收敛留给人类在场后续卡
```

### 2. 实际改动

- 新增 test-only 私有 `PlatformExcelT21Runtime`，只把
  `assistant-platform-windows::excel_com_runner` 的自包含 handle/snapshot 映射为现有
  Excel T2.1 领域类型；agent-core 不持有 COM 指针。
- 建立单 runner、单 application、单 open workbook 的有界状态机；domain handle 只在
  platform close 成功后释放，未知/重复/越序 handle 显式失败。
- open 后 metadata/path/read-only/process identity 校验任一失败时执行 workbook close；
  若 close 也失败，立即 shutdown runner、清空 mapping、标记 terminal，避免留下调用方
  拿不到 handle 的悬空 workbook。
- range selector/mode、named-range scope、cell type、workbook snapshot 与 platform error
  均无损映射；A1 address、cell count/坐标、read-only、platform mode echo 均二次校验。
- fake port 新增 32 次 open/read/close 收敛、typed error、重复 close、close 失败保留、
  cleanup shutdown、named-range/formula、worksheet-name/text、A1 drift 与 malformed shape
  测试；Windows-only ignored 真实 COM 三轮入口仅编译，本轮未执行。
- 更新 agent-core README 与 Stage 2 当前进度/批次行；生产 `tools.json`、ToolBus、
  rollback、assembly、protocol/schema、`platform/api` 与依赖零改动。

### 3. 门禁输出摘要

- `cargo test -p assistant-agent-core excel_t21_platform_runtime` → **10 passed / 1 ignored**。
- `cargo test -p assistant-agent-core excel_t21` → **37 passed / 1 ignored**。
- `cargo test -p assistant-platform-windows excel_com_runner` → **15 passed / 1 ignored**。
- `cargo fmt --all --check`、workspace clippy、agent-core clippy、Linux target
  `assistant-platform-windows` clippy → PASSED；只有仓库既有未知 lint warning。
- `cargo test --workspace`、assistant-core arch 10/10、schema 7/7、codegen 0 drift、
  hygiene / card-check / docscan / refscan / memory-counts / adr-index / check-ledger /
  check-comments → PASSED；软 warning 沿用仓库现状。
- `cargo deny check`、core replay 2 steps / 0 mismatch → PASSED。
- `cargo llvm-cov --workspace --fail-under-lines 75` → workspace line coverage **77.83%**。
- `assistant-agent-core` 的 Linux cross-target clippy 在依赖构建阶段被 `zstd-sys` /
  `libsqlite3-sys` 缺少 `x86_64-linux-gnu-gcc` 阻断，未进入本次 Rust 代码；
  Windows-only 新模块在 Linux 上按 `cfg` 不编译，PR 的 Ubuntu 原生 CI 负责非宿主覆盖。

### 4. DoD 核对

- [x] domain adapter 完全映射 platform open/read/close/shutdown，不携带 COM 类型。
- [x] workbook handle 有界且 close 后释放；32 次循环证明 mapping 与单 application 收敛。
- [x] open/read/read range/fingerprint/ownership 的成功与 typed failure 路径有测试。
- [x] platform path/read-only/mode/range/cell shape 二次校验有正负测试。
- [x] 生产 tools/ToolBus/rollback/assembly、protocol/schema/依赖零改动。
- [x] 专项、全量门禁与独立 review 完成；PR/CI/merge/post-merge main CI 已闭环。
- [x] 外部 automation JSONL 的 PR/CI/merge/main_ci/final 事件随运行日证据完成。

### 5. 偏差

- 临时 Linux cross-target clippy 命令命中既有 C 依赖交叉工具链缺口；未把该命令写成
  通过，改跑 platform-windows Linux target 并由 CI 原生 Ubuntu 覆盖。
- 首轮独立 review 发现 1 P1 + 2 P2：open cleanup 失败未进入可关闭终态、
  read-only/range shape 错误码不符合 Accepted spec、fake 未验证 selector/mode 透传；
  均已在同一实现 PR 前修复，复审判定 no findings（P0-P2）。
- 本轮按用户约束未执行真实 COM/`.xlsx`；ignored 入口不能冒充真实收敛证据。
- exec_command shell misuse：主会话 15 次、独立 reviewer 3 次，共 18 次误填 `bash`；
  均由项目 hook 重写为 PowerShell，没有 Bash 实际执行；全部为只读/状态命令。

### 6. 新增长期记忆

无。本轮没有新增真实 Excel 版本/UIA/COM 形状事实；映射层语义与 fake 证据已留在本卡。

### 7. 遗留与下一步

- 人类在场运行 Windows-only ignored 三轮真实 COM 入口，并补 digest / owned PID /
  process-tree convergence 证据。
- ownership journal、独立 watchdog、真实 process-tree kill 与并发启动竞态仍归后续卡；
  全部通过前 production tools/rollback/assembly/ToolBus 保持为空。
- `ExcelRangeSnapshot::mode()` 是请求模式回显，一致性命中只能证明映射不漂移，不能独立
  证明 `.Value2/.Formula/.Text` 的 Excel 实际语义；真实语义仍由后续真机验收确认。

### 8. 外部证据

- automation JSONL：
  `C:\Users\fexfe\.codex\automations-logs\2026-11-25\ai-assistant-task-round-20261125-1700.jsonl`
  （最终 PR / CI / merge / main CI / shell misuse / final audit 合并后追加）。
- 独立 reviewer 首轮 findings：open cleanup 终态、error-code 契约、selector/mode fake 真实性。
- 独立 reviewer 复审：**No findings (P0-P2)**；剩余仅为未运行真实 COM 与 mode echo 语义限制。
- implementation PR #858：checks **22（21 SUCCESS + 1 SKIPPED）**、pending 0 / failed 0，
  base=main、`MERGEABLE` / `CLEAN`；merge
  `9bb006ab71dcf1db1422bbba83b58a65705b8b47`；post-merge main CI `45597702500` success。

### 9. 给审阅者的关注点

- 最高风险是 open cleanup 的双失败路径；现在采用 runner shutdown + terminal，需确认这与
  后续 ownership journal/watchdog 卡的责任切分一致。
- 第二风险是 fake port 与真实 `ExcelComRunner` 的差异；真实 mode echo 不能证明底层属性语义，
  ignored 三轮入口仍需人类执行。
- 第三风险是 close/shutdown 长时间阻塞时持有 runtime mutex；当前保持线性化状态，
  但独立 watchdog 尚未实现。
