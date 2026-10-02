# TASK-218　Notepad L0/L1 物理快照创建与回滚执行验收

- 状态：**Ready（待人类确认；DRIFT-105 撤销链的落地卡）**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：TASK-105（真实运行证据）、TASK-024、TASK-103
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`tasks/TASK-105-notepad-t1-runtime-validation.md` §7、`docs/audits/stage-1a-runtime-validation-2026-10-02.md` §6、`crates/undo/**`、`adapters/com.microsoft.notepad/tasks/t1.2.replace-save-approval-undo.json`

---

## 目标（一句话）

把 T1.2 声明的 L0 `Ctrl+Z` 与 L1 disk snapshot 从"只校验方案"推进到"真实创建物理快照并执行回滚、回滚后复核内容与磁盘一致"。

## 背景（为什么现在做）

TASK-105 的 3×10 真实运行已证明主路径可执行，但 `prepare_rollback_anchors` 目前只校验 anchor 方案合法性，不创建物理快照，也没有任何执行器真正调用 `crates/undo` 的
`RollbackExecutor`。因此 T1.2 任务包声明的 `undo_l0_before_save` /
`undo_l1_after_save` / `undo_l1_when_l0_unavailable` 三条撤销路径没有可观测证据，
TASK-105 的撤销 DoD 不能勾。

`crates/undo` 已提供完整契约（`Anchor` / `RollbackRecipe` / `execute_rollback` /
`RollbackExecutor` / 冲突检测 / incident），本卡只补齐 binary 层的物理实现与真实运行测试，不改公共契约。

## write scope

- `apps/agent-core/src/**`、`apps/agent-core/tests/**`
- `fixtures/apps/notepad-like/**`
- `eval/tasks/notepad/**`
- `docs/audits/stage-1a-runtime-validation-*.md`
- `docs/adr/0062-*.md`、`docs/adr/README.md`、`docs/memory/decisions.md`
- `tasks/TASK-218-notepad-rollback-execution-verification.md`（本文件）
- `LEDGER.md` / `PLAN.md`（仅「当前状态」块）/ `README.md`（仅三处）/ `plans/stage-1-pilots.md`（仅本卡条目标记 + 「当前进度」句）/ `docs/PARKING_LOT.md` / `docs/memory/{facts,pitfalls}.md`（仅追加）

## In scope

- 物理快照捕获：在 `prepare_rollback_anchors` 执行时，读取编辑区规范化文本与目标文件原始字节，记录 SHA-256（复用 `assistant_storage::BlobId::of_content`），存进程内 anchor 注册表。
- 回滚执行器：把 `crates/undo` 的 `RollbackAction` 映射到 Notepad 平台动作：
  - `UndoStack` → Adapter 显式声明的 `Ctrl+Z`（`notepad.edit.undo` 或等价声明路径），执行后读回规范化文本比对 anchor；
  - `RestoreContentSnapshot` / `RestoreShadowCopy` → 用捕获的原始字节恢复目标文件并复核 digest；
  - 回滚后复核 `canonical_text == pre_replace_anchor` **且** 磁盘字节 digest == pre-save snapshot digest，二者缺一不算成功。
- 冲突检测接线：`execute_rollback` 的 `detect_conflict` 用捕获的 fingerprint / digest，缺失证据或用户改动一律 incident，不得猜成功。
- 真实入口测试：T1.2 在 `notepad-like` 上完整执行后，按任务包声明的三条 undo 路径分别取证，并落
  `docs/audits/stage-1a-runtime-validation-*.md`。
- 负向测试：L0 不可用时走 L1 fallback；快照缺失、digest 不匹配、用户改动三类必须 fail-closed。

## Out of scope（做了算漂移）

- 改 `crates/undo` / `crates/task-engine` / `crates/verify` 的公共接口或类型。
- 改 `adapters/com.microsoft.notepad/tasks/**` 的任务包声明（除非发现 spec 自相矛盾，走 DRIFT）。
- 真实商业 Notepad、Paint / Edge / Excel、无人值守授权。
- 新第三方依赖、新 crate、新表达式语言。
- 把 L2 compensating action 扩展成通用能力。

## 必须遵守

- **铁律 1**：快照捕获、回滚执行、回滚后复核的每一步失败都要带 `ErrorCode` 显式上报。
- **铁律 4**：每个写操作必须有 postcondition；回滚后的复核就是该 postcondition。
- **铁律 9**：`apps/agent-core` 与 `fixtures/**` 之外的文件不得动；需要扩 scope 先记 DRIFT。
- **铁律 10**：ADR-0062 未 Accepted 前不得写回滚执行实现。
- ADR-0028：写 `LEDGER.md` / `plans/*` / `docs/memory/*` 前先 `guard acquire`，写完立刻 `guard release`。
- ADR-0047：不引入自由字符串断言或表达式求值器。
- 真实运行必须走生产入口（`assemble_production_host` + `RuntimeExecutor`），不得用静态断言替代。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-agent-core --test production_root_uia -- --ignored --nocapture
python eval/tasks/notepad/t1.2/validate.py
cargo run -p xtask -- replay fixtures/recordings/core/notepad-like-basic.json
cargo run -p xtask -- docscan
```

并附至少一次真实 UIA 的完整回滚记录（L0 保存前回滚、L1 保存后恢复、L0 不可用 fallback），
以及快照缺失 / digest 不匹配 / 用户改动三类负向证据。

## 完成定义（DoD）

- [ ] `prepare_rollback_anchors` 真实创建 L0 fallback digest 与 L1 disk snapshot，缺一即失败。
- [ ] T1.2 真实运行中 L0 保存前回滚成功，回滚后编辑区规范化文本等于 pre-replace anchor。
- [ ] T1.2 真实运行中 L1 保存后恢复成功，回滚后内存文本与磁盘字节都等于 pre-replace anchor。
- [ ] L0 不可用时 L1 fallback 成功，且 evidence 标注 `used_fallback=true`。
- [ ] 快照缺失、digest 不匹配、回滚期间用户改动三类均产生 incident 而非成功。
- [ ] 撤销证据落 `docs/audits/stage-1a-runtime-validation-*.md`，TASK-105 撤销 DoD 可勾。
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

- TASK-105 在真实主路径上已 3×10 全绿，但撤销 DoD 仍缺；本卡完成后需回填 TASK-105 的 DoD 与审计报告。

### 8. 新增长期记忆

（待 Implementer 填写。）

### 9. 给审阅者的关注点

（待 Implementer 填写。）
