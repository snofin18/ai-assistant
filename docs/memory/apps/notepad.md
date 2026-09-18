# apps/notepad.md — Windows 记事本 应用档案（L2）

> ADR-0021 规定的 8 个固定小节。**缺数据的小节写「未测」，不得省略标题** ——
> 标题本身就是给下一个 agent 的清单：它说明"这里应该有什么"。
> 本档案是 Notepad Adapter 的**唯一事实源**。项目级的坑仍进 `../pitfalls.md`。
> 复现：`spikes/spike-a-notepad/probe-01..04-*.ps1`、`src/bin/uia_dep_proof.rs`。
> 报告：`docs/spike-reports/SPIKE-A.md`。契约：ADR-0022（定位）、ADR-0023（文本 EOL）。

**实测环境**（所有数据的共同前提，换环境必须重测）：
Windows 11 **25H2 build 26200.9457**；3200×2000 @ **200% 缩放**（AppliedDPI=192）；
NVIDIA GeForce RTX 5060 Laptop GPU；输入法首位 `zh-Hans-CN`（微软拼音 TIP）；
`rustc/cargo 1.98.1 stable-msvc`；PowerShell 5.1。
⚠️ `HKLM:\...\CurrentVersion` 的 `ProductName` **谎报 "Windows 10 Pro"**，判版本要用
`DisplayVersion` + `CurrentBuild` + `UBR`（见 `../pitfalls.md`）。

---

## 1. 身份与版本

| 项 | 实测值 | 取法 |
|---|---|---|
| 包名 | `Microsoft.WindowsNotepad` | `Get-AppxPackage Microsoft.WindowsNotepad` |
| 版本 | **11.2607.14.0 x64** | 同上 |
| 是否打包应用 | **是（MSIX）** —— 这一条决定了下面几乎所有定位策略 | 安装路径在 `WindowsApps` 下 |
| 可执行文件 | `C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11.2607.14.0_x64__8wekyb3d8bbwe\Notepad\Notepad.exe` | 同上 |
| **AUMID** | **`Microsoft.WindowsNotepad_8wekyb3d8bbwe!App`** | ADR-0022 D1 步骤 0 指定的启动方式 |
| UI 框架 | WinUI 3 / Windows App SDK（树里满是 `Microsoft.UI.Xaml.Controls.*` 与 `Microsoft.UI.Content.DesktopChildSiteBridge`） | probe-01 |
| 内存 | WorkingSet ≈ **118.8 MB**（空载 + 1 个 62 字符文件） | `Get-Process.WorkingSet64` |

> **不是** Win32 老版记事本。老版的编辑区是 `ControlType=Edit` + `ClassName=Edit`；
> 新版是 `Document` + `RichEditD2DPT`（见 §3）。Adapter 的 selector 候选链必须两者都覆盖。

## 2. 进程与窗口模型　★ 定位策略的唯一依据

| 事实 | 实测证据 |
|---|---|
| **启动返回的 PID ≠ 窗口属主 PID** | PowerShell：`Start-Process` 得 30464，窗口属主 20304（事先杀干净仍如此）。Rust/COM 独立复现：`launched_pid=2632`，`owner_pid=13168`，`same_pid=false`（`uia_dep_proof` E3） |
| `notepad.exe` 是**转交型 stub**：它把文件交给打包进程后自己退出 | 打开第二个文件后系统里有 **3 个 Notepad 进程、1 个窗口、2 个 TabItem** |
| **PID 不可用于定位** | 上一条的直接后果 |
| **窗口不是立刻就有的**，必须轮询 | Rust 侧在第 **3** 次枚举扫描后才匹配到（间隔 150 ms → ≈450 ms） |
| 属主 PID 只能从 `GetWindowThreadProcessId` 取 | ADR-0022 D1 |
| **`MainWindowHandle` 禁用** | 微软官方文档说明它是**启发式**结果（ADR-0022 证据 E3） |
| 单实例 + **多标签**：寻址单位是 **TabItem**，不是窗口 | probe-02 的多标签验证 |
| **`notepad -w` 已否决** | 实测：`-w` 被当成**文件名**处理 → 弹出模态框「文件名无效。」，`TabItem` 计数为 0；微软官方文档也查不到该开关（ADR-0022 D1） |
| **启动方式（唯一批准的）** | AUMID + `IApplicationActivationManager::ActivateApplication`（ADR-0022 D1 步骤 0） |
| 探针必须**先杀掉所有 Notepad** | 否则"打开一个已在标签页里的文件"不会重读磁盘（§6 坑 2） |
| **`notepad.exe` 必须用 `Stdio::null()` 启动** | stub 会继承父进程的 stdout 句柄 → 任何"等 EOF"的管道在 `main` 返回后一直挂着（本机实测，`uia_dep_proof`） |

