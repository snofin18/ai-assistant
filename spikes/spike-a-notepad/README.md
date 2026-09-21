# spikes/spike-a-notepad —— Spike A 的一次性验证代码

> **这里的东西永远不进 `crates/`。** 它是为了证伪/证实假设而写的探针，生命周期到
> `docs/spike-reports/SPIKE-A.md` 出结论为止（AGENTS.md 阶段 0 约束）。

| 文件 | 作用 | 依赖 |
|---|---|---|
| `probe-01-tree-survey.ps1` | 启动记事本 → 按标题定位窗口 → 全树遍历并打印控件普查表（ControlType / AutomationId / ClassName / Name）→ 找编辑区 → 读 `ValuePattern` → 检查 `TextPattern` 是否可用 | 仅 Windows 自带 UIAutomation 程序集 |
| `probe-02-text-and-timing.ps1` | 干净启动（先杀所有 Notepad）→ 校验读回文本与磁盘文件是否一致 → **10 次取中位数**（全树遍历 / 编辑区定位 / GetValue）→ `SetValue` 写中文并读回校验 → **单实例多标签**验证 | 同上 |
| `probe-03-eol-matrix.ps1` | **读入方向**的 EOL 矩阵：把 LF / CR / CRLF / MIXED / CRLF+CJK 五种磁盘形态分别写入临时文件，逐个用 UIA 读回，统计两侧的 `code10`/`code13` 计数、`raw_eq`/`norm_eq`，并顺带抓取**状态栏**报告的 EOL 风格 | 同上（结果落 `D:\csart\eol-probe\RESULT.txt`） |
| `probe-04-write-path-eol.ps1` | **写回方向**的 EOL 矩阵：`SetValue` 分别写入 LF / CR / CRLF → 读回 UIA → 触发保存 → 比对**保存后磁盘**的 EOL 风格是否保留；含 CJK 存活断言与 nonce 锚点定位 | 同上（结果落 `D:\csart\eol-probe\RESULT-04.txt`） |
powershell -NoProfile -ExecutionPolicy Bypass -File .\probe-05-large-file-timing.ps1
| `Cargo.toml` + `src/bin/uia_dep_proof.rs` | **Rust/COM 生产路径的依赖实证**（ADR-0024 D1/D1a）。E1~E6：`CoCreateInstance` → `GetRootElement` → 按 owner PID 定位窗口 → 有序候选链找文档区 → `ValuePattern` 读回并按 ADR-0023 归一化比对 → `FindFirst` 计时 → 负向对照 | **`windows` crate `=0.62.2`**（唯一第三方依赖，已登记 `docs/DEPENDENCIES.md`） |
| probe-05-large-file-timing.ps1 | **大文件读写 PoC**（B1.2）：3 sizes × 12 iter（10 + 2 warmup）× (read/write/memory) = 72 数据点。读 GetValue、写 SetValue、内存增量 Get-Process Notepad.WorkingSet64（3 poll × 100ms）。**go 判据 #3**：1MB read ≤ 2 s 且 write_dMB ≤ 100 MB（实测 0.32 ms + -0.02 MB = PASS）。结果落 D:\csart\eol-probe\RESULT-05.txt | 同上 |

## 运行

```powershell
# PowerShell 探针（零第三方依赖）
powershell -NoProfile -ExecutionPolicy Bypass -File .\probe-01-tree-survey.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\probe-02-text-and-timing.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\probe-03-eol-matrix.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\probe-04-write-path-eol.ps1

# Rust/COM 依赖实证（ADR-0024 D1a）。注意用 Start-Process 而不是管道：
# 管道会等 stdout 的 EOF，而 notepad.exe 是转交型 stub（脚本内部已 Stdio::null()，但
# 手工调试时若忘了这一点就会看到"程序跑完了却一个字都没有"）。
cargo build                                     # 首次约 1~2 分钟（编译 windows crate）
$p = Start-Process -FilePath ".\target\debug\uia_dep_proof.exe" -NoNewWindow -Wait -PassThru `
       -RedirectStandardOutput "out.txt" -RedirectStandardError "err.txt"
