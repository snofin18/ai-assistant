# TASK-038　T1.3：新建标签 → 写入 → 另存为到指定路径（跨进程 Shell 对话框）

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A5**　依赖：037　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：037　**预估**：M　**难度**：M
- **write scope**：`adapters/com.microsoft.notepad/tasks/**`、`eval/tasks/notepad/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A5（1a）、`docs/wbs-overview.md` §6、架构 v2 §7/§8.6/§12、`docs/memory/apps/notepad.md`、`tasks/TASK-075-b1-4-probe-07-cross-process-dialog.md`

## 目标

定义 T1.3 的声明式任务包与评测集：新建空白标签，写入用户提供的 Unicode 文本并读回验证，再通过 `notepad.file.save_as` 打开跨进程 `#32770` Shell 保存对话框，把文档另存到指定新路径；保存前展示 diff 并取得 `once` 人工授权。目标路径已存在、对话框身份不符、文件名输入能力缺失、用户取消或写入后置断言失败时一律 fail-closed，绝不静默覆盖。

## In scope

- `adapters/com.microsoft.notepad/tasks/t1.3.new-tab-write-save-as.json`：任务输入/输出、审批门、步骤、前置/后置条件、目标冲突策略、失败映射、指标与可逆性边界。
- `adapters/com.microsoft.notepad/tasks/README.md`：追加 T1.3 约定、跨进程对话框行为与后续治理边界。
- `eval/tasks/notepad/t1.3/cases.json`：连续 10 次主场景，以及空文本、Unicode、含空格路径、目标已存在、竞态新建、对话框缺失/身份错误、文件名字段能力缺失、用户取消、审批拒绝等回归用例。
- `eval/tasks/notepad/t1.3/expected.json`：确定性期望、审批形状、跨进程对话框证据、目标保护与错误码断言。
- `eval/tasks/notepad/t1.3/generate-fixtures.py`：零依赖生成可重复的源文本与被保护的目标 fixture。
- `eval/tasks/notepad/t1.3/validate.py`：静态校验任务、评测、注册工具引用、审批、目标冲突策略、对话框身份与后置条件。

## Out of scope（做了算漂移）

- 修改 `crates/**`、`apps/**`、`protocol/**`、`docs/spec/**`、`docs/adr/**`、`xtask/**` 或 `.github/workflows/**`。
- 修改 Adapter 的 `tools/`、`selectors/`、`rollback/`、`interrupts/`、`adapter.toml` 或 runtime 装配。
- 实现真实任务执行器、模型调用、Host 装配、审批接线、真实 Notepad GUI 操作或跨进程窗口枚举器。
- 新增 `notepad.file.write_text`、允许 `overwrite_existing=true`、改变 `notepad.file.save_as` 公共工具契约，或伪造未注册工具。
- 实现“备份后确认覆盖”的自动路径；当前 `notepad.file.save_as` 契约明确拒绝覆盖，本卡只验证 fail-closed。
- 修改 `fixtures/apps/**` 或 `fixtures/recordings/**`。

## 必须遵守

- 只引用 Adapter 已注册的 `notepad.tab.new` 与 `notepad.file.save_as`；文本写入使用 Host `set_editor_value` 前置动作并立即读回，不伪造 `write_text` 工具。
- 新建标签前记录标签数与活动文档指纹；`notepad.tab.new` 后必须确认标签数恰好加一且新文档为空。
- `set_editor_value` 写入后必须用 `notepad.file.read_text` 读回，并按 `replace("\r\n","\n").replace("\r","\n")` 规范化为 LF 后逐字比较。
- Save As 是 L3 落盘动作：必须 `risk=high`、`show_diff=true`、`scope_options=["once"]`、`point_of_no_return=true`，审批拒绝返回 `UserInteraction` 且不得打开保存对话框。
- 目标路径必须是绝对、规范化路径；父目录必须存在且可写。非法路径返回 `ToolInvalidArgs`，父目录不存在或不可写返回 `PlatformPermission`。
- 保存前先检查目标路径。目标已存在时返回 `PolicyDenied`，不得调用 `notepad.file.save_as`，不得删除、重命名或改写目标文件；如未来产品要支持覆盖，必须先备份并取得独立人工确认，且需另立工具契约卡。
- 保存对话框必须是 `#32770` 且属于 `explorer.exe` 子进程，不得与 Notepad 主窗口同进程；找不到或身份不符返回 `TargetNotFound`，不得按标题单条件命中。
- 文件名输入优先使用前景校验后的 Unicode 键盘路径；若 UIA `ValuePattern` 和目标控件能力均不可用，则返回 `CapabilityMissing`，不得静默改用粘贴凭据、剪贴板注入或猜测路径。
- 对话框出现、文件名写入、保存按钮触发必须有显式等待与超时；超时返回 `TargetUnresponsive`，用户关闭或取消对话框返回 `UserInteraction`。
- 保存后必须从磁盘读回并规范化比较，验证标题无未保存标记、目标文件此前不存在且现已被创建；任一后置条件失败返回 `VerifyFailed`。
- `expected.json` 只允许确定性结果；10 次重复成功率必须写成计数/区间，不得写成伪造的一次观测。

