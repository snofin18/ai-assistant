# 阶段 2 — Excel L1 COM + Adapter 规范化

> 周期 6~8 周　状态：**InProgress（2026-11-18）**；Stage 1 已完成；TASK-372 真实 Excel
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
| 1 | 待派单 | 架构/治理 | dispatch 时创建 | 未开始（通用只读 Adapter bundle validator） |
| 2 | 待派单 | Windows platform | dispatch 时创建 | 未开始（COM apartment/生命周期/回收与负向测试） |
| 3 | 待派单 | Excel Adapter | dispatch 时创建 | 未开始（Adapter 数据包 + Protected View/AutoSave/interrupts） |
| 4 | 待派单 | Excel T2.1 | dispatch 时创建 | 未开始（打开工作簿/读取命名区域/汇总，只读） |
| 5 | 待派单 | Undo/L1 | dispatch 时创建 | 未开始（文件影子副本 + 值快照 + 冲突检测） |
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
