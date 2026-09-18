# ADR-0022　Windows 目标身份与 UIA selector 稳定性契约

状态：Accepted　日期：2026-09-18　Supersedes：—　Superseded by：—
关联：`docs/memory/apps/notepad.md`、`docs/spike-reports/SPIKE-A.md`、ADR-0023（文本约定）、
架构 v2 §6（目标定位）/§17.4（录制回放）、`tasks/TASK-004`（Spike B 跨进程 Host）、人类指示 #1 与 #5（2026-09-18）

## 背景（为什么现在要决定）

Spike A 先导侦察（2026-09-17，Notepad 11.2607.14.0）暴露两个会**贯穿所有 Windows Adapter** 的问题：

- **问题 1**：`Start-Process notepad <file>` 返回的 PID **不是**持有窗口的进程。
  实测：返回 PID 28144（`MainWindowHandle = 0`），窗口属主 PID 5344；重跑一次为 28868 vs 32952。
  事先杀掉所有 Notepad 进程仍然如此。→ 「我启动了它，所以我用这个 PID 找窗口」这条最常见的做法直接失效。
- **问题 5**：`AutomationId` 是英文、`Name` 是本地化文本，且 `AutomationId` **不唯一**
  （状态栏 6 个 `aid='ContentTextBlock'`），编辑区的 `AutomationId` **为空**。
  → 主 selector 用什么？跨语言、跨版本还稳不稳？

这两条若不在此刻定成契约，每个 Adapter 会各写一套定位逻辑，且都会重踩同一个坑。

## 证据（本机实测 + 微软官方文档）

### E1　进程模型：`notepad.exe` 是打包应用的启动器

```text
Get-Command notepad -All  →  3 个命中：
  C:\WINDOWS\system32\notepad.exe                              360448 B, FileVersion 10.0.26100.8875
  C:\WINDOWS\notepad.exe
  C:\Users\<user>\AppData\Local\Microsoft\WindowsApps\notepad.exe   0 B（App Execution Alias 重解析点）

实测（Trial C，无开关）：
  Start-Process notepad <file> → returnedPID = 28868, MainWindowHandle = 0   ← 启动器/stub
  EnumWindows + GetWindowThreadProcessId → hwnd 8719038, pid = 32952, cls = Notepad
  Get-Process notepad → 2 个进程（28868 无窗口、32952 有窗口）
实测（Trial 首跑，无开关）：returnedPID 28144 vs 属主 5344（同型复现）
```

打开**第二个**文件后：系统里 **3 个 Notepad 进程、1 个窗口、2 个 TabItem**。
→ 结论：**进程数 ≠ 窗口数 ≠ 文档数**；PID 不是可用的身份维度。

### E2　官方文档：`MainWindowHandle` 是启发式猜测，不可作身份

| 来源 | 原文（关键句） |
|---|---|
| learn.microsoft.com `System.Diagnostics.Process.MainWindowHandle` | "The main window is the window opened by the process that **currently has the focus** (the TopLevel form)." / "If the associated process does not have a main window, the MainWindowHandle value is **zero**. The value is **also zero for processes that have been hidden**." / "consider using `WaitForInputIdle`" |
| devblogs.microsoft.com/oldnewthing 2022-01-24（Raymond Chen [MSFT]） | "The **MainWindowHandle property is just a guess based on heuristics**. There is **no formal definition of a 'main window'** for a process. It's a synthetic property driven by enumerating all the top-level windows that belong to the process and **trying to guess** which one is the main one."（附 BCL `IsMainWindow`：无 `GW_OWNER` 且 `IsWindowVisible` 即算） |
| learn.microsoft.com `GetWindowThreadProcessId` | "Retrieves the identifier of the thread that created the specified window and, **optionally, the identifier of the process that created the window**." |
| learn.microsoft.com `EnumWindows` | "The EnumWindows function does not enumerate child windows… **This function is more reliable than calling the GetWindow function in a loop.**" / "For Windows 8 and later, EnumWindows enumerates **only top-level windows of desktop apps**." |

