# TASK-105　Notepad T1.x 真实运行验收与 10 次证据

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：103、104
- 预估：M　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：TASK-033~038、`fixtures/apps/notepad-like/**`、`eval/tasks/notepad/**`

## 目标

在已经完成真实执行链路与 UI 接线后，让 T1.1 / T1.2 / T1.3 在 `notepad-like`
靶机上真实运行各 10 次，产出成功率、静默失败、撤销、审批和异常处理证据。

## In scope

- `fixtures/apps/notepad-like/**` 中运行必需的最小适配。
- `eval/tasks/notepad/**` 的运行编排与结果记录。
- `docs/audits/stage-1a-runtime-validation-*.md`。
- 本卡记录与必要的 memory 条目。

## Out of scope

- 修改 Core/Host/Policy 实现来规避失败。
- 操作真实商业 Notepad。
- Paint / Edge / Excel 任务。
- 伪造成功率或跳过失败样本。

## 必须遵守

- 每个用例必须从真实入口启动，不使用静态断言替代运行。
- 失败必须保留输入、输出、错误码、时间线与证据。
- 静默失败任何一次即本卡失败。
- 10 次运行结论必须是实测计数，不得从静态用例推算。
- 撤销与验证必须走实际链路。

## 验收命令

```powershell
cargo test --workspace
cargo run -p xtask -- replay fixtures/recordings/core/notepad-like-basic.json
python eval/tasks/notepad/t1.1/validate.py
python eval/tasks/notepad/t1.2/validate.py
python eval/tasks/notepad/t1.3/validate.py
cargo run -p xtask -- docscan
```

并附 3 组各 10 次真实运行结果。

## 完成定义（DoD）

- [ ] T1.1 ×10、T1.2 ×10、T1.3 ×10 均有可复现记录。
- [ ] 静默失败 = 0。
- [ ] 成功路径与失败恢复路径都有证据。
- [ ] 审批与 point-of-no-return 行为符合声明。
- [ ] 结果报告落 `docs/audits/`。
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
