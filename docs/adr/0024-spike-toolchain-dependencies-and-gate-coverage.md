# ADR-0024　Spike 的工具链、依赖与门禁覆盖

状态：Accepted　日期：2026-09-18　Supersedes：—　Superseded by：—
> ⚠️ D1 原列的 feature 清单**有一项不存在**，已由下方 **D1a**（同日实证）修正。
> D1 的裁决本身（用 `windows` crate、否决 `uiautomation`）**不变且已被实证支持**。
关联：`docs/DEPENDENCIES.md`、`.github/workflows/ci.yml`、`.github/workflows/gate-selftest.yml`、
ADR-0019（元门禁）、`docs/PARKING_LOT.md` PL-019、`docs/spike-reports/SPIKE-A.md` §5.2、
`spikes/spike-a-notepad/README.md`、人类指示 #6 #7 #8（2026-09-18）

## 背景（为什么现在要决定）

Spike A 报告 §5.2 列了三条阻塞，全部与「spike 用什么工具、什么依赖、受什么门禁约束」有关，
人类已于 2026-09-18 逐条给出方向（指示 #6 #7 #8）：

1. **#6 依赖**：Rust 侧 UIA 访问，用第三方封装 crate（`uiautomation`）还是
   `windows` crate **自带的 UIAutomation COM 绑定**？→ 人类裁定：**用 `windows` crate 自带绑定**。
2. **#7 门禁**：`Cargo.toml` 的 `exclude = ["spikes", "fixtures/apps", "tools"]` 使
   deny / fmt / clippy / test **四道硬门禁全都覆盖不到 spike 代码**（PL-019）→
   人类裁定：**给 spikes 单独一个只查 licenses + sources 的 deny job**。
3. **#8 普查工具**：卡面步骤 2 指定的 **Accessibility Insights for Windows 未安装** →
   人类裁定：**以 `inspect.exe` + 自写 UIA 树导出替代**。

## 决策

### D1　Rust 侧 UIA 一律用 `windows` crate 的 COM 绑定，**否决** `uiautomation` crate

| 维度 | `windows`（Microsoft 官方，0.62.2） | `uiautomation`（第三方封装，0.25.1） |
|---|---|---|
| 供应链 | 微软官方发布，已在 DEPENDENCIES.md「计划首批依赖」清单内 | 不在清单内，需新增一个外部维护者 |
| 与生产代码的一致性 | **产品代码（`crates/platform/windows`）本来就要用它** → spike 结论可直接迁移 | spike 用 A 库、产品用 B 库 → **spike 证明的东西不等于产品会遇到的东西** |
| 性能结论的可信度 | 直接反映 COM 调用开销（含缓存/批量取属性策略） | 封装层可能自带缓存，**掩盖**真实 COM 成本 → Spike A 的 go/no-go 判据会被污染 |
| 许可证 | MIT OR Apache-2.0（引入时由 `cargo deny check licenses` 机器确认） | 需单独确认 |
| 代码量 | 多（`CUIAutomation8` / `IUIAutomationElement` / BSTR / VARIANT 手工处理） | 少 |

**裁决理由（决定性的一条）**：Spike A 的 go/no-go 判据里最不确定的一项就是
「Rust 走 COM 是否与 PowerShell 走托管封装表现一致」（SPIKE-A §6）。
若 spike 也用一层第三方封装，这个不确定性**不会被消除，只会被推迟到阶段 1**，
而那时改动的成本已经高得多。**spike 的意义就是用生产路径去撞墙。**

**代价与缓解**：多写的胶水代码（约 100~200 行：`CUIAutomation8` 初始化、条件构造、
BSTR→String、VARIANT→值、CacheRequest 批量取属性）**不是浪费** —— 它就是
`crates/platform/windows` 的雏形。规则：spike 里的 UIA 胶水必须写成
「可整体搬走」的独立模块（单一文件、不依赖 spike 特有逻辑），并在文件头注明
`// 去向：crates/platform/windows/src/uia/`。

**需要的 feature**：~~见原清单~~ → **已由 D1a 实证修正，以 D1a 的表为准**。

### D1a　实证修正（2026-09-18，本机）：feature 名不是 `Win32_UI_UIAutomation`

D1 原清单里的 `Win32_UI_UIAutomation` **在 `windows` crate 里不存在**。落地时先做了机械验证，
双重证据：