→ **权威结论**：窗口属主 PID 的**唯一**正确来源是 `GetWindowThreadProcessId(hwnd)`；
`MainWindowHandle` 依赖焦点与可见性，本项目要处理最小化/失焦/多窗口场景，**必然**误判。

### E3　`-w` 开关在新版记事本上**不存在**（实测否决）

```text
notepad -w <file>   →  returnedPID 4816 == 属主 4816（单进程），但：
                       窗口 Name = '记事本'（无文件名）、TabItems = 0、无 RichEditD2DPT 编辑区
                       树内出现模态框：Window cls=Popup / Text name='文件名无效。' / Button aid='CloseButton' name='确定'
notepad <file> -w   →  同型（returnedPID 31768 == 属主 31768，同样 0 个 TabItem）
等待 12 s 后仍如此 → 不是"没加载完"，而是 -w 被当成文件名
```

→ `-w` 被解释为**文件名**并触发「文件名无效。」模态框。
「用 `-w` 强制新窗口，这样 PID 就对得上」这条路 **在 11.2607.14.0 上不通**（记入 `docs/memory/rejected.md`）。
附带收获：该模态框是**进程内 WinUI `Popup`**，而「另存为」是**跨进程 Shell 对话框** → 两类对话框必须分开处理。

### E4　官方文档：`AutomationId` / `Name` / `RuntimeId` 的契约边界

来源：learn.microsoft.com `uiauto-automation-element-propids`（UIA 属性 ID 参考）与
`dotnet/framework/ui-automation/use-the-automationid-property`。

| 属性 | 官方原文（关键句） | 对本项目的含义 |
|---|---|---|
| `AutomationId` (30011) | "When it is available, the AutomationId of an element **must be the same in any instance of the application, regardless of the local language**." | **跨语言不变是平台契约** → 可作主 selector 的第一优先 |
| 同上 | "The value **should be unique among sibling elements, but not necessarily unique across the entire desktop**." | 状态栏 6 个同 `aid` **合法**，不是 bug；必须配合**兄弟索引/祖先作用域** |
| 同上 | "support for AutomationId is always recommended…, **this property is not mandatory**." | 编辑区 `aid` 为空**合法** → selector 必须能在无 `aid` 时工作 |
| 同上 | "**Clients should make no assumptions regarding the AutomationId values exposed by other applications.**" | 不得假设 `aid` 语义；每个应用都要实测建档 |
| 同上 | "**AutomationId is not guaranteed to be stable across different releases or builds of an application.**" | ★ **最关键**：连 `aid` 都可能随版本变 → selector 必须带**应用版本区间 + 指纹**，失配要能降级而非崩 |
| `Name` (30005) | "The Name property **cannot be used as a unique identifier among siblings**… For test automation, the clients should consider using the **AutomationId or RuntimeId** property." | 官方明确否决「用可见文本作主键」→ 与 AGENTS.md §7 一致 |
| `LocalizedControlType` (30004) | "a text string describing the type of control…**localized**" | 本地化 → **禁用**；用 `ControlType`（programmatic name，不变） |
| `RuntimeId` (30000) | "The identifier is unique on the desktop, but it is **guaranteed to be unique only within the UI of the desktop on which it was generated**. **Identifiers can be reused over time. The format of RuntimeId can change.** …treated as an **opaque** value and used **only for comparison**" | 只能做**会话内**同一性比较（Lease/缓存），**不得**持久化、不得解析结构 |
| .NET `use-the-automationid-property` | "AutomationIdProperty **does not guarantee a unique identity throughout the tree; it typically needs container and scope information to be useful**." / "Each menu item could then be uniquely identified by its AutomationID **along with the AutomationID of its parent and, if necessary, its grandparent**." | ★ **官方认可「祖先作用域 + aid 链」** → 正是本 ADR 的 selector 链设计 |
| 同上 | "AutomationIdProperty is supported by all UI Automation elements in the control view **except top-level application windows**, …WPF controls that do not have an ID or x:Uid, and …Win32 controls that do not have a control ID." | 解释了「窗口本体 `aid=''`」是**规范行为**，不是记事本的缺陷 |
| 同上 | "Use a **persistent path** to return to a previously identified AutomationElement"（record/playback 场景） | 官方为录制回放推荐的正是「持久路径」而非单一 id |
| 同上（Caution） | "you should try to obtain only direct children of the RootElement. A search for descendants may iterate through hundreds…" | → 定位必须**逐层下钻**，禁止无约束的 `Descendants` 全树搜索 |

