# TASK-404 Excel T2.2 L1 anchor preparation

- 状态：**Review**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：5　依赖：TASK-403、ADR-0062、ADR-0107、ADR-0108

---

## write scope

- `tasks/TASK-404-excel-t22-l1-anchor-preparation.md`
- `apps/agent-core/src/excel_t21.rs`
- `apps/agent-core/src/excel_t21_l1.rs`
- `apps/agent-core/src/excel_t21_l1_tests.rs`
- `apps/agent-core/README.md`
- `docs/automations/2026-11-25-round-1.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 agent-core 内建立 test-only、私有的 Excel L1 anchor 准备层，把目标 Range 的
`value2` 快照、目标文件影子副本模型与写前/写后 workbook fingerprint 绑定为一个
有界、可审计且冲突时 fail-closed 的撤销锚点；生产 Tool、COM 与写路径保持未接线。

## In scope

- 新增 test-only 私有 `ExcelL1AnchorStore`，使用既有 `assistant-undo::Anchor` 表达
  `L1Snapshot + ShadowCopy`，不复制或改写 undo 公共类型。
- 捕获最多 4096 cells / 256 KiB 的 `value2` 值快照，并保留 range identity、cell
  coordinates、value type 与 canonical text。
- 以调用方提供的原始文件字节建立内存影子副本模型；按 `BlobId::of_content` 计算 digest，
  校验长度、总容量与重复/未知 handle。
- 在写前校验当前 workbook fingerprint 必须等于 anchor pre-fingerprint；写后记录一次
  post-fingerprint；回滚前复用 `detect_conflict`，用户改动或证据缺失必须失败。
- 提供显式 release/shutdown；anchor 数量、影子字节总量与值快照 payload 均有硬上限，
  超限拒绝而不是淘汰仍可回滚的 anchor。
- 增加成功、冲突、缺失、越序、重复 release、容量上限、payload 上限、非法路径/形状和
  重复 capture/release 收敛测试。
- 独立 review、PR、CI、merge 与 post-merge main CI 收口；Done 状态另行 docs-only backfill。
- 同步 agent-core README、Stage 2 当前进度、PLAN/README/LEDGER 与运行日 round 证据。

## Out of scope

- 不执行真实 Excel、COM、GUI 或 `.xlsx` 工作簿；不操作真实文件。
- 不实现 `com.range_write`、`com.workbook_save`、Tool handler 或 ToolBus 注册。
- 不填写 `adapters/com.microsoft.excel/tools/tools.json`、rollback recipe 或 assembly。
- 不实现磁盘影子副本目录、持久化 blob、ownership journal、watchdog 或进程回收。
- 不修改 `assistant-undo`、protocol/schema、公共 `platform/api` trait、ErrorCode、
  Policy、依赖或测试成功判据。
- 不以内存影子副本或 fake 测试冒充真实文件恢复、真实工作簿写入或真机验收。

## 必须遵守

1. anchor 只能绑定 owned workbook identity、目标 range 与同一时刻的 workbook fingerprint。
2. 值快照固定 `value2`，不得混入 formula/text；payload 超限整体失败，不截断。
3. 影子副本 digest 必须由原始字节计算并与记录一致；缺失或漂移显式失败。
4. 写前 fingerprint 漂移、post 缺失、重复记录、回滚前用户改动均 fail-closed。
5. active anchor 与影子字节只允许显式 release/shutdown 或测试结束回收；不得静默淘汰。
6. 生产 `tools.json`、ToolBus、rollback、assembly 与真实文件系统保持零改动。
7. 真实 Excel/COM 未运行时不得在 DoD 或记录中写成功。

## 验收命令

```powershell
cargo test -p assistant-agent-core excel_t21_l1
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
git diff --check
```

预期：L1 专项与全量门禁全绿；内存影子副本与冲突检测证据完整；真实 COM/文件写入
未执行且 production Excel surface 仍为空。

## DoD

- [ ] `value2` 值快照、文件字节 digest 与 `L1Snapshot + ShadowCopy` anchor 绑定一致。
- [ ] 写前/写后/回滚前 fingerprint 路径覆盖成功、冲突、缺失和重复记录。
- [ ] anchor 数量、影子字节总量与值快照 payload 有硬上限，超限拒绝且可显式释放。
- [ ] 非法路径/range/cell/payload/digest 均返回既有 `ErrorCode`，无静默降级。
- [ ] 生产 tools/ToolBus/rollback/assembly、protocol/schema/依赖零改动。
- [ ] 专项、全量门禁与独立 review 完成；PR/CI/merge/post-merge main CI 证据落到记录区。
- [ ] 外部 automation JSONL 记录 start/gates/work/pr/ci/merge/main_ci/final 与 shell misuse。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-404 Excel T2.2 L1 anchor preparation
【目标】实现有界的值快照、文件影子副本模型与写前/写后冲突指纹校验
【write scope】仅：TASK-404 卡、agent-core `excel_t21` 模块入口/L1 实现与测试/README、2026-11-25 round 1、Stage 2、PLAN、README、LEDGER
【铁律】无静默失败；不可信输入先校验；COM/blob 不跨进程；资源必须有界；证据不得伪造；公共契约先行
【禁止】不运行真实 Excel/GUI；不改 tools.json/ToolBus/rollback/assembly；不改 schema、公共 API、ErrorCode、Policy 或依赖
【验收】专项测试、fmt/clippy、Linux target/workspace/arch/schema/codegen/文档门禁/deny/replay/diff check → 全绿；真实 COM 保持未执行
【依赖】TASK-403 Done；ADR-0062/0107/0108 Accepted；Stage 2 批次 5 待派卡；LEDGER 已核对
【疑问】无；按 test-only 私有存储/逻辑推进，真实文件影子副本和写入留给人类在场后续卡
```