## 3. UIA 形状

`ControlViewWalker`，深度 ≤8，**32 个节点**（probe-01 实测；相对 go 判据有 47×~133× 余量）。

```text
Window       aid=''                  cls=Notepad                                     name='<file> - Notepad'
  Pane       aid=''                  cls=NotepadTextBox                              name=''
    Document aid=''                  cls=RichEditD2DPT                               name='文本编辑器'   ← ★ 编辑区
  Pane       aid=''                  cls=Microsoft.UI.Content.DesktopChildSiteBridge
    Pane     aid=''                  cls=InputSiteWindowClass
      Tab    aid='Tabs'              cls=Microsoft.UI.Xaml.Controls.TabView
        List aid='TabListView'       cls=ListView
          TabItem aid=''             cls=ListViewItem                                name='<file>. 未修改。'
            Text   aid=''            cls=TextBlock                                   name='<file>'
            Button aid='CloseButton' cls=Button                                      name='关闭标签页'
        Button aid='AddButton'       cls=Button                                      name='添加新标签页'
  Pane       cls=Microsoft.UI.Content.DesktopChildSiteBridge
    Pane     cls=InputSiteWindowClass
      MenuBar  aid='MenuBar'
        MenuItem aid='File'          cls=Microsoft.UI.Xaml.Controls.MenuBarItem       name='文件'
        MenuItem aid='Edit'          cls=Microsoft.UI.Xaml.Controls.MenuBarItem       name='编辑'
        MenuItem aid='View'          cls=Microsoft.UI.Xaml.Controls.MenuBarItem       name='查看'
      Button   aid='FREButton'       cls=Button                                       name='最近更新'
      Button   aid='SettingsButton'  cls=Button                                       name='设置'
      Window   aid='PrivacyTeachingTip'          cls=…Controls.TeachingTip             ← 瞬态
      Pane     aid='PsDownloadDetailTeachingTip' cls=…Controls.TeachingTip             ← 瞬态
      Pane     aid='PsDownloadLightTeachingTip'  cls=…Controls.TeachingTip             ← 瞬态
  Pane       cls=Microsoft.UI.Content.DesktopChildSiteBridge
    Pane     cls=InputSiteWindowClass
      Text   aid='ContentTextBlock'  cls=TextBlock                                    name='行 1， 列 1'  ← 状态栏（×6）
      …（DesktopChildSiteBridge / InputSiteWindowClass 噪声若干）
```

### 编辑区的 selector 候选链（ADR-0022 D5，已由 `uia_dep_proof` E4 验证命中第 1 项）

| # | ControlType | ClassName | AutomationId | 说明 |
|---|---|---|---|---|
| 1 | `Document`(50030) | `RichEditD2DPT` | — | **新版记事本，实测命中** |
| 2 | `Document`(50030) | （不限） | — | 类名变更时的降级 |
| 3 | `Edit`(50004) | `Edit` | — | Win10 经典记事本 |
| 4 | `Edit`(50004) | （不限） | — | 最后兜底 |

**关键否定事实**：编辑区的 `AutomationId` 是**空串**（`uia_dep_proof` E4 实测 `aid=""`）
→ **不能**用 AutomationId 定位编辑区。可用 `aid` 的元素是：`MenuBar` / `File` / `Edit` / `View` /
`Tabs` / `TabListView` / `CloseButton` / `AddButton` / `FREButton` / `SettingsButton` /
`ContentTextBlock`（**×6 重名，不唯一**）/ 三个 `*TeachingTip`。

### 禁止用作 selector 的属性（ADR-0022 D4，本地化会变）

`Name`（`'文本编辑器'` / `'文件'` / `'关闭标签页'` …）、`AccessKey`、`AcceleratorKey`、
`LocalizedControlType`、`ItemStatus`。
**同一控件实测对照**：`aid='File'` 而 `name='文件'` —— aid 是英文、name 是本地化文本。

### 其他形状事实

