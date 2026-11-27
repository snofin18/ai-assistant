# TASK-416 Automation round delivery and cadence

- 状态：**Review**
- 阶段：跨阶段 governance / automation evidence

---

## write scope

- `tasks/TASK-416-automation-round-delivery-and-cadence.md`
- `docs/automations/codex-automations-operations.md`
- `docs/audits/automation-round-delivery-and-cadence-2026-11-27.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

把“每个 automation 必须实际产出可追溯的
`docs/automations/<run-date>-round-<N>.md`”固化为建任务时的硬前置和建后核对项，
并用近期真实运行数据给出低重叠的槽位间隔建议。

本卡同时记录仓库外 `~/.codex/automations/` 的孤儿目录清理结果。

## In scope

- 在 automation 操作手册中增加 round 文件交付硬门禁：
  - 创建任务时，`automation.toml` 的 prompt 必须包含 round 文件路径、PR/merge
    证据和 `round_file` final 审计字段；
  - 创建成功后立即读取 `automation.toml` 逐项核对；
  - 缺少 round 文件或 round PR/merge 证据时禁止自删；
  - `final` 事件必须记录实际 round 路径与 PR/merge/main CI。
- 依据近期仓库外 JSONL 统计槽位完成时间和计划到最终完成的 wall time。
- 给出不修改 ADR-0046 下限的运行建议，降低相邻槽位重叠概率。
- 清理 `C:\Users\fexfe\.codex\automations` 下没有 `automation.toml` 的孤儿任务目录。
- 同步 `PLAN.md`、`README.md`、`LEDGER.md`。

## Out of scope

- 不重建 2026-11-26 缺失的五个 round 文件，不伪造历史时间戳。
- 不修改 ADR-0046 / ADR-0054 / ADR-0089 / ADR-0099 的已决规则。
- 不修改产品源码、schema、公共 API、ErrorCode 或依赖。
- 不删除 `.run-jitter-salt`、`human-session-reset.v1` 或其他运行时控制文件。
- 不新建、恢复或触发任何真实 automation。

## 必须遵守

1. 历史缺失只能记录，不能用事后补写文件伪装成当时已产出。
2. 新建 automation 时，round 文件硬门禁必须进入 prompt 正文，不能只在本手册中引用。
3. 自删是最后一步，且只能在 round 文件及其 PR/merge/main CI 已进入 `origin/main` 后执行。
4. 清理外部目录前必须验证目标绝对路径位于
   `C:\Users\fexfe\.codex\automations` 内，并确认目录没有 `automation.toml`。
5. 本卡只改文档和仓库外运行目录，不触碰产品代码。

## 验收

```powershell
rg -n "round_file|round 文件|禁止自删|3h30|17:15" docs/automations/codex-automations-operations.md
cargo run -p xtask -- automation-rounds --run-date 2026-11-26
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- check-ledger
git diff --check
```

预期：手册包含 round-file 硬门禁；`automation-rounds` 仍如实报告历史 round
缺失；文档门禁全绿；外部目录只剩两个运行时控制文件。

## DoD

- [ ] automation 手册包含可直接复制到 prompt 的 round 文件硬门禁。
- [ ] 建后核对明确要求读取 `automation.toml`，缺失硬门禁不得保留 `ACTIVE`。
- [ ] 审计报告包含近期槽位数据、最大运行时长和间隔建议。
- [ ] 历史 round 缺失仍保持缺失，不伪造回填。
- [ ] 外部 automation 孤儿目录清理完成，控制文件保留。
- [ ] 文档门禁全绿并完成 PR / CI / merge。

---

## 执行记录

### 1. 约束回执

```text
【任务】TASK-416 Automation round delivery and cadence
【目标】固化 round 文件硬前置，统计近期槽位时长并清理孤儿 automation 目录
【write scope】仅文档、任务卡、PLAN/README/LEDGER；仓库外只清理无 automation.toml 的孤儿目录
【铁律】无静默失败；证据不得伪造；资源清理路径必须验证；文档同步；不越权改 ADR
【禁止】不重建历史 round；不改产品代码/schema/API/依赖；不删除运行时控制文件
【验收】手册硬门禁检查 + automation-rounds + hygiene/card-check/docscan/check-ledger + diff check
【依赖】TASK-415 Done；2026-11-26 报告与五份 JSONL 已核对
【疑问】无
```

### 2. 实际修改

- `docs/automations/codex-automations-operations.md` 升级到 v1.23，在文件头加入
  每个 automation prompt 必须携带的 round 文件硬门禁，以及建后读取
  `automation.toml` 的核对要求。
- 新增 `docs/audits/automation-round-delivery-and-cadence-2026-11-27.md`，
  记录 2026-11-25 / 2026-11-26 最后两批 10 个槽位的完成时间、3h30 间隔建议和
  2026-11-26 round 文件缺失的事实边界。
- 清理 `C:\Users\fexfe\.codex\automations` 下两个无 `automation.toml` 的孤儿目录；
  保留 `.run-jitter-salt` 与 `human-session-reset.v1`。
- 未重建历史 round 文件，未修改 ADR/spec/产品代码/schema/公共 API/依赖。

### 3. 验收记录

- `cargo run --quiet -p xtask -- automation-rounds --run-date 2026-11-26`：
  `existing_rounds_for_run_date=0`、`n_auto=0`、`missing_before_latest_human=61`，
  如实保留历史缺失。
- `cargo run --quiet -p xtask -- hygiene`：PASSED，0 error / 261 warning。
- `cargo run --quiet -p xtask -- card-check`：PASSED，0 error / 387 warning。
- `cargo run --quiet -p xtask -- docscan`：PASSED，0 error / 175 warning。
- 外部控制文件：仅保留 `.run-jitter-salt` 与 `human-session-reset.v1`。
- 待提交后再跑 `check-ledger` 与 `git diff --check`，并在 PR/CI 中补证据。

### 4. DoD 核对

- [x] automation 手册包含可直接复制到 prompt 的 round 文件硬门禁。
- [x] 建后核对明确要求读取 `automation.toml`，缺失硬门禁不得保留 `ACTIVE`。
- [x] 审计报告包含近期槽位数据、最大运行时长和间隔建议。
- [x] 历史 round 缺失仍保持缺失，不伪造回填。
- [x] 外部 automation 孤儿目录清理完成，控制文件保留。
- [ ] 文档门禁全绿并完成 PR / CI / merge。

### 5. 偏差

无。

### 6. 新增长期记忆

无。

### 7. 遗留问题与下一步

- 2026-11-26 的 round 文件无法追溯补齐；后续 automation prompt 必须携带本卡固化的
  round-file 硬门禁。
- 近期最长的 planned-to-final 为 202.7 分钟，五槽新建任务建议使用 3h30 间隔；
  该建议不改变 ADR-0046 的 2h 下限。
- TASK-416 完成合并后，下一步继续处理 Stage 2 私有 production save caller /
  Host 边界，不直接开放 production Tool surface。

### 8. 外部证据

- 待提交后回填 implementation PR / CI / merge / main CI。

### 9. 给审阅者的关注点

- 核对 round 文件要求是否真正进入 prompt 正文，而不只是手册说明。
- 核对 3h30 仅作为运行建议，没有被冒充为 ADR 已决硬规则。
- 核对外部清理没有删除重置水位或调度抖动控制文件。