### E5　官方启动/激活路径，以及「**不存在**受支持的新窗口 CLI 开关」

| 来源 | 原文（关键句） | 含义 |
|---|---|---|
| learn.microsoft.com `IApplicationActivationManager::ActivateApplication` | `[out] processId`："receives **the process ID of the app instance that fulfills this contract**" | ★ 对**打包应用**，这是官方给出的「拿真实宿主 PID」的 API，优于 `Start-Process` 的返回值 |
| learn.microsoft.com Windows App SDK `applifecycle-instancing` | "Attempting to launch a second instance … **typically results in the first instance's main window being activated instead**"；`AppInstance.GetInstances` 返回全部运行实例 | 官方解释了「打开第 2 个文件不开新窗口、而是加 TabItem」 |
| learn.microsoft.com `IUIAutomation::ElementFromHandle` | HWND → UIA 元素的官方入口（另有 `ElementFromHandleBuildCache` 可预取属性） | 有了 hwnd 就应从这里进入 UIA，而不是从 RootElement 全桌面搜 |
| 本机一手：`C:\Windows\System32\notepad.exe`（360,448 B）内嵌字符串 | `\Microsoft\WindowsApps\Microsoft.WindowsNotepad_8wekyb3d8bbwe\notepad.exe`、`ms-windows-store://pdp/?PFN=...` | 证实 System32 的 `notepad.exe` 是**跳板 stub**；AppxManifest 声明 `desktop:ExecutionAlias Alias="notepad.exe"` |
| 本机一手：AppxManifest 文档化的动词 | 仅 `Edit=%1`、`Print=/p %1`、`PrintTo=/pt "%1" "%2" "%3" "%4"` | **官方文档化的命令行入口只有这些** |
| Microsoft Learn / Windows Insider Notepad release notes / Notepad 团队博客 | 全库检索**无** `-w` / `/W` / `-newWindow` / `/newWindow` 的说明 | ★ **不存在受支持的「强制新窗口」开关** |
| superuser.com（社区，非官方，两说互斥） | 一说 `/W` = 新窗口，一说 `/W` = 以 Unicode/宽字符打开 | 社区说法互相矛盾且均非官方 → **禁止**写进 Adapter（与 E3 的实测否决互为印证） |
| 本机一手：打包版 `Notepad.exe` 二进制内字符串 | `LaunchNewWindowProcess`、`FileOnLaunchWindowContext`、`TabTearingOutWindowContext`、`EditingSessionWindowContext` | 内部**确实**按启动参数分流「新窗口 vs 加标签」，但**未文档化**；`TabTearingOut` 说明**拖拽标签可撕出新窗口** → 窗口集合可在我们不知情的情况下变化 |

- 本机 AUMID = **`Microsoft.WindowsNotepad_8wekyb3d8bbwe!App`**。

### E6　`FindAll` 的作用域与顺序契约（官方）

| 来源 | 原文（关键句） | 含义 |
|---|---|---|
| learn.microsoft.com `IUIAutomationElement::FindAll` | "When searching for **top-level windows on the desktop**, be sure to specify `TreeScope_Children` … not `TreeScope_Descendants` … could … lead to a **stack overflow**" | 顶层窗口枚举**必须** `Children`（本 ADR 的探针已如此实现） |
| 同上 | "Elements are returned **in the order in which they are encountered in the tree**" | 顺序 = 当前树遍历顺序，官方**未承诺**跨会话/跨版本稳定 → 「FindAll + 固定索引」只能作**弱约束**，必须配指纹复验 |
| learn.microsoft.com `uiauto-obtainingelements` | "you should try to obtain only **direct children of the root element** … start your search from the **application window or from a container at a lower level**" | 官方最佳实践 = 逐层下钻（本 ADR D4 已采纳） |