### 2. 实际改动

- 新增 test-only `ExcelL1AnchorStore`；binding 只能从 valid `OpenedWorkbook` 派生
  ToolBus session / task / opaque workbook handle / owned process / workbook /
  worksheet identity，再把 `value2` 值快照、原始 workbook 字节 digest 与既有
  `assistant-undo` `L1Snapshot + ShadowCopy` anchor 绑定。
- 值快照固定 `value2`，重校验 range address/A1 bounds、4096 cells、256 KiB payload、
  worksheet identity、number/boolean/error 形状；runner 的相对 cell coordinates 转为
  绝对 workbook 坐标；无 sheet 前缀的 Excel `Range.Address` 可接受。不截断、不返回部分成功。
- 影子副本保存在内存中，digest 由 `assistant-storage::BlobId::of_content` 计算；anchor
  数上限 64、影子字节总量上限 32 MiB，超限拒绝，release/shutdown 显式回收。
- 写前准备要求 session/task/handle/process/workbook/worksheet identity 不变且当前
  fingerprint 等于 anchor pre-fingerprint；post 与回滚路径同样复核 identity；
  post-fingerprint 仅可记录一次；回滚前复用 `assistant-undo::detect_conflict`，
  用户改动或证据缺失返回 `VerifyFailed`。
- `excel_t21.rs` 只增加 test-only 模块入口；生产 assembly、ToolBus、`tools.json`、
  rollback、protocol/schema、平台 API 与依赖零接线或零变更。

### 3. 门禁输出摘要

- `cargo test -p assistant-agent-core 'excel_t21::l1'` → **26 passed / 0 failed**。
- `cargo test --workspace` → PASSED；assistant-agent-core lib **164 passed / 1 ignored**，
  xtask **587 passed**，其余 crate/doctest 全绿。
- `cargo fmt --all --check`、workspace Clippy、Linux-target platform Clippy、
  `cargo test -p assistant-core arch::` → PASSED；Clippy 仅有仓库既有 unknown-lint warning。
- schema/codegen/hygiene/card-check/docscan/refscan/memory-counts/adr-index/
  check-ledger/check-comments → 0 error。
- `cargo deny check` → advisories / bans / licenses / sources ok，保留既存重复版本与
  unmatched-license warnings。
- core replay → 2 steps / 0 mismatch；workspace coverage → **77.90%** line coverage。
- 独立 review 首轮 3 P1 + 2 P2 + 1 P3；修复后复审补出 2 P1 + 2 P2；再次修复后最终
  review **no P0/P1/P2 findings**。

### 4. DoD 核对

- [x] `value2` 值快照、文件字节 digest 与 `L1Snapshot + ShadowCopy` anchor 绑定一致。
- [x] 写前/写后/回滚前 fingerprint 路径覆盖成功、冲突、缺失和重复记录。
- [x] anchor 数量、影子字节总量与值快照 payload 有硬上限，超限拒绝且可显式释放。
- [x] 非法路径/range/cell/payload/digest 均返回既有 `ErrorCode`，无静默降级。
- [x] 生产 tools/ToolBus/rollback/assembly、protocol/schema/依赖零改动。
- [ ] 专项、全量门禁与独立 review 完成；PR/CI/merge/post-merge main CI 证据待落地。
- [ ] 外部 automation JSONL 的 PR/CI/merge/main_ci/final 事件待 PR 后写入。

### 5. 偏差

- 本卡只验证有界内存影子副本与 anchor/conflict 契约，没有真实文件复制或恢复写入；
  该限制不得冒充 T2.2 真机验收。
- `excel_t21_l1.rs` 因 open identity、worksheet/cell/address 二次校验与持久状态机集中在
  一个私有模块而达到 **837 行**，测试文件 **652 行**，均超过 600 行建议上限但低于
  900 行硬门禁；hygiene 以 warning 记录，后续真实 COM/file port 卡应拆出独立子模块。
- `exec_command` shell misuse 精确值只以外部 JSONL `final_audit` 为唯一事实源；主会话与
  reviewer 的误填均被项目 hook 重写为 PowerShell，没有 Bash 实际执行或外部副作用。

### 6. 新增长期记忆

无。本轮没有新增真实 Excel/COM/文件恢复事实。

### 7. 遗留与下一步

- 后续 T2.2 卡需接真实文件影子副本存储、真实写入、`.Value2/.Formula` 双读校验、
  保存前冲突检测和实际恢复；全部通过前 production Tool/rollback/assembly 保持为空。
- `ExcelL1AnchorStore` 的 32 MiB 总量是 test-only 安全模型；生产存储容量/保留策略需按
  ADR-0063 重新声明并验证。
- `Range.Address` 的无 sheet 前缀形状已覆盖；带 Excel quote 的 sheet 前缀仍需真实 COM
  或后续 fake snapshot 校准。

### 8. 外部证据

- automation JSONL：
  `C:\Users\fexfe\.codex\automations-logs\2026-11-25\ai-assistant-task-round-20261125-2200.jsonl`
  （PR / CI / merge / main CI / shell misuse / final audit 在后续事件中追加）。

### 9. 给审阅者的关注点

- 最高风险是 `OpenedWorkbook` 本身仍来自 test-only runtime；后续真实 shadow-copy
  executor 必须再次校验 canonical path、opened handle 与 owned process identity 存活。
- 第二风险是内存影子副本只证明契约，不代表磁盘恢复安全性；需确认后续卡不会把本卡
  fake 证据用于生产开放。
- 第三风险是 32 MiB 总量限制对大工作簿会拒绝而非降级；需确认这与 ADR-0063 的 fail-closed
  方向一致。
