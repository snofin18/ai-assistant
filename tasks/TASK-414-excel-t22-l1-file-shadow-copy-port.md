# TASK-414 Excel T2.2 L1 file shadow-copy port

- 状态：**Review**
- 阶段：2　子阶段：Stage 2 Excel T2.2　批次：6h　依赖：TASK-404、TASK-406、TASK-407、ADR-0062、ADR-0107

---

## write scope

- `tasks/TASK-414-excel-t22-l1-file-shadow-copy-port.md`
- `apps/agent-core/src/excel_t21_l1.rs`
- `apps/agent-core/src/excel_t21_l1_file_shadow.rs`
- `apps/agent-core/src/excel_t21_l1_file_shadow_tests.rs`
- `apps/agent-core/src/excel_t21_l1_tests.rs`
- `apps/agent-core/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

在 test-only Excel T2.2 L1 边界内实现受控根目录的文件影子副本端口，把 TASK-404 的
内存影子模型推进到真实本地文件复制、摘要校验、冲突门与逐字节恢复，并保持 COM、Excel、
production ToolBus 与公共 API 零接线。

## In scope

- 新增 test-only 文件影子端口：只接受仓库外、绝对、本地文件系统中的证言根目录与本地
  `.xlsx` 源文件，拒绝相对路径、UNC、verbatim 前缀、目录与越界影子路径。
- 捕获时按调用方给定的 `ShadowCopyPath` / `ContentDigest` 复制原始字节到影子根，
  使用 create-new 语义拒绝覆盖，限制单文件与活动总量并在成功后 `sync_all`。
- 恢复前复核目标文件当前 digest 等于调用方给出的 post-write digest；不匹配时不得写目标。
- 恢复只写 anchor 记录的本地 workbook 路径，完成后重新计算目标文件 digest，必须等于
  anchor 记录的原始 digest；失败必须保留影子副本与目标现场，返回既有 `ErrorCode`。
- 为 `ExcelL1AnchorStore` 增加 test-only 的 capture/restore 组合入口：文件影子捕获失败时
  回滚内存 anchor 容量，恢复前复用既有 fingerprint 冲突判定。
- fake/filesystem tests 覆盖成功捕获与恢复、目标冲突、目标已存在、digest 漂移、大小上限、
  路径越界、临时文件清理、重复恢复与显式 cleanup。
- 同步 agent-core README、Stage 2、PLAN、README、LEDGER，并完成独立 review、PR、CI、
  merge 与 post-merge main CI 证据回填。

## Out of scope

- 不启动或调用真实 Excel、COM、工作簿或 GUI；不读取、保存或覆盖用户 `.xlsx`。
- 不注册 `excel.*` production ToolBus Tool，不填写 `tools.json`、rollback recipe 或 assembly。
- 不实现 Host 崩溃后的 helper process、跨进程恢复、Job Object 或短时子进程。
- 不修改 `assistant-platform-api`、Windows runner、protocol schema、ErrorCode、Policy 或依赖。
- 不把 test-only 文件恢复证明为 production T2.2 写入事务或真实保存回滚完成。

## 必须遵守

1. 影子根、源 workbook 与恢复目标必须是本机绝对路径；任何 UNC、verbatim、相对路径或
   根外解析结果都 fail-closed。
2. 捕获与恢复前后都必须按 SHA-256 校验字节；不得用大小、mtime 或调用方布尔值替代 digest。
3. 影子文件必须 create-new，已有目标不得覆盖；失败的临时文件只能按真实路径显式清理。
4. 恢复前目标 digest 不匹配、内存 anchor 冲突或证据缺失时不得写文件。
5. 所有失败返回既有 `ErrorCode`；`UserInteraction` 不被改写成 `Fatal`，结构/IO 失败不得
   伪装成成功。
6. 测试只使用本卡创建的临时目录与合成 `.xlsx` 字节，不触碰用户目录或真实工作簿。
7. 不修改测试断言规避实现缺陷，不新增 `#[allow]`、`unsafe`、第三方依赖或公共接口。

## 验收命令

