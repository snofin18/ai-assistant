# 阶段 2 — Excel L1 COM + Adapter 规范化

> 周期 6~8 周　状态：**InProgress（2026-11-27）**；Stage 1 已完成；TASK-395 / TASK-396 / TASK-397 / TASK-398 / TASK-399 / TASK-400 / TASK-401 / TASK-402 / TASK-403 / TASK-404 / TASK-405 / TASK-406 / TASK-407 / TASK-408 / TASK-409 / TASK-410 / TASK-411 / TASK-412 / TASK-413 / TASK-414 / TASK-415 已完成。
> 2026-11-23 automation 运行日已由 PR #817 收口；TASK-397 的 typed `HandlerFailure`
> 错误桥已由 PR #819 merge `2c412d5` / main CI `45305239489` success 合入，TASK-395 恢复 Ready。
> TASK-372 真实 Excel
> COM preflight 已通过并以 **GO** 进入 Adapter 契约正式化；TASK-373 六件套就绪审计 Done，
> PR #736 / main CI `43839388347` 已回填；TASK-374 已把六项缺口收敛为可裁决 ADR/spec 决策包
> 并完成 PR #738 / main CI `43863545233`；TASK-375 已完成 Excel COM
> ownership/watchdog 生命周期回归，PR #740 / main CI `43894628610` 已回填。TASK-376 的
> D1-D6 已由人类于 2026-11-17 一次性批准，ADR-0104 与三份 spec 已合入；下一步是
> protocol schema 1.1 + 通用只读 validator，而不是直接创建 Excel pack 或改运行时。
> TASK-377 已完成运行日报告，
> PR #744 / main CI `43942329504` 已回填。TASK-378 已补齐 2026-11-16 五份 automation
> 轮次证据，并把机器结果从 `0 / 1 / 0` 修正为 `5 / 6 / *`；同时登记
> `automation-rounds` 读取工作区而非 `origin/main` 的 ADR-0054 D3 实现偏差。TASK-379
> 已把计数源改为 main commit 快照并补五类 git 状态测试；TASK-379 已完成，PR #751 merge `fa87cae6`，main CI `44030156733` success。TASK-380 已完成 ToolSchema 1.1、`AdapterToolEnvelopeV1` 生成类型与 1.0/1.1 fail-closed 门禁，PR #753 merge `06a7fa9`，main CI `44048983726` success；通用只读 validator 仍待后续卡。
> TASK-381 已完成 validator 实施前提只读审计，PR #755 merge `bbefb97`，post-merge main
> CI `44057360138` success。D2 projection 生成算法与 omission report、D3/D4 既有运行时语义
> 到 binder JSON 的 mapping、D5 recipe JSON 联合和 D6 action/human/resume 字段仍需
> Accepted 细化；`DRIFT-381-1` 只阻塞 validator 产品代码，不影响本次审计收口。下一步先补
> ADR/spec clarification，再派发 validator implementation。
> TASK-382 已完成 rmcp/chrono 依赖修复：固定 `rmcp 3.11.0` contributor commit
> `a95b8045`，移除 chrono 且未新增 advisory ignore；PR #758 merge `6679141` / main CI
> `44219594025` success。PR #757 已基于新 main 重跑，checks 21 success + 1 skipped，
> merge `67deedf` / main CI `44221880362` success。
> TASK-383 已关闭 `DRIFT-381-1`：ADR-0105 与 `docs/spec/adapter-bundle.md` 冻结 canonical
> JSON、projection manifest、selector typed union、共享 catalog、rollback anchor 与 interrupt
> action/resume 联合；PR #760 merge `2ae5a19` / main CI `44231680015` success。本卡不实现
> validator、不迁移现有 pack。
> TASK-385 已建立 Verify postcondition machine registry：
> `crates/verify/src/postcondition-registry-1.0.json` 固定 11 个 Adapter-supported kind，
> 每个 kind 均由现有 `parse_postconditions` 样例验收；`visual_assert` 以非空 reason 显式排除。
> PR #762 merge `4f92ed8` / main CI `44256975571` success；registry 合并后由后续
> validator implementation 卡改为唯一 catalog 输入。
> TASK-384 已完成 **Done**：`adapter-validate --report-only`、generated
> `adapter-catalog-1.0.json` 与 D1-D6 负向矩阵已补强，xtask `574/574`、workspace tests /
> Clippy 全绿；TASK-385 合并后 catalog 改为直接消费 Verify registry，三种 constraints
> 已进入 generated catalog 与 validator，`DRIFT-384-1` 关闭；PR #764 merge `6860901` /
> main CI `44285258665` success。
> TASK-387 已完成 **Done**：ADR-0106 增加 rollback-only `close_created_target`，validator
> 已封闭 target kind、empty 条件、write/L2 metadata、tab/new tags 与 whole-window
> fingerprint；PR #777 merge `a23a344` / main CI `44476000127` success，用于解除后续
> Notepad `tab.new` 的假 L1 recovery 阻塞。
> TASK-388 已完成 **Done**：修复 interrupt match 可选字段被误当成必填的 validator
> 偏差；PR #779 merge `9d6d558` / main CI `44490394734` success，下游 Notepad pack
> 重基后已 7 files / 0 error 通过。
> TASK-386 / TASK-389 / TASK-390 已完成 **Done**：Notepad 六件套迁移到 bundle v1，
> structured selector policy 与 ToolSchema 1.1 `none_readonly` 已进入生产运行时；
> PR #795 merge `29fb10b` / main CI `44664709243` success。
> TASK-391 已完成 **Done**：Windows COM apartment ownership guard 与 `ComApartment<T>`
> 同生命周期绑定落地，UIA 线程本地复用；5 类 apartment 测试、反向模型与 !Send 约束通过。
> PR #797 checks 11/11 success，merge `092c79a` / main CI `44695688650` success。
> TASK-392 已完成 **Done**：首个 `com.microsoft.excel` safety shell bundle 已落地，
> 以 `XLMAIN/#32770` 两个 provisional selector 和六类 human-only interrupt 冻结安全边界；
> tools/rollback 保持空数组并锁定 capability ADR 阻塞。PR #799 checks 11/11 success，
> merge `92024e8` / main CI `45101893667` success。
> TASK-393 已完成 **Done**：ADR-0107 冻结七个 experimental `com.*` capability、风险下限与
> T2.1-T2.3 消费边界；共享 validator 强制 open sole-capability 与四类 write floor。
> PR #802 checks 11/11 success，merge `c23e76b` / main CI `45115059165` success。
> TASK-394 已完成 **Done**：ADR-0108 冻结 T2.1 四个 Tool、ToolBus-isolated opaque session、
> 4096 cells / 256 KiB bounds、独立 watchdog 与 task-end cleanup；实现前 tools.json 保持空，
> 真实 COM/GUI 未执行。PR #806 checks 11/11 success，merge `614062a` / main CI `45133625235` success。
> TASK-397 已完成 **Done**：ADR-0109 以 typed `HandlerFailure` 承载既有 `ErrorCode`，
> 13 类 code round-trip 与 ToolBus error envelope 测试通过；PR #819 merge `2c412d5` /
> main CI `45305239489` success，不建立第二套错误桥。
> TASK-398 已完成 **Done**：四个 Excel T2.1 pure handler 已包装为私有 `ToolHandler`，
> session identity 从 registry 派生，13 类错误无损映射，四成功路径与 typed failure 均经
> in-process ToolBus 验证；PR #821 merge `0d96d8e` / main CI `45316595521` success。
> `tools.json` 与 production assembly 仍为零改动。
> TASK-395 已恢复 **Ready**：test-only fake runtime、session registry、path/range/read-mode
> parser、四个纯逻辑 handler 与 22 个负向测试已落地；`DRIFT-395-1` 已由 ADR-0109 / TASK-397
> 解除。`tools.json` 仍为空；按 ADR-0110，下一步先完成 private COM runner、真实 COM/GUI 与人类
> 在场 `.xlsx` 验收仍待后续。round 3 的 `DRIFT-395-2` 已由 TASK-396 运行日收口，保留历史记录。
> TASK-403 已完成 **Done**：test-only `ExcelT21Runtime` 私有映射到 `ExcelComRunner`，
> fake port 验证重复 open/read/close 的 handle/worker 收敛；PR #858 checks 22
>（21 success + 1 skipped），merge `9bb006a` / main CI `45597702500` success；
> 本轮未运行真实 COM，生产 tools/ToolBus 继续为空。
> TASK-404 已完成 **Done**：test-only `ExcelL1AnchorStore` 从 opened workbook 派生
> ToolBus session / task / owned process / workbook / worksheet identity，绑定有界
> `value2` 值快照、内存 workbook 影子副本与既有 `L1Snapshot + ShadowCopy` anchor；
> 覆盖绝对 cell 坐标及写前/写后/回滚前 identity + fingerprint 漂移，专项 26 passed。
> PR #861 merge `0435419` / main CI `45641443101` success；真实文件复制/恢复、
> 真实写入与 production 注册仍未开放。
> TASK-405 已完成 **Done**：test-only 私有 scalar write 输入门直接解析原始 JSON 词法，
> 保留 number 精度并拒绝 array/object、公式/Default_Ignorable 前缀、256 KiB 超界输入；
> 合法 scalar 再复用 TASK-404 的 pre-fingerprint single-use 写前准备。PR #865 checks
> 22（21 success + 1 skipped），merge `a5c5534` / main CI `45665526687` success；
> 真实写入/保存/恢复与 production 注册仍未开放。
> TASK-406 已完成 **Done**：test-only 私有 scalar range write port 绑定 anchor canonical
> address/worksheet/dimensions，固定 write → recalculate → Value2 → Formula →
> post-fingerprint，任一端口或双读失败均不记录 post 且禁止同 anchor 重试。PR #867
> checks 22（21 success + 1 skipped），merge `0426cd6` / main CI `45687778120` success；
> 真实写入/保存/恢复与 production 注册仍未开放。
> TASK-407 已完成 **Done**：新增 `#[cfg(test)]` writable open、`Range.Value2` scalar
> property put、`Application.Calculate` 与 `Workbook.Save`；真实 COM 临时副本验收已完成
> 单/双 cell 写入、`.Value2/.Formula` raw 校准、保存 digest 变化、shadow restore 与
> `EXCEL.EXE=0`。PR #870 merge `fc024e4` / main CI `45743239854` success；production
> `tools.json`、ToolBus、rollback、assembly 与公共 API 仍零改动。
> TASK-408 已完成 **Done**：新增 `tools/audit-rollout-shell.ps1`，用
> `FileShare.ReadWrite | Delete` + 严格 UTF-8 流式读取活跃 rollout，按逻辑 `turn_id`
> 合并多段 `turn_context` 并统计 `exec_command` shell。实测上一 TASK-407 turn 为
> PowerShell 194 / bash 54 / cmd 0；PR #872 merge `b8afc7a` / main CI
> `45753779989` success。
> TASK-409 已完成 **Done**：真实 Excel COM 连续 10/10 使用新 runner、进程、session
> 与工作簿副本；每轮 Save 后 digest 改变，shadow restore 回源 digest，源 fixture
> 不变，旧 handle 拒绝，最终 `EXCEL.EXE=0`。after-write failure injection 回收
> owned Excel；after-open 20 秒 watchdog 以 journal + Job 成员相关性复核 handle
> 后终止，Job 成员 6 → 0。PR #874 merge `b18603c` / main CI
> `45773046164` success；production `tools.json`、ToolBus、rollback、assembly、
> schema、公共 API 与依赖仍零改动。
> TASK-410 已完成 **Done**：只读对照 ADR-0108 D8/D9、current production
> `ExcelComRunner` 与 TASK-409 harness，识别 durable journal、独立 watchdog、
> unbounded join、cleanup 错误映射、verified termination、production task-end caller、
> ownership creation race 与 closing 状态顺序缺口。PR #876 merge `e6a6696` / main CI
> `45783616793` success；production 代码与 Tool surface 仍零改动。

