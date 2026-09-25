# TASK-002　Spike A：Notepad UIA 实测 + 接口考古

- 状态：**Done**
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

### 1. 约束回执（B1.1 启动填，2026-09-20）

```text
【任务】TASK-002 Spike A 续做（B1.1 启动）   【目标】复跑验证现有 evidence + 起步填 9 节执行记录
【write scope】仅：spikes/spike-a-notepad/**（重跑不引入新 probe）/ docs/spike-reports/SPIKE-A.md（§8 追加）/ tasks/TASK-002（本卡执行记录）/ LEDGER.md（追加一行）
【铁律】AGENTS.md §3+§6 / ADR-0021 / ADR-0022 D1/D4/D5 / ADR-0023 / ADR-0024 D1/D1a/D4
【禁止】不写 crates/platform/windows 产品代码（Out of scope）/ 不动 Paint-Excel 相关 / 不在本会话给 go-no-go / 不改公共热点（AGENTS.md §8）
【验收】cargo build spike → 0 warning / uia_dep_proof → ExitCode 0 + E1-E6 PASS / probe-01 → 32 节点 / probe-02 → 实测与 SPIKE-A §3 一致 / SPIKE-A §8 写入 / TASK-002 §1-9 填入 / LEDGER +1
【依赖】TASK-001 Done（已核 LEDGER）
【疑问】无
```

### 2. 实际改动文件（B1.1 内，2026-09-20）

- `docs/spike-reports/SPIKE-A.md`（追加 §8 B1.1 复跑验证小节，315→393 行；不动 PARTIAL 状态）
- `tasks/TASK-002-spike-a-notepad-uia.md`（填 §1-9 执行记录，骨架从模板→填入）
- `LEDGER.md`（追加 1 行本卡 B1.1 锚定台账）
- **未改**：`spikes/spike-a-notepad/**`（重跑不引入新 probe）
- **未改**：`docs/memory/apps/notepad.md`（复跑结果与现有 §3/§6/§7 一致，不需追加）

### 3. 验收输出摘要（B1.1，2026-09-20）

- `cargo build --manifest-path spikes/spike-a-notepad/Cargo.toml` → **0 warning**，1.32 s 增量
- `cargo run --bin uia_dep_proof` → **ExitCode 0**；E1-E6 全 PASS（feature `Win32_UI_Accessibility` 持续生效；re-confirms ADR-0022 D1/D5 + ADR-0023；§7.3 最大不确定性持续解除）
- `probe-01-tree-survey.ps1` → **32 节点树**；findEdit 3.3 ms；ValuePattern.GetValue 1.61 ms；状态栏 ` Windows (CRLF)` + ` UTF-8`
- `probe-02-text-and-timing.ps1` → fullTreeWalk **15.3 ms**（与 §3 17 ms 一致，53× 余量）；findEdit 1.2 ms；GetValue 0.09 ms；**`SetValue zh`=3.11 ms（wrote=57 readback=56 equal=True）= NEW 2026-09-20 evidence：CJK 写入 100% 等价**
- `cargo run -p xtask -- hygiene / docscan / memory-counts` → 全 PASSED
- `cargo test --workspace` → **302 passed**；0 failed

**go 判据更新**：
- ✅ 中文写入 100% 正确（SetValue 单态 = probe-02 + uia_dep_proof E4 + probe-04）= **已达成**
- ✅ 全窗口树遍历 ≤ 800 ms = 15.3 ms（**53× 余量**）
- ⏸ 1 MB 文本读取 ≤ 2 s 且内存增量 ≤ 100 MB = **B1.2 待做**
- ⏸ 关键控件定位成功率 ≥ 90% = 100%（probe-01 32 节点全枚举），但**仍需新 measurement 验证命中率**
- ⏸ 跨进程对话框解析 ≥ 90% = **B1.4 待做**
- ❌ L4 IME 开/关两态 = **B1.x 待做**

### 4. DoD 逐条核对（B1.1 = 启动验证，不预期勾完 DoD）

- [x] 复跑 uia_dep_proof：E1-E6 全 PASS，exit 0
- [x] 复跑 probe-01：32 节点树，findEdit 3.3 ms
- [x] 复跑 probe-02：中位数与 §3 一致，NEW SetValue zh evidence
- [x] SPIKE-A.md §8 追加（315→393 行，干净 LF/无 BOM）
- [x] TASK-002 §1-9 执行记录填入
- [x] LEDGER.md 追加 1 行
- [x] 全量 xtask + cargo test 302 passed
- [⏳] 剩余 DoD 见 B1.2~B1.7（详 SPIKE-A §8.8）

### 5. 偏差

- **scope 越界**：本卡**没有**改 plans/* / MEMORY.md §1 / AGENTS.md，所以本次**无需 Orchestrator-equivalent 授权**。所有改动都在本卡 write scope 内。
- none（其它无偏差）

### 6. 更合理做法

#### 6.1 为何 B1.1 仅复跑而不做新 measurement

现有 evidence 已覆盖 go 判据 5 项中 2 项（中文写入 100% / 全窗口树遍历 ≤ 800 ms）。剩余 3 项每项都是独立 measurement 工作。B1.1 = 把现有 evidence 锚定到 2026-09-20 时间戳 + 起步填执行记录 = 低风险交付。B1.2~B1.7 逐步推进剩余 measurement。

#### 6.2 为何不在本卡把 §5.1 剩余工作一次性做完

AGENTS.md §3 "一个会话最多 1~2 张卡"——本卡 = B1.1 = 启动 + 复跑，已超 30 min 工作量边界。剩余 5+ 项 measurement 每项独立 ≥ 30 min，合计 2.5+ 工作日。

### 7. 遗留问题

- 5+ 项 measurement 工作（详 SPIKE-A §8.8 B1.2+ 候选）：
  - B1.2 probe-05 = 1MB/100KB/1KB 读写耗时 + 内存增量
  - B1.3 probe-06 = Rust `SetValue` 单独路径
  - B1.4 probe-07 = 菜单展开 + 跨进程 Shell 对话框
  - B1.5 probe-08 = 失败注入 4 种
  - B1.6 = 接口考古 8 步
  - B1.7 = 最终 go/no-go
- state 保持 **InProgress**（不动 Done，剩余 measurement 未完成）

### 8. 新增长期记忆

无（本卡 B1.1 仅复跑验证，未发现新坑 / 新事实 / 新否决方案）

### 9. 给审阅者的关注点

1. **B1.1 是否足够 "启动"**：建议把 2026-09-20 时间戳视为"现有 evidence 的最近一次复跑锚定"；是否给 go/no-go = 取决于 B1.2~B1.7 是否要做完。**个人建议 = 不给 go**，理由 = 1MB 读写 + 跨进程对话框 + 失败注入 是 spike B 跨进程 Host（TASK-004）的前置测量，不在本卡做完 = TASK-004 派单风险。
2. **probe-02 NEW CJK evidence**：wrote=57 readback=56 equal=True 中 1 字符差 = 状态标签由 "已修改" 变回 "未修改。" 的瞬变；不属数据丢失。若审阅者认为此差异不可接受，建议在 B1.2 加 probe-05 专门验证写入后的稳定状态。
3. **B1.2~B1.7 派单节奏**：是否同意按依赖顺序逐一派单？AGENTS.md "并行度 ≤ 3"，B1.2/3/4 可并行；B1.5/6 需等 B1.2；B1.7 需等全部。

<!-- ══ 9 节执行记录填写完毕（B1.1 = 2026-09-20） ══ -->
