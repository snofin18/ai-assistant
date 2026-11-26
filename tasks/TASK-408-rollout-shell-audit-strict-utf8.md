# TASK-408 rollout shell-misuse audit strict UTF-8

- 状态：**Done**
- 阶段：2　子阶段：Stage 2 automation evidence guard　批次：evidence guard
- 依赖：ADR-0100、TASK-342、TASK-345

---

## write scope

- `tasks/TASK-408-rollout-shell-audit-strict-utf8.md`
- `tools/audit-rollout-shell.ps1`
- `docs/automations/codex-automations-operations.md`
- `docs/memory/pitfalls.md`
- `MEMORY.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

建立一个只读、可自测的 rollout JSONL 审计工具。工具必须能在 Codex 正在写 rollout
时，以严格 UTF-8 流式读取文件，逐行验证 JSON，按 `turn_context` 边界精确统计
`exec_command` 的 `shell=bash/powershell/cmd`，并输出指定 turn 或最近 N 个 turn 的
机器可读证据。

## In scope

- 使用允许并发写入的 `FileShare.ReadWrite | Delete` 打开活跃 rollout。
- 使用严格 UTF-8 decoder（允许 BOM，拒绝非法字节），禁止系统默认编码。
- 每个非空 JSONL 行都解析；失败时报告物理行号和解析错误。
- 以 `turn_context.payload.turn_id` 为唯一边界，不按时间或自然语言猜测。
- 只审计 `function_call.name=exec_command`，解析其嵌套 JSON `arguments`。
- 精确统计 `powershell`、`bash`、`cmd/cmd.exe`、缺失/未知 shell。
- 支持 `-TurnId` 精确选择与 `-LastTurns N` 最近轮次选择；可附带 bash 命令原文。
- 自带合成 fixture selftest：CJK、单/多 turn、多 shell、活跃写句柄、非法 UTF-8、
  宽松/坏 JSON、UTF-16 BOM、orphan、超长行、增长快照与 turn 选择。
- 文档记录用法、根因和禁止事项。

## Out of scope

- 不修改 rollout 文件、历史记录、thread history 或 quota 水位。
- 不修复 Codex/Guard 本身，也不替代 ADR-0100 guard。
- 不统计 subagent rollout（除非显式提供其路径）；不从 prose 或终端回显猜计数。
- 不新增 Rust 依赖、xtask 公共命令、schema、产品代码或自动化调度。

## 必须遵守

1. 脚本源文件保持纯 ASCII，CJK fixture 在运行时用码点构造。
2. 脚本要求 `pwsh 7`；PowerShell 5.1 必须由 `#requires` 明确拒绝。默认读取失败必须
   非零退出，禁止静默跳过非法行或非法 UTF-8。
3. 输出只包含自包含计数和命令证据；不修改任何输入文件。
4. 活跃文件使用共享读取，不因 Codex 持有写句柄而误报“文件损坏”。
5. selftest 与真实 rollout 审计分别输出证据，不得互相冒充。
6. 计数必须按 turn 分组；历史 turn 与当前 turn 不能混在一个模糊总数里。

## 验收命令

```powershell
pwsh -NoProfile -File tools/audit-rollout-shell.ps1 -SelfTest
pwsh -NoProfile -File tools/audit-rollout-shell.ps1 -RolloutPath '<rollout.jsonl>' -LastTurns 2 -IncludeCommands -Format Json
cargo run -p xtask -- hygiene
cargo run -p xtask -- docscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
git diff --check
```

预期：selftest 全绿；真实 rollout 严格 UTF-8 与 JSON 全量通过；指定 turn 的
`bash/powershell/cmd` 计数和命令列表可重复；历史文件字节不变。

## DoD

- [x] 工具在活跃 rollout 上以共享读打开成功，并明确区分“文件占用”与“内容损坏”。
- [x] 非法 UTF-8、宽松/坏 JSON、UTF-16 BOM、缺 turn_context、orphan、未知 shell、
  超长行与增长快照均有负向测试和显式失败/分类。