### E7　哪些属性会本地化（官方清单）+ 为什么 `AutomationId` 会是空的

| 来源 | 原文（关键句） | 含义 |
|---|---|---|
| learn.microsoft.com `ui-automation-properties-overview` | "UI Automation providers should present the following properties **in the language of the operating system**：`AcceleratorKeyProperty`, `AccessKeyProperty`, `HelpTextProperty`, `LocalizedControlTypeProperty`, **`NameProperty`**" | ★ 官方本地化清单**共 5 项**，比本 ADR 原先只禁 `Name`/`LocalizedControlType` 更宽：**`AccessKey` 与 `AcceleratorKey` 也会随语言变**（助记键如「文件(&F)」）→ 一并禁用 |
| learn.microsoft.com `uiauto-supportmenuitemcontroltype` | "If the menu item is **dynamically populated and not predictable, leave the AutomationId property blank**" | 「动态项留空 aid」是**官方建议**，不是缺陷 → 解释了 TabItem / 部分 Document 的 aid 为空 |
| learn.microsoft.com `FrameworkElementAutomationPeer.GetAutomationIdCore`（WPF） | 返回值 "is AutomationId property, and **if that isn't set, it's the Name property**"（`x:Name` → AutomationId） | **WPF 会自动回退**，而 **WinUI3/UWP 无此文档化回退** → 这正是记事本（WinUI3）大量空 aid、而 WPF 应用不会的**根因** |
| learn.microsoft.com `windows/apps/develop/ai-assisted/testing` | "For `winapp ui click` to target elements reliably, **set `AutomationProperties.AutomationId` in your XAML**"；提供 `winapp ui inspect/search/click/invoke/set-value` | 微软自己的 WinUI3 可测试性指导也承认「aid 要靠开发者显式设置」；`winapp` CLI 是官方工具（见 ADR-0024 对普查工具的裁决） |
| 关于状态栏 6×`aid='ContentTextBlock'` | 官方要求 aid "should be **unique among sibling elements**"、"uniquely identifies a UI Automation element **from its siblings**" | 这是 `should` 级要求的**实现瑕疵**；但官方同时给出**处置办法**：「needs **container and scope information** to be useful」→ 本 ADR D4 的祖先作用域 + 兄弟索引即官方解法 |

### E8　DPI 与坐标系（官方，直接关系到 200% 缩放的靶机）

| 来源 | 原文（关键句） |
|---|---|
| learn.microsoft.com `uiauto-screenscaling` | "**The UI Automation API does not use logical coordinates**"；`GetClickablePoint` / `ClickablePointProperty` / `FromPoint` / `BoundingRectangle` 均为**物理像素** |
| 同上 | "a UI Automation client application running in a **non-96-dpi environment will not be able to obtain correct results**" → 客户端进程**必须** dpi-aware |
| learn.microsoft.com `ui-automation-and-screen-scaling` | 明确 "**Do not use `Cursor.Position`**"（.NET 的该属性是逻辑坐标） |

→ 本机靶机为 **3200×2000、AppliedDPI=192（200% 缩放）**，正是官方点名的「非 96 dpi 环境」。

### E9　会话恢复与设置存储位置（探针的**前置条件**）

| 来源 | 事实 |
|---|---|
| blogs.windows.com（Windows Insider 官方博客，2023-08-31） | Notepad "automatically save your **session state** … restore previously open tabs as well as **unsaved content**"，可在应用设置中关闭 |
| blogs.windows.com（2023-01-19，tabs 上线） | "a new app setting lets you customize whether files open in **new tabs or a new window** by default" |
| 本机一手 | `HKCU\Software\Microsoft\Notepad` **不存在**；设置实体在 `%LOCALAPPDATA%\Packages\Microsoft.WindowsNotepad_8wekyb3d8bbwe\Settings\settings.dat`（16 KB） |

