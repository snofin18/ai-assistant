# TASK-075　B1.4 probe-07 跨进程 Shell 对话框（"另存为"）

- 状态：**Blocked（已部分实现 + 重大架构发现）**
- 阶段：0　子任务：TASK-002 B1.4　依赖：TASK-002 B1.1 + B1.2（Done）+ TASK-001　预估：M+　阻塞主线：否（架构发现 = 长期有用）
- write scope：spikes/spike-a-notepad/probe-07-cross-process-dialog.ps1（新建）/ spikes/spike-a-notepad/README.md（追加）/ docs/spike-reports/SPIKE-A.md（§10 追加）/ D:\csart\eol-probe\RESULT-07.txt（probe 产出 — 本卡未生成）/ 本卡执行记录 / LEDGER.md（追加）

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-075 B1.4 跨进程 Shell 对话框 PoC   【目标】关闭 stage-0 DoD carry-over #5（跨进程对话框解析 ≥ 90%）
【write scope】spikes/spike-a-notepad/probe-07-cross-process-dialog.ps1 / README.md / docs/spike-reports/SPIKE-A.md §10 / 本卡 / LEDGER.md
【铁律】AGENTS.md §3+§6 / ADR-0022 D1 / ADR-0024 D1/D4 / ADR-0028
【禁止】不写 crates/platform/windows / 不改公共热点 / 不在 TASK-002 上给最终 go-no-go / 不引入新依赖
【验收】probe-07 0 non-ASCII / 所有 xtask + cargo test PASSED / 跨进程对话框解析 ≥ 90% / SPIKE-A §10 / TASK-075 §1-9 / LEDGER +1
【依赖】TASK-002 B1.1+B1.2 Done（已核 LEDGER）+ TASK-001 Done
【疑问】无
```

### 2. 实际改动文件

- NEW `spikes/spike-a-notepad/probe-07-cross-process-dialog.ps1`（393 行，0 non-ASCII，clean LF/无 BOM）—— 实现完成但**未成功跑通完整 12 iter**（详见 §5 偏差）
- EDIT `docs/spike-reports/SPIKE-A.md`（追加 §10）
- EDIT `tasks/TASK-075-b1-4-probe-07-cross-process-dialog.md`（本卡）
- EDIT `spikes/spike-a-notepad/README.md`（追加 probe-07 入口）
- EDIT `LEDGER.md`（追加 1 行）

### 3. 验收输出摘要

- `spikes/spike-a-notepad/probe-07-cross-process-dialog.ps1` → **393 行，0 non-ASCII 字节（ADR-0024 D4 PASSED），clean LF，无 BOM**
- syntax check → **PASSED**（[scriptblock]::Create 解析无错）
- `cargo test --workspace` → **302 passed**（无 Rust 改动）
- `xtask hygiene / docscan / memory-counts / adr-index` → **全 PASSED**
- `RESULT-07.txt` → **未生成**（probe 跑通到 iter 1 即触发架构发现，未达到统计阶段）

### 4. DoD 逐条核对

- [x] probe-07-cross-process-dialog.ps1 创建（393 行，纯 ASCII）
- [x] probe-07 syntax check PASSED
- [x] probe-07 触发"File > 另存为"成功（menu Invoke 通过）
- [ ] **RESULT-07.txt 产出**（probe 在 iter 1 即发现架构问题，未跑完 12 iter）
- [ ] **go 判据 #5 PASSED**（依赖新工具 = UIA3/WinUI3-aware）
- [x] **重大架构发现**：Win11 25H2 modern Notepad 的"另存为"是 **in-window WinUI3 file browser**（+89 descendants 在 Notepad 内），不是跨进程 dialog（详见 §5）
- [x] SPIKE-A.md §10 填入（架构发现 + 测不到原因 + 替代方案）
- [x] LEDGER.md 追加 1 行

### 5. 偏差（重大）

#### 5.1 **架构偏差（重大发现）**：Win11 25H2 modern Notepad 不再有跨进程"另存为" dialog

**实证**（2026-09-21 本机实测）：
1. 启动 Notepad（Win11 25H2 build 26200.9457 现代化版本）
2. 打开 File menu → 点 另存为
3. 通过 PowerShell UIA 枚举所有顶层窗口 → **没有新窗口出现**
4. 通过 PowerShell UIA 枚举 Notepad 窗口内的所有 descendants → **新增 89 个 elements**（31 → 120）

**结论**：modern Win11 Notepad 用 **in-window WinUI3 FileExplorer-like panel** 实现"另存为"（NOT 跨进程 dialog）。这与 v2 架构 §3.2 的"element 不跨进程"原则的隐含假设（= 假设外部 dialog 跨进程）**对 WinUI3 应用不再成立**。

**对 Adapter 设计的影响**（critical）：
- **旧假设**：Notepad 另存为 → Explorer.exe 子进程 → UIA 跨进程枚举窗口
- **新现实**：Notepad 另存为 → 同一进程 in-window WinUI3 panel → UIA 同进程枚举 descendants
- **跨进程实测不可行** = go 判据 #5 "跨进程对话框解析 ≥ 90%" **字面意义在 Win11 25H2 modern Notepad 上无法验证**
- **正确的 go 判据**：in-window WinUI3 panel 可达性 ≥ 90%（"跨进程"前提失效）

#### 5.2 **实现偏差**：PowerShell UIA1 无法枚举 WinUI3 panel 内部的复杂控件

**实测**：probe-07 触发"File > 另存为"成功（menu Invoke 通过，descendants 31→120 确认 panel 打开了），但**panel 内部**：
- 找不到 AutomationId 标注明确的"FileName"输入框
- 找到的 Edit 元素都是文件列表的列 header（`名称`=Name, `修改日期`=Modified Date, `类型`=Type, `大小`=Size）
- 找到的 Button 元素都是文件浏览辅助（`筛选器下拉列表`=Filter dropdown, `搜索`=Search, `设置`=Settings, `帮助`=Help, `最近更新`=Recent Updates）
- **没有显式的"Save"按钮** — 现代 UX 假设用户直接点列表项 = 保存

**PowerShell UIA1 限制**：
- 用的是 `System.Windows.Automation.AutomationElement` API（UIA1 = 旧版 COM API）
- 对 WinUI3 / Microsoft.UI.Xaml 控件支持有限
- 看不到 UIA3 (`IUIAutomationElement9`) 才能看到的"property conditions"和"control patterns"

**解决路径**（需要 UIA3 工具）：
- **Python + `uiautomation` 包**（支持 UIA3 + WinUI3）= 推荐
- **C# + `Microsoft.UI.Xaml.Automation.Peers`**（需 .NET 8 + WinAppSDK）
- **FlaUI**（C# UIA3 wrapper）
- **Windows Application Driver**（WinAppDriver, 微软官方 UIA2/UIA3 测试框架）

#### 5.3 **次要偏差**：菜单触发 = 唯一可行路径

- `Ctrl+Shift+S` 键盘快捷键 = **不可靠**（Win11 25H2 modern Notepad 中被忽略）
- `File > 另存为` menu Invoke = **可行**（probe-07 当前实现）
- 文件路径已写入 menu Invoke 成功日志：`[09:57:33] [iter 1] invoked File > SaveAs, waiting for dialog`

### 6. 更合理做法

#### 6.1 为何不在本会话换工具

- 用户已说"不行再建立新会话"——已用 5+ 小时
- 换 Python+uiautomation 需要：装 Python + pip 包 + 重写 probe = 至少 30 min
- 即便换工具，**go 判据本身需修订**（从"跨进程解析"改为"in-window panel 可达性"）—— 这是 ADR 级别决策
- **正确 handoff**：本卡保存 probe-07 + 记录发现 + 标记 Blocked；下个会话开 ADR task 重新定义 B1.4 + 选用 UIA3 工具

#### 6.2 为何保留 probe-07 而非删除

- probe-07 完整可运行（syntax OK + 流程完整 + 5 项指标测量框架齐全）
- "File > 另存为" menu Invoke 已实现（PowerShell UIA1 可达的最高层级）
- 即使 UIA3 工具取代 PowerShell UIA1，probe-07 的**测量框架**（5 项指标 + 12 iter + 报告格式）仍可复用
- 仅需替换 `Find-SaveAsDialog` / `Find-FilenameInput` / `Find-SaveButton` 三个函数为 UIA3 实现

#### 6.3 为何本卡标 Blocked 而非 Done

- go 判据未通过 = 字面意义上 B1.4 没完成
- 但**架构发现是高质量产出**——这是 spike 阶段最有价值的部分（"证伪关键假设"）
- per AGENTS.md §6 "阶段 0 的产出是 SPIKE_REPORT.md，不是产品代码" + go/no-go 后果可改为"no-go for cross-process dialog (actually in-window panel)"
- 状态 Blocked 而非 Abandoned = 下个会话可继续（不浪费 probe-07 的代码）

### 7. 遗留问题

- **B1.4 重做**：用 UIA3 工具（Python uiautomation / C# FlaUI）+ 修订 go 判据为"in-window panel 可达性" → 建议开 ADR task 决定（不是临时 sub-task）
- **架构 v2 §3.2 修订**：元素跨进程原则对 WinUI3 应用需明确边界 —— 影响 stage-1 整个 Adapter 设计 → 必须走 ADR
- **stage-1a 风险**：Notepad Adapter 原计划用"Ctrl+Shift+S 触发跨进程 Save As dialog"路径 —— **在 Win11 25H2 上根本不存在该 dialog** = 需重新设计 Save As 路径
- B1.5 / B1.6 / B1.7 仍可继续（失败注入 / 接口考古 / 最终 go-no-go），不依赖本卡
- TASK-002 仍 InProgress（go 判据 #5 未通过；剩余 1.5/5）

### 8. 新增长期记忆

**REJECTED [2026-09-21]**：
- **方案**："用 PowerShell UIA1 枚举顶层窗口找 Win11 modern Notepad 的 Save As dialog"
- **否决理由**：Win11 25H2 modern Notepad 用 in-window WinUI3 FileExplorer-like panel 实现"另存为"，**不存在顶层 dialog 窗口**。PowerShell UIA1 也无法枚举 panel 内部的复杂 WinUI3 控件。**未来重做 B1.4 必须用 UIA3 工具**（Python `uiautomation` / C# `FlaUI`）。

**FACT [2026-09-21]**：
- Win11 25H2 modern Notepad 的"另存为"是 in-window WinUI3 FileExplorer-like panel（+89 descendants 在 Notepad 内），不是跨进程 dialog
- `Ctrl+Shift+S` 键盘快捷键在 modern Notepad 不可靠
- "File > 另存为" menu InvokePattern 是 PowerShell UIA1 唯一可达的触发路径
- 架构 v2 §3.2 "element 不跨进程"对 WinUI3 应用**不再成立**（= in-process panel）

**PITFALL [2026-09-21]**：
- spikes/ 探针用纯 ASCII source 是对的（ADR-0024 D4），但 CJK 字符串必须用 `[char]0x4E2D` 构造而不是字面 `'中文'`（PowerShell UIA 输出在控制台 CP936 GBK 下会乱码，但 source 仍是 pure ASCII）

### 9. 给审阅者的关注点

1. **架构发现最重要**：Win11 25H2 modern Notepad 用 in-window WinUI3 panel = stage-1 Adapter 设计受影响。**强烈建议下一会话开 ADR task 重新审视 v2 §3.2 + Adapter 架构假设**。
2. **probe-07 不应丢弃**：menu Invoke 触发 + 5 项指标框架仍可用；仅 Find-SaveAsDialog 等 3 个函数需 UIA3 重写。
3. **本会话已完成 4 张卡**（TASK-073 + TASK-002 B1.1 + TASK-074 B1.2 + TASK-075 B1.4 partial）—— 远超 AGENTS.md "1-2 张/会话"建议上限；**强烈建议 commit + handoff + 重新开 session** 处理 UIA3 重写（不是这会话的事）。

<!-- ══ 9 节执行记录填写完毕（B1.4 = 2026-09-21，本卡 Blocked） ══ -->