- [x] 已完成 TASK-407 turn 的 shell 计数可从工具输出复现，并附 54 条 bash 命令证据。
- [x] 工具源文件纯 ASCII；CJK fixture 使用码点构造；PowerShell 5.1 明确拒绝。
- [x] 文档记录根因、用法、输出字段与禁止用默认编码/宽松 JSON 的规则。
- [ ] 全部门禁、独立 review、PR/CI/merge/post-merge main CI 收口。

<!-- 分隔线以下由 Implementer 填写；正文区只读，状态行按 ADR-0083 更新。 -->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-408 rollout shell-misuse audit strict UTF-8
【目标】建立严格 UTF-8、共享读取、按 turn 精确统计 exec_command shell 的只读审计工具
【write scope】仅：本卡、audit-rollout-shell.ps1、automation operations、pitfalls、MEMORY 规模表、Stage 2、PLAN、README、LEDGER
【铁律】不可信 JSONL 全量逐行校验；非法 UTF-8/JSON fail-closed；不修改 rollout；输出无伪精确值；热门文档写前 guard
【禁止】不改 rollout 字节、不改产品/schema/依赖、不把 selftest 当真实证据、不按默认编码猜
【验收】selftest + 当前活跃 rollout + 上一 turn 精确计数和命令列表 + docs 门禁
【依赖】ADR-0100、TASK-342/345；main=c5d8dfe
【疑问】无
```

### 2. 实际改动

- 新增 `tools/audit-rollout-shell.ps1`。脚本要求 `pwsh 7`，通过 inline C# 受限逐行
  byte reader 捕获初始文件长度，使用 `FileShare.ReadWrite | Delete` 只读打开，
  逐行严格 UTF-8 解码，并以 `System.Text.Json.JsonDocument` 拒绝 Comment、trailing
  comma、single quote 与 `NaN`。
- 同一逻辑 `turn_id` 的多个 `turn_context` 段合并后再计数；支持精确 `-TurnId` 与
  `-LastTurns`。输出区分 `snapshot_complete`、`is_latest_turn` 与 `turn_complete`。
- 默认遇文件增长、partial tail、orphan、超长行、超量 turn/command 或非法 JSON 时
  fail-closed；`-AllowGrowth` 仅输出诊断快照并明确标 incomplete。
- selftest 覆盖 CJK round-trip、单/重复 turn、cmd/cmd.exe、missing/unknown、共享写句柄、
  非法 UTF-8 行号、宽松 JSON、UTF-16 BOM、orphan then turn、超长行、增长状态与选择器。
- 更新 automation operations 的 rollout 审计入口、Excel app pitfalls 与当前进度文档。

### 3. 门禁输出摘要

- `pwsh -NoProfile -File tools/audit-rollout-shell.ps1 -SelfTest` → `SELFTEST PASSED`。
- Windows PowerShell 5.1 调用 → 由 `#requires -Version 7.0` 明确拒绝，不执行半兼容逻辑。
- 源文件纯 ASCII：`37,024` bytes、非 ASCII bytes `0`。
- 真实活跃 rollout 首轮严格 UTF-8/JSON 全量：`292,070,878` bytes、`109,905` 行、
  `0` JSON parse error。
- 最终真实快照：`110,759` lines、`110,759` JSON lines、`orphan_exec_calls=0`；
  TASK-407 已完成 turn = PowerShell 194 / bash 54 / cmd 0，54 条 bash 命令均有物理行号。
- 当前 turn 在读取期间文件增长，工具标 `snapshot_complete=false`、`is_latest_turn=true`、
  `turn_complete=false`；快照为 PowerShell 68 / bash 15 / cmd 0，不把当前计数作为最终完成值。
- 独立 review 首轮 3 P1 + 6 P2/P3；第二轮再发现跨 segment 上限与 turn off-by-one。
  修复后复审 **No findings (P0-P2)**。
- `docscan`、`hygiene`、`memory-counts`、`card-check`、`git diff --check` → PASSED。

### 4. DoD 核对