$p.ExitCode; Get-Content out.txt -Encoding UTF8
```

`uia_dep_proof` 的期望输出（本机 2026-09-18 实测，`ExitCode = 0`）：

```text
E1: PASS  CoCreateInstance(CLSID_CUIAutomation) -> IUIAutomation
E2: PASS  GetRootElement name="桌面 1" class="#32769"
E3: PASS  hwnd=0x540d38 owner_pid=13168 launched_pid=2632 same_pid=false
E4: PASS  matched_candidate="Document + RichEditD2DPT (Notepad 11)" class="RichEditD2DPT" aid="" raw_chars=25 raw_cr=3 raw_lf=0 norm_eq=true cjk_kept=true
E5: PASS  resolve_candidate_chain median_us=2349 min_us=1976 max_us=3049
E6: PASS  absent control type resolved to None = true
RESULT: ALL PASS
```

## 已知注意事项

1. **会强制关闭记事本**（`taskkill /IM notepad.exe /F /T` 或 `Stop-Process Notepad -Force`）
   —— 运行前保存你手工打开的内容。`uia_dep_proof` 同样如此（它在开头杀干净、结尾只 `WM_CLOSE` 自己开的那个窗口）。
2. **控制台中文可能乱码**（PowerShell 控制台代码页），判定以脚本内比较结果为准。
3. 所有探针都**不修改仓库内任何文件**，临时文件写在 `%TEMP%` 并自行删除。
4. `probe-01` 曾因「同名文件已在标签页中打开 → 记事本**不重新加载磁盘内容**」得到假阴性，
   `probe-02` 起改为**先杀进程 + 用带时间戳的唯一文件名**。新写探针请沿用该做法。
   > **[2026-09-18 人类裁定]** 「已打开的文件不重载」**不是本项目要解决的问题** ——
   > 用户手工双击同一文件时记事本行为完全一样，属**应用自身行为**。
   > 仍然有效的是：① 上面的探针方法论；② 产品侧要注意 L1「文件契约」与 L3「UI 态」可能不一致。
5. **`.ps1` 必须纯 ASCII**（ADR-0024 D4）。PowerShell 5.1 会把无 BOM 的 `.ps1` 按 **GBK** 解码，
   导致**报错行号错乱**、反引号转义 `` `r `` **被吞**，故障现象与真因毫无关系（probe-03 实测踩过）。
   中文说明一律写进本 README / 报告；脚本内用英文注释 + 指向 ADR 编号。
   校验方法：`[IO.File]::ReadAllBytes($f) | ? { $_ -gt 127 } | Measure-Object` 应为 **0**。
   `.rs` 不受此限（Rust 源码默认 UTF-8，`uia_dep_proof.rs` 内有中文注释）。
   > **[2026-09-18 补正]** 本条不只是「乱码」问题，它会**把测量变成假的**。`probe-03` 已改为纯 ASCII
   > （CJK 样本改用 `[char]0x4E2D` 等码点拼接）并重跑：CJK 用例从 `14 字符 / 34 字节 / code13=1`
   > 修正为 `10 / 22 / code13=2`（与理论值吻合），其余 4 个用例逐字节不变。
   > 旧输出留存在 `D:\csart\eol-probe\RESULT-pre-ascii-fix-20260918.txt`。
   > **仍存量违反**：`probe-01` / `probe-02` 是 ADR-0024 D4 之前提交的，含中文注释（788 / 941 个非 ASCII 字节）——
   > 已记 **PL-026**，本会话不动它们（它们已产出过被引用的证据，改写需重跑验证，不属本次范围）。
6. **断言的 `False` 要结合输入读**：`probe-04` 的 `cjk_survived_on_disk` 在 5 个用例里有 4 个是
   `False`，那是**预期**（那 4 个用例输入是纯 ASCII，为绕开第 5 条的编码坑）—— 不是失败。
   详见 `docs/memory/apps/notepad.md` §4.2 的注记。

## 控件普查工具（ADR-0024 D3，人类指示 #8）