| 事实 | 出处 |
|---|---|
| 窗口矩形 UIA 报的是**物理像素**（200% 缩放下 `448,458,2300,1446`）；Adapter 必须声明 **Per-Monitor V2** | ADR-0022 D8 |
| **`FindFirst` 在根元素上必须用 `TreeScope_Children`**，否则 UIAutomationCore 会遍历整个桌面（栈溢出风险）；在**单个应用窗口**上用 `Descendants` 是有界的 | ADR-0022 E6 + `uia_dep_proof` 注释 |
| `RuntimeId` **只在会话内有效**，不得持久化 | ADR-0022 |
| 三个 `*TeachingTip` 是**瞬态**的 → 树不稳定，普查必须容忍其出现/消失 | probe-01 |
| 会话恢复、TeachingTip、标签拖撕 三项需**显式关闭/处理** | ADR-0022 D9 |

## 4. 文本约定（`text_conventions`）★ 人类指示 #2 要求的"每应用习惯列表"实例

契约见 **ADR-0023**。以下是记事本的**实测实例**（probe-03 读入方向 / probe-04 写回方向）。

### 4.1 读入方向（磁盘 → UIA）

| 磁盘形态 | disk_chars | disk_bytes | uia_chars | 磁盘 EOL | UIA EOL | 状态栏显示 | raw_eq | **norm_eq** |
|---|---|---|---|---|---|---|---|---|
| CRLF | 15 | 15 | 12 | code10=3 code13=3 | **code13=3** | ` Windows (CRLF)` | False | **True** |
| LF | 12 | 12 | 12 | code10=3 | **code13=3** | ` Unix (LF)` | False | **True** |
| CR | 12 | 12 | 12 | code13=3 | code13=3 | ` Macintosh (CR)` | **True** | **True** |
| MIXED | 13 | 13 | 12 | code10=2 code13=2 | **code13=3** | ` Windows (CRLF)` | False | **True** |
| CRLF+CJK | 14 | **34** | 13 | code10=2 code13=1 | code13=2 | ` Windows (CRLF)` | False | **True** |

**结论**：**无论磁盘是什么形态，UIA 一律返回裸 `\r`（CR），不返回 `\n`。**
（`TextPattern.DocumentRange.GetText(-1)` 与 `ValuePattern` 逐字符相同，SPIKE-A §3。）
状态栏那一列**独立**报告文件的真实 EOL 风格 → 可作为"文件契约"的旁证来源。

### 4.2 写回方向（`SetValue` → UIA → 保存 → 磁盘）

| 用例 | 写入 CR/LF | SetValue 后 UIA CR/LF | 保存后磁盘 CR/LF | EOL 风格保留 | 断言 |
|---|---|---|---|---|---|
| disk=CRLF, write=LF | 0/2 | **2/0** | 2/2 | ✅ | 三项全 True；**CJK 存活 True** |
| disk=CRLF, write=CRLF | 2/2 | **2/0** | 2/2 | ✅ | 三项全 True |
| disk=CRLF, write=CR | 2/0 | 2/0 | 2/2 | ✅ | 三项全 True |
| disk=LF, write=LF | 0/2 | **2/0** | **0/2** | ✅ | 三项全 True |
| disk=LF, write=CRLF | 2/2 | **2/0** | **0/2** | ✅ | 三项全 True |

**两条结论**：
1. **`SetValue` 会把任何输入（LF / CR / CRLF）一律归一成裸 CR** —— 与读入方向对称。
2. **保存后磁盘的 EOL 风格 = 文件原本的风格（5/5 保留）**，与你写进去的是什么**无关**。
   → 这意味着「用记事本改一个 LF 文件」不会把它变成 CRLF，Adapter 不需要为此做补偿；
   但也意味着**不能指望通过 UI 改变文件的 EOL 风格**（要改必须走 L1 文件契约直接写盘）。

> ⚠️ 上表 2~5 行的 `cjk_survived_on_disk=False` 是**预期结果，不是失败**：
> probe-04 的这几个用例输入是**纯 ASCII**（为了绕开 PowerShell 5.1 的编码坑，见 §6 坑 6），
> 输入里没有 CJK，自然"没存活"。CJK 无损的证据是第 1 行（True）与 `uia_dep_proof` E4
> （`cjk_kept=true`，Rust 路径独立复现）。

### 4.3 规范化契约（必须逐字照做，ADR-0023 D1/D2）

```text
规范形 = LF（"\n"）
归一化 = text.replace("\r\n", "\n").replace("\r", "\n")     ← 顺序不可颠倒
```