→ 后果：① 社区流传的「用 `.reg` 改 `TabWindowPreference`」方案**在本机不可用**；
② 探针若不先关闭/清空会话恢复，**上一次的标签页会污染本次初始状态**（这是「3 进程 / 1 窗口 / 2 TabItem」的另一半解释）；
③ 「文件默认开在新标签还是新窗口」是**用户可改的设置**，因此 Adapter **不得假设**任一行为。

## 决策

### D1　目标身份**绝不**来自启动 PID（`TargetIdentity` 契约）

```text
禁止：let pid = child.id();  let window = find_window_by_pid(pid);
必须：let window = discover_window_by_content_anchor(anchor);
      let owner_pid = get_window_thread_process_id(window.hwnd);   // 属主 PID 只从这里取
```

**窗口发现算法（Windows，L3 通道）**：

0. **启动方式（打包应用优先走官方激活 API）**：目标是 MSIX 打包应用时，
   用 **AUMID + `IApplicationActivationManager::ActivateApplication`** 启动，
   其 `[out] processId` 是官方定义的「履行该契约的应用实例 PID」（E5）。
   记事本 AUMID = `Microsoft.WindowsNotepad_8wekyb3d8bbwe!App`。
   → 这**改善**了 PID 的正确性，但**不免除** D1 的锚点验证：
   `ActivateApplication` 在已有实例时可能只是激活既有窗口（E5 的单实例语义），
   因此「拿到的 PID」仍不等于「我要的那个文档所在的窗口」。
   **禁止**依赖任何未文档化的命令行开关（E3 + E5：官方查无 `-w`，社区两说互斥）。
1. **锚点优先（首选，确定性）**：用一个**只有我们知道的 nonce** 定位窗口。
   - 文件类应用（记事本/Excel/Word）：创建唯一命名文件（含 nonce），打开它，
     在候选窗口的 UIA 树里找**含该 nonce 的文档/标签节点** → 其顶层窗口即目标。
   - 浏览器类：打开含 nonce 的本地页面或设置含 nonce 的标题。
   - 无文档载体的应用（画图/计算器）：见步骤 2。
2. **窗口集合差分（次选）**：启动前快照 `{hwnd → (owner_pid, ClassName, IsVisible)}`，
   启动后轮询**新增**且 `ClassName` 匹配、`IsWindowVisible = true` 的顶层窗口；
   命中唯一 → 采纳；命中多个 → **不猜**，返回 `AmbiguousTarget` 让上层消歧（铁律 1）。
3. **等待 UI settled**：命中窗口后轮询到「连续 N 次（默认 3，间隔 100 ms）树节点数与关键子树指纹不变」
   才认定就绪。理由：E1/E3 都表明存在瞬态节点（`TeachingTip`、加载中占位窗口）。
4. **登记 Lease**：把 `(hwnd, owner_pid, ClassName, 命中的 selector 候选序号, RuntimeId, 指纹, 应用版本)`
   写入 Lease；**每次操作前**用指纹复验，失配 → 重解析（Spike B / TASK-004 的主题）。

> 为什么不用「等 stub 退出后剩下的那个 PID」：E1 显示 stub 进程 10 s 后仍存活，
> 且打开第二个文件后是 3 进程 1 窗口 —— 「剩下哪个」本身无定义。

### D2　禁用 `Process.MainWindowHandle` / `MainWindowTitle` 作为身份来源

依据 E2。允许的唯一用途：**诊断日志**（打印出来帮助人类排查），且必须在日志里标注
`heuristic=true`。产品代码里出现 `main_window_handle` 作为定位输入 → review 直接打回。

### D3　窗口枚举统一走 `EnumWindows` + `GetWindowThreadProcessId`