```powershell
cargo test -p assistant-agent-core file_shadow
cargo test -p assistant-agent-core excel_t21
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

预期：专项文件影子与 Excel 私有测试全绿；全量门禁通过；无真实 Excel/COM/GUI；
production `tools.json`、ToolBus、rollback、assembly、schema、公共 API 与依赖零改动。

## DoD

- [ ] 文件影子端口只接受受控绝对本地根与 workbook 路径，越界和网络路径显式失败。
- [ ] 捕获 create-new、写入、sync、digest 与单文件/总量上限均被测试覆盖。
- [ ] 恢复前检查 post-write digest；冲突时目标字节与影子副本均不变。
- [ ] 恢复后目标 digest 精确回到 anchor digest，失败保留影子副本与显式错误。
- [ ] `ExcelL1AnchorStore` 组合入口在影子捕获失败时回收 anchor，在恢复前复用 fingerprint 冲突门。
- [ ] 测试使用独立临时目录且成功/失败路径均无本卡临时残留。
- [ ] production Tool/ToolBus、`tools.json`、rollback、assembly、schema、公共 API、ErrorCode、
      Windows runner 与依赖零改动；未启动真实 Excel/GUI。
- [ ] 全量门禁、独立 review no P0-P2、PR/CI/merge/main CI 证据落入记录区。
- [ ] 外部 automation JSONL 记录 start/gates/work/pr/ci/merge/main_ci/final 与 shell misuse。

<!-- 分界线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-414 Excel T2.2 L1 file shadow-copy port
【目标】在 test-only L1 边界实现受控文件影子副本的捕获、冲突门与逐字节恢复
【write scope】仅：本卡、`apps/agent-core/src/excel_t21_l1.rs`、新增 file-shadow 实现/测试、TASK-404 测试模块、agent-core README、Stage 2、PLAN、README、LEDGER
【铁律】无静默失败；每个写操作有 postcondition；五类输入先校验；资源有界；不扩公共边界；热点文档先取 guard
【禁止】不碰真实 Excel/COM/工作簿/GUI；不注册 Tool/ToolBus、`tools.json`、rollback、assembly；不改 schema、公共 API、ErrorCode、依赖
【验收】专项 `file_shadow` / Excel tests + workspace fmt/clippy/test + arch/schema/codegen/文档门禁/deny/diff-check → 全绿且 production surface 零改动
【依赖】TASK-404/406/407 Done；ADR-0062、ADR-0107 已核对；`N_auto=0`
【疑问】无；production 真实 task 回滚仍留后续卡
```

### 2. 实际改动

- 新增 `apps/agent-core/src/excel_t21_l1_file_shadow.rs`，在 test-only Excel L1 子模块中实现：
  仓库外绝对本地根校验、本地 `.xlsx` 有界读取、`ShadowCopyPath` containment、create-new
  临时文件发布、SHA-256 校验、恢复前 post-write digest 门与恢复后 anchor digest 复核。
- 文件恢复先把 target 移到 backup 并对 backup 验 post-write digest，再以 no-replace
  hard link 发布恢复内容；target 竞态重现时保留 target+backup 并显式失败。
- 在 `ExcelL1AnchorStore` 上提供 test-only `capture_with_file_shadow` /
  `restore_with_file_shadow`：物理 shadow 发布失败会回收刚插入的内存 anchor；
  恢复前复用既有 fingerprint 冲突门，成功后释放 active anchor。
- 在 `excel_t21_l1.rs` 仅新增 test-only 子模块声明，避免 870 行文件越过 900 行硬门禁。
- 新增 13 个文件系统专项测试，覆盖成功捕获/恢复/cleanup、create-new 冲突、目标 drift、
  shadow 自身篡改、源路径形状、active byte 上限、路径越界、anchor 容量回滚、
  `AlreadyAtAnchor`、repo root 边界、rollback no-replace、UserInteraction 保持与临时文件残留。
- 未修改 Windows runner、COM、production Tool/ToolBus、`tools.json`、rollback recipe、
  assembly、protocol schema、platform API、ErrorCode 或依赖。

### 3. 门禁输出摘要