1. 从 `static.crates.io` 下载 `windows-0.62.2.crate` 原包，其 `Cargo.toml` 的 `Win32_UI_*`
   feature 共 26 项（Accessibility / Animation / ColorSystem / Controls / Controls_Dialogs /
   Controls_RichEdit / HiDpi / Input / Input_Ime / Input_Ink / Input_KeyboardAndMouse /
   Input_Pointer / Input_Radial / Input_Touch / Input_XboxController / InteractionContext /
   LegacyWindowsEnvironmentFeatures / Magnification / Notifications / Ribbon / Shell /
   Shell_Common / Shell_PropertiesSystem / TabletPC / TextServices / WindowsAndMessaging / Wpf）
   —— **没有任何 UIAutomation 项**。全包里唯一带 `UIAutomation` 字样的是 **WinRT** 命名空间的
   `UI_UIAutomation` 与 `UI_UIAutomation_Core`（即 `Windows.UI.UIAutomation`，主要是 provider 侧）。
2. crates.io 稀疏索引实测 `windows` 共 81 个版本、**最新即 0.62.2** → 「换个版本就有」不成立。

**真相**：客户端 UIA COM 绑定**确实存在**，只是与 MSAA（`IAccessible`）和 provider 侧接口
（`IRawElementProviderSimple`、各 `I*Provider`）**一起放在 `Win32::UI::Accessibility` 模块里**。
实测 `windows-0.62.2/src/Windows/Win32/UI/Accessibility/mod.rs`（约 2 万行）含：
`CUIAutomation`（CLSID GUID，L904）、`IUIAutomation`（L6806）、`IUIAutomationElement`、
`IUIAutomationElementArray`、`IUIAutomationTreeWalker`、`IUIAutomationCacheRequest`、
`IUIAutomationCondition`、`IUIAutomationValuePattern`，以及 `UIA_*PropertyId` /
`UIA_*ControlTypeId` / `UIA_*PatternId` / `TreeScope_*` / `PropertyConditionFlags_*` 全套常量
（共 1381 个 trait/struct/fn 条目）。

**修正后的 feature 清单（D1 原清单作废）**：

| feature | 提供什么 | 少写会怎样（本机实测报错） |
|---|---|---|
| **`Win32_UI_Accessibility`** | **`IUIAutomation` 全套客户端 COM 绑定** | `unresolved import` |
| **`Win32_System_Ole`** | **`VARIANT` 类型本体** | `no VARIANT in Win32::System::Variant` —— `VARIANT` 被 `#[cfg(all(feature = "Win32_System_Com", feature = "Win32_System_Ole"))]` **双重 gate**，而 `CreatePropertyCondition` 的签名需要 `VARIANT` |
| `Win32_System_Variant` | `VARIANT` 所在模块路径 | 同上 |
| `Win32_System_Com` | `CoInitializeEx` / `CoCreateInstance` / `CLSCTX_INPROC_SERVER` | `unresolved import` |
| `Win32_Foundation` | `HWND` / `WPARAM` / `LPARAM` / `CloseHandle` | 注意 **`BOOL` 在 0.62 已移到 `windows::core`**，写 `Win32::Foundation::BOOL` 会报 `no BOOL in Win32::Foundation` |
| `Win32_UI_WindowsAndMessaging` | `EnumWindows` / `GetWindowThreadProcessId` / `PostMessageW` | `unresolved import` |
| `Win32_System_Threading` | `OpenProcess` / `QueryFullProcessImageNameW`（owner PID → 映像名） | `unresolved import` |
| `Win32_UI_HiDpi` | ADR-0022 D8 的 Per-Monitor V2 声明 | 本次探针未用到 → **阶段 1 再加**（不提前引入未验证的 feature） |
| `Win32_UI_Input_KeyboardAndMouse` | L4 合成输入 | 同上，阶段 1 再加 |

**结论：D1 的裁决不变**，且前提从「推测」升级为「实证」。

#### 实证载体与结果

`spikes/spike-a-notepad/src/bin/uia_dep_proof.rs`（`windows = "=0.62.2"`，只用上表前 7 项 feature）。
本机 2026-09-18：`cargo build` **零警告**，运行 **E1~E6 全 PASS，exit 0**。