依据 E2（"more reliable than calling GetWindow in a loop"、"only top-level windows of desktop apps"）。
注意 `EnumWindows` **不枚举子窗口** → WinUI3 的 `Popup`（E3 的模态框）若表现为顶层窗口可被枚举到，
但 UIA 树内它挂在主窗口下 → **两条路径都要查**，否则模态框出现时定位会静默拿到主窗口。

### D4　Selector = **有序候选链**，不是单个表达式

```text
SelectorChain {
  app_id: "windows.notepad",
  app_version_range: ">=11.2607 <12",        // E4：aid 不保证跨版本稳定 → 必须带版本区间
  target_role: "editor",
  candidates: [                              // 按置信度降序，命中即停
    C1 { control_type: Document, class_name: "RichEditD2DPT",
         ancestor: { control_type: Pane, class_name: "NotepadTextBox" }, sibling_index: 0 },
    C2 { control_type: Document, class_name: "RichEditD2DPT", sibling_index: 0 },
    C3 { control_type: Document, ancestor: { control_type: TabItem }, sibling_index: 0 },
    C4 { automation_id: "<实测后填>", … },    // 本应用编辑区 aid 为空，故排最后且可能永不命中
  ],
  fingerprint: { node_count_band: [25,45], required_class_names: ["NotepadTextBox"], … },
  on_all_missed: EscalateToHuman,            // 禁止静默降级到"随便找个 Document"
}
```

规则：
- **逐层下钻**（`Children` 优先），禁止无约束 `Descendants` 全树搜索（E4 Caution）。
- 每次解析**记录命中了第几个候选**；若长期命中 C2/C3 而非 C1 → 说明 UI 变了，
  触发「Selector 复审」事件（写入审计日志，供阶段末评审）。
- 候选链本身进 `docs/memory/apps/<app>.md` 第 3 节，**是数据不是代码**（阶段 1 起做成 schema）。

### D5　本地化策略：**不变量优先，本地化名只做最后信号**

优先级（从高到低，全部有官方依据，见 E4）：

1. `ControlType`（programmatic name，不变）
2. `ClassName`（框架类名，不变：`RichEditD2DPT` / `NotepadTextBox` / `Microsoft.UI.Xaml.Controls.TabView`）
3. `AutomationId`（官方契约要求跨语言一致，但**可为空、可重复、可跨版本变**）
4. 结构位置（祖先作用域 + 兄弟索引）
5. `Name` 的**每语言别名表**（`docs/memory/apps/<app>.md` 第 3 节，形如
   `zh-CN:'文件' / en-US:'File' → 语义键 menu.file`）—— **只用于**：① 人类可读日志；
   ② 前 4 项全部失配时的**低置信度**兜底，且必须把结果标 `confidence=low` 并强制人工确认（铁律 6 的同族要求）。

**明确禁止**：`LocalizedControlType`（本地化）、裸 `Name` 作主键、把别名表当主 selector。

### D6　`RuntimeId` 只作会话内比较

依据 E4。可以放进 Lease 与内存缓存；**不得**写库、不得写 fixture、不得解析其整数结构
（"format can change"、"identifiers can be reused"）。跨会话一律重新解析 selector。

### D7　录制回放 fixture 必须容忍瞬态节点

E1/E3 与 2026-09-17 普查均出现 `TeachingTip`（`PrivacyTeachingTip`、`PsDownloadLightTeachingTip`
——后者文案提到「本地 AI 模型必须完成下载」，即**首次运行态与 AI 模型下载态会增删节点**）。
→ fixture 采集必须在 D1 步骤 3 的 UI settled 之后；比对采用「关键子树指纹 + 白名单忽略路径」，
**不做**全树逐节点相等比较。（细化归 TASK-034，此处只定原则。）

### D8　坐标一律按**物理像素**处理，客户端进程必须 dpi-aware

依据 E8。规则：

- Host 进程（以及任何调用 UIA 的进程）必须在启动早期声明 **Per-Monitor V2** dpi-aware；
  未声明 → 官方明示「无法获得正确结果」，且**不会报错**（属静默错误，铁律 1 的头号敌人）。
