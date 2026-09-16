# MEMORY.md — 项目长期记忆

> **用途**：让任何新会话的 agent（或新加入的人）在 5 分钟内恢复"项目已经知道什么、否决过什么、踩过什么坑"。
> **硬上限 300 行**；超出时由 Orchestrator 在阶段末压缩，把最老条目归档到 `docs/memory/archive-YYYYMM.md` 并在此保留一行索引。

## 四种记录文件的分工（不可混用）

| 文件 | 记录什么 | 时间性 | 写法 |
|---|---|---|---|
| **`MEMORY.md`** | **认知**：已知事实、已否决方案、踩过的坑 | 长期有效 | 条目式，带日期与来源 |
| `LEDGER.md` | **事件**：哪张卡、哪个 commit、验收结果 | 流水 | 只追加，一行一事件 |
| `PLAN.md` + `plans/*` | **意图**：现在做什么、不做什么 | 当前阶段 | 可覆写，带变更历史 |
| `docs/adr/*` | **决策理由**：为什么这么定 | 永久 | 只增不改，可被新 ADR 取代 |

## 条目格式（单行、可 grep）

```text
- [YYYY-MM-DD][标签][src:来源] 一句话结论 → 因此怎么做
标签：FACT 已验证事实 ｜ DECISION 决策(指向 ADR) ｜ REJECTED 已否决方案(含理由)
      PITFALL 踩坑 ｜ OPEN 未决 ｜ ASSUMPTION 假设(待验证)
```

## 更新职责

- **Implementer**：完成任务卡时若产生新 FACT / PITFALL / REJECTED，**必须**追加条目（属于 DoD 的一部分）。
- **Orchestrator**：阶段末更新 §1 快照、压缩与归档、核对 §6。
- **禁止**：删除或改写他人条目（更正用新条目 + `[supersedes:日期]` 标注）。

---

## §1 项目当前状态快照（可覆写，最近更新：2026-09-16）

```text
阶段        ：阶段 0（Spike 前的地基已完成）—— **代码仓库已建立**（git 分支 main，首提交 58fed3d）
已产出文档  ：架构 v2.2、应用可行性 v1.1、AGENTS.md、gov、MEMORY.md、PLAN.md、
              plans/stage-0-spikes.md（TASK-001~010）、plans/stage-1-pilots.md（TASK-011~058）、
              docs/{governance-ai-agent-execution, subagent-orchestration, storage-design,
              wbs-overview, overnight-automation-charter}、docs/spec/naming.md、
              README / LEDGER / PARKING_LOT / DEPENDENCIES / xtask README
已产出代码  ：xtask（零第三方依赖的只读护栏工具，7 个模块 ~2800 行，99 个白盒测试全绿）
              + CI（三平台矩阵，6 硬门禁 / 9 软门禁 + deny + deferred-inventory）
待产出文档  ：docs/spec/*（其余 6 份，含 testing.md）、docs/adr/*（约 19 条）、
              docs/OPEN_SOURCE_CHECKLIST.md、8 份 Spike 报告
下一步      ：① 人类审阅 TASK-001（DRIFT-001-1/2/3 待裁决）
              ② 执行 Spike A/A2/B（TASK-002/003/004）与可并行的 C/H（TASK-005/009）
执行方式    ：AI coding agent（Codex/opencode/Claude Code）实现，人类规划+审阅+裁决；
              夜间由 heartbeat automation 推进（每日 23:30 与 02:30，见章程 §11）
工具链      ：rustup/cargo/rustc 1.98.1 stable-msvc ✅；cargo-deny ❌、cargo-llvm-cov ❌（PL-006）
平台基线    ：Windows 11 24H2/25H2（唯一正式基线）
试点顺序    ：Notepad → Paint → Edge/Chrome（阶段 1）→ Excel（阶段 2）→ Photoshop（阶段 3）
```

---

## §2 已确认事实（FACT）

### 平台与生态

