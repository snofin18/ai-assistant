# SPIKE-A　Notepad（Windows 11 新版记事本）UIA 实测报告

> ## ⚠️ 状态：**PARTIAL（进行中）** —— 不得当作 go/no-go 结论使用
>
> **已完成**：卡面步骤 **1（环境记录）全部**、步骤 **2 的一部分**（窗口/编辑区/标签/菜单栏/状态栏普查；
> **未含**菜单展开后的子项与「另存为」跨进程对话框）、步骤 **5 的一部分**（定位与遍历耗时中位数、
> 中文 `SetValue` 正确性；**未含**大文件与 IME 两态）。
> **未完成**：步骤 **3（Rust 生产路径，阻塞在依赖批准）**、步骤 **4（接口考古 8 步）**、
> 步骤 **5 剩余**、步骤 **6（失败注入 4 种）**。
> **执行方式**：人类会话的**先导侦察**（2026-09-17），非 TASK-002 正式执行。
> 正式报告由 TASK-002 会话续写；续写时**不要删本文件已有内容**，按「只追加 + 标注更正」处理。
> **复现脚本**：`spikes/spike-a-notepad/probe-01-tree-survey.ps1`、`probe-02-text-and-timing.ps1`
> （零第三方依赖，只用 Windows 自带 UIAutomation 程序集）。

---

## 1. 环境（卡面步骤 1，已完成）

| 项 | 实测值 | 取法 |
|---|---|---|
| OS | **Windows 11 25H2，build 26200.9457** | `HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion` 的 `DisplayVersion` + `CurrentBuild` + `UBR`。⚠️ 同键的 `ProductName` **谎报 "Windows 10 Pro"** |
| 记事本 | **`Microsoft.WindowsNotepad` 11.2607.14.0 x64**（打包版、含标签页；非 Win32 老版） | `Get-AppxPackage Microsoft.WindowsNotepad`；路径 `C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11.2607.14.0_x64__8wekyb3d8bbwe\Notepad\Notepad.exe` |
| 显示 / 缩放 | **3200×2000，AppliedDPI=192（200% 缩放）**，NVIDIA GeForce RTX 5060 Laptop GPU | `Win32_VideoController` + `HKCU:\Control Panel\Desktop\WindowMetrics` |
| 输入法 | `zh-Hans-CN`（首位，微软拼音 TIP `0804:{81D4E9C9-1D3B-41BC-9E6C-4B40BF79E35E}{FA550B04-5AD7-411F-A5AC-CA038EC515D7}`）、`en-US`、`en-GB` | `Get-WinUserLanguageList` |
| 窗口矩形 | `448,458,2300,1446`（200% 缩放下，UIA 报的是**物理像素**） | `AutomationElement.Current.BoundingRectangle` |
| 记事本内存 | WorkingSet ≈ **118.8 MB**（空载 + 1 个 62 字符文件） | `Get-Process.WorkingSet64` |
| a11y 工具 | `inspect.exe` **已就绪**：`C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\inspect.exe`（另有 x86 / arm64）。**Accessibility Insights for Windows 未安装** | 文件系统探测 |

## 2. 控件普查（步骤 2 的一部分）

整棵控件树 **32 个节点**（`ControlViewWalker`，深度 ≤8）。原始输出（`probe-01`）：

```text
Window       aid=''                  cls=Notepad                                     name='<file> - Notepad'
  Pane       aid=''                  cls=NotepadTextBox                              name=''
    Document aid=''                  cls=RichEditD2DPT                               name='文本编辑器'
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
      Window   aid='PrivacyTeachingTip'          cls=…Controls.TeachingTip
      Pane     aid='PsDownloadDetailTeachingTip' cls=…Controls.TeachingTip
      Pane     aid='PsDownloadLightTeachingTip'  cls=…Controls.TeachingTip             name='本地 AI 模型必须完成下载才可在注销后使用此功能。'
  Pane       cls=Microsoft.UI.Content.DesktopChildSiteBridge
    Pane     cls=InputSiteWindowClass
      Text   aid='ContentTextBlock'  cls=TextBlock                                    name='行 1， 列 1'
      Text   aid='ContentTextBlock'  cls=TextBlock                                    name='65 个字符'
      Button aid='ContentButton'     cls=Button                                       name=''
        Text aid='ContentButtonText' cls=TextBlock                                    name=''
      Text   aid='ContentTextBlock'  cls=TextBlock                                    name=''
      Text   aid='ContentTextBlock'  cls=TextBlock                                    name='缩放'
      Text   aid='ContentTextBlock'  cls=TextBlock                                    name=' Windows (CRLF)'
      Text   aid='ContentTextBlock'  cls=TextBlock                                    name=' UTF-8'
```

