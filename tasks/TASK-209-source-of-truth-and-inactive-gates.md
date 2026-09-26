# TASK-209　事实源与失效软门禁纠偏

- 状态：**Done**
- 阶段：跨阶段（**治理池 200~299**）　子阶段：—　批次：—（**不在** stage-1 批次表内）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md` §「跨阶段治理卡」；号段依据 **ADR-0037 D1**（200~299 = 治理池）。
- 来源：人类 2026-09-26 确认按审计报告中的低风险事实源整改先做，并明确授权只修事实源与明显失效的软门禁，不改产品代码。

---

## 目标（一句话）

让当前阶段状态、计划完成标记、治理目标清单和 CI 实际启用状态彼此一致；删除三条**永久不能成功执行**的软门禁，避免后续会话继续把它们当作有效绿灯。

## 背景（为什么现在做）

| # | 事实 | 证据 |
|---|---|---|
| 1 | 任务状态存在直接漂移 | `plans/stage-1-pilots.md` 的 TASK-028 条目仍写 Ready，而同文件头部与 LEDGER 均已记 Done |
| 2 | CI 存在永久无效的软门禁 | `replay --suite core` 实测退出 2（未知选项）；`check-comments` 未实现；commitlint 只是 echo 占位 |
| 3 | `continue-on-error` 会把上述失败伪装成正常 CI 结果 | `.github/workflows/ci.yml` 三条软门禁均为 `continue-on-error: true` |
| 4 | gov §5.1 是目标门禁清单，但未区分“已启用”与“待实现” | `docs/governance-ai-agent-execution.md` §5.1 表头写“全部必过”，与当前 CI 实际不一致 |
| 5 | 仍有事项不适合在本卡越权改 | README 构建段 / 许可证段、架构 v2 状态行、任务卡正文状态行分别受 ADR 或 Orchestrator 权限约束 |

## write scope

- `tasks/TASK-209-source-of-truth-and-inactive-gates.md`（本文件）
- `.github/workflows/ci.yml`
- `PLAN.md`（仅「当前状态」块）
- `README.md`（仅状态行 / 当前阶段 / 最近进展）
- `plans/stage-1-pilots.md`（仅完成标记与「当前进度」块；不改条目正文、排期或批次顺序）
- `docs/governance-ai-agent-execution.md`（仅 §5.1 的目标清单 / 当前启用状态说明）
- `docs/adr/README.md`（仅修复空行造成的登记表断表）
- `docs/PARKING_LOT.md`（仅追加）
- `LEDGER.md`（仅追加）

## Out of scope（写了就停）

- 不改任何 `crates/**`、`apps/**`、`protocol/**`、前端或产品行为
- 不实现 `replay` / `commitlint` / `check-comments`
- 不把 `refscan` 151 error 清零或接成硬门禁
- 不改任务卡正文状态行，不裁决 TASK-075 / 076 / 079 的最终状态
- 不改 README 构建段 / 许可证段，不改架构 v2 状态行
- 不新增依赖、不修改 ADR 决策内容、不放宽任何 lint 或门禁判据

## 必须遵守

- 公共热点文件先取得 `guard` 锁，写完立即释放
- 只追加 `LEDGER.md` / `docs/PARKING_LOT.md`，不改写既有行
- `plans/*` 只加完成标记 / 只改「当前进度」块；任务条目正文与排期不动
- CI 删除项只限已证明永久不可执行的误导项；其余软门禁转为硬门禁仍需 ADR + 负向验证
- 不得用“改测试断言”或“放宽判据”让门禁变绿

## 验收命令

```powershell
cargo fmt --all --check
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- check-migrations
cargo run -p xtask -- refscan
git diff --check
```

预期：前 10 条文档 / 测试门禁通过；`refscan` 保持 151 个既有 error，不新增；`git diff --check` 无输出。

## DoD

- [ ] `plans/stage-1-pilots.md` 的 TASK-028 条目带 Done 标记，且不再写 Ready
- [ ] CI 不再执行 replay / commitlint / check-comments 三条永久无效软门禁
- [ ] gov §5.1 明确区分目标清单与当前启用状态
- [ ] `PLAN.md` / `README.md` / `plans/*` / `LEDGER.md` 同步当前状态
- [ ] 不适合本卡越权修的事项已登记 `docs/PARKING_LOT.md`
- [ ] 上列验收命令达到预期，warning / error 不高于既有基线

---

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-209 事实源与失效软门禁纠偏
【目标】让阶段状态、完成标记、治理目标清单与 CI 实际启用状态一致。
【write scope】仅本卡、.github/workflows/ci.yml、PLAN.md 当前状态块、README.md 三处、
plans/stage-1-pilots.md 完成标记与当前进度块、docs/governance-ai-agent-execution.md §5.1、
docs/adr/README.md 断表、docs/PARKING_LOT.md、LEDGER.md。
【铁律】1 无静默失败；9 不得静默扩大范围；10 契约先行；公共热点文件先取 guard 锁。
【禁止】产品代码、ADR 决策内容、任务卡正文状态、新增依赖、放宽门禁判据。
【验收】fmt / test / arch:: / hygiene / memory-counts / adr-index / check-ledger /
card-check / docscan / check-migrations 全通过；refscan 保持 151 既有 baseline。
【依赖】无；已核对 LEDGER 与 PLAN：TASK-028 Done、下一张主线卡 TASK-207。
【疑问】任务卡正文状态与 TASK-075 / 076 / 079 状态裁决仍归 Orchestrator / 人类；
无效软门禁按人类 2026-09-26 确认从 CI 移除，不改其目标门槛地位。
```

### 2. 实际改动文件

- `.github/workflows/ci.yml`：移除 replay / commitlint / check-comments 三条无效软门禁，软门禁计数改为 2。
- `docs/governance-ai-agent-execution.md`：明确 §5.1 是目标清单，并标出当前未启用项。
- `docs/adr/README.md`：删除登记表中的断表空行。
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`：同步 TASK-209 完成状态与 TASK-028 的 Done 标记。
- `docs/PARKING_LOT.md`：登记 PL-093~PL-098，记录不在本卡权限内的真实缺口。
- `tasks/TASK-209-source-of-truth-and-inactive-gates.md`、`LEDGER.md`：本卡记录与台账。

### 3. 验收输出摘要

- `cargo fmt --all --check`：退出 0，0 diff。
- `cargo test --workspace`：退出 0，全部测试通过。
- `cargo test -p assistant-core arch::`：5 passed / 0 failed。
- `cargo run -p xtask -- hygiene`：0 error / 4 warning（既有 `file-too-long` 基线），实现状态仍为 3/13。
- `cargo run -p xtask -- memory-counts`：0 error / 0 warning。
- `cargo run -p xtask -- adr-index`：0 error / 0 warning，scanned 38。
- `cargo run -p xtask -- check-ledger`：0 error / 0 warning。
- `cargo run -p xtask -- card-check`：0 error / 27 warning（既有骨架 warning 基线）。
- `cargo run -p xtask -- docscan`：0 error / 468 warning（修复 ADR 登记表断表后回到既有 warning 基线）。
- `cargo run -p xtask -- check-migrations`：0 error / 0 warning。
- `cargo run -p xtask -- refscan`：151 error / 0 warning，与既有 baseline 一致，本卡新增 0。
- `git diff --check`：无输出。
- 远端分支 CI：commit `11dbe34116e499dfe76ea62609e149deabeb73f2` 的 **8/8 check-runs 全部 success**（三平台 check、cargo deny、spike-deny、doc consistency、gate negative verification、deferred inventory）。

### 4. DoD 逐条核对

- [x] TASK-028 的 plan 条目已由 Ready 改为 Done
- [x] replay / commitlint / check-comments 三条无效软门禁已移出 CI
- [x] gov §5.1 已区分目标清单与当前启用状态
- [x] `PLAN.md` / `README.md` / `plans/*` / `LEDGER.md` 已同步
- [x] 不可越权事项已登记 PL-093~PL-098
- [x] 验收命令与 baseline 已复跑；`refscan` 未新增，其它门禁通过

### 5. 偏差

无新增 DRIFT。发现的三类越权事项按声明范围登记为 PL-093~PL-098，未在本卡修改。

### 6. 更合理做法

门禁状态应以 CI 可执行步骤为 SSOT，目标清单只描述“将来要达到什么”，不要把未实现命令直接挂成 `continue-on-error`。派生计数若要保留，应由机器生成或指向单一事实源。

### 7. 遗留问题

- PL-093：README 构建段 / 许可证段与架构 v2 状态行仍需 Orchestrator 处理。
- PL-094：ADR-0053 的 `core` 依赖白名单缺机器断言。
- PL-095：`codegen --check` 与渲染器共用同一实现，无法发现 schema 与模板同时漏字段。
- PL-096：审计反规范化列未参与链校验。
- PL-097：`task-engine::commit_step` 未在类型上强制验证结果。
- PL-098：元素句柄表累积与 pointer 盲发路径。

### 8. 新增长期记忆

无。新增事实记录在停车位；未改 `MEMORY.md` 或 L1 记忆文件，避免触发规模表与只读边界。

### 9. 给审阅者的关注点

1. 检查从 CI 删除的三条软门禁是否确实全部属于“永久不能成功”，而不是被误删的将来门禁入口。
2. 检查 `plans/*` 是否只改了完成标记和当前进度块，没有改写任务条目正文或排期。
3. 重点复核 PL-094~PL-098 是否准确覆盖审计报告中的结构性风险，并有清晰归属建议。
