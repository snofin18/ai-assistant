# TASK-219　运行时 pure 步骤与回滚观察字段的真实性修复

- 状态：**Ready（2026-10-02，第三轮审计发现的剩余真实缺陷）**
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

（待 Implementer 领取本卡时填写。）

### 2. 实际改动文件

（无 —— 本卡尚未开工。）

### 3. 验收输出摘要

（无 —— 本卡尚未开工。）

### 4. DoD 逐条核对

（待 Implementer 填写。）

### 5. 偏差

（无 —— 本卡尚未开工。）

### 6. 更合理做法

（待评估。）

### 7. 遗留问题

- 本卡来自第三轮审计；T1.1 的 `analyze` 步骤此前被静默跳过，是真实证据被高估的直接原因。

### 8. 新增长期记忆

（待 Implementer 填写。）

### 9. 给审阅者的关注点

（待 Implementer 填写。）
