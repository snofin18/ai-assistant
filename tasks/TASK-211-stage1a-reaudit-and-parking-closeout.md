# TASK-211　阶段 1a 复验准备与停车位收口

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：102~105、087、210
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：TASK-039、`docs/audits/stage-1a-integration-audit-2026-09-29.md`、PL-018 / PL-056 / PL-058 / PL-092 / PL-094

## 目标

在所有补救卡完成后，更新阶段 1a 的停车位状态、汇总新证据，并准备一次
`stage-1a-integration-audit` 复验；本卡只做治理和复验，不实现产品能力。

## In scope

- `docs/PARKING_LOT.md` 的状态更新和关闭记录。
- `docs/audits/**` 的复验准备清单或新审计入口。
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`LEDGER.md` 的状态同步。
- 本卡记录。

## Out of scope

- 修改产品代码、CI workflow、测试或依赖。
- 在未完成依赖卡前提前宣布 1a 通过。
- 删除历史缺口记录。

## 必须遵守

- 只关闭有实际证据的 PL；先核对新增验证输出。
- PL-018、PL-056、PL-058 必须各自有明确证据后再更新。
- 1a 复验仍由独立会话执行。
- 不把“计划完成”写成“能力已通过”。

## 验收命令

```powershell
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-ledger
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
```

## 完成定义（DoD）

- [ ] PL-018 / PL-056 / PL-058 的证据矩阵完整。
- [ ] 未闭环项保持 open，不被误关。
- [ ] 新审计复验清单和入口明确。
- [ ] PLAN / README / plans / LEDGER 同步。
- [ ] 验收命令全部通过。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