- [x] 活跃文件共享读取、严格 UTF-8/JSON、turn 合并与增长标记均落地。
- [x] PowerShell 5.1 明确拒绝；CJK、BOM、宽松 JSON、行号与 orphan 均有 selftest。
- [x] 已完成 TASK-407 turn 的 PowerShell 194 / bash 54 / cmd 0 可复现。
- [x] 普通路径输出有界；超长行、turn 数、shell 值数与非 powershell 命令证据数有硬上限。
- [x] 文档同步完成，未修改 rollout 字节。
- [x] 独立 review 复审 **No findings (P0-P2)**；PR #872 checks **11/11 SUCCESS**，
  pending 0 / failed 0，base=main，`MERGEABLE` / `CLEAN`；merge
  `b8afc7ad760fa85c475629940b3ed606cd8f8610`；post-merge main CI
  `45753779989` success。

### 5. 偏差

- 首版工具使用默认 StreamReader BOM auto-detect 与 PowerShell `ConvertFrom-Json`，
  且未合并同 turn_id 多段。独立 review 发现 3 个 P1 + 多个 P2；已改为 `pwsh 7`、
  手动 UTF-8 BOM 处理、`System.Text.Json`、C# 受限 reader、初始长度快照与合并逻辑。
- 第二轮 review 发现同一 turn 跨 segment 合并时可绕过 shell-value/command 上限，以及
  最终 turn 段 off-by-one。已把上限检查放入合并路径和最终段加入路径，并用低阈值
  selftest 正向复现拒绝。
- 首次未先取 guard 就更新了 Stage 2 / PLAN / README / pitfalls；发现后立即补取 guard。
  锁定是运行时互斥，不能追溯保护已写内容，因此如实记录该治理偏差。
- 当前 turn 仍在持续写入，真实审计只把 TASK-407 已完成 turn 作为精确断言；当前 turn
  只保留带 incomplete 标志的诊断快照。

### 6. 新增长期记忆

- `docs/memory/pitfalls.md` 新增 2026-11-26 PITFALL：默认编码、共享模式与宽松 JSON
  会把活跃 rollout 误报为损坏；标准入口必须是严格 UTF-8 + `System.Text.Json` 工具。

### 7. 遗留与下一步

- 合入后，后续所有 shell_misuse 最终计数必须调用 `tools/audit-rollout-shell.ps1`；
  活跃当前 turn 不能发布最终精确值。随后继续真实 COM 重复收敛卡。

### 8. 外部证据

- 当前 thread rollout：`C:\Users\fexfe\.codex\sessions\2026\10\27\rollout-2026-10-27T10-20-27-01a17b28-b2a8-7411-aa32-9ffec7474b88.jsonl`
- TASK-407 turn id：`01a2157a-784b-7212-849c-f2ce9619f1bb`；当前 turn
  `01a215de-5e4b-7ff2-a1c1-d98995e8220b`。
- 已完成 TASK-407 turn：PowerShell `194`、bash `54`、cmd `0`、missing `0`、
  unknown `0`；54 条 bash 命令逐条带物理行号。
- 当前 active turn 的最终快照：PowerShell `68`、bash `15`、cmd `0`，但
  `turn_complete=false`，只证明快照范围内计数。
- 独立 review 复审：**No findings (P0-P2)**。
- implementation PR #872：https://github.com/snofin18/ai-assistant/pull/872
  checks **11/11 SUCCESS**，pending 0 / failed 0，base=main，`MERGEABLE` / `CLEAN`。
- merge `b8afc7ad760fa85c475629940b3ed606cd8f8610`；post-merge main CI
  `45753779989` success。

### 9. 给审阅者的关注点

- C# reader 以构造时的文件长度为快照上限，写入器后续追加不会破坏已解析前缀。
- `System.Text.Json` 负责严格语法；`ConvertFrom-Json` 只做已校验文本的对象投影。
- 当前 turn 必然可能是 incomplete；任何最终计数只能引用 `turn_complete=true` 的 turn。