- [2026-09-16][FACT][src:GNOME 50 发布说明] **GNOME 50（2026-03）已从 Mutter 完全移除 X11 后端**，成为 Wayland-only（X11 应用仍可经 XWayland 运行）→ Linux 侧必须 Wayland-first，不得再按 X11 会话设计。
- [2026-09-16][FACT][src:Fedora 43 / Ubuntu 25.10 发布说明] Fedora 43（2025-11）移除 GNOME on Xorg 会话；Ubuntu 25.10（2025-10）移除 Xorg 会话，26.04 LTS 延续。
- [2026-09-16][FACT][src:Microsoft 支持文档] **Windows 10 已于 2025-10-14 结束支持**（消费者 ESU 仅延至 2026-10）→ 不作为正式基线。
- [2026-09-16][FACT][src:Qt 6 文档 QAccessible] Qt 的 AT-SPI 桥在 DBus 属性 `org.a11y.Status.ScreenReaderEnabled` 为 true 时激活；替代方式是环境变量 `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1` → Linux 上 Qt 应用的 a11y **可以不重启应用就激活**。
- [2026-09-16][FACT][src:at-spi2-core bus/README + cua blog] `at-spi-bus-launcher` 读取 gsettings `org.gnome.desktop.interface toolkit-accessibility` 并据此设置 `org.a11y.Status` → Tier 0 激活开关就是这条 gsettings。
- [2026-09-16][FACT][src:at-spi2-core devel docs] **GTK4 直接实现 AT-SPI DBus（不经 ATK）**；GTK3 仍走 ATK 桥；ATK 自 2.46 起弃用 → GTK4 应用的 a11y 覆盖度需逐应用实测，"有节点但无 action"是已知形态。
- [2026-09-16][FACT][src:AT-SPI 接口定义] `org.a11y.atspi.EditableText` 提供 `SetTextContents` / `InsertText` / `DeleteText`；`Action.DoAction` 可触发点击类动作 → **Wayland 下无需注入输入即可读文本、写文本、点控件**（走 DBus，与合成器无关）。
- [2026-09-16][FACT][src:Wayland 协议现状] Wayland **无全局窗口列表 API**（社区仍在讨论 Top Level Tag）；无任意输入注入与任意截屏 → 窗口枚举改用 AT-SPI 根节点（APPLICATION → FRAME），身份主键改用 `app_id`。
- [2026-09-16][FACT][src:wlr 协议采用情况] `wlr-virtual-pointer` / `wlr-screencopy` **KWin 与 Mutter 均不实现** → Linux 必须按**合成器家族**（Mutter / KWin / wlroots / 其他）分流，不能按发行版。
- [2026-09-16][FACT][src:KDE bug 497778、input-leap issue 2258] KDE 的 portal `restore_token` 持久化存在已知 bug；**锁屏会移除 libei 输入设备** → 长任务必须监听会话状态并在锁屏时暂停。
- [2026-09-16][FACT][src:cua blog 2026-06-18] 业界开源 Linux computer-use 驱动（cua-driver 0.5.7）**正式支持的仍是 X11/XWayland，原生 Wayland 仍是 preview**（`CUA_DRIVER_RS_ENABLE_WAYLAND=1`），且原生 Wayland-only 应用可能完全不可见 → Wayland-first 是可行但需自建的方向，不能指望现成驱动。
- [2026-09-16][FACT][src:MCP 规范与 rust-sdk] MCP 最新规范为 **2026-07-28**（stateless-first、Extensions、多轮往返请求取代 elicitation、Tasks 移出核心）；官方 Rust SDK 为 **`rmcp`**，兼容 2025-11-25 及更早。规范把更多安全责任交给实现方 → 权限/审批/审计必须自持。
- [2026-09-16][FACT][src:Chrome/Edge 136 变更] **Chrome 与 Edge 136 起，`--remote-debugging-port` 对默认用户数据目录失效**，必须同时指定自定义 `--user-data-dir`（防 Cookie 窃取）→ 无法附加用户日常浏览器实例，必须专用 profile。
- [2026-09-16][FACT][src:Chrome 开发者博客] Chrome 138 起 Windows 上**默认启用原生 UI Automation**（此前需 `--force-renderer-accessibility`）。
- [2026-09-16][FACT][src:Microsoft Q&A] **New Outlook 不支持 COM / VSTO / VBA 扩展模型**；经典 Outlook 支持 → 两者是两个完全不同的目标，New Outlook 写操作列为 T3。
- [2026-09-16][FACT][src:Excel VBA/Undo 文档与社区一致结论] **任何触及工作表的程序化修改都会清空 Excel 的 undo 栈**；`Application.Undo` 只能撤销一个过程内的最后一次修改 → Excel 的 COM 写入必须按 **L1 快照**建模，禁止声明 L0。
- [2026-09-16][FACT][src:Adobe 开发者文档] **UXP 是 Photoshop v22.0（2021）及以后才有的插件/脚本平台**；Photoshop 2026 = v27。Adobe 正逐步结束 ExtendScript 支持（Premiere Pro 已于 2026-09 结束）→ 通道探测顺序 UXP → JSX → COM，JSX 标注 `deprecated_upstream`。
- [2026-09-16][FACT][src:Office 版本体系] Office 2016 / 2019 / 2021 / Microsoft 365 的**内部版本号都是 16.0**，`Application.Version` 无法区分 → 需用 `Application.Build` 或注册表 `ClickToRun\Configuration` 的 `DisplayVersion`/`ProductReleaseIds`【具体方式待实测 M1】。
- [2026-09-16][FACT][src:Windows 平台动向] 微软在 Build 2026 将 Windows 定位为 agent 平台（identity/isolation/containment/governance），Agent Workspace 与 Copilot Actions 自 2025-11 起 Insider 灰度；Microsoft Agent Framework 1.0 已发布但**无 Rust SDK** → 预留 `PlatformAgentChannel`，把资产沉淀在 Adapter 而非平台胶水。
- [2026-09-16][FACT][src:Windows 安全机制] UIPI 阻止低完整性进程操作提权窗口 → 操作管理员身份运行的目标需要独立提权 Host；Session 0 隔离使服务无法直接访问交互桌面。
- [2026-09-16][FACT][src:macOS TCC 行为] 签名身份变化 / 重装 / 移动位置会**重置 TCC 授权**（辅助功能、屏幕录制等）→ 必须用稳定 Developer ID 签名，并在更新流程中检测与引导重新授权。