- `BoundingRectangle` / `GetClickablePoint` / `FromPoint` 的取值**不得**再除以缩放比。
- **禁用**逻辑坐标 API（如 .NET `Cursor.Position`）作为点击依据。
- 该契约是 TASK-003（Paint 坐标精度矩阵）的前置条件：不先声明 dpi-aware，
  那张矩阵测出来的「误差」全是假的。

### D9　绑定前置条件：会话恢复与瞬态节点必须**显式处置**，不得靠等待

依据 E9 与 E1/E3 中反复出现的 `TeachingTip`。规则：

1. Adapter 的 `preconditions` 必须显式声明「目标应用的会话恢复/上次状态**不影响本次目标**」
   的处置方式：或关闭该设置（经设置 UI，因 `HKCU\Software\Microsoft\Notepad` 不存在），
   或在发现算法里**容忍并跳过**已有标签页。
2. 瞬态节点（`PrivacyTeachingTip` / `PsDownloadDetailTeachingTip` / `PsDownloadLightTeachingTip`）
   → **检测到就显式关闭**（`Invoke` 其关闭按钮）后重新定位；
   **禁止**用「等 N 毫秒它就消失了」这种时序赌注（官方无生命周期文档，E7/(c)）。
3. 「文件默认开在新标签还是新窗口」是**用户可改设置**（E9）→ Adapter 不得假设任一行为，
   两种都要能工作；测试前置步骤必须显式校验或统一该设置。
4. 标签可被**拖拽撕出**成独立窗口（E5 的 `TabTearingOutWindowContext`）→ Lease 复验必须
   容忍「目标文档迁移到另一个 hwnd」，处置方式 = 重新走 D1 发现流程，而不是报 `TargetNotFound`。

## 影响（需要改的文件）

- `docs/memory/apps/notepad.md`：本 ADR 的**每应用实例**（进程模型、候选链、别名表、文本约定）。
- `docs/spike-reports/SPIKE-A.md`：§2.1 结论表补 D1~D7 的落点；§4 坑 3 升级为「已定契约」。
- `plans/stage-0-spikes.md` TASK-002 步骤 3 / TASK-004：Rust 侧必须实现 D1 的发现算法与 D3 的枚举方式。
- `crates/platform/windows`（阶段 1）：`WindowDiscovery` / `SelectorResolver` / `Lease` 三个组件按本 ADR 实现。
- `docs/spec/`：阶段 1 需要一份 `target-descriptor.md`（把 D4 的 `SelectorChain` 变成 schema）→ 已登记 `docs/memory/open.md`。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 用启动 PID 定位窗口 | ❌ 否决 | E1 实测直接失效（stub 进程模型） |
| 2 | 用 `Process.MainWindowHandle` | ❌ 否决 | E2 官方明示是启发式猜测，且依赖焦点/可见性 |
| 3 | 用 `notepad -w` 强制新窗口以对齐 PID | ❌ 否决 | E3 实测：`-w` 被当文件名，触发「文件名无效。」，0 个 TabItem |
| 4 | 用 `AppDiagnosticInfo` / AUMID 按应用身份聚合进程 | ⏸ 推迟 | 能解决「同应用多实例分组」，但**仍不能**回答「哪个窗口是我刚打开的这个文档」→ 不能替代 D1 的锚点法；登记为阶段 1 的补充候选 |
| 5 | 只用 `AutomationId` 作主键 | ❌ 否决 | E4：可为空、可不唯一、**不保证跨版本稳定** |
| 6 | 只用本地化 `Name` + 每语言字符串表 | ❌ 否决 | E4 官方明示 Name 不能作同级唯一标识；且 AGENTS.md §7 已禁 |
| 7 | 视觉/OCR 兜底定位（L5） | ⏸ 保留但降级 | 违反铁律 5 的优先级；仅在 L3 全链失配且人工确认后使用 |
| 8 | **锚点式窗口发现 + 有序候选链 + 版本指纹** | ✅ **采纳** | 每一环都有官方依据或实测证据；失败模式是「显式升级」而非「静默错定位」 |
| 9 | AUMID + `ActivateApplication` 取真实宿主 PID | ✅ **采纳为 D1 步骤 0** | 官方 API，比 `Start-Process` 正确；但**不能替代**锚点验证（单实例语义下可能只是激活既有窗口） |
| 10 | 依赖 `notepad -w` / `/W` 强制新窗口 | ❌ **否决（双重证据）** | E3 实测：被当文件名并弹「文件名无效。」；E5：官方文档查无此开关，社区两说互斥（新窗口 vs Unicode 打开） |
| 11 | 「FindAll + 固定索引」作主定位 | ❌ 否决 | E6：官方只承诺「按树中遇到的顺序返回」，**未承诺**跨会话稳定 → 索引只能作弱约束并配指纹复验 |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| nonce 锚点法对「无文档载体」应用不适用 | D1 步骤 2 的窗口差分 + Lease 互斥；差分命中多个时返回 `AmbiguousTarget`，不猜 |
| 候选链是手工实测数据，应用升级后静默失效 | D4 要求记录「命中第几个候选」并触发 Selector 复审；`app_version_range` 失配 → 显式失败而非降级 |
| 别名表随语言/版本膨胀 | 别名表只服务日志与低置信兜底，不追求完备；缺项 = 兜底不可用 = 显式升级，不是错误 |
| UI settled 判定过松 → 拿到瞬态树 | 连续 N 次指纹不变 + 关键子树必需类名（`NotepadTextBox`）双条件 |
| `EnumWindows` 漏掉 UIA 树内的 `Popup` | D3 要求两条路径都查 |
| 进程未声明 dpi-aware → 坐标全错但**不报错** | D8：启动早期声明 Per-Monitor V2；TASK-003 的坐标矩阵必须先验证 dpi-aware 已生效 |
| 会话恢复把上次的标签页带进来 → 定位到错误文档 | D9 第 1 条：preconditions 显式处置；探针一律「先杀干净 + 唯一文件名 nonce」 |
| 用户拖拽标签撕出新窗口 → hwnd 变了 | D9 第 4 条：Lease 复验失败时重走 D1 发现流程，而非直接判 `TargetNotFound` |

