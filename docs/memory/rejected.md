# rejected.md — 已否决方案（REJECTED）★ 动手前必读

> 标签 `[REJECTED]` = 曾经提出、已被否决的方案**及其否决理由**。
> **本文件存在的唯一目的：防止同一个方案被反复重新提出、反复重新辩论。**
> AGENTS.md §3 的启动协议要求每个会话动手前读本文件全量（它通常是最短的一个）。

---

> **本文件是 `MEMORY.md` 分层结构（ADR-0021）的 L1 层之一。**
> 写入规则见 `MEMORY.md`「条目格式」与「更新职责」；路由规则见 `docs/memory/README.md`。
> **只追加，不改写他人条目**；更正用新条目 + `[supersedes:日期]` 标注。
> 应用专属的条目**不进本文件**，进 `docs/memory/apps/<app>.md`。

- [2026-09-16][REJECTED][src:v2 §13.4.1] **Linux 放弃 Wayland、只支持 X11 会话** —— X11 会话已被 GNOME 50/Fedora 43/Ubuntu 25.10+ 移除，方案没有落地对象。
- [2026-09-16][REJECTED][src:v2 §2.2] **给模型注册 `execute_code` / 通用 shell / 通用 PowerShell 工具** —— 等同于放弃可控性；改为具名封装工具 + 声明式受限执行。
- [2026-09-16][REJECTED][src:v2 §4.5] **用 WASM 作为"操作应用"类技能的运行时** —— WASI 无系统访问能力，只能做纯计算；扩展形态改为进程外 MCP server。
- [2026-09-16][REJECTED][src:v2 §6.4] **用控件可见文本（Name/AXTitle）作主 selector** —— 多语言/主题/改版/动态文案即失效；只能作最低分兜底并标 `locale_dependent`。
- [2026-09-16][REJECTED][src:v2 §3.2] **把 OS Adapter 挂在 Skill Runtime 之下** —— Core 也需要平台能力，会造成反向依赖；改为公共 Platform Service。
- [2026-09-16][REJECTED][src:v2 §5.4] **自定义 Tool 协议 + 第三阶段再引入 MCP（两套工具系统）** —— 后期合并代价大；改为 MCP-first，第一天就用。
- [2026-09-16][REJECTED][src:v2 §9.2] **用全局默认 `Ctrl+Z` 实现撤销** —— 终端里是 SIGTSTP（挂起进程）、焦点错位会撤销别的应用、粒度与深度不可控；必须由 Adapter 声明。
- [2026-09-16][REJECTED][src:v2 §12.4] **反提示注入只写"把外部内容视为数据"** —— 不可实现；改为四层机制（通道隔离 / 污点追踪 / 权限衰减 / 干净上下文复核）。
- [2026-09-16][REJECTED][src:feasibility P2] **用 `DisplayAlerts=false` 提升 Excel 自动化效率** —— 会静默覆盖文件，违反"无静默失败"红线；列入 Adapter `forbidden_properties`。
- [2026-09-16][REJECTED][src:feasibility P5] **附加到用户日常浏览器实例做 CDP** —— Chrome/Edge 136+ 已禁止；且复制 Cookie 等同凭据窃取。改为专用 profile + 用户自行登录。
- [2026-09-16][REJECTED][src:feasibility §2.3] **用 OCR 作为股票数据的唯一来源** —— 读错一个数字在交易/解读场景是致命的；必须官方 API 或导出/剪贴板，OCR 仅辅助且需双通道交叉校验。
- [2026-09-16][REJECTED][src:feasibility §2.3] **用 UI 自动化模拟点击下单** —— 无法可靠验证是否成交；未来若开放交易只经厂商官方接口。
- [2026-09-16][REJECTED][src:v2 §20.3] **在只有一个真实 Adapter 时就抽象跨平台/跨应用接口** —— trait 会被第一个平台的假设污染；至少两个真实 Adapter 之后再抽象。
- [2026-09-16][REJECTED][src:v2 §11.6] **一开始就上向量数据库做记忆** —— 先用 SQLite + FTS5，确有需要再引入（且用本地嵌入模型避免出域）。
- [2026-09-16][REJECTED][src:gov §4.4] **靠单个超长会话完成整个阶段** —— 上下文稀释导致漂移；改为一会话 1~2 张卡，**重开会话是正常操作而非失败**。
- [2026-09-16][REJECTED][src:v2 §13.4.9] **在 GNOME 下依赖 `org.gnome.Shell.Screenshot` 做高频静默截图** —— GNOME 49 起第三方受限（gnome-screenshot 亦停止工作）；改走 portal 并接受"需授权"。
- [2026-09-16][REJECTED][src:v2 §13.6.1] **把 Windows 10 纳入正式支持基线** —— 已 EOL，且记事本/画图结构完全不同需各写一套 Adapter；仅对跨版本一致目标提供 C 级支持。
- [2026-09-16][REJECTED][src:TASK-001] **`rustfmt newline_style = "Windows"`（全仓库 CRLF）** —— 三平台 CI 矩阵下，Linux runner 签出的是 LF 而 rustfmt 期望 CRLF，`cargo fmt --check` 会**永久红灯**；若改为依赖各人 `core.autocrlf`，等于把"文件长什么样"交给每个人的 git 配置决定，正是 gov §0.1 里最难查的环境差异 bug。
- [2026-09-16][REJECTED][src:TASK-001] **在 `rustfmt.toml` 保留 stable 通道无效的 unstable 选项** —— 不生效却每次刷 warning，会让人误以为注释宽度受控，并把 CI 输出训练成"可忽略的噪声"。
- [2026-09-16][REJECTED][src:Codex automation 实测] **用 cron 型 automation 实现"每轮全新会话"的夜间任务** —— 本机创建失败（见 §2 FACT）。"每轮新会话"做不到，改为 heartbeat + **每轮强制重锚** + `.nightly.lock` 互斥 + 上下文累积到阈值就换新 thread（章程 §11.1/§11.5）。
- [2026-09-16][REJECTED][src:TASK-001] **把 `hygiene` 做成"占位实现"以严格贴合卡面** —— 与卡面验收命令第 5 条（`xtask hygiene` 必须能跑）直接矛盾；且一个已就绪的防线留作软门禁等于白白放弃。改为实现 3/11 项 + 其余显式登记（DRIFT-001-1，待人类确认）。
- [2026-09-17][REJECTED][src:探针实测 2026-09-17][supersedes:2026-09-16] **在第三方 Responses 端点上用 Codex 内置 automation（heartbeat / cron 皆然）做无人值守** —— 否决理由与旧条目**不同**：不是因为 cron 建不起来（它建得起来，旧条目记载有误），而是因为两者**共用同一投递机制**、都产生畸形 `function_call_output`。cron 首轮投递时 Codex 会临时生成非法 `at_<uuid>` id（**从不落盘** → 等长补丁法无效），两次探针均在 2.6s 内零产出失败。heartbeat 更糟：毒项沉入长驻会话 + `disable_response_storage=true` 全量重放 → 该会话**永久损坏**，连白天交互也受牵连。改为 `codex exec` CLI + Windows 任务计划程序（已实测可行，见 §2 FACT 2026-09-17）。