## In scope

- `platform/windows` 内建立最小、可审计的 COM 生命周期与调用边界。
- 正式化 `adapter.toml`、App Map、selector、tool、rollback、interrupts 的声明式契约。
- 交付 `com.microsoft.excel` Adapter 数据包，接入 T2.1/T2.2/T2.3。
- Excel 写入一律按 L1（值快照 + 文件影子副本）建模，禁止声明为 L0。
- Protected View、文件占用/只读、AutoSave/OneDrive、模态对话框前置探测。
- 命名区域优先于 A1 引用；断言明确使用 `.Value2` / `.Formula` / `.Text`。
- COM 对象、Excel 进程与临时文件必须具备成功/失败路径回收证据。

## Out of scope

- Word / PowerPoint / Outlook；宏运行、VBA 注入、外部链接执行。
- 通用 `run_shell` / `run_powershell` / 通用 COM 任意调用工具。
- `DisplayAlerts=false` 或任何静默覆盖确认属性。
- 真实业务工作簿、云端 OneDrive/SharePoint 写入、无人值守 L3 操作。
- 在 Stage 2 preflight 完成前新增 COM 依赖、平台模块或公共 schema。

## 批次表

| 批次 | 卡号 | 合作方 | 卡片文件 | 状态 |
|---|---|---|---|---|
| 0 | **TASK-372 ✅** | 环境/架构 | `tasks/TASK-372-excel-com-preflight.md` | Done（COM preflight passed；PR #734 checks 11/11；main CI `43825737477` success） |
| 1a | **TASK-373 ✅** | 架构/治理 | `tasks/TASK-373-adapter-contract-readiness-audit.md` | Done（PR #736 checks 11/11；merge `2fab59f`；main CI `43839388347` success） |
| 1a.1 | **TASK-374 ✅** | 架构/治理 | `tasks/TASK-374-adapter-contract-adr-decision.md` | Done（PR #738 checks 11/11；merge `7e5805c`；main CI `43863545233` success） |
| 1a.2 | **TASK-375 ✅** | Windows platform/eval | `tasks/TASK-375-excel-com-lifecycle-failure-evidence.md` | Done（PR #740 checks 11/11；merge `c5cb52f`；main CI `43894628610` success；5 场景 ownership/watchdog 回归） |
| 1 gate | **TASK-376 ✅** | 架构/治理 | `tasks/TASK-376-adapter-contract-adr-spec-landing.md` | Done（D1-D6 已获人类批准；PR #749 checks 21 success + 1 skipped；merge `f29ae61`；main CI `44015728457` success） |
| 2b evidence | **TASK-377 ✅** | automation/治理 | `tasks/TASK-377-automation-run-day-report-2026-11-16.md` | Done（PR #744 checks 11/11；merge `9b2b9d8`；main CI `43942329504` success；运行日报告） |
| 2b evidence | **TASK-378 ✅** | automation/治理 | `tasks/TASK-378-automation-round-evidence-backfill-2026-11-16.md` | Done（PR #747 checks 21 success + 1 skipped；merge `bccc7c9`；main CI `43997916273` success；round 1~5 已补齐，计数来源偏差已登记） |
| 2b evidence | **TASK-379 ✅** | automation/治理 | `tasks/TASK-379-automation-rounds-main-only-source.md` | Done（PR #751 checks 21 success + 1 skipped；merge `fa87cae6`；main CI `44030156733` success；main-only 快照 + 有界 Git 清理） |
| Adapter schema | **TASK-380 ✅** | protocol/治理 | `tasks/TASK-380-protocol-tool-schema-1-1.md` | Done（PR #753 checks 21 success + 1 skipped；merge `06a7fa9`；main CI `44048983726` success；validator 后续） |
| 1 preflight | **TASK-381 ✅** | 架构/治理 | `tasks/TASK-381-adapter-bundle-validator-readiness.md` | Done（审计完成；PR #755 merge `bbefb97`，main CI `44057360138` success；`DRIFT-381-1` 阻塞下游 validator，下一步补 Accepted ADR/spec） |
| dependency gate | **TASK-382 ✅** | 依赖/治理 | `tasks/TASK-382-rmcp-chrono-rustsec-remediation.md` | Done（固定 `rmcp` commit `a95b8045`；chrono 已移除；PR #758 merge `6679141`；PR #757 merge `67deedf`；main CI `44221880362` success） |
| validator contract | **TASK-383 ✅** | 契约/架构 | `tasks/TASK-383-adapter-validator-machine-profile.md` | Done（ADR-0105 + `docs/spec/adapter-bundle.md` 冻结 D2-D6；PR #760 merge `2ae5a19`；main CI `44231680015` success） |
| registry prerequisite | **TASK-385 ✅** | Verify/治理 | `tasks/TASK-385-verify-postcondition-registry.md` | Done（machine registry + parser 同步测试；PR #762 merge `4f92ed8`；main CI `44256975571` success） |
| validator implementation | **TASK-384 ✅** | xtask/治理 | `tasks/TASK-384-adapter-validate-report-only.md` | Done（catalog 从 Verify registry 生成并携带 constraints；51 个 adapter 测试全绿；PR #764 merge `6860901`；main CI `44285258665` success） |
| rollback builtin | **TASK-387 ✅** | Adapter contract | `tasks/TASK-387-rollback-created-tab-builtin.md` | Done（ADR-0106 + validator 正负矩阵；不新增模型可见 Tool；PR #777 merge `a23a344`；main CI `44476000127` success） |
| interrupt contract repair | **TASK-388 ✅** | xtask/契约 | `tasks/TASK-388-interrupt-optional-match-fields.md` | Done（可选 match 字段 union + 空白字段拒绝测试；PR #779 merge `9d6d558`；main CI `44490394734` success；下游 Notepad pack 0 error） |
| runtime policy repair | **TASK-389 ✅** | agent-core/契约 | `tasks/TASK-389-runtime-selector-on-not-found.md` | Done（structured `on_not_found` + legacy string 兼容；PR #794 merge `9658bf0`；main CI `44658088616` success） |
| Notepad bundle convergence | **TASK-386 ✅** | Adapter/系统 | `tasks/TASK-386-notepad-adapter-bundle.md` | Done（pack v1；PR #795 merge `29fb10b`；main CI `44664709243` success） |
| runtime ToolSchema repair | **TASK-390 ✅** | tool-bus/agent-core | `tasks/TASK-390-runtime-tool-schema-1-1.md` | Done（ToolSchema 1.1 + `none_readonly` 深 runtime；PR #795 merge `29fb10b`；main CI `44664709243` success） |
| COM apartment guard | **TASK-391 ✅** | Windows platform | `tasks/TASK-391-com-apartment-guard.md` | Done（`ComApartment<T>` 将值与 guard 绑定；UIA 线程本地复用；`S_OK`/`S_FALSE`/错误/panic 配平、反模型与 !Send；PR #797 merge `092c79a`；main CI `44695688650` success） |
| 1 | 待派单 | 架构/治理 | dispatch 时创建 | 未开始（通用只读 Adapter bundle validator） |
| 2 | 待派单 | Windows platform | dispatch 时创建 | 未开始（COM apartment/生命周期/回收与负向测试） |
| 3 | **TASK-392 ✅** | Excel Adapter | `tasks/TASK-392-excel-adapter-safety-pack.md` | Done（safety shell bundle + Protected View/AutoSave/interrupts；2 shell targets / 6 interrupts；COM tools 等待 capability ADR；PR #799 merge `92024e8`；main CI `45101893667` success） |
| 3a | **TASK-393 ✅** | 架构/治理 | `tasks/TASK-393-excel-com-capability-vocabulary.md` | Done（七个 experimental `com.*` capability ADR；open sole-capability + 四类 write floor；PR #802 merge `c23e76b`；main CI `45115059165` success） |
| 3b | **TASK-394 ✅** | Excel T2.1 contract | `tasks/TASK-394-excel-t21-readonly-tool-contract.md` | Done（四 Tool、ToolBus-isolated session、4096 cells / 256 KiB、ownership/watchdog/cleanup；PR #806 merge `614062a`；main CI `45133625235` success） |
| 4 | **TASK-395 Ready/WIP** | Excel T2.1 | `tasks/TASK-395-excel-t21-fake-runtime-session-handlers.md` | Ready：fake runtime/session/parser/纯逻辑 handlers + 22 tests 已落地；两轮 review 5 P1 + 4 P2 已修复；`DRIFT-395-1` 已由 ADR-0109 / TASK-397 解除 |
| 4b evidence | **TASK-396 ✅** | automation/治理 | `tasks/TASK-396-automation-run-day-report-2026-11-23.md` | Done（round 3 + 2026-11-23 运行日报告；PR #817 merge `ed1c177`；main CI `45249757927` success；未改产品代码或 `tools.json`） |
| 4c bridge | **TASK-397 ✅** | ToolBus/治理 | `tasks/TASK-397-toolbus-handler-error-bridge.md` | Done（ADR-0109 typed `HandlerFailure`；13 类 ErrorCode round-trip；PR #819 merge `2c412d5`；main CI `45305239489` success） |
| 4d wrappers | **TASK-398 ✅** | Excel/ToolBus | `tasks/TASK-398-excel-t21-toolbus-handler-wrappers.md` | Done（四 wrapper + registry-derived session id + 13-code mapping；PR #821 merge `0d96d8e`；main CI `45316595521` success；production/tools.json 零改动） |
| 4e runner prototype | **TASK-399 ✅** | Windows platform | `tasks/TASK-399-excel-t21-com-runner-prototype.md` | Done（ADR-0110 gate 顺序；test-only dedicated worker + capacity-1 FIFO + bounded shutdown/cleanup + Windows IDispatch 编译后端；8/8 runner tests；PR #823 merge `b27ff85` / main CI `45338066396` success；tools.json/rollback/ToolBus 零改动） |
| 4f evidence | **TASK-400 ✅** | automation/治理 | `tasks/TASK-400-task395-closeout-round-evidence.md` | Done（TASK-395 closeout + 2026-11-24 round 1 evidence；PR #825 merge `d72ed16` / main CI `45366856008` success；docs-only） |
| 4g injection + smoke | **TASK-401 ✅** | Windows platform / agent-core boundary | `tasks/TASK-401-excel-t21-runner-injection-boundary.md` | Done（ADR-0111 窄公共 runner handle；真实 COM 单 `.xlsx` open/read/close 通过，digest 不变且 `EXCEL.EXE=0`；production tools/ToolBus 零注册；PR #851 merge `c629434` / main CI `45533518843` success） |
| 4h read surface | **TASK-402 ✅** | Windows platform | `tasks/TASK-402-excel-t21-read-surface.md` | Done（ADR-0112、完整只读 metadata/range/fingerprint/ownership；专项 14 passed / 1 ignored，真实 COM 1 passed、digest 不变且 `EXCEL.EXE=0`；PR #853 merge `d958b52` / main CI `45559864439` success） |
| 4i private mapping | **TASK-403 ✅** | agent-core / Windows runner | `tasks/TASK-403-excel-t21-platform-runtime-mapping.md` | Done（test-only private adapter；fake port 10 passed / 1 ignored；独立 review no P0-P2 findings；PR #858 merge `9bb006a` / main CI `45597702500` success；真实 COM 尚未执行；生产 tools/ToolBus 保持空） |
| 5 | 待派单 | Undo/L1 | dispatch 时创建 | 未开始（文件影子副本 + 值快照 + 冲突检测） |
| 5a L1 anchor | **TASK-404 ✅** | agent-core / L1 | `tasks/TASK-404-excel-t22-l1-anchor-preparation.md` | Done（opened-workbook identity + absolute cells + value snapshot + in-memory shadow copy + identity/fingerprint conflict；专项 26 passed，最终 review no P0-P2；PR #861 merge `0435419` / main CI `45641443101` success；真实文件副本/恢复与 production 注册未开放） |
| 6a scalar input gate | **TASK-405 ✅** | agent-core / L1 | `tasks/TASK-405-excel-t22-scalar-write-input-gate.md` | Done（test-only 原始 JSON scalar 词法、公式/Default_Ignorable 前缀拒绝、256 KiB 边界与 pre-fingerprint 写前准备；专项 7 passed；最终 review no P0-P2；PR #865 merge `a5c5534` / main CI `45665526687` success） |
| 6b private write port | **TASK-406 ✅** | agent-core / L1 | `tasks/TASK-406-excel-t22-private-write-port.md` | Done（anchor-bound selector/address/dimensions + single-use write/recalculate/Value2/Formula/post-fingerprint；专项 12 passed，最终 review no P0-P2；PR #867 merge `0426cd6` / main CI `45687778120` success；production Excel surface 仍为空） |
| 6c real write calibration | **TASK-407 ✅** | Windows platform / Excel COM | `tasks/TASK-407-excel-t22-real-com-write-acceptance.md` | Done（test-only writable open + scalar property put + Calculate + Save；真实 COM 单/双 cell 写入、Value2/Formula 双读、save/restore digest、EXCEL.EXE=0、review no P0-P2；PR #870 checks 11/11，merge `fc024e4` / main CI `45743239854` success） |
| evidence guard | **TASK-408 ✅** | automation/evidence | `tasks/TASK-408-rollout-shell-audit-strict-utf8.md` | Done（严格 UTF-8/共享读取/按 turn shell 审计；TASK-407 turn 精确 bash=54，复审 no P0-P2；PR #872 checks 11/11，merge `b8afc7a` / main CI `45753779989` success） |
| 6d repeated write convergence | **TASK-409 ✅** | Windows platform / Excel COM | `tasks/TASK-409-excel-t22-repeated-write-convergence.md` | Done（test-only 10/10 真实 COM 重复写入、逐轮 digest/stale handle/process convergence、after-write failure injection、after-open watchdog；review no P0-P2；PR #874 checks 22（21 success + 1 skipped），merge `b18603c` / main CI `45773046164` success） |
| 6e production cleanup audit | **TASK-410 ✅** | Windows platform / lifecycle | `tasks/TASK-410-excel-t22-production-cleanup-boundary-audit.md` | Done（ADR-0108 D8/D9 对照审计；durable journal/watchdog/unbounded join/cleanup 错误映射/verified termination/task-end caller/creation race/closing 状态缺口；review no P0-P2；PR #876 merge `e6a6696` / main CI `45783616793` success） |
| 6f ownership journal | **TASK-411 ✅** | Windows platform / lifecycle | `tasks/TASK-411-excel-t22-platform-private-ownership-journal.md` | Done（platform-private journal schema v1、MoveFileExW no-replace 原子发布、8-file 上限、session 级 opening/closing 收敛、cleanup 失败保留与 D9 映射；专项 182 passed / 8 ignored，host+Linux/macOS clippy、全量测试与 doctest 全绿；独立 review no P0-P2；PR #878 checks 22（21 success + 1 skipped），merge `1862df8` / main CI `45798133151` success；production Excel surface 仍为空） |
| 6g watchdog | **TASK-412 ✅** | Windows platform / lifecycle | `tasks/TASK-412-excel-t22-platform-private-watchdog.md` | Done（process-internal watchdog、arm 即 capture + 50 ms tracked-tree refresh、完整 identity 后 verified process-tree termination、256 handles / 8 snapshots 上限、snapshot-time FILETIME ceiling、sticky monitor error、bounded startup quarantine、三层 deadline；专项 196 passed / 8 ignored，workspace tests/doctest、host/Linux/macOS Clippy 与全量门禁全绿；独立 review no P0-P2；PR #880 checks 22（21 success + 1 skipped），merge `0571dce` / main CI `45823476783` success） |
| 4j task-end shutdown | **TASK-413 ✅** | agent-core / lifecycle | `tasks/TASK-413-excel-t21-private-task-end-shutdown.md` | Done（私有 running/closing/terminal、open 与 session 注册同锁线性化、全部 session 先 closing、runner 独立 mutex 与失败 mapping 保留；专项 6 passed，Excel 域 88 passed / 1 ignored，workspace/doctest、Clippy 与全量门禁全绿；review no P0-P2；PR #882 checks 22（21 success + 1 skipped），merge `58ceb69` / main CI `45845203826` attempt 2 success） |
| 6h file shadow | **TASK-414 ✅** | agent-core / L1 | `tasks/TASK-414-excel-t22-l1-file-shadow-copy-port.md` | Done（test-only filesystem shadow-copy port、repo-external owned root、create-new + SHA-256、staged target conflict gate、no-replace hard-link restore 与 evidence retention；专项 13 passed，Excel 域 103 passed / 1 ignored，workspace/doctest、Clippy 与全量门禁全绿；独立 review no P0-P2；PR #884 checks 22（21 success + 1 skipped），merge `8f2bb74` / main CI `45870164073` success；production Excel surface 仍为空） |
| 6i scalar file transaction | **TASK-415 ✅** | agent-core / L1 | `tasks/TASK-415-excel-t22-l1-scalar-file-transaction-coordinator.md` | Done（test-only scalar file-transaction coordinator、捕获前输入门、anchor+write-port+file-shadow 组合、借用 receipt 的 post-fingerprint + saved-file digest 双门回滚、失败证据保留与 cleanup；专项 7 passed，Excel 域 110 passed / 1 ignored，workspace/doctest、Clippy、77.96% coverage 与全量门禁全绿；独立 review no P0-P2；PR #887 checks 22（21 success + 1 skipped），merge `e91ecdf` / main CI `45884850506` success；production Excel surface 仍为空） |
| 6 | 待派单 | Excel T2.2 | dispatch 时创建 | 未开始（写入/重算/.Value2 校验/保存） |
| 7 | 待派单 | Excel T2.3 | dispatch 时创建 | 未开始（图表生成/PNG 导出/不覆盖） |

