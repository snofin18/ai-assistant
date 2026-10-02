# TASK-219　运行时 pure 步骤与回滚观察字段的真实性修复

- 状态：**Done（2026-10-02，pure 步骤真实执行 + 未知 operation fail-closed + 回滚观察字段真实比较）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-218、TASK-105
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`apps/agent-core/src/task_package_render.rs`、`apps/agent-core/src/notepad_rollback.rs`、`docs/adr/0061-*`、`docs/adr/0062-*`

---

## 目标（一句话）

修掉两处会产生"看起来成功、实际没做"的运行时缺陷：任务包里的 `pure` 步骤被静默跳过，以及回滚结果里的 `editor_matches_anchor` 被硬编码为 `true`。

## 背景（为什么现在做）

第三轮审计实测确认：

1. `apps/agent-core/src/task_package_render.rs` 对
   `pure` + `count_lines_and_keyword_paragraphs` 直接 `declared_not_executed.push(...)` 后
   `return Ok(None)`；`platform` / `l1_file` / `policy` 也静默跳过。T1.1 的 `analyze` 步骤因此
   从未执行，但真实 UIA 报告仍显示 10/10 PASS —— 与 ADR-0061 D5/D6 的 fail-closed 要求冲突，
   也让证据质量被高估。
2. `apps/agent-core/src/notepad_rollback.rs` 的 `execute()` 在结果 JSON 里写死
   `"editor_matches_anchor": true`；而 `verify_after_rollback` 在 `restore_file=false` 时可能
   返回 `Ok(false)` 而不报错，因此该字段可能撒谎。

## write scope

- `apps/agent-core/src/task_package_render.rs`、`apps/agent-core/src/runtime_tools.rs`、
  `apps/agent-core/src/reserved_pure.rs`、`apps/agent-core/src/reserved_invoker.rs`