### 项目自身

- [2026-09-16][FACT][src:本项目定位] 被控对象是**不可修改源码的通用 Windows 应用** → L1 通道的工作是「接口考古」而非「协商加接口」；App Adapter 是核心资产与主要工作量。
- [2026-09-16][FACT][src:行业观察 2026 中] computer-use agent 在 OSWorld 类基准可达约 85%，但真实长流程业务失败率仍高 → **基准分 ≠ 生产可用性**；可靠性来自 App Adapter 契约，不来自模型更聪明。
- [2026-09-16][FACT][src:v2 §21] 25 个真实场景推演中，**没有一个能靠"模型更聪明"解决**，全部依赖确定性工程机制（后置验证、租约、指纹、幂等、HITL）。
- [2026-09-16][FACT][src:本机实测 TASK-001] Rust 工具链 1.98.1（stable-x86_64-pc-windows-msvc）+ clippy/rustfmt/llvm-tools 已就位。`winget install Rustlang.Rustup` 下载 rustup-init 耗时约 **14 分钟**（static.rust-lang.org 慢；DeliveryOptimization 先超时失败、WinINet 重试成功）→ 装工具链要预留时间，别以为卡死了。
- [2026-09-16][FACT][src:Codex 桌面 automation 实测] `automation_update` 的 `kind="cron"`（独立项目任务、每次全新会话）在本机**创建失败**（返回 `Failed to create automation.`；日志显示非 ChatGPT 鉴权，本机用自定义 model provider）。只有 `kind="heartbeat"` 可用，且**一个 thread 只允许一个 heartbeat**、必须挂在已存在的 thread 上。
- [2026-09-16][FACT][src:xtask 实测] `rustfmt` 的 `wrap_comments` / `comment_width` / `format_code_in_doc_comments` / `normalize_comments` 在 **stable 通道不生效**，只打印一行 warning。
- [2026-09-16][FACT][src:xtask 实测] clippy `pedantic`+`nursery` 全开 + `-D warnings`，在约 2800 行 Rust 上产生 **12 条**必须处理的告警（`option_if_let_else`、`format_collect`、`needless_pass_by_value`、`missing_const_for_fn`、`doc_markdown`、`useless_let_if_seq`、bool→int）→ 可行，但要边写边按这些习惯来，事后收拾成本高。
- [2026-09-16][FACT][src:xtask 实测] `cargo test` 的工作目录是 **package 根**（`xtask/`）而不是 workspace 根 → 测试里不能用相对路径假设 cwd；要仓库根就用 `env!("CARGO_MANIFEST_DIR")` 的父目录推导。