- **顺序为什么不能颠倒**：先把每个 `\r` 换成 `\n`，会把一个 CRLF 变成**两个** LF，凭空多出空行。
- **postcondition 必须用规范形比较**（ADR-0023 D5）；原始长度差必须能被 EOL 计数**完全解释**
  （实测：磁盘 28 字符 / UIA 25 字符 / 3 个 CRLF → 28−3=25 ✅，`uia_dep_proof` E4）。
- **出口方向**按本表 4.2 的习惯写回；记事本的保存路径**不需要**你做 EOL 转换（它自己保留原风格）。
- 编码：记事本默认 **UTF-8（可带/不带 BOM）**；CJK 全程无损（34 字节 → 14 字符）。
- **绑定期自检**（ADR-0023 D6）：Adapter 绑定到目标时必须跑一次"写入→读回→规范形比较"，
  失败则拒绝绑定，而不是等到用户任务中途才发现。

## 5. 性能实测

| 操作 | 中位数 | min / max | n | 路径 | 出处 |
|---|---|---|---|---|---|
| 全树遍历（32 节点） | **17 ms** | — | 10 | PowerShell 托管封装 | probe-02 |
| 编辑区定位 | **1.5 ms** | — | 10 | PowerShell 托管封装 | probe-02 |
| `GetValue`（读全文） | **0.11 ms** | — | 10 | PowerShell 托管封装 | probe-02 |
| `SetValue`（写全文） | **4.79 ms** | — | 10 | PowerShell 托管封装 | probe-02 |
| **候选链定位（4 项，命中第 1 项）** | **2.349 ms** | 1.976 / 3.049 ms | 10 | **Rust + `windows` crate COM** | `uia_dep_proof` E5 |
| 窗口出现所需时间 | ≈**450 ms** | — | 1 | 启动 → 第 3 次枚举扫描命中 | `uia_dep_proof` E3 |

**★ 最重要的一条结论**：Rust/COM 路径（2.35 ms）与 PowerShell 托管封装（1.5 ms）
**同一数量级**，比值 ≈1.6×，**不存在"差一个数量级"的风险**。
这正是 `docs/spike-reports/SPIKE-A.md` §6 点名的最大不确定性 → **已解除**。
架构 v2「Host 进程内完成定位与执行」的批量取属性策略**无需重估**。

**未测**：带 `CacheRequest` 批量取属性的对比（绑定存在于 `Win32::UI::Accessibility`，
`ElementFromHandleBuildCache` / `FindFirstBuildCache` / `GetChildrenBuildCache` 等均在），
以及 1 KB / 100 KB / 1 MB 的读写耗时与内存增量。

## 6. 已知坑（本应用专属；跨应用的进 `../pitfalls.md`）

1. **UIA 返回裸 `\r`，磁盘是 `\r\n`** → 读回校验必须先归一化，否则必然假阴性。
   只做 `replace("\r\n","\n")` **不够**，还要 `replace("\r","\n")`。→ 已升格为契约 **ADR-0023**。
2. **打开一个"已在标签页里"的文件，记事本不重新加载磁盘内容**（只切到那个标签页）。
   `[supersedes:2026-09-18]` **人类裁定：这不是本项目要解决的问题** —— 用户手工双击同一个文件时
   记事本的行为完全一样，属于**应用自身行为**，不是自动化引入的偏差。
   **仍然有效的部分**：① 探针方法论 —— 必须先杀干净所有 Notepad 进程 + 用带时间戳的唯一文件名，
   否则会得到假阴性；② 产品侧仍要注意「外部改了 `.txt` 后记事本内存态不会自动更新」，
   即 L1「文件契约」与 L3「UI 态」**可能不一致**，撤销/校验逻辑必须显式处理这个偏差。
3. **PID 不可用于定位**（见 §2）→ 一律走 `GetWindowThreadProcessId` + AUMID 激活。
4. **`SetValue` 不经键盘，IME 开/关对它无影响** → 卡面"IME 两态"只对 **L4 合成键盘输入**有意义。
   `[supersedes:2026-09-18]` 人类裁定：**按"无影响"版本改卡面措辞**（DRIFT-002-1 已裁决），
   日后若发现例外再改回。
5. **PowerShell 控制台输出中文乱码**（代码页问题），但进程内字符串比较不受影响
   → 判定看 `True/False` 与长度，不要肉眼看控制台。