- `apps/agent-core/src/notepad_rollback.rs`
- `apps/agent-core/tests/task_package.rs`、`apps/agent-core/tests/production_root.rs`
- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`（仅补 analyze 的输入/输出声明）
- `tasks/TASK-219-runtime-pure-step-and-rollback-observability-fix.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅本卡条目标记 + 「当前进度」句）/ `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- 让 `pure` 步骤走完成整的注册/执行链：`tool_for_step` 增加
  `count_lines_and_keyword_paragraphs` 映射，`reserved_pure.rs` 实现该纯 operation
  （输入规范化文本 + keywords + max_keyword_paragraphs，输出 line_count_total /
  line_count_analyzed / keyword_paragraphs / paragraphs_truncated），删除 `render_step`
  里的静默 `return Ok(None)`。
- `platform` / `l1_file` / `policy` 仍可作为任务前置条件，但不得静默吞掉后续引用：
  如果后续步骤引用它们的输出，构造期 fail-closed。
- `notepad_rollback.rs` 的 `verify_after_rollback` 改为返回
  `(editor_matches_anchor, file_matches_snapshot)` 两个真实布尔；`execute()` 的 JSON 用真实值，
  只有 `restore_file=true` 时 file 不匹配才升级为 `Err`。
- 负向用例：未知 `pure` operation fail-closed；`editor_matches_anchor=false` 不再被覆盖；
  T1.1 的 analyze 步骤真实出现在 Plan 中并执行。

## Out of scope（做了算漂移）

- 改 `crates/**` 公共接口、`PlanStep` / `Plan` / `Planner` 形状。
- 引入表达式语言、脚本引擎、新第三方依赖、新 crate。
- 真实 LLM、真实商业 Notepad、Pain/Edge/Excel。

## 必须遵守

- **铁律 1**：未知 `pure` operation 与未覆盖的步骤种类一律显式失败，禁止静默跳过。
- **铁律 4**：步骤有 postcondition；分析输出必须可验证。
- **铁律 9**：只改 write scope 内文件；需要扩 scope 先记 DRIFT。
- **铁律 10**：ADR-0061 D5/D6 与 ADR-0062 已 Accepted，本卡只实现既有契约。
- ADR-0028：写热点文件前 `guard acquire`，写完立刻 `guard release`。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test task_package
cargo test -p assistant-agent-core --test production_root
cargo test -p assistant-agent-core --test production_root_uia -- --ignored --nocapture
cargo run -p xtask -- docscan
```

并附 T1.1 analyze 步骤真实执行、未知 pure operation 拒绝、`editor_matches_anchor=false` 三条负向证据。

## 完成定义（DoD）

- [ ] `count_lines_and_keyword_paragraphs` 作为闭集 `pure` operation 真实执行，输出结构化结果。
- [ ] `render_step` 不再对 `pure` 静默 `return Ok(None)`；未知 operation fail-closed。
- [ ] `platform` / `l1_file` / `policy` 的输出被后续步骤引用时构造期失败。
- [ ] T1.1 真实 UIA 运行中 analyze 步骤出现在 Plan 并提交。
- [ ] `notepad_rollback.rs` 不再硬编码 `editor_matches_anchor`：字段反映真实比较结果。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

【任务】TASK-219 运行时 pure 步骤与回滚观察字段的真实性修复
【目标】让 T1.1 的 analyze 真正执行、删除静默 skip 改 fail-closed，并让 `editor_matches_anchor` 反映真实比较
【write scope】`apps/agent-core/src/{task_package_render,runtime_tools,reserved_pure,reserved_invoker,notepad_rollback}.rs`、`apps/agent-core/tests/{task_package,production_root,production_root_uia}.rs`、`adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`、本卡与状态同步文件
【铁律】1 无静默失败；4 写操作有 postcondition；9 不扩 scope；10 只实现既有 ADR-0061 D5/D6 / ADR-0062
【禁止】改 `crates/**` 公共形状；加表达式/脚本/依赖；真实 LLM/商业应用
【验收】`fmt` / `clippy -D warnings` / `test --workspace` / `task_package` / `production_root` / 真实 `production_root_uia` / `docscan`
【依赖】TASK-218 已合并 `3fbe275`；TASK-105 已合并（LEDGER 核对）
【疑问】无（analyze 需补 `text: $text` 参数以消费前序输出，属实现必需，不改公共形状）

### 2. 实际改动文件

- `apps/agent-core/src/runtime_tools.rs`：新增 `TOOL_PURE_COUNT_LINES_AND_KEYWORD_PARAGRAPHS`，加入闭集、`tool_for_step` 映射与 Planner schema（声明数 8→9）。
- `apps/agent-core/src/reserved_pure.rs`：实现 `count_lines_and_keyword_paragraphs` 纯 operation（行数 + 段落关键词匹配，输出 `line_count_total` / `line_count_analyzed` / `keyword_paragraphs` / `paragraphs_truncated`），并抽出参数解析 helper。
- `apps/agent-core/src/reserved_invoker.rs`：把该工具分派到新 handler。
- `apps/agent-core/src/task_package_render.rs`：删除 `pure` + `count_lines_and_keyword_paragraphs` 的静默 `return Ok(None)`，未知 pure operation 现经 `resolve_step_tool` fail-closed；`platform` / `l1_file` / `policy` 仍是前置（其输出若被后续引用会因未注册而构造期失败）。
- `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`：`analyze` 增加 `text: $text` 参数与四个输出声明。
- `apps/agent-core/src/notepad_rollback.rs`：`verify_after_rollback` 返回 `(editor_matches_anchor, file_matches_snapshot)` 真实布尔；`execute()` 用真实值，不再硬编码。
- `apps/agent-core/tests/{task_package,production_root,production_root_uia}.rs`：更新锁定旧行为的断言，并新增"未知 pure operation 必须拒绝"负向用例。

### 3. 验收输出摘要

```text
cargo fmt --all --check                         -> clean
cargo clippy --all-targets -- -D warnings       -> EXIT 0
cargo test --workspace                          -> EXIT 0
cargo test -p assistant-agent-core --test task_package   -> 15 passed（含未知 pure operation 负向）
cargo test -p assistant-agent-core --test production_root -> 13 passed
真实 UIA T1.1 / T1.2 / T1.3                      -> 各 1 passed（T1.1 现含 analyze，2 snapshots）
三个 validate.py / xtask docscan                 -> PASSED
```

### 4. DoD 逐条核对

- [x] `count_lines_and_keyword_paragraphs` 作为闭集 `pure` operation 真实执行，输出结构化结果。
- [x] `render_step` 不再对 `pure` 静默 `return Ok(None)`；未知 operation fail-closed（新负向用例）。
- [x] `platform` / `l1_file` / `policy` 的输出被后续步骤引用时构造期失败（未注册输出 → `MissingInput`）。
- [x] T1.1 真实 UIA 运行中 analyze 步骤出现在 Plan 并提交（2 snapshots）。
- [x] `notepad_rollback.rs` 不再硬编码 `editor_matches_anchor`：字段反映真实比较。
- [x] 未修改 Out of scope 文件。

### 5. 偏差

**DRIFT-219-1（更新锁定旧行为的测试断言）**

1. **现象**：`test_t1_1_plan_only_contains_tool_steps` 断言"只有 read 工具进 Plan"、`test_t1_1_plan_records_the_declared_not_executed_kinds` 断言 `pure` 在未执行清单、`production_root` / `production_root_uia` 断言 `snapshots.len()==1` 且含 `analyze` 的包会失败。
2. **影响**：这些断言**正是**旧静默 skip 行为的锁；不更新它们本卡无法通过。属漂移触发器 ⑦。
3. **处理**：因为本卡明确要求删除该静默 skip，断言更新为正确行为（read + analyze 都在 Plan、未执行清单只剩 `platform`/`l1_file`、2 snapshots）；**同时新增**"未知 pure operation 必须拒绝"的负向用例，确保不是单纯放宽。
4. **限制**：只动了这四处旧断言；其余断言未变。

### 6. 更合理做法

把"可选分析步骤"做成真正可执行的闭集 `pure` operation，比让它静默消失更符合"要么成功要么显式失败"。T1.1 现在确实产出 `line_count_total` / `keyword_paragraphs`。

### 7. 遗留问题

- T1.1 的 `read_file_channel`（大于 `max_text_bytes` 的 L1 降级路径）仍不在运行时覆盖集内；大文件路径留待后续。

### 8. 新增长期记忆

- **FACT**：T1.1 `analyze` 现为真正执行的 `pure` operation（`assistant.runtime.pure_count_lines_and_keyword_paragraphs`）；纯步骤被静默 skip 的旧行为已删除，未知 pure operation 在构造期拒绝。

### 9. 给审阅者的关注点

1. 本次更新了四处锁定旧行为的断言（DRIFT-219-1）；请重点核对这些断言是否只反映"analyze 现在执行"，没有掩盖失败。
2. `count_lines_and_keyword_paragraphs` 用 `text.lines()` 计行、按空行分段落、大小写敏感的子串匹配；与 T1.1 声明一致。
3. `editor_matches_anchor` 现在来自回滚后真实重读。