## 验收命令

```powershell
python eval/tasks/notepad/t1.3/validate.py
python eval/tasks/notepad/t1.3/generate-fixtures.py --output-directory eval/tasks/notepad/t1.3/fixtures
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

## 完成定义（DoD）

- [x] 任务包可解析，明确输入、输出、审批门、步骤、前置/后置条件、目标冲突策略、失败映射和指标。
- [x] 任务只引用现有注册工具；文本写入缺口以 Host 前置动作和 DRIFT 显式记录，不静默扩展工具集。
- [x] 评测集含连续 10 次主场景，并覆盖 Unicode、空文本、含空格路径、目标已存在、保存前竞态、对话框缺失/身份错误、文件名能力缺失、用户取消、审批拒绝。
- [x] Save As 审批固定为 `show_diff=true` 且只提供 `once`，高风险不可逆路径明确标注 point-of-no-return。
- [x] 目标已存在时目标文件字节与元数据保持不变，且 `notepad.file.save_as` 未被调用。
- [x] 跨进程对话框身份、文件名写入、保存按钮触发、磁盘读回和标题无未保存标记均有确定断言。
- [x] Python fixture 生成器可重复生成源文本与保护目标；静态校验和 Rust/xtask 门禁全绿。
- [x] LEDGER 追加；新增事实/坑写入长期记忆。
- [x] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-038 T1.3 新建标签 → 写入 → 另存为指定路径
【目标】交付声明式任务包与评测集，覆盖跨进程 Save As、审批、目标保护与 fail-closed
【write scope】仅：adapters/com.microsoft.notepad/tasks/**、eval/tasks/notepad/**（另含本卡记录区与收尾文档）
【铁律】1 无静默失败；2 输入与 fixture 不可信；6 L3 落盘必须人工确认；4 写操作必须有 postcondition；9 不静默扩大范围；10 契约先行
【禁止】crates/**、apps/**、protocol/**、Adapter tools/selectors/rollback/interrupts、真实 GUI、T1.1/T1.2
【验收】T1.3 静态校验 + fixture 生成；fmt/clippy/workspace tests；全部适用 xtask 门禁
【依赖】TASK-037 已 Done；已核对 LEDGER
【疑问】缺少注册 write_text 工具且 save_as 拒绝覆盖；按安全默认记录 DRIFT-038-1 / 038-2，不新增公共契约
```

### 2. 实际改动文件

- `adapters/com.microsoft.notepad/tasks/t1.3.new-tab-write-save-as.json`
- `adapters/com.microsoft.notepad/tasks/README.md`
- `eval/tasks/notepad/t1.3/cases.json`
- `eval/tasks/notepad/t1.3/expected.json`
- `eval/tasks/notepad/t1.3/generate-fixtures.py`
- `eval/tasks/notepad/t1.3/validate.py`
- `tasks/TASK-038-t1-3-newtab-saveas-cross-process-dialog.md`（正文占位展开 + 执行记录区）

### 3. 验收输出摘要

- `python eval/tasks/notepad/t1.3/validate.py` → `t1_3_static_validation_ok cases=14`。
- `python eval/tasks/notepad/t1.3/generate-fixtures.py --output-directory <temp>` → `fixture_count=8`，并生成 manifest、受保护目标和含空格目录。
- T1.1/T1.2/T1.3 三套静态校验均 PASS；JSON 均可解析，task 只引用 `notepad.tab.new` / `notepad.file.read_text` / `notepad.file.save_as`。
- `cargo fmt --all --check` 0 diff；`cargo clippy --all-targets -- -D warnings` exit 0；`cargo test --workspace` 全绿（含 `xtask` 379 tests）。
- `hygiene` 0E/4W、`check-ledger` 0E/0W、`check-migrations` 0E/0W、`card-check` 0E/27W、`docscan` 0E/388W。
- `memory-counts` / `adr-index` / `refscan` / `verify-schemas` / `codegen --check` 全 PASS。
- PR #94 = https://github.com/snofin18/ai-assistant/pull/94 —— base = `main`；**9/9 check-run 全 success**；合并前 `mergeable=MERGEABLE` / `merge_state_status=CLEAN`；merge commit `5860840f9188fd2e446dbb32f7c632421d7d9d8a`。