---

## §3 已决策（DECISION，详情见 ADR / v2 对应章节）

- [2026-09-16][DECISION][ADR:待建 0001] 核心语言 **Rust**；UI 用 **Tauri 2 + React/TS**；工具协议 **MCP-first（`rmcp`）**，内部工具同样以 MCP 表达。
- [2026-09-16][DECISION][ADR:待建 0002] **API 优先**为纲领原则一：L1 应用接口 > L2 命令 > L3 无障碍 > L4 合成输入 > L5 视觉。
- [2026-09-16][DECISION][ADR:待建 0003] **Linux Wayland-first**，X11 仅作 XWayland 兼容通道；按合成器家族分流；只承诺支持矩阵内组合。
- [2026-09-16][DECISION][ADR:待建 0004] **element/句柄不跨进程**：Host 边界切在"定位之后"，对外只传可序列化 descriptor。
- [2026-09-16][DECISION][ADR:待建 0005] **可逆性四级模型（L0~L3）**；撤销快捷键必须由 Adapter 显式声明；不可逆动作后置。
- [2026-09-16][DECISION][ADR:待建 0006] **无人值守暂不支持但三处预留**（类型/契约/能力 + UI 占位），未来仅白名单应用。
- [2026-09-16][DECISION][ADR:待建 0007] **出域策略用户可选三档**（`local_only`/`redacted`/`full`）+ 逐应用与逐内容类型覆盖；默认 `redacted`；降级不静默。
- [2026-09-16][DECISION][ADR:待建 0008] **股票类软件只读 + 解读 + 图形展示**；`TradingGate` 恒拒绝并预留，未来只经厂商官方接口开放。
- [2026-09-16][DECISION][ADR:待建 0009] 平台基线 **Windows 11 24H2+**；Win10 仅对跨版本一致目标提供 C 级尽力支持。
- [2026-09-16][DECISION][ADR:待建 0010] 版本支持：**Office 2019+**（2016 技术可用不承诺）；**Photoshop 最低 2021(v22)**（2020 无 UXP，列尽力而为）。
- [2026-09-16][DECISION][ADR:待建 0011] 试点顺序 **Notepad → Paint → Edge/Chrome**（阶段 1）→ Excel（2）→ Photoshop（3）；理由：零许可零登录依赖、三种通道覆盖、安全底座提前。
- [2026-09-16][DECISION][ADR:待建 0012] 存储四层：**内存热缓存 / SQLite(WAL) / 内容寻址 blob(zstd+去重) / 冷归档**；只有 Core 写库。
- [2026-09-16][DECISION][ADR:待建 0013] 分发：**当前内部使用，但按开源规范建设**（建议 `MIT OR Apache-2.0`），`adapters/` 与 `adapters-private/` 从第一天分开。
- [2026-09-16][DECISION][ADR:待建 0014] 执行模式：**AI agent 实现 + 人类裁决**；防漂移靠 SSOT 分层、任务卡 write scope、约束回执、14 项 CI 门禁、验收分离。
- [2026-09-16][DECISION][ADR:待建 0015] 命名要求"一眼可懂"+ **受控词汇表**；注释密度偏高，公共 API 100% 文档注释，坑用 `PITFALL(app=…)` 结构化标签。
- [2026-09-16][DECISION][ADR:待建 0016] **全仓库统一 LF**：`.gitattributes` 设 `* text=auto eol=lf` + 显式二进制标记，`rustfmt.toml` 设 `newline_style = "Unix"`。理由见 §4 的对应 REJECTED 条目。
- [2026-09-16][DECISION][ADR:待建 0017] **未实现项必须显式登记 + 显式失败**：xtask 未实现的子命令以退出码 3 失败并打印归属卡号；`hygiene` 每次运行都声明"11 项只实现 3 项"；CI 有 `deferred-inventory` job 专门验证"未实现子命令仍然失败"。理由：一条"因为没写所以返回 0"的门禁比没有这条门禁更危险（制造虚假安全感）。
- [2026-09-16][DECISION][ADR:待建 0018] 夜间自动化用 **heartbeat**（每日 23:30 与 02:30 唤醒当前 thread）：每夜 ≤3 轮、每次唤醒 ≤2 轮、`.nightly.lock` 互斥、只在 `nightly/<date>` 分支提交、不合并 main、不操作真实应用、仅失败时通知、晨间报告落 `docs/nightly/<date>-report.md`。详见 `docs/overnight-automation-charter.md` §11。
- [2026-09-16][DECISION][ADR:待建 0019] **`#[allow]` 的唯一合法位置**是 `#[cfg(test)] mod tests` 上的 `clippy::unwrap_used` / `expect_used` / `panic`（gov §5.2 授权：测试失败就该炸）。产品代码里加 allow = 放宽护栏 = 漂移触发器，夜间自动化一律禁止。需写进 `docs/spec/testing.md`。
- [2026-09-16][DECISION][ADR:待建 0020] TASK-001 先落 **MIT 单许可**（而非 gov 建议的 `MIT OR Apache-2.0`）：内部阶段不需要双许可复杂度，且该决定**可逆**（追加 Apache-2.0 是加法）。M5 仍待人类在阶段 0 结束前正式确认。
- [2026-09-16][DECISION][ADR:待建 0018 补充] 夜间工作的**选取顺序是三级回退**：① `tasks/` 下 Ready 任务卡 → ② 章程 §13「当前夜间工作单」（人类白天维护，夜间只能执行不能改写）→ ③ 白名单 P2~P8。理由：阶段 0 的 10 张卡**没有一张是夜间安全的**（Spike 全都要真机 GUI 或新增依赖，而黑名单两者都禁止），若坚持"必须有 Ready 卡"，夜间自动化会永久空转。首批工作单 W1（rustscan 函数扫描器）/ W2（hygiene 函数级 3 条规则）/ W3（testing spec 草案）/ W4（ADR 0016~0020 草稿）已写入章程 §13。

