# 阶段 1a 运行验收（TASK-105）— 2026-10-01：T1.1 ×10 已取，T1.2 / T1.3 受阻

> 类型：阶段 1a **运行验收证据**（TASK-105）
> 上位：`tasks/TASK-105-notepad-t1-runtime-validation.md`、`docs/spec/runtime-execution.md`、
> ADR-0058、`docs/automations/2026-09-30-report.md`
> 性质：**部分结论**。T1.1 有实测证据；T1.2 / T1.3 **没有**证据，原因是运行时缺口（见 §2），
> 不是"跑失败"。**本文件不改变阶段 1a 的 NO-GO 结论。**

---

## 1. T1.1 × 10（读全文）：10/10 通过

**命令**（单次）：`cargo test -p assistant-agent-core --test production_root_uia -- --ignored --nocapture`

**执行**：同一命令连跑 10 次（2026-10-01），逐次记录：

```text
run  1 : PASS (1.1s)      run  6 : PASS (1.0s)
run  2 : PASS (1.0s)      run  7 : PASS (1.0s)
run  3 : PASS (1.0s)      run  8 : PASS (1.0s)
run  4 : PASS (1.0s)      run  9 : PASS (1.0s)
run  5 : PASS (1.0s)      run 10 : PASS (1.0s)
T1.1 dry-run: pass=10 fail=0；均值 1.02s
```

**这条证据真正证明了什么**（读 `apps/agent-core/tests/production_root_uia.rs` 可复核）：

1. 用 `powershell.exe` **真实启动** `fixtures/apps/notepad-like/notepad-like.ps1`，等它写出
   `status = "ready"` 的 state file；
2. 生产根从**仓库内适配包** `adapters/com.example.notepad-like/` 读 target 描述；
3. 走真 `WindowsPlatform` 的 **UIA** 定位主窗口与 `EditorTextBox`；
4. 经**真 MCP `ToolBus`** 调用 `notepad.file.read_text`，返回信封带合法 `fingerprint`；
5. `verify_postconditions_with_receipt` 铸造 receipt，由 task-engine 提交；
6. 断言最终 `TaskStatus::Completed` 且 `snapshots.len() == 1`。

**这条证据没有证明什么**（不要让下个会话误读）：

- 没有覆盖**审批**（`AllowWithConfirmation` → HITL）路径；
- 没有覆盖 **point-of-no-return**、**撤销 / 回滚**、**异常恢复**；
- 没有覆盖 `notepad.file.replace_text` / `save` / `tab.new` / `save_as` 这四个工具；
- 因此**不能**用来主张 1a 的 DoD 达成。

## 2. T1.2 / T1.3：**没有可运行对象**（三个根因，全部在 TASK-105 的 write scope 之外）

### 2.1 任务包里的 `$input.` 绑定没有解析层

ADR-0058 D2 的确定性 Plan 来源（`apps/agent-core/src/task_package.rs`）**刻意拒绝**
仍带 `$input.` 占位的参数（`TaskPackageError::UnboundArguments`，fail-closed，见 TASK-214）。
而两份任务包的 tool 步骤本来就是未绑定的：

```text
T1.2  notepad.file.replace_text  {"old_text": "$input.old_text", "new_text": "$input.new_text",
                                  "expected_replacements": "$input.expected_replacements"}
T1.3  notepad.file.save_as       {"target_path": "$normalized_target_path", "overwrite_existing": false}
```

T1.1 的 `notepad.file.read_text` 步骤 `args = null`，所以它能渲染成 Plan —— 这正是
T1.1 能跑、T1.2/T1.3 跑不起来的直接原因。要修需要一个**任务输入绑定层**（把 `$input.*` /
`$normalized_*` 解析成具体值），落在 `apps/agent-core`，**不在 TASK-105 的 write scope**。

### 2.2 运行时只执行 `kind = "tool"` 的步骤

T1.2 / T1.3 的步骤里还有 `platform` / `host_service` / `pure` / `hitl` / `verify` / `l1_file`
等种类，生产根目前**只把 `kind = "tool"` 的步骤放进 Plan**。于是：

- `hitl | request_approval`（T1.2 两次、T1.3 一次，含 `point_of_no_return: true`）**从不执行**
  → TASK-105 DoD 的「审批与 point-of-no-return 行为符合声明」**没有任何可观测对象**；
- `host_service | prepare_rollback_anchors` 不执行 → **撤销 / 锚点证据同样没有对象**；
- `verify | verify_postconditions` 不执行（`RuntimeExecutor` 自己按 PlanStep 的
  `postconditions` 验证，这没错，但任务包里那套 `set` 级断言没有落地）。

### 2.3 `notepad.tab.new` 被硬编码为 fail-closed

T1.3 的第一个 tool 步骤是 `notepad.tab.new`，而 handler 里写着
`tab_count_increased_by_one cannot be observed through the current ...`（`notepad_handlers.rs`
第 490 行附近，并有专门用例 `test_new_tab_fails_closed_without_tab_count_observation`）。
靶机**现在已经有了** `TabCountText`（TASK-215），但 handler 还没去读它 —— 改 handler 同样
落在 `apps/agent-core`，不在 TASK-105 的 write scope。

## 3. 结论

- **T1.1 ×10 = 10/10，静默失败 0**（就 T1.1 而言）。
- **T1.2 / T1.3 = 0 次运行，且不是"跑失败"**：当前运行时无法渲染它们的 Plan（§2.1），
  即使能渲染也缺可执行步骤（§2.2 / §2.3）。硬跑只能得到伪造或静态推算的成功率。
- 因此 TASK-105 的六条 DoD 里 **只有 1 条可判**（静默失败 = 0，仅限 T1.1）：
  「T1.1/T1.2/T1.3 各 10 次」「审批与 point-of-no-return」「成功 + 失败恢复证据」三条不成立。
- **阶段 1a 仍为 NO-GO。**

## 4. 建议的下一步（需人类裁决）

| 选项 | 内容 | 代价 |
|---|---|---|
| **①（推荐）** | 先立「运行时补齐」卡：任务输入绑定层 + `hitl` / `rollback` / `verify` 步骤的落地（或明确它们不进 1a） + `notepad.tab.new` 改为读 `TabCountText` | 一张 L 卡，落在 `apps/agent-core`；做完 TASK-105 才有对象 |
| ② | 缩小 TASK-105 的判据：只验收 T1.1，把 T1.2/T1.3 与审批/撤销证据移到后续里程碑 | 需要人类裁决 DoD；等于 1a 的"三个任务闭环"缩水 |
| ③ | 把 T1.2/T1.3 改为在**真实 Notepad** 上跑 | 与 TASK-105 Out of scope「操作真实商业 Notepad」冲突，且失去确定性 |

## 5. 变更历史

| 日期 | 变更 | 依据 |
|---|---|---|
| 2026-10-01 | 建立本文件：T1.1 ×10 证据 + T1.2/T1.3 三根因 | TASK-105 |
