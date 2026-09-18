# TASK-002　Spike A：Notepad UIA 实测 + 接口考古

- 状态：**InProgress**
- 阶段：0　Spike：**A**　依赖：001　预估：2 天　阻塞主线：是
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-0-spikes.md`。

---


- 依赖：TASK-001　预估：2 天　难度：M
- **write scope**：`spikes/spike-a-notepad/**`、`docs/spike-reports/SPIKE-A.md`、
  `docs/memory/apps/notepad.md`（追加/更新）、`docs/memory/{facts,pitfalls,rejected,open}.md`（追加）、
  `LEDGER.md`（追加）。
  > **ADR-0021 修订（2026-09-18）**：原为 `MEMORY.md`（追加）。记忆已分层，`MEMORY.md` 是 L0 索引、
  > 由 Orchestrator 维护，**不在本卡 write scope 内**；条目一律进 L1/L2（应用专属 → `apps/notepad.md`）。
- **Out of scope**：`crates/platform/windows` 的任何产品代码；Paint/Excel 相关

**步骤**
1. 环境记录：Windows 版本（24H2/25H2）、记事本版本（`Get-AppxPackage Microsoft.WindowsNotepad`）、DPI 与显示器配置、输入法状态
2. 控件普查：编辑区、标签项、菜单、状态栏、"另存为"对话框的 `ControlType` / `AutomationId` / `ClassName` / `Name` / 支持的 Pattern
   > **裁决修订（2026-09-18，ADR-0024 D3，人类指示 #8）**：**不装 Accessibility Insights for Windows**，
   > 改用 **SDK `inspect.exe`（已就绪：`C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\inspect.exe`）
   > + 自写 UIA 树导出**（`probe-01-tree-survey.ps1` 即该导出器）。理由：前者输出**不可 diff、不可计时、
   > 不可重复**，对 AI agent 协作无增益。普查结果必须**同时**写进 `docs/memory/apps/notepad.md` §3。
   > 官方 `winapp ui inspect` CLI 列为候选，试用归 TASK-003（ADR-0024「重新评估触发条件」）。
   > **已完成部分**：窗口 / 编辑区 / 标签 / 菜单栏 / 状态栏（32 节点树，见 SPIKE-A §2 与 `apps/notepad.md` §3）。
   > **待完成**：菜单**展开后**的子项、"另存为"**跨进程** Shell 对话框。
3. 用 Rust 实现最小验证程序，**只用 `windows` crate 自带的 UI Automation COM 绑定**：
   > **裁决修订（2026-09-18，ADR-0024 D1 + D1a，人类指示 #6）**：**否决**第三方 `uiautomation` crate。
   > 理由：spike 的意义就是**用生产路径去撞墙**；若 spike 也套一层第三方封装，
   > 「Rust 走 COM 是否与 PowerShell 走托管封装一致」这个最大不确定性只会被推迟到阶段 1。
   > **feature 名（最容易记错）**：**`Win32_UI_Accessibility`** —— `Win32_UI_UIAutomation` **不存在**；
   > 另需 `Win32_System_Ole`（`VARIANT` 被双重 gate）。逐条 feature 理由见
   > `spikes/spike-a-notepad/Cargo.toml` 的行内注释与 ADR-0024 D1a。
   > 依赖已登记 `docs/DEPENDENCIES.md`；受 `ci.yml` 的 **spike-deny**（gov §5.1 #8b）门禁约束。
   - 枚举记事本窗口 → 解析编辑区元素
     > ✅ **已完成**：`spikes/spike-a-notepad/src/bin/uia_dep_proof.rs`（E1~E6 全 PASS，2026-09-18）。
     > 它同时实证了 ADR-0022 D1（启动 PID ≠ 窗口属主 PID）与 ADR-0023（UIA 返回裸 CR）。
   - `ValuePattern.GetValue` / `TextPattern` 读全文；`SetValue` 写文本
     > 读全文 ✅ 已完成（`uia_dep_proof` E4）；**`SetValue` 写路径仍待做**（目前只在 PowerShell 侧验证过）。
   - 触发菜单动作（新建标签、保存）　⏳ 待做
   - 处理"另存为"跨进程对话框（定位 Shell 进程的对话框与文件名输入框）　⏳ 待做
   - **带 `CacheRequest` 的批量取属性对照**　⏳ 待做（ADR-0024 D1a 已确认绑定存在；`docs/memory/open.md` N2）
   > **胶水代码的去向**：spike 里的 UIA 胶水必须写成**可整体搬走**的独立模块，
   > 文件头注明 `// 去向：crates/platform/windows/src/uia/`（ADR-0024 D1）。`uia_dep_proof.rs` 已照此组织。
4. **接口考古 8 步**（feasibility §5）：确认记事本是否有 L1 通道（结论预期：仅"文件契约"，即直接读写 `.txt`），记录结果
5. 实测指标（每项至少 10 次取中位数）：
   - 元素定位成功率、全窗口树遍历耗时、局部搜索耗时
   - 大文件（1 KB / 100 KB / 1 MB）读写耗时与内存
   - 中文文本写入的正确性：**`SetValue` 路径只需单态验证**；**IME 开/关两态仅针对 L4 合成键盘输入路径**
     > **裁决修订（2026-09-18，`DRIFT-002-1` 已裁决，人类指示 #4）**：原措辞要求 `SetValue` 分 IME 两态测。
     > 实测表明 `SetValue` 走 UIA Pattern、**不经过键盘与 IME**，两态对照对它是**空操作**
     > （证据：SPIKE-A §4.4、`docs/memory/apps/notepad.md` §6 坑 4）。
     > 人类裁定：**先按"无影响"的版本改卡面**，日后若发现例外再改回。
     > 因此本项拆成两条：① `SetValue` + 中文 → 单态验证（**已完成**：probe-04 与 `uia_dep_proof` E4，CJK 无损）；
     > ② L4 合成键盘 + 中文 → IME 开/关两态（⏳ 待做，属 L4 路径验证）。
   - 跨进程对话框解析成功率
6. 失败注入：记事本被用户关闭 / 最小化 / 在另一虚拟桌面 / 未保存弹窗出现时，观察各调用的行为与耗时

**go 判据（全部满足）**
- 关键控件（编辑区、标签、菜单项、另存为文件名框）定位成功率 **≥ 90%**
- 全窗口树遍历 **≤ 800 ms**（或局部搜索 ≤ 200 ms）
- 1 MB 文本读取 **≤ 2 s** 且内存增量 ≤ 100 MB
- 中文写入 **100% 正确**：`SetValue` 路径单态验证（✅ 已达成，probe-04 + `uia_dep_proof` E4）；
  **L4 合成键盘路径**才需要 IME 开/关两态均 100% 正确（`DRIFT-002-1` 已裁决，2026-09-18）
- 跨进程对话框解析成功率 **≥ 90%**

**no-go 后果**：记事本降为 T3 → 阶段 1a 换目标（候选：Windows 设置 `ms-settings:` + UIA，或 7-Zip CLI+UIA）；同时重估整个 L3 通道的可行性。

**报告必须包含**：环境、控件普查表、实测数据表、发现的坑、结论（go/no-go + 理由）、对架构 v2 的修正建议（若有，走 ADR 提案，**不得自行改 spec**）。
> **ADR-0021 修订**：「发现的坑」的去向由 `MEMORY.md` §5 改为 —— **应用专属**进
> `docs/memory/apps/notepad.md` §6，**跨应用**进 `docs/memory/pitfalls.md`；
> 新事实进 `docs/memory/facts.md`，被否决的方案进 `docs/memory/rejected.md`。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执（**动手前**填，`AGENTS.md` §3 的固定格式）

```text
【任务】TASK-002 <标题>          【目标】<一句话>
【write scope】仅：<文件清单>     【铁律】<本卡最相关 3~6 条>
【禁止】<本卡 Out of scope 要点>  【验收】<命令> → <期望>
【依赖】<前置卡号，已核对 LEDGER>  【疑问】<有则列出+你的默认处理；无则写"无">
```

> 回执与正文区不符 → 上下文已污染 → **请人类重开会话**（比纠正更省成本）。

### 2. 实际改动文件（逐个核对是否在 In scope 内）

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

### 4. DoD 逐条核对

### 5. 偏差：none / DRIFT-0NN-x（全文按 gov §4.3 格式：现象 / 影响 / 建议 / 已停工作）

### 6. 实施中发现的更合理做法（非漂移，已直接落地 + 理由）

### 7. 遗留问题（进 `docs/PARKING_LOT.md` 的编号）

### 8. 新增长期记忆（FACT / PITFALL / REJECTED 条目原文；无则写"无"）

### 9. 给审阅者的关注点（风险最高的 1~3 处）