### 2.1 关键结论

| 观察 | 后果（对 Adapter 设计） |
|---|---|
| **编辑区 `Document` 的 AutomationId 为空**，ClassName `RichEditD2DPT`，Name `文本编辑器`（本地化） | 主 selector 只能用 **ControlType.Document + 父级 ClassName `NotepadTextBox`** 组合；不能依赖 AutomationId，也不能用本地化 Name |
| `ValuePattern` 可读写（`IsReadOnly=False`），**`TextPattern` 也支持**，两者返回文本**完全相同** | L3 读全文有两条等价通道；**优先 `TextPattern`**（能做范围/选区/插入），`ValuePattern` 作降级 |
| **AutomationId 是英文、Name 是本地化文本**（`aid='File'` ↔ `name='文件'`；`aid='CloseButton'` ↔ `name='关闭标签页'`） | 实证支持 AGENTS.md「禁止用控件可见文本作主 selector」；跨语言稳定性靠 AutomationId |
| **AutomationId 不唯一**：状态栏有 **6 个** `Text aid='ContentTextBlock'` | 状态栏必须靠**索引 + Name 模式**区分（行/列、字符数、缩放、换行符、编码）；`aid` 不能当主键 |
| 标签页：`Tab aid='Tabs'` → `List aid='TabListView'` → `TabItem`（Name 形如 `<file>. 未修改。`）+ `Button aid='CloseButton'`；新建标签 `Button aid='AddButton'` | **寻址单位是 TabItem 不是窗口**；TabItem 的 Name **自带脏标记**（"未修改。"），可作辅助信号，但属本地化文本、不可作主 selector |
| 存在**瞬态** `TeachingTip`（`PrivacyTeachingTip` / `PsDownloadDetailTeachingTip` / `PsDownloadLightTeachingTip`，后者提到"本地 AI 模型"下载） | **UIA 树不稳定**：首次运行态、AI 模型下载态会增删节点。录制回放 fixture（架构 v2 §17.4）必须在 UI **settled** 后采集，且比对要容忍这类瞬态节点 |
| 窗口本体 `aid=''`、`cls=Notepad`、Name `<file> - Notepad` | 窗口定位只能靠 **ClassName `Notepad` + Name 后缀 `- Notepad`**；Name 含文件名 → 多标签时标题只反映**当前**标签 |

## 3. 实测数据（步骤 5 的一部分；每项 **10 次取中位数**，`probe-02`）

| 指标 | 中位数 | min | max | 卡面 go 判据 | 判定 |
|---|---|---|---|---|---|
| 全窗口树遍历（32 节点） | **17.0 ms** | 15.6 | 30.0 | ≤ 800 ms | ✅ **余量 47×** |
| 同上，**冷启动首次** | 74.2 ms | — | — | ≤ 800 ms | ✅（首次仍远低于阈值） |
| 局部搜索编辑区（`Descendants` + OrCondition） | **1.5 ms** | 1.2 | 1.9 | ≤ 200 ms | ✅ **余量 133×** |
| `ValuePattern.GetValue`（59 字符） | **0.11 ms** | 0.08 | 0.25 | — | ✅ |
| `ValuePattern.SetValue`（57 个中英混排字符） | **4.79 ms** | — | — | — | ✅ 读回归一化后**完全一致** |
| 1 KB / 100 KB / 1 MB 读写耗时与内存增量 | **未测** | — | — | 1 MB ≤ 2 s 且内存增量 ≤ 100 MB | ⏳ |
| 中文写入 IME 开/关两态 | **未测**（见 §5.3） | — | — | IME 开启下 100% 正确 | ⏳ |
| 跨进程「另存为」对话框解析成功率 | **未测** | — | — | ≥ 90% | ⏳ |
| 关键控件定位成功率 | 编辑区 **10/10 = 100%**；菜单子项 / 另存为框 **未测** | — | — | ≥ 90% | ⏳ 部分 |

**文本往返正确性**：磁盘文件 62 字符（CRLF）→ `ValuePattern` 读回 **59 字符**（裸 CR）→
把 `\r\n`、`\r`、`\n` 全部归一化后与原文**完全相等**（`True`）。`TextPattern.DocumentRange.GetText(-1)`
返回与 `ValuePattern` **逐字符相同**的 59 字符。

## 4. 已发现的坑（已同步 `MEMORY.md` §5）