| 证据 | 实测输出 | 印证 |
|---|---|---|
| E1 | `CoCreateInstance(CLSID_CUIAutomation) -> IUIAutomation` PASS | feature 名正确、COM 类可实例化 |
| E2 | `GetRootElement name="桌面 1" class="#32769"` | COM 调用真的打到 UIAutomationCore（不是空壳绑定） |
| E3 | `hwnd=0x540d38 owner_pid=13168 launched_pid=2632 same_pid=false`；窗口在第 **3** 次枚举扫描后出现（≈450 ms） | **ADR-0022 D1**：启动 PID ≠ 窗口属主 PID，在 Rust/COM 路径同样成立；且必须轮询等待，不能"启动后立刻查" |
| E4 | 命中候选链**第 1 项** `Document + RichEditD2DPT`；`aid=""`；`raw_chars=25 raw_cr=3 raw_lf=0 norm_eq=true cjk_kept=true`（磁盘 32 字节 / 28 字符 / CRLF） | **ADR-0022 D5**（有序候选链有效）、**ADR-0022 D4**（文档区 AutomationId 为空 → 根本不能当 selector）、**ADR-0023**（UIA 返回裸 CR；28−3=25 与"3 个 CRLF 各丢 1 字符"精确吻合；归一化后逐字符相等；CJK 无损） |
| E5 | 候选链定位中位数 **2349 µs**（min 1976 / max 3049，n=10） | **解除 SPIKE-A §6 的最大不确定性**：Rust COM 路径与 PowerShell 托管封装（1.5 ms 定位）**同一数量级**，不存在"差一个数量级"的风险 → 架构 v2「Host 进程内完成定位与执行」的批量取属性策略无需重估 |
| E6 | 树中不存在的 ControlType（DataGrid）解析为 `None` | 「没找到」与「出错」可区分（见下方陷阱 1） |

#### 由此新增的坑（同步进 `docs/memory/pitfalls.md`）

1. **`FindFirst` 的「没找到」= `Err(HRESULT(0x00000000))`。** UIA 用 `S_OK` + **NULL 元素指针**
   表示无匹配，`windows`-rs 把它转成 `Err`，而这个 error 的 `code` 恰恰是「成功」。
   直接 `?` 会得到一条 `message: "操作成功完成。"` 的荒谬错误（本机首次运行就是这样失败的）；
   按「HRESULT 是不是错误码」分支则会**判反**。
   **规则**：显式把 `code == 0` 映射成 `None`，其余 HRESULT 原样上抛 →
   产品侧对应 `ErrorCode::TargetNotFound`，与「selector 写错」「目标已消失」必须可区分
   （AGENTS.md 铁律 1：无静默失败，也不得用荒谬错误冒充失败原因）。
2. **记事本文档区：`ControlType=Document`（不是 `Edit`）、`ClassName=RichEditD2DPT`、
   `AutomationId=""`。** 只按 `Edit` 找会静默落空并触发陷阱 1。→ 已固化为
   `docs/memory/apps/notepad.md` 的 selector 候选链。
3. **spike 自己的 `Cargo.toml` 必须有 `license` 字段**，否则 `cargo deny check licenses` 报
   `error[unlicensed]: <crate> is unlicensed`（本机实测 exit 4，`windows` 全家反而全部通过）。
   这条与「依赖的许可证」无关，是**被检查的包自身**没声明许可证 → 已在 spike manifest 写
   `license = "MIT"`（与仓库根 `LICENSE`、根 `Cargo.toml` 的 `[workspace.package].license` 一致）；
   并建议 TASK-015 把「所有 `Cargo.toml` 必须有 license 字段」做成 `xtask hygiene` 规则。
4. **`notepad.exe` 必须用 `Stdio::null()` 启动。** Notepad 11 是把文件转交给打包进程的 stub，
   若继承探针的 stdout 句柄，任何「等 EOF」的管道会在 `main` 返回后**一直挂着**
   （本机首次运行即因此看不到任何输出）。
5. **`windows` 0.62 的签名细节**：`OpenProcess` 的 `binherithandle` 是 **`bool`**（不是 `BOOL`）；
   `QueryFullProcessImageNameW` 收 **`PWSTR`**（不是 `PCWSTR`）；
   `PostMessageW` 收 `Option<HWND>` + `WPARAM`/`LPARAM`（不是 `None, None`）；
   edition 2024 下 `unsafe fn` 体内仍需显式 `unsafe {}` 块。

### D2　`spikes/` 单独一道 deny 门禁：**只查 `licenses` + `sources`**（PL-019 采纳方案 ①）