- `cargo test -p assistant-agent-core file_shadow`：**13 passed / 0 failed**。
- `cargo test -p assistant-agent-core excel_t21`：**103 passed / 0 failed / 1 ignored**
  （ignored 为既有 TASK-403 human-present real-COM smoke）。
- `cargo test --workspace`：全部 workspace tests 与 doctests 通过。
- `cargo clippy --all-targets -- -D warnings`：exit 0；仅仓库既有
  `clippy::assert_is_empty` unknown-lint warning。
- `cargo run -p xtask -- hygiene`：**0 error / 261 warning，PASSED**；新增实现文件 900 行，
  在 900 行硬门禁内。
- 最终 PR/main CI 与其余门禁在代码 review 后复跑并回填。

### 4. DoD 核对

- [x] 文件影子端口只接受受控绝对本地根与 workbook 路径，越界和网络路径显式失败。
- [x] 捕获 create-new、写入、sync、digest 与单文件/总量上限均有实现和专项测试。
- [x] 恢复前检查 post-write digest；目标冲突时目标字节与影子副本均不变。
- [x] 恢复后目标 digest 精确回到 anchor digest；失败保留影子副本并返回显式错误。
- [x] anchor 组合入口在影子捕获失败时回收 anchor，在恢复前复用 fingerprint 冲突门。
- [x] 测试使用独立临时目录；成功和 create-new 冲突路径断言无 `.tmp` / backup 残留。
- [x] cleanup 只删除登记文件、ownership marker 与空目录；现有非空/repo 根被拒绝且不受损。
- [x] 未改 production Tool/ToolBus、`tools.json`、rollback、assembly、schema、公共 API、
      ErrorCode、Windows runner 或依赖；未启动真实 Excel/GUI。
- [x] 独立 review 最终 **No findings (P0-P2)**；残余 P3 已记录。
- [ ] PR/CI/merge/main CI 证据待回填。
- [ ] 外部 automation JSONL 最终事件与 shell misuse 精确值待回填。

### 5. 偏差

- 文件恢复定位为 test-only 端口；同目录 backup + no-replace hard link 是有界、可验证的测试实现，
  不是 production atomic replacement / 断电恢复契约。已在 README 中明确。
- 首轮独立 review 发现 root cleanup 越界删除、target digest TOCTOU、未记账 shadow 与
  验收过滤器 0 tests；第二轮又发现 rollback 仍可能删除重现 target、repo root 解析与
  900 行硬门禁问题；第三轮发现 UserInteraction 在组合回滚路径被改写成 Fatal。均已按
  根因修复，最终复审 no P0-P2。
- 本轮中途多次把只读查询误填为 `shell: "bash"`；最终以 rollout shell 审计值为准，
  不使用手工估算冒充精确值。

### 6. 新增长期记忆

- 无。稳定事实由任务卡、agent-core README 与 LEDGER 承载，未修改 `MEMORY.md` /
  `docs/memory/*`。

### 7. 遗留与下一步

- production task 的临时根选择、atomic replacement、异常退出恢复、真实保存后
  `.Value2/.Formula` 复读与 cleanup caller 仍待后续卡。
- production `tools.json`、ToolBus、rollback recipe、assembly registration 继续为空。

### 8. 外部证据

- implementation PR / CI / merge / post-merge main CI：待回填。
- 外部 automation JSONL：
  `C:\Users\fexfe\.codex\automations-logs\2026-11-26\ai-assistant-task-round-20261127-0030.jsonl`。

### 9. 给审阅者的关注点

- 重点核对恢复的 target/backup/temp 状态机是否在每一步失败都保留可恢复现场。
- 重点核对 root containment、Windows verbatim 剥离、canonical path 大小写和
  `ShadowCopyPath` 相对路径校验。
- 重点核对 test-only 组合入口是否会留下无物理 shadow 的 active anchor，或把 test-only
  文件恢复误解为 production T2.2 已完成。
- 残余 P3：marker 失败空目录、cleanup 非幂等、hardlink target 身份复核、600 行软警告；
  不属于本卡 DoD。