---

---

## 2026-09-18 追加（人类指示 #6 #8 #9 #11 的裁决结果）

- [2026-09-18][REJECTED][src:ADR-0024 D1，人类指示 #6] **否决用第三方 `uiautomation` crate 做 spike 的 UIA 访问**，改用 `windows` crate 自带的 COM 绑定。**决定性理由**：Spike A 的 go/no-go 判据里最不确定的就是「Rust 走 COM 是否与 PowerShell 走托管封装表现一致」；若 spike 也用一层第三方封装，这个不确定性**不会被消除，只会被推迟到阶段 1**，而那时改动成本高得多 —— **spike 的意义就是用生产路径去撞墙**。附带理由：封装层自带缓存会**掩盖**真实 COM 开销、污染性能判据；且它不在 DEPENDENCIES.md 的「计划首批依赖」清单内，等于多引入一个外部维护者。代价（多写 100~200 行胶水）**不是浪费**：它就是 `crates/platform/windows/src/uia/` 的雏形。（**结果**：E5 实测 2.35 ms vs 1.5 ms，同一数量级 → 该不确定性已解除。）
- [2026-09-18][REJECTED][src:ADR-0024 D3，人类指示 #8] **否决补装 Accessibility Insights for Windows** 作为控件普查工具，改用 **`inspect.exe` + 自写 UIA 树导出**。理由：前者输出**不可 diff、不可计时、不可重复**（GUI 交互 + 随版本变化的可视化），对 AI agent 协作**无增益**；后者的输出是纯文本、可入库 diff、可被脚本计时，且 probe-01/02 已证明效果足够。（`inspect.exe` 已就绪：`C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\inspect.exe`。）官方 `winapp ui inspect` CLI 列为**候选**，试用归 TASK-003。
- [2026-09-18][REJECTED][src:ADR-0022 D1 实测] **否决 `notepad -w`（"等待进程结束"）作为同步手段**：实测 `-w` 被记事本当成**文件名**处理 → 弹出模态框「文件名无效。」，`TabItem` 计数为 0；微软官方文档也查不到该开关。→ 同步一律靠**轮询枚举 + 按内容匹配**（实测 ≈450 ms 内出现），启动一律走 **AUMID + `IApplicationActivationManager::ActivateApplication`**。
- [2026-09-18][REJECTED][src:ADR-0022 D1/E3，微软官方文档] **否决两种"看起来更简单"的窗口定位法**：① **用启动返回的 PID 找窗口** —— 实测 PID 不等于窗口属主（PowerShell 30464 vs 20304；Rust 2632 vs 13168），打开第二个文件后系统里有 **3 个 Notepad 进程、1 个窗口、2 个 TabItem**；② **用 .NET 的 `Process.MainWindowHandle`** —— 微软官方文档明确它是**启发式**结果，不保证是你要的那个窗口。→ 属主 PID **只能**从 `GetWindowThreadProcessId` 取。
- [2026-09-18][REJECTED][src:ADR-0022 D4] **否决把任何本地化属性用作 selector**，禁令覆盖 **5 项**：`Name`、`AccessKey`、`AcceleratorKey`、`LocalizedControlType`、`ItemStatus`。实测同一控件 `aid='File'` 而 `name='文件'` → aid 是英文、name 是本地化文本，换语言即失效。（AGENTS.md §7 原已禁"用控件可见文本作主 selector"，本条把范围从 `Name` 扩到全部 5 项。）
- [2026-09-18][REJECTED][src:ADR-0021，人类指示 #11] **否决把 Codex 自带的 `~/.codex/memories/` 当作项目事实源**（仅作个人环境备忘）。理由：不随仓库版本化、**不能被 opencode / Claude Code 共享**、含本机路径与隐私因而**无法开源**（本项目将来要上 GitHub）、不在 AGENTS.md 的裁决链内、且需要显式请求才写入。→ 项目记忆一律进仓库内 `docs/memory/`。若某天它支持仓库内路径 + 版本化 + 跨 agent，重新评估（已在 `open.md` 登记）。
- [2026-09-18][REJECTED][src:ADR-0021 选项表] **否决两种记忆压缩方案**：① 「维持单体、把上限从 300 提到 600 行」—— 只推迟问题，每次会话的必读成本随文件线性增长而命中率不变；② 「按时间归档最老条目」（原 MEMORY.md 头部设计）—— **记忆价值不按时间衰减**，2026-09-16 的「GNOME 50 已移除 X11 后端」比 2026-09-18 的某条实测数据更长期有效，按时间归档会**先扔掉最稳定的结论**。→ 改为分层（L0 索引 / L1 主题 / L2 应用 / L3 归档），且**归档按体积（>400 行）触发，不按时间**。
- [2026-09-18][REJECTED][src:ADR-0024 D2 选项表] **否决把 `spikes/` 纳入主 workspace**（那会让 fmt/clippy/test 覆盖一次性代码）；**否决 spike-deny 也查 `advisories`**（需联网 clone advisory-db，与"快而确定"的门禁定位冲突，且 spike 不在发布链路上）；**否决 PL-019 的方案 ③「只在卡面 DoD 写依赖已登记、靠人工核对」**（人工核对正是 ADR-0019 要替换的东西）；**否决用 `pwsh` 7 取代"纯 ASCII"约束**（纯 ASCII 在 5.1/7.x 都成立，门槛更低，且 CI 的 windows runner 默认是 5.1）—— 保留为逃生口。
- [2026-09-18][REJECTED][src:人类指示 #9 + rollout 取证] **否决继续用 `create_thread` 派生任务卡会话**（至少在上游 openai/codex#36315 关闭前）。根因不是权限而是参数结构（`target` 被序列化成 JSON 字符串 / 前两次直接缺 `target`），本地 schema 校验 0.05 s 内当场拒绝。→ 改为**人类手工新建会话**或 `fork_thread`。
- [2026-09-21][REJECTED][src:TASK-075 B1.4 probe-07, 本机实测 2026-09-21] **用 PowerShell UIA1（System.Windows.Automation）测试 Win11 25H2 modern Notepad 的"另存为"跨进程 dialog** —— Win11 25H2 modern Notepad（11.2607.14.0）的"另存为"**不是跨进程 dialog**，而是 **in-window WinUI3 FileExplorer-like panel**（+89 descendants 在 Notepad 窗口内；AutomationId `SaveDialogLabel` 出现 2 次证实）。`Ctrl+Shift+S` 键盘快捷键在 modern Notepad 不可靠；menu Invoke 触发"File > 另存为"成功，但 PowerShell UIA1 = 旧版 COM API，对 WinUI3 / `Microsoft.UI.Xaml` 控件支持有限，无法枚举 panel 内部复杂控件（FileName 输入框 + Save 按钮均无明确 AID）。**未来重做 B1.4 必须用 UIA3 工具**（Python `uiautomation` 包 / C# `FlaUI` / WinAppDriver）。**probe-07 的 5 项指标框架 + menu Invoke 触发可保留复用**，仅 3 个 Find 函数需重写。**对 v2 §3.2 的隐含影响**：元素跨进程原则对 WinUI3 应用不再成立 = 必须走 ADR 重新定义 Win32 vs WinUI3/UWP 两类的 Adapter 边界差异
- [2026-09-21][REJECTED][src:TASK-075 B1.4 probe-07 修订, 本机实测 2026-09-21] **[supersedes:2026-09-21] 之前错判 WinUI3 in-window panel** —— Win11 25H2 modern Notepad 的"另存为"实际是 Win32 #32770 dialog 跨进程（独立 explorer 子进程），不是 WinUI3 in-window panel。**否决真正原因**：之前用 `RootElement.FindAll(TreeScope.Children)` 枚举顶层窗口，**PowerShell UIA1 API 不返回 Win32 #32770 class**（虽然 Win32 EnumWindows 能看到 hwnd=1641050, pid=4828）。**正确方法**：用 Win32 EnumWindows 找 `#32770` + title 含 `Save As`/`另存为` 的窗口 → 拿到 hwnd → 用 `AutomationElement.FromHandle(hwnd)` 转 UIA element → 继续用 UIA 操作。**否决失效方案**：之前我提议的"Python uiautomation / C# FlaUI UIA3 工具"作为未来重做路径，**实际并不必需**——PowerShell UIA1 + Win32 SendMessage 即可完成全部测量（Win32 直接操作 #32770 dialog 反而比 UIA3 更可靠）
- [2026-09-25][REJECTED][src:TASK-204 §2 + 人类 chat 2026-09-25「`format` 和 `pattern` 也按我的意思去做」] **否决在工具参数校验器里支持 `pattern` / `patternProperties`**：正则引擎 = **新依赖**（`jsonschema` 路线已因传递依赖 `borrow-or-share` 的 `MIT-0` 被否，见本文件同日 deny 条目），且「拿不可信输入去匹配」是 **ReDoS 面**。**替代**：`enum` / `const` 收窄取值，或**由 handler 校验值**（校验器管**形状**、handler 管**值的语义安全**：参数化查询 / 路径规范化 / 不拼 shell）。
- [2026-09-25][REJECTED][src:TASK-204 §2] **否决把 draft-07 的 `format` 当「软约束」保留**：draft-07 的 `format` 语义**实现自定**（注解级），我们无法强制；保留它 = 作者以为有约束、实际没有 = **静默失败**（铁律 1）。**替代**：`enum` / `const`，或 handler 自校验。
- [2026-09-25][REJECTED][src:TASK-204 §2] **否决 `$ref` / `definitions`（多文档 schema 与引用解析）**：工具 schema 必须**单文档自包含** —— 引用要解析远程/多文档 + 递归，等于把 SSRF 与无限递归引进**注册期**。**替代**：内联子 schema（`allOf` 复用）。
- [2026-09-25][REJECTED][src:TASK-204 §2] **否决条件式关键字 `if`-`then`-`else` / `dependencies`，以及元组形式 `items`（含 `additionalItems`）**：二者都让「合法输入集」**不可枚举** —— 条件式要靠推理、位置式要靠数序号，而模型与人都必须能从 schema**直接读出**合法输入。**替代**：`oneOf` + `const` 判别式；**具名对象字段**（`properties`）。
- [2026-09-25][REJECTED][src:TASK-204 §2] **否决把 `contentEncoding` / `contentMediaType` 当注解忽略**：draft-07 里它们确是注解（不参与校验），但作者写它们时**按断言读** → 忽略 = 静默失败。**替代**：handler 校验。
- [2026-09-25][REJECTED][src:TASK-204 §2] **否决「按语言关键字 / SQL 注入词做黑名单」防注入**：JSON Schema 的关键字是**规范固定闭集**，塞语言关键字进去既不合规也不生效；且**黑名单是 fail-open**（漏一个就放行）。**正解**：白名单 + 在**值层**做参数化查询 / 不拼 shell / 路径规范化（那条归 policy 与 handler，不归 schema 校验器）。
- [2026-09-25][REJECTED][src:ADR-0047（人类 2026-09-25「PL-086：按你建议做」授权）；PL-086 / DRIFT-023-4] **否决在后置断言里保留自由字符串字段 `assert`**（如 `"assert": "target.text.contains(new_text)"`）—— 支持它等于引入一门**表达式语言**（lexer / parser / evaluator / scope）= 新抽象层；且它与 §7.4 已否决的 `if`-`then`-`else` 同病：**模型无法枚举可写形式**，写错只能得到运行时失败（静默失败面）。**替代**：结构化 `field` + `op` + `value` 与各 kind 专用字段，**未知字段解析期拒绝**（fail-closed，实现见 `crates/verify` / TASK-023）。附录 A 的 `"assert": { "type": "string" }` 已按本裁决**删除**。