---

## §4 已否决方案（REJECTED）★ 动手前必读，避免重复辩论

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

---

## §5 踩坑记录（PITFALL）

- [2026-09-16][PITFALL][src:v2 §9.2] `Ctrl+Z` 在终端类目标是 **SIGTSTP（挂起前台进程）**；撤销快捷键必须由 Adapter 声明，终端/远程会话目标禁用键盘 undo。
- [2026-09-16][PITFALL][src:v2 §9.2] 撤销会连带撤销**用户**在 Agent 之后做的改动 → 默认用"内容快照反向应用 diff"，而非"步数级 undo"。
- [2026-09-16][PITFALL][src:v2 §9.2] 保存后 undo 仍可执行但磁盘已变 → 区分"内存态可逆"与"持久态可逆"，文件写盘一律配影子副本。
- [2026-09-16][PITFALL][src:v2 §7.7] 点击"发送"后网络卡住 → Agent 误判失败并重试 → 发出两条。非幂等动作**验证前禁止重试**。
- [2026-09-16][PITFALL][src:v2 §13.2.6] 中文 IME 激活时模拟键盘输入会被吞或变拼音 → 文本写入优先用 `ValuePattern.SetValue` / `AXValue` / AT-SPI `EditableText`（不经 IME）。
- [2026-09-16][PITFALL][src:v2 §6.1] 虚拟化列表（UIA VirtualizedItems、Qt model/view、Web 虚拟列表）只实例化可见项 → 索引不可作唯一依据，需搜索或滚动加载。
- [2026-09-16][PITFALL][src:v2 §6.9] XWayland 应用的 AT-SPI 坐标可能是 X11 逻辑坐标，与 Wayland 输出坐标在分数缩放下不一致【待 Spike D 实测】→ 首次使用某显示器组合必须校准。
- [2026-09-16][PITFALL][src:feasibility P1] Win11 记事本"另存为"打开的是 **Shell 进程的对话框**，不属于记事本进程 → TargetDescriptor 必须支持跨进程解析。
- [2026-09-16][PITFALL][src:feasibility P1] 关闭未保存文档会弹三态对话框（保存/不保存/取消）→ 注册为 interrupt，**默认选"取消"**，禁止 Agent 自行选"不保存"。
- [2026-09-16][PITFALL][src:feasibility P2] Excel **Protected View**（网络来源文件）下 UI 树与 COM 行为都不同 → 必须探测；"启用编辑"是安全边界，需人工确认。
- [2026-09-16][PITFALL][src:feasibility P2] Excel COM 对象不显式释放会残留 `EXCEL.EXE` 进程 → Host 需要 COM 生命周期管理与看门狗。
- [2026-09-16][PITFALL][src:feasibility P2] `Range.Value` / `.Formula` / `.Text` 三者语义不同 → 断言必须指明读哪一个。
- [2026-09-16][PITFALL][src:feasibility P3] 画图**画布无子控件结构**，且抗锯齿导致像素不精确一致 → 断言必须用容差或感知哈希，不能逐像素相等；画布坐标 ≠ 屏幕坐标（受缩放与滚动影响）。
- [2026-09-16][PITFALL][src:feasibility P4] Photoshop 启动 10~30 s 且有首选项/登录/许可弹窗 → "进程存在"不等于"就绪"，必须做就绪探测。
- [2026-09-16][PITFALL][src:feasibility P5] Edge/Chrome 专用 profile **没有用户登录态** → 首次需用户手动登录；禁止复制 Cookie。
- [2026-09-16][PITFALL][src:v2 §13.4.7] Linux 锁屏会移除 libei 输入设备 → 长任务必须监听会话状态并暂停，解锁后重建会话。
- [2026-09-16][PITFALL][src:v2 §13.4.5] 用 Tier 1（重启应用注入环境变量）激活 a11y 会**丢失用户未保存工作** → 必须先做 dirty 检查并提示保存。
- [2026-09-16][PITFALL][src:v2 §13.4.6] Chromium/Electron 开启 a11y 会显著增加 CPU/内存，且 Web 树可达数千节点 → 必须裁剪，且只在需要时开启。
- [2026-09-16][PITFALL][src:v2 §5.5] 工具数 > 20~30 时模型选择准确率显著下降 → 按目标动态挂载 + 检索式工具选择。
- [2026-09-16][PITFALL][src:v2 §7.1] UIA/AT-SPI 全窗口树遍历可达秒级 → 限定 scope 与 depth、缓存、批量取属性、按需展开。
- [2026-09-16][PITFALL][src:gov §2.2] 代码先改而文档未跟上 → **下个会话的 agent 会照旧文档把新实现改回去**，形成来回震荡 → 契约必须先行。
- [2026-09-16][PITFALL][src:TASK-001] **构造函数丢弃参数**是最隐蔽的静默失败：`Report::new(command)` 曾把 `command` 置空，编译通过、测试也通过（因为测试自己又赋了一次值），只有报告标题一直空着。→ 凡是构造函数的参数没有被存进结构体，就要停下来问"这个参数是干什么的"。已修 + 加回归测试。
- [2026-09-16][PITFALL][src:TASK-001] 护栏工具会**先拦住自己**：`main.rs` 写到 644 行时被自己的 `file-too-long`（>600 告警）拦下。这是好事，但意味着写工具时要预留拆分成本 —— 一开始就按"IO 在边界、规则是纯函数"分层能省一次返工。
- [2026-09-16][PITFALL][src:TASK-001] 在**文档注释**里解释"哪些标签被禁止"时，直接写出标签字面量会被自己的规则判违规（`rustscan` 只抹字符串字面量，不抹注释）。当前靠改措辞绕过；是否豁免反引号引用待裁决（PL-004）。
- [2026-09-16][PITFALL][src:TASK-001] **PowerShell 数组扁平化会造成灾难性误替换**：`@( @('old','new') )` 会被扁平成两个元素，于是 `$pair` 变成字符串、`$pair[0]` 变成"第一个字符"，`Replace` 变成全局单字符替换（本会话真的把两个 md 文件里所有 `|` 替换成了空格）。→ 单 pair 必须写 `@( ,@($old,$new) )`；并给替换函数加三道防护：pair 必须是长度 2 的数组、锚点必须足够长、锚点必须唯一。
- [2026-09-16][PITFALL][src:TASK-001] PowerShell here-string 里 `'@` 必须**独占行首**，数组字面量中嵌套 here-string 会解析失败；`Split-Path -LiteralPath X -Parent` 与部分参数集冲突会报 null。→ 批量改文件用"一次一个 pair 的函数 + 逐个打印 OK/MISS"，别一次塞太多。