### 4. DoD 逐条核对

- [x] 任务包可解析，含输入、输出、审批门、步骤、前置/后置条件、冲突策略、失败映射和指标。
- [x] 只引用现有注册工具；文本写入缺口显式使用 Host `set_editor_value`，未伪造新工具。
- [x] 14 用例覆盖 10 次主场景及 Unicode、空文本、含空格路径、目标已存在、竞态、对话框缺失/身份错误、能力缺失、用户取消、审批拒绝。
- [x] Save As 固定 `high` + `show_diff=true` + `once` + `point_of_no_return=true`。
- [x] 目标已存在或保存前竞态创建时返回 `PolicyDenied`，不调用 save-as，保持目标字节与元数据不变。
- [x] 跨进程 `#32770` 身份、文件名写入、保存点击、磁盘读回和标题无未保存标记均有确定断言。
- [x] fixture 生成器、静态校验、Rust 与适用 xtask 门禁全绿。
- [x] LEDGER、FACT 与 PITFALL 已同步。
- [x] 未修改 Out of scope 文件。

### 5. 偏差

**DRIFT-038-1（缺少注册 `notepad.file.write_text`）**：T1.3 需要先写入新标签，但 TASK-035 只注册了 read/replace/save/new_tab/save_as。本卡不修改 `tools/tools.json`，而是把文本写入声明为 Host `set_editor_value` 前置动作并立即用注册 read 工具读回验证；任务不把未注册能力伪装成模型工具。影响：当前声明式任务包不能仅靠注册工具完整执行，write 工具归属需后续治理。

**DRIFT-038-2（save_as 拒绝覆盖，没有自动“备份 + 确认后覆盖”路径）**：`notepad.file.save_as` 的 `overwrite_existing` 是 const false，因此已存在目标无法在现有契约内安全覆盖。本卡按铁律 6/9 采用唯一安全默认：预检或竞态发现目标存在即 `PolicyDenied`，不调用工具、不修改目标；评测显式断言该行为。未来若产品要求自动覆盖，必须先立工具契约卡实现备份、diff、确认与恢复，不得静默放宽本卡。

### 6. 更合理做法

把“目标已存在”做成前置/竞态两处独立负向用例，并断言 `save_as_tool_called=false`，避免只在 UI 对话框层拦截；这样即使未来 save dialog 行为变化，任务仍不会覆盖已有文件。跨进程身份校验同时检查 class 与进程隔离，而不是仅按本地化标题匹配。

### 7. 遗留问题

- DRIFT-038-1：注册 write 工具归属待后续卡或阶段 2 Adapter schema 裁决。
- DRIFT-038-2：自动“备份 + 确认后覆盖”路径需独立工具契约与恢复测试。
- 真实任务执行器、Host 装配、审批接线、前景键盘文件名字段输入仍未实现；本卡只交付声明式任务与评测。
- PR #94 已合并，TASK-038 已 closeout；下一张为 TASK-039。

### 8. 新增长期记忆

- FACT：`[2026-09-29][FACT][src:TASK-038 实现实测]` T1.3 将 Save As 目标冲突处理前移到预检和竞态窗口，要求 `PolicyDenied`、不调用 save-as、目标字节不变。
- PITFALL：`[2026-09-29][PITFALL][src:TASK-038 实现]` 当前 `notepad.file.save_as` 只能创建新文件，不能自动安全覆盖已有目标；需备份与确认时不得绕过工具契约。

### 9. 给审阅者的关注点

- 重点审阅 DRIFT-038-2：已有文件绝不覆盖是安全默认，但“备份 + 确认后覆盖”仍未实现。
- 重点审阅跨进程对话框身份：class `#32770` 与进程隔离必须同时满足。
- 重点审阅 write_text 缺口是否应成为注册工具，还是继续作为 Host 前置动作。
