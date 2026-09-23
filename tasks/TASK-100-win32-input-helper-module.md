# TASK-100　Win32 Input Helper Module 单测（SendInput 替换 SendKeys 的前置）

- 状态：**Ready**
- 阶段：0（spike 派生）　子任务：用户 chat 2026-09-22 三步需求第 1+2 步（研究→单测新代码）　依赖：—　预估：M（~60-90 min）　阻塞主线：否
- 本文件 = **卡片正文 + 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-0-spikes.md`。

- **write scope**（严格按此范围；超此 = 漂移触发器 ⑤ → 停手开 DRIFT）：
  - `spikes/spike-a-notepad/Win32-Input.psm1`（**新建 PS module** = 主交付物）：封装 `Send-SendInputVk` / `Send-SendInputUnicode` / `Set-Win32ForegroundFocus` 三件套（基于 `docs/memory/win32-input-research.md` §1/§2/§9 推荐 pipeline）
  - `spikes/spike-a-notepad/probe-11-sendinput-vk.ps1`（**新建 probe-11** = 单测 1：VK 字符如 `'Z'` / `'a'` / `'0'` / 修饰键 `^` + 字母）
  - `spikes/spike-a-notepad/probe-12-sendinput-unicode.ps1`（**新建 probe-12** = 单测 2：CJK 与混合 Unicode 字符串）— 用 `[char]0x...` 构造保证 0 non-ASCII 字节（ADR-0024 D4）
  - `spikes/spike-a-notepad/probe-13-sendinput-blockinput.ps1`（**新建 probe-13** = 单测 3：聚焦+前后台对比，验证本卡研究的"焦点问题"假说）
  - `spikes/spike-a-notepad/README.md`（追加 probe-11/12/13 表行）
  - `docs/memory/apps/notepad.md`（追加 §6 Win32 Input Pipeline 节 + §8 references 增 win32-input-research.md 链接）
  - `docs/memory/pitfalls.md`（如发现新坑，**必追加** supersede 旧 entry）
  - `docs/memory/facts.md`（如发现新硬事实，**必追加**）
  - 本卡 `tasks/TASK-100-...md`（执行记录 9 节）
  - `LEDGER.md`（本卡完成时 +1 行）

- **Out of scope**（**禁止做**——发现需要做 → `docs/PARKING_LOT.md` +1 行 + 本会话停手）：
  - **不替换**现有 `probe-04/07/08/10` 中的 `SendKeys::SendWait` → 那属于下一张卡（TASK-101 「probe 集成替换」）
  - **不写** `crates/platform/windows/` 产品代码 → stage-1 才涉及
  - **不动** MEMORY.md / AGENTS.md / plans/* / docs/adr/*（§8 只读）
  - **不引入** 新依赖（不允许新 crate；PS 5.1 内置 + `Add-Type` Win32 P/Invoke 即可）
  - **不与** Task-002 go 判据直接挂钩（这是 Win32 Input 子系统**单测**而非 Notepad Adapter 集成；本卡交付 = 单测 PASS + helper module 可独立 import）
  - **不动** 任务卡号段（ADR-0037：100~199 业务池，本卡 = 100 起始）

**背景与必要性（用户 chat 2026-09-22 三步需求 #1+#2）**

用户在 2026-09-22 chat 中明示三步走：

> 1. 网络官方资料进行搜索学习，以后需要一直用到类似 sendkeys 这种操作其他 app 的函数，我需要有很高可靠性的代码去实现，保证我自己的程序正确和健壮。
> 2. 回头通过学习的经验，检视之前相关代码是否健壮和正确，如果有更成熟的代码，就尝试去替换旧的，并验证可用性。在这之前，每个要进入的新代码需要测试通过才行。
> 3. 都完成后，可以尝试 stage-1。

**Step 1（研究）** = 已落地 `docs/memory/win32-input-research.md`（193 行，9 节，6 个 Microsoft Learn URL + probe-08 4-test 实测）→ **已 Done**。

**Step 2（检视 + 替换 + 单测）** = 本卡 TASK-100 负责**单测**部分：把研究产出的推荐 API（`SendInput` + `SetForegroundWindow` + `SetFocus`）封装成 PS module，并**先单独跑测过**（vs 旧 `SendKeys::SendWait` = 微软标记 deprecated 的 `keybd_event` 包装）→ 之后才能进 TASK-101 做 probe 替换 + 集成回归。

**Step 3（stage-1 尝试）** = TASK-101 + 后续 stage-1 1a 批次卡（TASK-011/012/013/014/015）的链路，不属本卡。

**研究关键结论**（详 `docs/memory/win32-input-research.md`，核心 4 条）：

1. **`SendInput` 是 PRIMARY**（微软官方明确说 supersedes `keybd_event`）；`keybd_event` 仅作 Win98 兼容保留
2. **`.NET SendKeys::SendWait` 内部就是调 `keybd_event`** → 因此**本质 deprecated**；后台 PS + UWP 应用下 Win11 25H2 实测 4 test 中有 3 test 失败（probe-08 已锁定）
3. **`SetForegroundWindow` 只设 z-order，不设 focus** → focus 要单独 `SetFocus(hwnd)`（test 1g 已证 `SetFocus + keybd_event` 可 work）
4. **UIPI 不是真 blocker**（双方同 medium integrity）→ 真 blocker = **focus establishment 失败**

**本卡验收**：3 个 probe 全 PASS（0 non-ASCII，CRLF=0，BOM=False）；helper module 可独立 `Import-Module`；UIA SetValue vs UIA SetValue+SendInput 字符对照测试确认新路径 `SetValue ≥ 旧 SendKeys` 的字符一致性。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执（TASK-100 启动填，2026-09-22）

```text
【任务】TASK-100 Win32 Input Helper Module 单测 【目标】把 SendInput / SetForegroundWindow / SetFocus 封装为 PS module + 3 个 probe 单测全 PASS = 旧路径 `SendKeys::SendWait` 的成熟替代品可独立 import
【write scope】仅：spikes/spike-a-notepad/Win32-Input.psm1（NEW）/ probe-11-sendinput-vk.ps1（NEW）/ probe-12-sendinput-unicode.ps1（NEW）/ probe-13-sendinput-blockinput.ps1（NEW）/ spikes/spike-a-notepad/README.md（追加表行）/ docs/memory/apps/notepad.md（追加 §6 + §8 ref）/ docs/memory/pitfalls.md（如有发现）/ docs/memory/facts.md（如有发现）/ 本卡 / LEDGER.md
【铁律】AGENTS.md §3+§6+§8 / ADR-0024 D4（spike 一律纯 ASCII，源 0 non-ASCII 字节）/ ADR-0028（公共热点前取锁）/ ADR-0030（memory-counts 必跑）
【禁止】不替换现有 probe-04/07/08/10 中的 SendKeys（→ TASK-101）/ 不写 crates/platform/windows/（→ stage-1）/ 不动 MEMORY.md / AGENTS.md / plans/* / docs/adr/*（§8 只读）/ 不引入新依赖 / 不挂钩 TASK-002 go 判据
【验收】xtask hygiene / docscan / memory-counts / adr-index 全 PASSED；Win32-Input.psm1 Import-Module 不报错；probe-11/12/13 0 non-ASCII / CRLF=0 / BOM=False；3 个 probe run 全 GO（criteria = UIA 读到目标字符 + Notepad title 加 `*` 标 dirty + disk 文件最终比对一致）；RESULT-11/12/13.txt 含中位数 + go 判据；本卡 §1-9 填入；LEDGER +1
【依赖】docs/memory/win32-input-research.md 已落地（= Step 1 Done）/ TASK-002 B1.5（probe-08 4-test 数据可复用）
【疑问】无（per probe-08 4-test：SetFocus + keybd_event work；SendInput 应等价；CJK 路径 KEYEVENTF_UNICODE 在 Win11 25H2 实测待本卡）
```

### 2-9 在 helper module + 3 个 probe 实施后填入

<!-- ══ 9 节执行记录填写完毕 ══ -->
