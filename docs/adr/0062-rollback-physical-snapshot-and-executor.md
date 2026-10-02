# ADR-0062　回滚物理快照与执行器接线

状态：**Accepted**（2026-10-02 人类「继续往下做吧」接受 TASK-218 及其 ADR-0062）
日期：2026-10-02
Supersedes：—
Superseded by：—
关联：**TASK-218**、**TASK-105 `DRIFT-105-4` / §7**、TASK-024、TASK-103、
ADR-0047、ADR-0059、ADR-0060、`docs/spec/runtime-execution.md`、`crates/undo/**`

---

## 背景

`prepare_rollback_anchors` 目前只校验 anchor 方案合法性：它检查
`required_levels`、`replace_recipe` / `save_recipe` 非空并读取前序 fingerprint，但
**不创建任何物理快照，也不执行任何回滚**。`crates/undo` 已提供完整契约
（`Anchor` / `RollbackRecipe` / `execute_rollback` / `RollbackExecutor` / 冲突检测 /
incident），但 binary 层没有任何实现把它接到 Notepad 平台动作上。

后果：T1.2 任务包声明的 `undo_l0_before_save` / `undo_l1_after_save` /
`undo_l1_when_l0_unavailable` 三条撤销路径没有可观测证据，TASK-105 的「撤销与验证
必须走实际链路」DoD 不能勾，阶段 1a 维持 NO-GO。

直接放开 `verify_postconditions` 或把回滚降级为静态断言会制造静默失败：
「声称可撤销」而实际不能回到锚点，比明确报错危险得多。

## 决策（一句话）

在 binary 层为 1a 补齐 **物理快照捕获 + `crates/undo` 回滚执行器 + 回滚后复核**：
`prepare_rollback_anchors` 在读取前序 fingerprint 的同时**真实捕获**编辑区规范化文本
与目标文件原始字节并记录 SHA-256；新增保留运行时步骤执行回滚，把
`UndoStack` 映射到 Adapter 声明的 `Ctrl+Z`、把 `RestoreContentSnapshot` /
`RestoreShadowCopy` 映射到目标文件字节恢复，回滚后必须同时复核
**内存文本等于 pre-replace anchor** 且 **磁盘字节 digest 等于 pre-save snapshot**。

## 决策细化

| # | 内容 |
|---|---|
| **D1 捕获时机** | 物理捕获发生在 `assistant.runtime.prepare_anchors` 执行时，即任何写步骤之前。缺编辑区文本、缺目标文件路径、文件读失败 → fail-closed，不进入 replace/save。 |
| **D2 捕获内容** | 捕获 ① 编辑区规范化文本（`\r\n`/`\r` → `\n`）与其 SHA-256；② 目标文件原始字节与其 SHA-256；③ 捕获时的 fingerprint。digest 复用 `assistant_storage::BlobId::of_content`，不新增依赖。 |
| **D3 注册表** | anchor 注册表在 binary 层内存中按 `task_id` 索引，存进程内；它是一次运行的工作状态，不写库、不跨进程传递句柄或元素。 |
| **D4 L0 执行** | `RollbackAction::UndoStack` 映射到 Adapter 显式声明的 undo 路径（Notepad 为 `Ctrl+Z`）；执行后读回规范化文本，必须等于 anchor 文本。 |
| **D5 L1 执行** | `RollbackAction::RestoreContentSnapshot` / `RestoreShadowCopy` 映射到目标文件字节恢复：用捕获的原始字节写回，并复核写回后的 SHA-256。内存文本同步恢复到 anchor 文本。 |
| **D6 回滚后复核** | 回滚成功 = 内存文本等于 pre-replace anchor **且**（目标文件存在时）磁盘 digest 等于捕获 digest。任一不满足 = 失败，不得返回成功。 |
| **D7 冲突与缺失证据** | 复用 `crates/undo` 的 `detect_conflict` 与 `execute_rollback`；快照缺失、digest 不匹配、回滚期间用户改动一律产生 incident 并停止后续自动化，不得猜成功。 |
| **D8 L0 fallback** | L0 不可用或 undo 后未回到 anchor 时走 L1 字节恢复，结果 evidence 标注 `used_fallback=true`。 |
| **D9 形状不变** | 不改 `crates/undo` / `crates/task-engine` / `crates/verify` 的公共接口或类型，不改 `PlanStep` / `Plan` / `Planner` 形状，不新增 crate 或第三方依赖。 |
| **D10 无人值守边界** | 回滚不是新的放行点：它只在任务已声明 anchor 且人工授权按既有 `once` 语义消费后执行；`point_of_no_return` 与不可逆动作仍按铁律 6 拒绝无人值守。 |

## 被否决的选项

| 选项 | 结论 | 理由 |
|---|---|---|
| 保持现状（只校验方案） | ❌ | TASK-105 的撤销 DoD 无法勾，且「已准备 anchor」是假信号（静默失败）。 |
| 只做 L0 `Ctrl+Z`、不做 L1 字节恢复 | ❌ | 记事本保存后 `Ctrl+Z` 不能撤销已落盘内容（`docs/memory/apps/notepad.md` §6 坑 11）；只做 L0 会报告错误结果。 |
| 把回滚实现塞进 `crates/undo` | ❌ | `crates/undo` 的边界明确为「无平台 API / 无文件系统」；执行器必须由 binary 层注入。 |
| 新增 `PlanStep` 字段承载 anchor 数据 | ❌ | `crates/**` 公共形状变更，波及 Planner、TaskEngine、Executor 与全部既有测试。 |
| 用自由字符串断言表达回滚后复核 | ❌ | 与 ADR-0047 冲突；复核必须是结构化字段比较。 |
| 把回滚期间用户改动视为可自动覆盖 | ❌ | 违反「撤销冲突 100% 被检测」与铁律 1；默认最保守，冲突即 incident。 |

## 影响

- TASK-218 可在本 ADR Accepted 后开工；TASK-105 撤销 DoD 在其完成并回填后可勾。
- `apps/agent-core` 新增物理快照与回滚执行实现；`fixtures/apps/notepad-like` 仅做运行必需的最小适配。
- `docs/audits/stage-1a-runtime-validation-*.md` 新增三条 undo 路径的真实运行证据。
- **不改** `crates/**` 公共接口、不加依赖、不新建 crate；阶段 1a 在撤销链闭环前仍为 NO-GO。

## 验证方式

1. T1.2 真实运行中 L0 保存前回滚成功，回滚后编辑区规范化文本等于 pre-replace anchor。
2. T1.2 真实运行中 L1 保存后恢复成功，回滚后内存文本与磁盘字节都等于 pre-replace anchor。
3. L0 不可用时 L1 fallback 成功，evidence 标注 `used_fallback=true`。
4. 快照缺失、digest 不匹配、回滚期间用户改动三类均产生 incident 而非成功。
5. `crates/undo` / `task-engine` / `verify` 公共形状不变；`cargo test --workspace` 全绿。