## 验证方式

1. TASK-002 步骤 3（Rust 路径）用 D1 算法在**已存在实例**与**全新启动**两种前提下各定位 10 次，成功率 100%。
2. 故意把候选链的 C1 类名改错 → 必须命中 C2 并记录「候选序号 = 2」；把全部候选改错 → 必须返回显式失败（不得静默成功）。
3. 系统显示语言切到 en-US（或用 `Set-WinUILanguageOverride`）后重跑 → C1/C2 仍命中（证明未依赖本地化 Name）。
4. 最小化窗口 / 切到另一虚拟桌面后重跑 → 仍命中（证明未依赖 `MainWindowHandle` 与可见性）。
   （第 3、4 项即 TASK-002 步骤 6 失败注入的一部分。）
5. 用 AUMID + `ActivateApplication` 启动（D1 步骤 0），断言返回的 `processId` 与
   `GetWindowThreadProcessId(hwnd)` 一致；再用 `Start-Process notepad` 做对照，断言两者**不一致**
   （这条对照测试的价值是「防止有人哪天把启动方式改回 `Start-Process` 而没人发现」）。
6. 断言 Host 进程的 dpi-awareness context 为 Per-Monitor V2（`GetThreadDpiAwarenessContext`），
   并把 `BoundingRectangle` 与 `GetPhysicalCursorPos` 在同一点上交叉验证（D8）。

## 重新评估的触发条件

- 记事本或任何目标应用发布新版本且候选链全链失配 → 说明 D4 的版本区间机制需要升级为「自动重学习」。
- 阶段 1 出现「同一应用多窗口且无文档载体」的真实需求 → 重新评估选项 4（AUMID 聚合）。
- 微软为打包应用提供官方的「启动 → 窗口」关联 API（例如 `AppInstance` 级别的窗口句柄回传）→ 可简化 D1。