**不用 Accessibility Insights for Windows**（未安装，且已**裁决否决**补装：输出不可 diff、不可计时、
不可重复，对 AI agent 协作无增益）。改用两件东西：

1. **`inspect.exe`**（Windows SDK 自带，已就绪）—— 看**单个**控件的属性：
   ```powershell
   & "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\inspect.exe"
   ```
   另有 `x86` / `arm64` 版本。它是 GUI 工具，用于人工确认某个控件的 `ControlType` /
   `AutomationId` / `ClassName` / 支持的 Pattern；**产出不可入库 diff**，所以只作辅助。
2. **自写 UIA 树导出器** = `probe-01-tree-survey.ps1` —— 这是**可入库、可 diff、可计时**的那一个，
   普查结果一律以它为准（输出见 `docs/spike-reports/SPIKE-A.md` §2 与
   `docs/memory/apps/notepad.md` §3）。

> 官方 `winapp ui inspect` CLI 是**候选**替代品（ADR-0024「重新评估触发条件」）：若能用于第三方应用，
> 可能替代部分自写导出。试用归 **TASK-003**（`docs/memory/open.md` N7）。

## 定位与文本的既定契约（写探针前必读）

| 主题 | 契约 | 本目录里的实证 |
|---|---|---|
| 窗口身份 | **禁用启动 PID**；属主 PID 只从 `GetWindowThreadProcessId` 取；**禁用 `MainWindowHandle`**（官方=启发式）；启动走 AUMID `Microsoft.WindowsNotepad_8wekyb3d8bbwe!App` + `IApplicationActivationManager::ActivateApplication`；**`notepad -w` 已否决**（被当文件名 → 模态框「文件名无效。」） | `uia_dep_proof` E3：`owner_pid=13168 ≠ launched_pid=2632` |
| selector | **有序候选链 + `app_version_range` + 状态指纹**；禁用本地化属性（`Name` / `AccessKey` / `AcceleratorKey` / `LocalizedControlType` / `ItemStatus`）；`FindFirst` 在**根元素**上必须用 `TreeScope_Children` | `uia_dep_proof` 的 `DOCUMENT_CANDIDATES`（4 项，实测命中第 1 项） |
| 文本 EOL | 规范形 = **LF**；归一化 = `replace("\r\n","\n").replace("\r","\n")`（**顺序不可颠倒**）；出口按每应用习惯表；postcondition 用规范形比较，且原始长度差必须能被 EOL 计数完全解释 | probe-03（读入）/ probe-04（写回）/ `uia_dep_proof` E4 |
| 「没找到」 | UIA 用 `S_OK + NULL` 表示无匹配，`windows`-rs 转成 `Err(HRESULT(0))` → **必须显式映射成 `None`** | `uia_dep_proof` 的 `find_first()` + E6 负向对照 |

ADR 全文：`docs/adr/0022-*.md`（定位）、`0023-*.md`（EOL）、`0024-*.md`（工具链与依赖）。

## 尚未实现（TASK-002 剩余步骤）

- 菜单项（文件/编辑/查看**展开后**的子项）与「另存为」**跨进程 Shell 对话框**的普查
- 大文件 1 KB / 100 KB / 1 MB 的读写耗时与内存增量
- **IME 开 / 关两态**对照 —— **仅限 L4 合成键盘输入路径**
  （`DRIFT-002-1` 已裁决，2026-09-18：`SetValue` 走 Pattern 不经键盘、IME 对它无影响，
  卡面已按「无影响」改述；日后若发现例外再改回）
- 失败注入：记事本被关闭 / 最小化 / 在另一虚拟桌面 / 出现未保存弹窗
- 接口考古 8 步（`target-apps-feasibility.md` §5）
- **Rust 生产路径验证**：~~阻塞在依赖批准~~ → **依赖已批准并实证**（ADR-0024 D1/D1a，PL-019 已关闭）。
  `uia_dep_proof` 已覆盖「枚举窗口 / 定位文档区 / 读全文」；
  **仍待做**：Rust 侧 `SetValue` 写路径、触发菜单动作、跨进程对话框、带 `CacheRequest` 的批量取属性对照
  （`docs/memory/open.md` N2）。
