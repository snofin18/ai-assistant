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

【任务】TASK-105 Notepad T1.x 真实运行验收与 10 次证据
【目标】让 T1.1 / T1.2 / T1.3 在 `notepad-like` 靶机上真实运行各 10 次并产出可复现证据
【write scope】仅：`fixtures/apps/notepad-like/**`、`eval/tasks/notepad/**`、`docs/audits/stage-1a-runtime-validation-*.md`、本卡记录、必要 memory 条目
【铁律】1 无静默失败；4 每个写操作必有 postcondition；8 element/句柄不跨进程；10 契约先行
【禁止】改 Core/Host/Policy 规避失败；操作真实商业 Notepad；Paint/Edge/Excel；伪造成功率或跳过失败样本
【验收】`cargo test --workspace`、`xtask replay`、三个 `validate.py`、`xtask docscan` + 3 组各 10 次真实运行
【依赖】103、104 —— `LEDGER.md` 均已记 Done
【疑问】**无法开工**：本卡的三条硬前置都不存在（见 §5 `DRIFT-105-1`）。默认处理 = 记录阻塞并转 TASK-214，不伪造运行证据。

### 2. 实际改动文件

（无 —— 本卡未进入实现阶段，仅登记阻塞。见 §5。）

### 3. 验收输出摘要

（无 —— 无运行对象可跑。见 §5。）

### 4. DoD 逐条核对

- [ ] T1.1 ×10、T1.2 ×10、T1.3 ×10 均有可复现记录 —— **未做**（无生产装配根 / 无 handler / 无 Plan 来源）
- [ ] 静默失败 = 0 —— **不可度量**（没有一次真实运行）
- [ ] 成功路径与失败恢复路径都有证据 —— **未做**
- [ ] 审批与 point-of-no-return 行为符合声明 —— **未做**
- [ ] 结果报告落 `docs/audits/` —— **未做**
- [x] 未修改 Out of scope 文件 —— 本次只写本卡记录

### 5. 偏差

**DRIFT-105-1（阻塞：本卡无运行对象，不属于"可做但未做"）**

1. **现象**：本卡的 In scope 要求"每个用例必须从真实入口启动，不使用静态断言替代运行"。但截至 2026-09-30，三条前置实体全部不存在：
   - **(a) 生产装配根** —— `apps/agent-core/src/main.rs` 只有 `--self-check`，其文件头自述 *"The production composition root will supply a real Provider, persistent `SessionStore`, and adapter package."*；`HostAssembly`（`assembly.rs` 的 `HostComponents`）不拥有 `TaskEngine` / `RuntimeExecutor` / `ToolRegistry` 内容 / `UiServer` / `SnapshotEventSource`。
   - **(b) Notepad Host handler** —— 生产 `ToolRegistry` 为空（self-check 传 `ToolRegistry::new()`）；handler 只在测试里（`apps/agent-core/tests/runtime_toolbus.rs` 的 `WriteTextHandler`）。ToolBus 上没有任何可调工具。
   - **(c) Plan 来源** —— `Planner` 需要 `Arc<dyn ModelProvider>`，生产实现不存在；`NoopProvider` 只产 usage+stop，产不出 `Plan`。
2. **影响**：本卡的 6 条 DoD 中 5 条无法开工（唯一"不可度量"的一条也没有数据可计）。若强行跑，只能得到伪造或静态推算的成功率 —— 违反铁律 1、本卡「不得从静态用例推算」与 Out of scope「伪造成功率」。**阶段 1a 因此仍为 NO-GO**，且这与 TASK-039 / TASK-211 的结论一致。
3. **建议**：立 **TASK-214**（生产装配根 + Notepad Host handler + 1a Plan 来源），其契约依据为 **ADR-0058（Proposed）**。ADR 需要裁决的关键点是"1a 的 Plan 来源口径"——`docs/memory/open.md` **M3** 已把本地模型选型判给 1c 前，故建议 1a 用确定性「任务包 → `Plan`」的 `ModelProvider`，真实 LLM 另立卡 + 网络/依赖 ADR。
4. **已停工作**：本卡未写任何 fixture / eval 编排 / 审计报告，未产生任何运行证据，未修改 Out of scope 文件。**等待人类接受 ADR-0058 并完成 TASK-214 后再开工。**

**DRIFT-105-2（T1.2 / T1.3 在 `notepad-like` 靶机上没有可执行对象）**