```yaml
spike-deny:
  name: cargo deny (spikes only)
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: enumerate spike manifests
      # 铁律 1：无 spike crate 时必须显式打印「无」再 exit 0，不得静默
    - name: cargo deny check licenses sources --manifest-path <each>
```

设计取舍：

| 决定 | 理由 |
|---|---|
| **只查 `licenses` + `sources`** | 这两项**不需要联网**（`advisories` 要 clone advisory-db，慢且与 spike 的一次性性质不匹配）；而 spike 最真实的风险恰恰是「agent 图方便 `cargo add` 一个许可证不兼容或来源不明的包」 |
| **不查 `bans`** | spike 之间、spike 与主 workspace 之间出现重复版本是**正常的**（不同 spike 可能钉不同版本），查了只会得到永久噪声 |
| **不查 `advisories`** | 一次性验证代码不进产物；安全公告的意义在发布链路上，spike 不在发布链路上 |
| **遍历而非硬编码路径** | `spikes/*/Cargo.toml` 由脚本枚举 → 新增 spike 自动被覆盖，不会漏 |
| **无 spike crate 时显式打印「无」** | 铁律 1（无静默失败）。否则「一个都没查到」与「查了都通过」在日志里无法区分 —— 这正是 ADR-0019 要消灭的失败模式 |
| **fmt / clippy / test 仍不覆盖 spikes** | 阶段 0 的 spike 是一次性验证代码，让它们过 clippy pedantic 会把精力从「验证假设」转移到「让 lint 满意」。这是**有意的**豁免，写在这里而不是默默存在 |

**元门禁（ADR-0019 要求）**：新硬门禁必须配负向验证 → 在 `gate-selftest.yml` 增加
`spike-deny` canary：造一个**本地 path 依赖 + 假 GPL 许可证**的 fixture crate（免联网），
断言 `cargo deny check licenses` 非零退出。ADR-0019 登记表补一行。

**落地实测（2026-09-18，本机）**：

- 负向 fixture `spike-canary-bad`（本地 path 依赖 + `license = "GPL-3.0-only"`）→
  exit **4**，输出含 `error[rejected]` / `license is not explicitly allowed` / `GPL-3.0-only` ✅
  canary 的断言就钉在这三段具体文本上（ADR-0019：断言不得与故障模式脱钩）。
- 正向：`spikes/spike-a-notepad/Cargo.toml`（真实 `windows =0.62.2` 依赖）→
  `licenses ok, sources ok`，exit **0** ✅ —— 这也意味着 spike-deny 门禁**从此有真实检查对象**，
  不再走「无 spike manifest，显式通过」那条分支。
- 途中抓到陷阱 3（spike 自身缺 `license` 字段 → `error[unlicensed]`）。
  这正是本门禁的价值：它在**第一个真实依赖**进来时就抓到了一个与许可证无关、
  但会让门禁变红的配置错误。

### D3　控件普查工具：`inspect.exe` + **自写 UIA 树导出**，不安装 Accessibility Insights

| 工具 | 状态 | 用途 |
|---|---|---|
| `inspect.exe`（Windows SDK，本机已装 `C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\inspect.exe`） | ✅ 就绪 | **人工点选**单个元素，看它的 `ControlType`/`AutomationId`/`Name`/`ClassName`/Pattern 支持位/坐标 |
| **自写 UIA 树导出**（`spikes/spike-a-notepad/probe-01-tree-survey.ps1`） | ✅ 已有 | **整棵树**结构化导出 + 计时 + 可重复运行 + 结果可 diff（`inspect.exe` 做不到这三件） |
| Accessibility Insights for Windows | ❌ 不安装 | 功能与上两者重叠；多一个 GUI 依赖，且**其输出无法进入版本控制/diff**，对 AI agent 协作无增益 |
| `winapp ui inspect/search/click/invoke/set-value`（微软官方 WinUI3 测试 CLI，见 ADR-0022 E7） | ⏸ 记为候选 | 官方工具、命令行可脚本化，**比 Accessibility Insights 更适合本项目**；但它面向「自己开发的 WinUI3 应用」，对第三方应用的适用性未验证 → 登记 `docs/memory/open.md`，TASK-003（Paint）时顺手一试 |

**为什么自写导出是必须的**（不只是「够用」）：
① 结果要能**进 git 并 diff**（ADR-0022 D7 的 fixture 与「树是否稳定」判断都依赖这点）；
② 要能**计时**（Spike A 的 go 判据是性能阈值）；
③ 要能**重复运行取中位数**；④ 输出必须是**纯 ASCII 结构化文本**，
避免控制台中文乱码干扰判断（见 D4）。