1. **UIA 返回的文本用裸 `\r` 分行，磁盘文件是 `\r\n`** → 读回校验必须先归一化，否则必然假阴性。
2. **打开一个已在标签页中的文件，记事本不重新加载磁盘内容** → 探针必须先杀进程 + 用带时间戳的唯一文件名；
   对产品同样重要：外部改了 `.txt`，记事本内存态不会自动更新（"文件契约"与 UI 态可能不一致）。
3. **`Start-Process notepad` 返回的 PID ≠ 持有窗口的 PID**（实测 30464 vs 20304，事先杀干净仍如此）；
   打开第二个文件后系统里有 **3 个 Notepad 进程、1 个窗口、2 个 TabItem** → **PID 不可用于定位**。
4. **`SetValue` 不经键盘，IME 开/关对它无影响** → 卡面的 IME 两态要求只对 **L4 合成键盘输入**有意义。
5. **PowerShell 控制台输出中文会乱码**，但进程内字符串比较不受影响 → 判定看 `True/False` 与长度，不要肉眼看控制台。

## 5. 未完成项、阻塞与对卡面的疑问

### 5.1 未完成（TASK-002 剩余）

- 步骤 2 剩余：展开「文件 / 编辑 / 查看」普查子菜单项；触发并普查**「另存为」跨进程 Shell 对话框**
  （文件名输入框、目录树、保存按钮，注意它属**另一个进程**）。
- 步骤 3：**Rust 生产路径验证**（`windows` crate + `uiautomation`）—— go/no-go 最终必须以它为准。
- 步骤 4：接口考古 8 步（`target-apps-feasibility.md` §5）。预期结论是记事本只有"文件契约"这一条 L1 通道。
- 步骤 5 剩余：1 KB / 100 KB / 1 MB 的读写耗时与内存增量；IME 开/关两态（限 L4 路径）。
- 步骤 6：失败注入 —— 记事本被关闭 / 最小化 / 在另一虚拟桌面 / 未保存弹窗出现。

### 5.2 阻塞：需人类裁决 3 项

1. **spike 依赖**：`windows` crate 已在 `docs/DEPENDENCIES.md`「计划首批」清单内，但 **`uiautomation` 不在**。
   是否批准？还是改用 `windows` crate 自带的 UIAutomation COM 绑定（少一个供应链依赖，代价是多写胶水代码）？
2. **PL-019**：`Cargo.toml` 的 `exclude = ["spikes", …]` 使 deny/fmt/clippy/test 四道硬门禁**都不覆盖 spike 代码**。
   三个候选方案见 `docs/PARKING_LOT.md` PL-019。
3. **步骤 2 指定工具**：Accessibility Insights for Windows **未安装** —— 补装（`winget install Microsoft.AccessibilityInsights`），
   还是以 `inspect.exe` + 自写 UIA 树导出替代？（本次普查已用后者完成，效果足够，可作为默认答案）

### 5.3 对卡面措辞的疑问（**走 DRIFT/ADR，不自行改**）

卡面步骤 5 要求「中文文本经 `SetValue` 写入的正确性（**分别在 IME 开/关两种状态下测**）」。
实测表明 `SetValue` 走 UIA Pattern、**不经过键盘与 IME**，因此该两态对照对 `SetValue` 是空操作。
建议把该条改为「**L4 合成键盘输入**路径下分别在 IME 开/关两态测中文写入；`SetValue` 路径只需单态验证」。
→ 这是卡面措辞变更，应由 TASK-002 会话记为 `DRIFT-002-x` 交人类裁决，**本次侦察不改卡面**。

## 6. 初步倾向（**不是结论**，待步骤 3/5/6 完成后才能定 go/no-go）

- L3（UIA）通道对新版记事本**极其宽松**：树只有 32 节点、遍历 17 ms、读写都是亚毫秒到毫秒级，
  相对 go 判据有 47×~133× 余量。定位稳定性的风险不在性能，而在
  **① AutomationId 为空/不唯一、② 瞬态 TeachingTip 让树不稳定、③ 寻址单位是 TabItem 而非窗口**。
- **真正的风险集中在还没测的三处**：跨进程「另存为」对话框、1 MB 大文件、以及 **Rust 侧是否与
  PowerShell 侧表现一致**（PowerShell 用的是 UIAutomation 托管封装，Rust 走 COM，
  两者的缓存/批量取属性行为不同，耗时可能差一个数量级）。
- 因此**在 Rust 路径跑通之前不应给 go**。若 Rust 侧性能显著劣于托管侧，需要重新评估
  架构 v2 里「Host 进程内完成定位与执行」的批量取属性策略。