6. **PowerShell 5.1 会把无 BOM 的 `.ps1` 按 GBK 解码** → 报错行号错乱、反引号 `` `r `` 被吞。
   → **`.ps1` 一律纯 ASCII**（ADR-0024 D4），中文说明放 `.md`。（`.rs` 不受此限，UTF-8 是默认。）
7. **`FindFirst` 的"没找到"= `Err(HRESULT(0x00000000))`**（`windows`-rs 把 UIA 的
   `S_OK + NULL` 转成 error，而这个 error 的 code 恰恰是"成功"）→ 必须显式映射成 `None`。
   本机首次运行 `uia_dep_proof` 就是这样失败的，报错文本是「操作成功完成。」。
   → 跨应用通用，已同步 `../pitfalls.md`。
8. **只按 `ControlType=Edit` 找编辑区会静默落空**（新版是 `Document`）→ 必须用 §3 的候选链。
9. **`notepad.exe` 要 `Stdio::null()` 启动**（见 §2）。
10. **spike crate 自己的 `Cargo.toml` 必须有 `license` 字段**，否则 `cargo deny check licenses`
    报 `error[unlicensed]`（exit 4）→ 与依赖许可证无关，是**被检查的包自身**没声明。

## 7. 通道结论（L1~L5，AGENTS.md 铁律 5「API 优先」）

| 层 | 通道 | 记事本是否可用 | 说明 |
|---|---|---|---|
| **L1** | 应用编程接口 | **仅"文件契约"** | 记事本没有 COM/CLI/插件 API。`.txt` 文件本身是唯一的一等接口 —— 但见 §6 坑 2：改了磁盘不等于改了 UI 态 |
| **L2** | 命令行 / 快捷键 | **几乎没有** | `notepad <file>` 只能打开；**`-w` 已实测否决**（§2）；官方无其他文档化开关。快捷键可作为 L4 的一部分 |
| **L3** | 无障碍接口（UIA） | **✅ 极其宽松，主通道** | 32 节点、17 ms 遍历、读写亚毫秒~毫秒级；`ValuePattern` + `TextPattern` 均可用且结果一致 |
| **L4** | 合成输入 | 可用但**未测** | 唯一受 IME 影响的层（§6 坑 4）；终端类目标禁用 `Ctrl+Z`，记事本不属此类 |
| **L5** | 视觉兜底 | 未测 | 预期不需要 |

**撤销（AGENTS.md 铁律：`Ctrl+Z` 必须由 Adapter 显式声明）**：
记事本**支持** `Ctrl+Z` 且属"有回退机制"的应用类；但 UIA 的 `SetValue` 是一次性替换全文，
**`Ctrl+Z` 只能回到上一次 SetValue 之前**，不能逐字符回退 → Adapter 仍应在写前落**快照锚点**
（架构 v2 §9），不得把 `Ctrl+Z` 当唯一撤销手段。

**风险级**：读 = 低；`SetValue` 写 = 中（可撤销但会清空应用内 undo 粒度）；
「另存为 / 覆盖保存」= **高**（落盘不可逆，须 postcondition + 人工确认）。

## 8. 未测项（明确列出，防止把"没测"误读成"不行"）

- 菜单**展开后**的子项普查（文件 / 编辑 / 查看）
- **「另存为」跨进程 Shell 对话框**（文件名输入框、目录树、保存按钮；注意它属**另一个进程**）
  —— SPIKE-A §5.2 列为三大真实风险之一
- **1 KB / 100 KB / 1 MB** 的读写耗时与内存增量（判据：1 MB ≤ 2 s 且内存增量 ≤ 100 MB）
- **IME 开 / 关两态**（仅对 L4 合成键盘路径有意义，见 §6 坑 4）
- **失败注入 4 种**：记事本被关闭 / 最小化 / 在另一虚拟桌面 / 出现未保存弹窗
- **接口考古 8 步**（`target-apps-feasibility.md` §5）—— 预期结论：只有"文件契约"这一条 L1
- **Rust 侧 `SetValue` 写入**（`uia_dep_proof` 只做了读；写路径仍只在 PowerShell 侧验证过）
- **`CacheRequest` 批量取属性**的耗时对比（绑定已确认存在，见 §5）
- **多标签寻址**：TabItem 切换后编辑区元素是否变化、`RuntimeId` 是否随之改变
- **会话恢复**：重启记事本后标签页自动恢复对定位的影响（ADR-0022 D9 要求显式关闭）