---

## §6 未决问题（OPEN）与假设（ASSUMPTION）

**OPEN（待实测/待确认）**
- [2026-09-16][OPEN][M1] Office 2019/2021/365 的准确区分方式（`Application.Build` vs ClickToRun `DisplayVersion`）→ Spike（阶段 2 前）。
- [2026-09-16][OPEN][M2] Photoshop 2020(v21) 能否正常登录激活；Illustrator 的 UXP 起始版本 → Spike（阶段 3 前）。
- [2026-09-16][OPEN][M3] `local_only` 档所需的本地模型选型与硬件门槛（Ollama / llama.cpp）→ 阶段 1c 前。
- [2026-09-16][OPEN][M4] 内部是否存在必须支持的 Win10 机器 → 阶段 0。
- [2026-09-16][OPEN][M5] 开源许可证最终选择（建议 `MIT OR Apache-2.0`）→ TASK-001。
- [2026-09-16][OPEN][src:v2 §13.4.5] KDE Plasma 的辅助功能开关对应的底层配置键名 → Spike D。
- [2026-09-16][OPEN][src:v2 §13.4.7] GNOME 50 的 RemoteDesktop 是否能接管当前会话（而非仅 headless）；restore_token 静默重建的实际表现 → Spike D。
- [2026-09-16][OPEN][src:v2 §13.3.2] macOS 屏幕录制权限的周期性重授权间隔 → 阶段 5 前。
- [2026-09-16][OPEN][M6] 夜间 automation 是否需要"晨间显式提醒"？当前策略是**仅失败时通知**，晨间报告靠人类自己打开 `docs/nightly/<date>-report.md`；若要早晨收到提醒，代价是 23:30 与 02:30 也各响一次 → 人类裁决（章程 §11.6）。
- [2026-09-16][OPEN][M7] `check-comments` / `check-ledger` / `card-check` 三个护栏子命令**无任何任务卡认领**，gov §5.1 的 #15 #16 门禁因此无人负责 → 需补卡或并入 TASK-015（PL-002）。
- [2026-09-16][OPEN][M8] TASK-001 的三条 DRIFT 待裁决（hygiene 范围 / CI 硬门禁数量 / 执行记录落盘位置），其中第三条会牵动 gov §3.2 任务卡模板与所有后续卡的 write scope → 阶段 0 内裁决。

**ASSUMPTION（假设，未验证，不得当作结论使用）**
- [2026-09-16][ASSUMPTION][src:feasibility §2.3] 同花顺/通达信/东方财富的行情与交易界面以 GDI 自绘为主，UIA 覆盖差 → 需 Spike 实测后升级为 FACT 或推翻。
- [2026-09-16][ASSUMPTION][src:feasibility §2.3] 上述软件普遍支持"键盘精灵 + 数据导出/复制到剪贴板"作为可行只读通道 → 需实测。
- [2026-09-16][ASSUMPTION][src:v2 §13.4.6] Java Swing/AWT 在 Linux 上需要 `libatk-wrapper` 或 JVM 参数才暴露 AT-SPI → 需实测。
- [2026-09-16][ASSUMPTION][src:v2 §15.4] 树快照 zstd 压缩比 5~10x、内容寻址去重收益显著 → 需 Spike H 实测。

---

## §7 归档索引

（暂无。超过 300 行时，最老的 §2/§5 条目移入 `docs/memory/archive-YYYYMM.md` 并在此登记。）
