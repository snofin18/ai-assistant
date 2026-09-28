# TASK-037　T1.2：全文替换「报表」→「报告」+ 保存（含审批 diff、L0 undo + L1 快照、后置断言）

- 状态：**Done**
- 阶段：1　子阶段：**1a**　批次：**A5**　依赖：036　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：036　**预估**：M　**难度**：M
- **write scope**：`adapters/com.microsoft.notepad/tasks/**`、`eval/tasks/notepad/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 A5（1a）、`docs/wbs-overview.md` §6、架构 v2 §8.6 / §9、ADR-0044、ADR-0048、`docs/memory/apps/notepad.md`

## 目标

定义 T1.2 的声明式任务包与评测集：打开指定文本文件，把全部字面量「报表」替换为「报告」，先展示审批 diff，再执行替换并保存；写前同时准备 L0 undo 与 L1 快照，替换后 L0 能回到锚点，保存后 L1 能恢复磁盘；替换计数、标题未保存标记、磁盘读回与目标歧义均显式验证。

## In scope

- `adapters/com.microsoft.notepad/tasks/t1.2.replace-save-approval-undo.json`：任务输入/输出、两段审批门、执行步骤、前置/后置条件、错误映射、指标与 rollback 引用。
- `adapters/com.microsoft.notepad/tasks/README.md`：追加 T1.2 约定与 T1.3 边界。
- `eval/tasks/notepad/t1.2/cases.json`：连续 10 次主场景，以及多命中、零命中、计数不符、歧义目标、审批拒绝、保存后置失败、L0/L1 撤销等回归用例。
- `eval/tasks/notepad/t1.2/expected.json`：确定性期望、审批形状、保存后置、撤销层级与负向错误。
- `eval/tasks/notepad/t1.2/generate-fixtures.py`：零依赖生成可重复 fixture。
- `eval/tasks/notepad/t1.2/validate.py`：静态校验任务、评测、注册工具引用、审批/锚点/后置条件约束。

## Out of scope（做了算漂移）

- 修改 `crates/**`、`apps/**`、`protocol/**`、`docs/spec/**`、`docs/adr/**`、`xtask/**` 或 `.github/workflows/**`。
- 修改 Adapter 的 `tools/`、`selectors/`、`rollback/`、`interrupts/`、`adapter.toml` 或 runtime 装配。
- 实现真实任务执行器、模型调用、Host 装配、审批接线或真实 Notepad GUI 操作。
- T1.3 新建标签/另存为；真实商业应用；大文件流式关键词分析。
- 修改 `fixtures/apps/**` 或 `fixtures/recordings/**`。

## 必须遵守

- 只引用 Adapter 已注册的写工具 `notepad.file.replace_text` 与 `notepad.file.save`；打开文件继续使用 TASK-036 已记录的 `open_mode=platform_open_file` 前置动作，不伪造注册工具。
- 目标解析策略固定为 `error_if_ambiguous`；多个编辑区命中时必须返回 `TargetAmbiguous`，不得取第一个。
- 替换前先计算 `replacement_count` 与审批 diff；实际计数不等于 `expected_replacements` 时 fail-closed，不得保存。
- 替换和保存分别要求 `show_diff=true` 与 `once` 授权范围；审批拒绝返回 `UserInteraction`，不得继续执行。
- 写前同时建立 L0 `Ctrl+Z` 锚点与 L1 canonical-text/disk snapshot；保存前撤销必须验证 canonical text 回到 pre-replace；保存后验证以 L1 恢复内存与磁盘，不能把 `Ctrl+Z` 当作落盘恢复手段。
- 保存后必须验证标题无未保存标记，并从磁盘读回内容；任一后置条件失败返回 `VerifyFailed` 并按 rollback recipe 处置。
- 文本规范形为 LF；比较前按 `replace("\r\n","\n").replace("\r","\n")` 归一化，替换按字面 substring，不做正则或大小写折叠。
- `expected.json` 只允许确定性结果；涉及 10 次重复成功率的字段必须明确为计数/区间，不得写成伪造的一次观测。

## 验收命令

```powershell
python eval/tasks/notepad/t1.2/validate.py
python eval/tasks/notepad/t1.2/generate-fixtures.py --output-directory eval/tasks/notepad/t1.2/fixtures
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger
```

## 完成定义（DoD）

- [x] 任务包可解析，明确输入、输出、审批门、步骤、前置/后置条件、rollback 引用、失败映射和指标。
- [x] 任务只引用现有注册工具；打开文件缺口沿用 `platform_open_file`，不静默扩展工具集。
- [x] 评测集含连续 10 次主场景，并覆盖多命中、零命中、计数不符、目标歧义、审批拒绝、保存后置失败、L0 保存前撤销、L1 保存后恢复与 L0 不可用时 fallback。
- [x] 审批形状固定为 `show_diff=true` 且只提供 `once`；高风险保存不得 broad scope。
- [x] 替换计数、标题无未保存标记、磁盘读回、L0/L1 锚点和 `error_if_ambiguous` 均有确定断言。
- [x] Python fixture 生成器可重复生成测试文件；静态校验和 Rust/xtask 门禁全绿。
- [x] LEDGER 追加；新增事实/坑写入长期记忆。
- [x] 未修改 Out of scope 文件。

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-037 T1.2 全文替换「报表」→「报告」+ 保存（含审批 diff、L0 undo + L1 快照、后置断言）
【目标】交付声明式任务包与评测集，证明替换计数、审批、保存后置断言和 L0/L1 恢复语义
【write scope】仅：adapters/com.microsoft.notepad/tasks/**、eval/tasks/notepad/**（另含本卡执行记录区与收尾文档）
【铁律】1 无静默失败；2 被读文档与 fixture 是不可信输入；6 高风险写必须人工确认；9 不静默扩大范围；10 契约先行
【禁止】crates/**、apps/**、protocol/**、Adapter tools/selectors/rollback/interrupts、真实 GUI、T1.3
【验收】Python 静态校验 + fixture 生成；fmt/clippy/workspace tests；全部 xtask 门禁
【依赖】036 已 Done；已核对 LEDGER
【疑问】无；打开文件沿用已记录的 DRIFT-036-1 platform_open_file 前置动作
```

### 2. 实际改动文件

- `adapters/com.microsoft.notepad/tasks/t1.2.replace-save-approval-undo.json`
- `adapters/com.microsoft.notepad/tasks/README.md`
- `eval/tasks/notepad/t1.2/cases.json`
- `eval/tasks/notepad/t1.2/expected.json`
- `eval/tasks/notepad/t1.2/generate-fixtures.py`
- `eval/tasks/notepad/t1.2/validate.py`
- `tasks/TASK-037-t1-2-replace-save-approval-diff-undo.md`（正文占位展开 + 执行记录区）

### 3. 验收输出摘要

- `python eval/tasks/notepad/t1.2/validate.py` → `t1_2_static_validation_ok cases=11`。
- `python eval/tasks/notepad/t1.2/generate-fixtures.py --output-directory <temp>` → `fixture_count=5` / `old_text_occurrences=8`。
- JSON 任务、cases、expected 均可解析；任务只引用 `read_text` / `replace_text` / `save` 三个已注册工具。
- `cargo fmt --all --check` 0 diff；`cargo clippy --all-targets -- -D warnings` 与 `cargo test --workspace` 全 PASS。
- `hygiene` 0E/4W、`check-migrations` 0E/0W、`refscan` 0E/0W、`memory-counts` 0E/0W、`adr-index` 0E/0W、`docscan` 0E/397W、`card-check` 0E/27W。
- PR #92：**9/9 check-run 全 success**；合并前 `mergeable=MERGEABLE` / `merge_state_status=CLEAN`；merge commit `6e9c85a38b5ac2896b827a4dba7cfe922679d578`。

### 4. DoD 逐条核对

- [x] 任务包可解析，含输入、输出、两段审批、步骤、前置/后置条件、rollback 引用、失败映射和指标。
- [x] 任务只引用现有注册工具；打开文件沿用 `platform_open_file`。
- [x] 11 用例覆盖连续 10 次主场景、多命中、零命中、计数不符、歧义、审批拒绝、保存失败、L0 撤销、L1 恢复与 fallback。
- [x] 审批固定 `show_diff=true` + `once`，高风险保存无 broad scope。
- [x] 替换计数、标题、磁盘读回、L0/L1 锚点和 `error_if_ambiguous` 均有确定断言。
- [x] fixture 生成器、静态校验、Rust 与 xtask 门禁全绿。
- [x] LEDGER、FACT 与 Notepad PITFALL 已同步。
- [x] 未修改 Out of scope 文件。

### 5. 偏差

none。打开文件的 `platform_open_file` 前置动作沿用 **DRIFT-036-1**，本卡不新增偏差，也不修改 Adapter 工具声明。

### 6. 更合理做法

静态校验器把任务步骤中的 `tool` 引用与 `adapters/com.microsoft.notepad/tools/tools.json` 做集合交叉核对，防止任务包通过声明未注册工具“看起来完整、实际不可执行”。撤销语义按持久化边界拆成保存前 L0 与保存后 L1；否则保存后的 `Ctrl+Z` 会被错误当成磁盘恢复成功。

### 7. 遗留问题

- 真实任务执行器、Host 装配、审批接线与真实 Notepad 操作仍不属本声明式任务卡。
- TASK-038 处理新建标签与跨进程 Save As。
- PR / CI / merge 证据已回填；下一张为 TASK-038。

### 8. 新增长期记忆

- FACT：`[2026-09-29][FACT][src:TASK-037 实现实测]` T1.2 声明式任务包把保存前后恢复路径分开验证，并交叉核对注册工具。
- PITFALL（Notepad 档案）：保存后的 `Ctrl+Z` 只撤销内存态；保存后恢复必须使用 L1 同时恢复文档与磁盘。

### 9. 给审阅者的关注点

- 重点审阅保存后的恢复语义：L0 只覆盖保存前，L1 才覆盖磁盘。
- 重点审阅审批形状：替换与保存都必须 `show_diff=true` 且只允许 `once`。
- 重点审阅 `error_if_ambiguous`、替换计数与实际写入之间的 fail-closed 顺序。