## 关键契约

### Excel 可逆性

- 单元格/区域写入：L1。
- 结构性改动：L1 + 反向脚本。
- 保存：保存前影子副本。
- 另存为/导出：L2，新产物进入受控影子目录。
- 任何 COM/VBA 程序化修改都不得声明可由 Excel undo 栈恢复。

### 身份与选择

- TargetDescriptor 至少包含 workbook + worksheet 身份。
- 优先命名区域；A1 字符串只作显式后备。
- 公式断言必须固定读取口径：`value2`、`formula` 或 `text`。

### 中断

- Protected View、只读/占用、AutoSave 冲突、另存为/覆盖、更新链接、宏警告均注册为 interrupt。
- “启用编辑”、覆盖、外链更新、宏执行只允许人工白名单动作，不提供静默继续。

## DoD

- T2.1/T2.2/T2.3 各自重复运行成功率 ≥85%，失败必须保留真实证据。
- COM 写入 100% 具备 L1 恢复路径，0 次把程序化修改标成 Excel undo 可恢复。
- 成功、异常、取消三条路径后 `EXCEL.EXE` 与临时工作簿均收敛。
- 新增普通 T1 应用的声明式 Adapter 边际成本 ≤3 人日。
- `cargo fmt/clippy/test`、arch、schema/codegen、deny、文档门禁与真机验收全绿。

## 阶段出口

- Adapter 六件套 schema 与 owner 边界由 ADR/spec 冻结。
- Excel Adapter 与 COM runner 不使用应用专用公共 trait。
- L1 回滚在真实保存前后可重复验证，冲突显式升级人工。
- 形成 Stage 2 审计报告后才能切换 Stage 3。