### D4　Spike 脚本的编码硬约束：**`.ps1` 必须纯 ASCII**

本次真实踩坑（2026-09-18，probe-04）：Windows PowerShell **5.1** 读取**无 BOM** 的 `.ps1`
时按 ANSI（本机 ACP=936/GBK）解码。脚本里的中文注释、`—`（em dash）、`→`（箭头）等
非 ASCII 字节被错误配对后**会吞掉后续的结构字符**，产生一个
「报错行号与文件实际内容对不上」的解析错误（实测：报 line 89 是 `)`，而该行其实是 `Start-Sleep`；
parser 的 token dump 里 `'probe-04 write-path EOL 鈥?result'` 直接暴露了 mojibake）。

同一坑还有一个更隐蔽的后果：probe-03 的 CJK 用例里，`` `r `` 转义被 GBK 前导字节吞掉，
导致「中文字面量变形 + 少了一个 CR」，测出的数据不可用（已由 probe-04 用码点构造重测）。

**规则**：
1. `spikes/**/*.ps1` **只允许 ASCII**（注释、字符串、输出全算）。
2. 非 ASCII 测试数据用码点构造：`[char]0x4E2D`，或从**外部 fixture 文件**按显式编码读入。
3. 中文说明写在 `README.md` / 报告（`.md` 是 UTF-8 无 BOM，读取方是编辑器/agent，不受此坑影响）。
4. 本机装有 **PowerShell 7.6.5**（`pwsh`，默认 UTF-8 无 BOM）可作逃生口，但**不作为默认**：
   要求所有环境都有 pwsh 会增加门槛，而「纯 ASCII」在 5.1 与 7.x 上都成立。
5. 建议给 `xtask hygiene` 增加规则 `hygiene/ps1-ascii-only`（Error 级）→ 已登记 PARKING_LOT，归 TASK-015。

### D5　spike 依赖的登记方式（补齐 DEPENDENCIES.md 规则 1 的可操作性）

`docs/DEPENDENCIES.md` 规则 1 是「先登记，后引入」。spike 场景下的具体做法：

- `windows` crate **登记为 spike 与产品共用**，「使用方」列写
  `spikes/spike-a-notepad（阶段 0）→ crates/platform/windows（阶段 1）`；
- 版本要求写 `0.62`（次版本钉定；`windows` crate 的 0.x 版本间**有**破坏性变更，必须钉）；
- `uiautomation` **也要登记一行**，状态 = `Rejected`，否决理由写 D1 的表格结论
  （DEPENDENCIES.md 规则 5：删除/否决不删行，保留决策历史）。

## 影响（需要改的文件）

- `docs/DEPENDENCIES.md`：新增 `windows` 行（Planned→Approved for spikes）、`uiautomation` 行（Rejected）；
  PL-019 的「已知覆盖缺口」段落改为指向本 ADR。
- `.github/workflows/ci.yml`：新增 `spike-deny` job（硬门禁）。
- `.github/workflows/gate-selftest.yml`：新增 `spike-deny` canary（N3 负向验证）。
- `docs/adr/0019-hard-gate-negative-verification.md`：登记表补 `spike-deny` 一行。
- `docs/governance-ai-agent-execution.md` §5.1：硬门禁由 6 → **7** 项（新增 spike-deny）。
- `spikes/spike-a-notepad/README.md`：固化 D3 的工具用法与 D4 的编码约束。
- `spikes/spike-a-notepad/Cargo.toml` + `src/bin/uia_dep_proof.rs`：**新增**（D1/D1a 的实证载体，
  同时是 spike-deny 门禁的第一个真实检查对象）。UIA 胶水按 D1 的规则写成可整体搬走的独立段，
  去向 `crates/platform/windows/src/uia/`。
- `plans/stage-0-spikes.md` TASK-002 步骤 2 / 步骤 3：按 D1 / D3 修订卡面措辞（人类指示已批准）。
- `docs/PARKING_LOT.md`：PL-019 关闭；新增 `hygiene/ps1-ascii-only` 提案。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | spike 用 `uiautomation` crate（少写胶水） | ❌ 否决 | 见 D1：会污染 go/no-go 的性能结论，把风险推迟到阶段 1 |
| 2 | spike 不引依赖，继续用 PowerShell 托管封装 | ❌ 否决 | 托管封装与产品的 COM 路径**行为不同**（缓存/批量取属性），SPIKE-A §6 已点名这是最大不确定性 |
| 3 | PL-019 方案 ②：`xtask hygiene` 检查「spike 依赖必须已登记」 | ⏸ 推迟 | 方向对，但归属 TASK-015，且它检查的是「有没有登记」而非「登记的东西能不能用」→ 与 D2 互补，不互替；已记入 PARKING_LOT |
| 4 | PL-019 方案 ③：仅在卡面 DoD 写「依赖已登记」靠人工核对 | ❌ 否决 | 人工核对正是 ADR-0019 要替换的东西 |
| 5 | 把 `spikes/` 纳入主 workspace | ❌ 否决 | 会让 fmt/clippy/test 覆盖一次性代码，拖慢主线门禁并制造永久噪声（见 D2 表格） |
| 6 | spike-deny 也查 advisories | ❌ 否决 | 需联网 clone advisory-db，与「快而确定」的门禁定位冲突；spike 不在发布链路上 |
| 7 | 补装 Accessibility Insights | ❌ 否决 | 见 D3：输出不可 diff、不可计时、不可重复 → 对 AI agent 协作无增益 |
| 8 | 用 `pwsh` 7 取代「纯 ASCII」约束 | ❌ 否决（保留为逃生口） | 纯 ASCII 在 5.1/7.x 都成立，门槛更低；且 CI 的 windows runner 默认是 5.1 |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `windows` crate 的 COM 胶水写起来慢，拖长 TASK-002 | D1 已明确胶水的「去向」是 `crates/platform/windows` → 不是沉没成本；且只要求最小验证程序 |
| `windows` crate 0.x 版本间破坏性变更 | DEPENDENCIES.md 钉 `0.62`；升级 = 漂移触发器（AGENTS.md §4 ①） |
| spike-deny job 在没有任何 spike crate 时「假绿」 | D2：必须显式打印「无 spike manifest」再 exit 0；并由 gate-selftest canary 做负向验证 |
| canary 自身需要联网装 cargo-deny | canary 用**本地 path 依赖 + 假 GPL 许可证**的 fixture，只需 `licenses` 检查，免 advisory-db |
| 「纯 ASCII」约束让 spike 脚本可读性下降 | 中文说明一律放 README/报告；脚本内用英文注释 + 指向 ADR 编号 |

## 验证方式

1. `spike-deny` job 在**有** spike crate 时打印每个 manifest 的检查结果；在**无**时打印「无」并 exit 0。
2. 手工触发 `gate-selftest` → `spike-deny` canary 两步都打印 `OK:`（正向通过、负向变红）。
3. 负向实测：临时给某个 spike 加一个 GPL 许可的 path 依赖 → `spike-deny` **必须变红**（验证后移除）。
4. `powershell -NoProfile -File <每个 .ps1>` 均无解析错误；
   `[IO.File]::ReadAllBytes` 检查每个 `.ps1` 的非 ASCII 字节数 == 0。
5. TASK-002 步骤 3 的 Rust 程序**只用** `windows` crate 完成 SPIKE-A §5.1 列出的全部动作
   （枚举窗口、解析编辑区、读写全文、触发菜单、处理跨进程对话框）。
   **进度（2026-09-18）**：`uia_dep_proof` 已覆盖「枚举窗口 / 定位文档区 / 读全文」三项
   并全 PASS（D1a 表）；「写全文（`SetValue`）/ 触发菜单 / 跨进程对话框」仍属 TASK-002 步骤 3。

## 重新评估的触发条件

- `windows` crate 的 UIAutomation 绑定出现无法绕过的缺陷 → 重估 D1。
  **2026-09-18 核查结果**：`IUIAutomationCacheRequest` 与全部 `*BuildCache` 方法
  （`ElementFromHandleBuildCache` / `FindFirstBuildCache` / `GetChildrenBuildCache` …）
  **均在 `Win32::UI::Accessibility` 内**，批量取属性策略可实现 → 该触发条件**未命中**。
  注：D1a 的 E5 是**不带 CacheRequest** 的单次定位耗时（2.35 ms）；带缓存的对比留给 TASK-002。
- 官方 `winapp` CLI 被证实可用于第三方应用 → 重估 D3（可能替代部分自写导出）。
- 阶段 1 起 spike 代码需要长期维护（不再是「一次性」）→ 应把它纳入主 workspace 门禁，D2 的豁免失效。