1. **现象**：本卡要求 T1.1 / T1.2 / T1.3 都在 `fixtures/apps/notepad-like` 上真实运行。但该靶机自述「**It does not read or write user files. It does not implement encoding or EOL normalization.**」（`fixtures/apps/notepad-like/README.md`），核对证据：
   - T1.2 要求「替换 + 保存 + 保存后标题无 `*` + L0 undo + L1 快照」。靶机只有一个 `EditorTextBox`，`SaveButton` / `OpenButton` / `SaveAsButton` 在 XAML 里存在但**脚本里没有任何点击处理**（`notepad-like.ps1` 只在 `--fault busy` 分支把它们 `IsEnabled = $false`），点击不会写文件，也没有 `*` 未保存标记。
   - T1.3 要求「新建标签 → 写入 → 另存为到指定路径（**跨进程 Shell 对话框**）」。靶机**没有标签页**，也没有 Shell 对话框；`automation-ids.json` 里只有单个 `EditorTextBox`。
   - 另：`adapters/com.microsoft.notepad/selectors/targets.json` 的候选链针对**真实 Notepad**（class `Notepad` / `RichEditD2DPT` / `Microsoft.UI.Xaml.Controls.TabView` / 另存为 `#32770`），与靶机的 AutomationId（`MainWindow` / `EditorTextBox` / `SaveButton` …）**不匹配**。
2. **影响**：T1.1 的只读路径可以在靶机上跑（窗口 + `EditorTextBox` + 读文本），但 **T1.2 / T1.3 按卡面无法在靶机上取得证据**。硬跑只能得到"按钮点了没反应"或伪造的成功率。
3. **建议（三选一，需人类裁决）**：① **扩靶机**：给 `notepad-like` 加文件读写 / 标签页 / 一个真实的跨进程对话框，并补一份 `com.example.notepad-like` 的 selector 与工具声明（改动落在本卡 write scope 与 `adapters/**`，需另立卡）；② **改判据**：把 T1.2 / T1.3 的真实运行改到**真实 Notepad**（与 `TASK-105` Out of scope「操作真实商业 Notepad」冲突，需裁决）；③ **拆卡**：T1.1 先在靶机收口，T1.2 / T1.3 各自落到能提供对应能力的靶机上。**推荐 ①**（保持"不碰商业应用 + 确定性"两条既有约束）。
4. **已停工作**：本卡仍不产生任何运行证据；阶段 1a 维持 **NO-GO**。

### 6. 更合理做法

先补"可运行对象"再谈"运行验收"。原 WBS 把 TASK-029（装配）当作已交付，但 TASK-029 的 DoD 只要求"装配点可执行 + 缺组件 fail-closed"，所以它交付的是 self-check 而非生产根；这两者在 WBS 上同名不同物，是本次阻塞的结构性根因。

### 7. 遗留问题

- **TASK-214 未完成前本卡不可开工**（ADR-0058 已于 2026-09-30 Accepted，TASK-214 已解锁；本行随 TASK-214 完成而失效）。
- **`DRIFT-105-2` 未裁决前，T1.2 / T1.3 没有可执行对象**：靶机 `notepad-like` 不读写文件、无标签页、无跨进程对话框，selector 也与 `adapters/com.microsoft.notepad` 不匹配。推荐方案 ①（扩靶机 + 补 `com.example.notepad-like` 适配声明），需另立卡。
- `PLAN.md` 当前状态块里「真实 ModelProvider」的措辞需同步澄清为「1a = 确定性任务包 provider；真实 LLM 归 1c 前」（ADR-0058 已接受，待 TASK-214 收口时一并改）。
- 靶机 `notepad-like` 的 UIA 通道需在 TASK-214 里被真实驱动一次（当前只有 TASK-033 的自测脚本）。

### 8. 新增长期记忆

（无长期记忆新增 —— 阻塞本身记在 §5 与 `docs/PARKING_LOT.md`；若 ADR-0058 被接受，再按 ADR 追加 `decisions.md` 条目。）

### 9. 给审阅者的关注点

1. 本卡**没有**产出任何运行证据，请勿把本文件当成 T1.x 已验收的证据。
2. 阻塞根因是 WBS 缺口（无生产装配根），不是执行失败；请优先裁决 ADR-0058 的 Proposal 状态。
3. 若认为"1a 必须包含真实 LLM"，请直接否决 ADR-0058 D2/D3 —— 那会改变 1a 的范围与依赖。
