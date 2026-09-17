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

## §1 项目当前状态快照（可覆写，最近更新：2026-09-17）

```text
阶段        ：阶段 0（Spike 前的地基已完成）—— **代码仓库已建立**（git 分支 main，首提交 9d0fbee；原哈希 58fed3d 因 2026-09-17 提交身份重写而失效，映射见 §2）
已产出文档  ：架构 v2.2、应用可行性 v1.1、AGENTS.md、gov、MEMORY.md、PLAN.md、
              plans/stage-0-spikes.md（TASK-001~010）、plans/stage-1-pilots.md（TASK-011~058）、
              docs/{governance-ai-agent-execution, subagent-orchestration, storage-design,
              wbs-overview, overnight-automation-charter}、docs/spec/naming.md、
              README / LEDGER / PARKING_LOT / DEPENDENCIES / xtask README
已产出代码  ：xtask（零第三方依赖的只读护栏工具，7 个模块 ~2800 行，99 个白盒测试全绿）
              + CI（三平台矩阵，6 硬门禁 / 9 软门禁 + deny + deferred-inventory）
待产出文档  ：docs/spec/*（其余 6 份，含 testing.md）、docs/adr/*（约 19 条）、
              docs/OPEN_SOURCE_CHECKLIST.md、8 份 Spike 报告
下一步      ：① TASK-001 收尾：DRIFT-001-1 / -2 已裁决关闭（2026-09-17），
                仅 **DRIFT-001-3**（执行记录落盘位置，牵动 gov §3.2 模板 → 需 ADR）与
                **M5 许可证**、**PL-001 / PL-011** 待裁决
              ② 首次 push 后确认 GitHub Actions 三平台**真**绿 —— TASK-001 的 DoD
                「CI 在空 workspace 上三平台全绿」此前**从未被机器验证过**（仓库从未推送，
                见 ADR-0019 背景）
              ③ 执行 Spike A/A2/B（TASK-002/003/004）与可并行的 C/H（TASK-005/009）
执行方式    ：AI coding agent（Codex/opencode/Claude Code）实现，人类规划+审阅+裁决；
              夜间由 heartbeat automation 推进（每日 23:30 与 02:30，见章程 §11）
工具链      ：rustup/cargo/rustc 1.98.1 stable-msvc ✅；cargo-deny 0.20.2 ✅、cargo-llvm-cov 0.9.1 ✅
              （2026-09-17 装入 ~/.cargo/bin → PL-006 解除；安装配方仍未落文档 → PL-017）
git 远端    ：origin = https://github.com/snofin18/ai-assistant.git（公开库，2026-09-17 建）
              提交身份 = snofin18 (via Codex) <snofin@gmail.com>（仓库本地 config，人类裁决方案 B）
              github.com 经本地代理 http://127.0.0.1:30000（global config；**只对 HTTP/HTTPS 生效**）
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
- [2026-09-17][FACT][src:探针实测 2026-09-17][supersedes:2026-09-16] `automation_update` 的 `kind="cron"` 在本机**能创建也能触发**（旧条目"创建失败"的记载有误）。真实原因：cron 强制要求显式提供 `model` + `projectId` + `executionEnvironment="local"`，缺任一只回一句 `Failed to create automation.` 而不指明是哪个字段。实测两次触发（约定 11:16/11:24 → 实际 11:17:54/11:25:54，约 **+2 分钟 jitter**），各新建独立会话。
- [2026-09-17][FACT][src:探针实测 2026-09-17] **cron 与 heartbeat 共用同一投递机制**：automation 的 prompt 一律被包装成合成 `FunctionCallOutput{id:None, call_id:None, name:"automation_update", namespace:"codex_app"}` 提交，**不是** user message。→ 换 `kind` 无法规避畸形条目问题，"每轮全新会话"能拿到、"干净投递"拿不到。
- [2026-09-17][FACT][src:CLI 实测 2026-09-17] `codex.exe exec --skip-git-repo-check "<prompt>"`（CLI v0.154.0-alpha.6.2）走**标准 user message** 投递：实测会话 `01a0adcd-2b86-…` 的 rollout 中 `function_call_output` 计数 **0**、无 `automation_update`、无 `at_` 前缀 id、assistant 正常回复、退出码 0、19422 tokens。→ 本机唯一可行的无人值守投递方式，应由 Windows 任务计划程序调用（外部调度无 jitter，时间更准）。
- [2026-09-17][FACT][src:实测 2026-09-17] 百炼 `/models` 返回 `{"first_id","data":[…]}`，缺 Codex 解码器要求的 `models` 字段 → `list_models` 永久失败、`models_cache.json` 解析出 0 个 id。推理不受影响（`model` 硬写在 config.toml），仅模型选择器无法在线刷新。另 CLI 警告 `Model metadata for qwen3.8-max not found`；fallback 元数据下 cron 探针会话 `task_started` 里 `model_context_window=828400`，而 config.toml 配的是 `1000000` → **fallback 可能覆盖显式配置**，需核实，否则 auto-compact 会提前触发。
- [2026-09-17][FACT][src:阿里云官方文档 help.aliyun.com/zh/model-studio/text-generation-model] `qwen3.8-max` 官方上下文窗口 = **1M**（推荐模型表：`qwen3.8-max | 1M | 思考模式支持 | Function Calling 支持 | 内置工具支持 | 结构化输出支持`；快照版 `qwen3.8-max-0902` 同为 1M）。同档 1M 的还有 `qwen3.8-flash`/`qwen3.7-plus`/`deepseek-v4-pro`/`glm-5.2`；`kimi-k2.7-code` 256k、`MiniMax-M3` 192k。
- [2026-09-17][FACT][src:本机实测] Codex 的 `task_started.model_context_window` 记的是 **`max_context_window × effective_context_window_percent%`**（即当轮**可用**窗口），**不是** config.toml 里的 `model_context_window`；后者在桌面 app 路径下**不会覆盖** catalog / fallback 元数据。实测两组值可精确反推：catalog 内 `qwen3.8-max` = `max_context_window 983616 × 95% = 934435`；fallback 元数据 = `272000 × 95% = 258400`。
- [2026-09-17][FACT][src:本机实测] `~/.codex/cc-switch-model-catalog.json` 提供 `qwen3.8-max` 的元数据，靠 config.toml 的 `model_catalog_json = "cc-switch-model-catalog.json"` 挂载。该键**一旦丢失**，模型落入 fallback → 可用窗口从 **934435 骤降到 258400（损失 72%）** → 长会话立刻撞 ContextLimit 并触发 auto-compact。2026-09-17 10:48:57 config 被重写时该键被丢弃，10:49:52 起本 thread 即从 934435 变 258400（rollout 内 13 条 `task_started` 可逐条对上），11:52 已加回。
- [2026-09-17][FACT][src:本机网络实测] `github.com` 的 **release 资源下载与 `git clone` 均被重置**（`objects.githubusercontent.com` 亦不通），但 **`api.github.com` 直连可用**（可取 release 的 `digest` 字段做校验）。可用镜像：**`ghproxy.net`（同时支持 HTTP 下载与 git clone）**、`gh-proxy.com`（仅 HTTP）；`gitclone.com` 返回 502、`github.moeyy.xyz` 不可达。`index.crates.io` / `static.crates.io` 直连可用（后者根路径 403 属正常）。
- [2026-09-17][FACT][src:本机实测] `cargo-deny 0.20.2` 的 `[advisories] db-path` **只接受字符串**；写成数组会让整个 deny.toml 解析失败（`expected a string`）→ deny 门禁形同虚设。`cargo deny check` 需 clone `RustSec/advisory-db`，本机可用**进程级** git 环境变量重定向到镜像，无需改全局 git 配置、也无需把镜像写进仓库：`GIT_CONFIG_COUNT=1`、`GIT_CONFIG_KEY_0=url.https://ghproxy.net/https://github.com/.insteadOf`、`GIT_CONFIG_VALUE_0=https://github.com/`。
- [2026-09-17][FACT][src:本机实测] 本地工具链已补齐：`cargo-deny 0.20.2`、`cargo-llvm-cov 0.9.1`（均取 GitHub 官方 release 预编译包，经 api.github.com 的 `digest` 逐个 sha256 校验通过）+ `llvm-tools` 组件已装。**TASK-001 此前缺的两条验收命令现已全绿**：`cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`（退出码 0）；`cargo llvm-cov --workspace --fail-under-lines 75` → 99 测试通过、**行覆盖 96.31%** / 函数 94.26% / 区域 95.18%（退出码 0）。
- [2026-09-17][FACT][src:人类裁决 2026-09-17] **DRIFT-001-1 已裁决：接受**「`hygiene` 实现 gov §5.4 的 **3/11 项** + 其余 8 项规则与 `verify-schemas`/`codegen`/`replay` 三个子命令在 `deferred.rs` 显式登记并运行时失败（退出码 3）」这一产出边界；§4 中 2026-09-16 那条 REJECTED 的「待人类确认」到此解除。卡面已同步修订：`plans/stage-0-spikes.md` TASK-001 的 In scope #1 加裁决注记，验收命令第 5 条拆为两条独立命令并注明**判定以 `-- deferred-rules` 摘要行为准、`verdict=PASSED` 不等于 11 项全过**。**PL-001 / PL-011 未一并裁决**；若采纳 PL-011（新增 CRLF 规则）则 `3/11` 需改述为 `3/12`。
- [2026-09-17][FACT][src:本机实测] **本机 `http://127.0.0.1:30000` 有可用 HTTP 代理，github.com 经它完全正常** —— 实测：`git clone --depth=1 https://github.com/RustSec/advisory-db` 成功取得 1265 个文件；release 资源可下载，且经代理取到的 `.sha256` 内容与直连 `api.github.com` 的 `digest` **逐字一致**；`cargo deny check` 仅靠该代理即通过（无需 `ghproxy.net` 镜像）。→ **优先用本地代理而非第三方镜像**（镜像有被替换风险，代理是直连语义）。仅代理 github 的持久写法：`git config --global http.https://github.com/.proxy http://127.0.0.1:30000`；进程级临时写法（不落盘）：`GIT_CONFIG_COUNT=1` + `GIT_CONFIG_KEY_0=http.https://github.com/.proxy` + `GIT_CONFIG_VALUE_0=http://127.0.0.1:30000`。注意本机原先**没有任何 global gitconfig**（PL-008），首次 `--global` 写入会新建该文件。
- [2026-09-17][FACT][src:本机实测] TASK-001 的验收已从 **5/6 变为 6/6**：`cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`（退出码 0）；`cargo llvm-cov --workspace --fail-under-lines 75` → 99 passed、**行覆盖 96.31%** / 函数 94.26% / 区域 95.18%（退出码 0，阈值 75）。逐文件行覆盖：`cli.rs` 100%、`deferred.rs` 100%、`hygiene.rs` 99.17%、`report.rs` 98.70%、`rustscan.rs` 95.27%、`repowalk.rs` 93.64%、`main.rs` 90.39%。→ `tasks/TASK-001-repo-skeleton.md` §7.4 标注的「安装工具后应回填实测数字」可用此数据回填。
### 工程护栏与仓库治理

- [2026-09-17][FACT][src:人类裁决 2026-09-17] **DRIFT-001-2 已裁决：接受**，a/b/c 照建议执行 —— **a** CI 硬门禁确认为 **6 项**（fmt、clippy `-D warnings` 含 `[workspace.lints]` 禁用项、test、deny、build、`xtask hygiene`），计数口径澄清为「gov §5.1 共 **16** 行，#3 由 #2 覆盖 → **6 硬 + 9 软 = 15 个 CI 步骤 ↔ 16 行清单**」（卡面原文 5+9=14 是重复计了 #3、又漏掉了已就绪的 hygiene）；**b** `plans/stage-0-spikes.md` TASK-001 In scope #4 已改为 6 项并补口径注记；**c** 采纳**元门禁** → **ADR-0019（Accepted）**「硬门禁必须配负向验证」，定义 N1 单元负向用例 / N2 CI 显式失败步骤 / N3 canary 工作流三种形式，并规定**软门禁转硬的那张卡必须同时提交负向验证并在登记表补一行**（自 TASK-015 起适用）。**未一并处理**：PL-001（gov §5.1 行数 vs stage-1「14 项门禁」表述）、PL-011（第 12 项 CRLF 规则）、fmt/clippy/build 三项 canary 缺失 → 新登记 **PL-018**。
- [2026-09-17][FACT][src:本机实测 cargo-deny 0.20.2] **ADR-0019 的 deny canary 双向均已验证**：正向 `cargo deny check licenses bans sources` → `bans ok, licenses ok, sources ok`，**exit 0**，且**不需要联网**（只有 `advisories` 要 clone advisory-db）；负向 `cargo deny --config <把 db-path 写成数组的坏配置> check licenses bans sources` → `error[wanted]: expected a string` + `failed to deserialize config`，**exit 1**。→ 这两条构成 `gate-selftest.yml` 的正/负断言。负向断言**必须同时校验 stderr 含 `expected a string`**：只断言"非零退出"的话，任何无关错误（网络、路径、权限）都会让 canary **假绿**。
- [2026-09-17][FACT][src:api.github.com 实测] `EmbarkStudios/cargo-deny-action` 的 **release `v2.1.1` 标题即 "Release 2.1.1 - cargo-deny 0.20.2"**，且该 action **没有 `version` 输入**（inputs 仅 command / arguments / command-arguments / manifest-path / log-level / rust-version / credentials / ssh-key / ssh-known-hosts / use-git-cli）→ **钉 action 的 release tag 就等于钉 cargo-deny 版本**，不需要在 CI 里 `cargo install`（省 5~8 分钟）。`ci.yml` 已由 `@v1` 改为 `@v2.1.1`，与本机同版本 → **PL-016 根因关闭**。
- [2026-09-17][FACT][src:本机 git 实测] **远端已建立、提交身份已重写（PL-008 关闭）**：`origin = https://github.com/snofin18/ai-assistant.git`（公开库，2026-09-17T07:36Z 建，建库时为空）；`git filter-branch --env-filter` 把全部 **7** 个提交的 author+committer 从占位身份 `Codex (ai-assistant agent) <codex@localhost.invalid>` 重写为 **`snofin18 (via Codex) <snofin@gmail.com>`**（人类裁决方案 B：保留 AI 溯源 + 邮箱合法），**原始 author date 全部保留**，工作树逐字节未变（`git diff backup/pre-author-rewrite-20260917 main` 为空）。哈希映射：`58fed3d→9d0fbee`、`c30c502→a03d2f9`、`bcc890e→3a90c7c`、`5cbab04→2cab794`、`d56ba6b→da3b9b1`、`2b31301→ab1068d`、`2788edd→dbbb3a0`。旧提交保留在**本地**分支 `backup/pre-author-rewrite-20260917`（不推送，确认远端无误后可删）。仓库本地 `user.name` 已设为 `snofin18 (via Codex)`（global 仍为 `snofin18`）以保持后续提交一致。
- [2026-09-17][FACT][src:GitHub Actions run 35198879508] **CI 首次运行即三平台全绿**（08:17:23→08:20:42Z，约 3m19s）：`cargo deny`（用钉好的 `cargo-deny-action@v2.1.1`）、`check (ubuntu/windows/macos-latest)`、`xtask deferred inventory` 五个 job 全部 `success`。→ **TASK-001 的 DoD「CI 在空 workspace 上三平台全绿」到此才真正被机器验证**（此前仓库从未推送、CI 一次都没跑过，正是 ADR-0019 背景②所指）；`deny.toml` 的 `db-path` 修复与 action 钉版本也得到端到端确认。附带观察：软门禁 #9（覆盖率）在 CI 里用 `cargo install cargo-llvm-cov --locked` 从源码编译，是三平台 job 的主要耗时来源 → TASK-015 转硬时应改为下载预编译二进制。
- [2026-09-17][FACT][src:本机只读侦察，TASK-002 步骤 1] **靶机环境基线（Spike A 的实测前提）**：OS = **Windows 11 25H2，build 26200.9457**（真值取自 `DisplayVersion`+`CurrentBuild`；`ProductName` 谎报 "Windows 10 Pro"，见 §5）；记事本 = **`Microsoft.WindowsNotepad` 11.2607.14.0 x64**，打包版，装于 `C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11.2607.14.0_x64__8wekyb3d8bbwe`（即含标签页的新版 Notepad，非 Win32 老版）；显示 = **3200×2000、AppliedDPI=192（200% 缩放）**、NVIDIA RTX 5060 Laptop GPU；输入法 = `zh-Hans-CN` 首位、TIP `0804:{81D4E9C9-1D3B-41BC-9E6C-4B40BF79E35E}{FA550B04-5AD7-411F-A5AC-CA038EC515D7}`（微软拼音），另有 `en-US`/`en-GB` → 卡面要求的「IME 开/关两态测中文写入」**本机可测**；`inspect.exe` **已就绪**（`C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\inspect.exe`，SDK 10.0.26100.0，另有 x86/arm64）；**Accessibility Insights for Windows 未安装**（`%LOCALAPPDATA%\Programs\Accessibility Insights for Windows` 不存在）→ 卡面步骤 2 需人类裁决：补装，或以 `inspect.exe` + 自写 UIA 树导出替代。
- [2026-09-17][FACT][src:Spike A 先导实测 probe-01/02，详见 docs/spike-reports/SPIKE-A.md（状态 PARTIAL）] **新版记事本 11.2607.14.0 的 UIA 形状与性能已实测**：整棵控件树仅 **32 节点**；全树遍历中位数 **17.0 ms**（冷启动首次 74.2 ms，min 15.6 / max 30.0）、编辑区 `Descendants` 搜索 **1.5 ms**、`ValuePattern.GetValue` **0.11 ms**（59 字符）、`SetValue` 写 57 个中英混排字符 **4.79 ms** 且读回归一化后完全一致；记事本 WorkingSet ≈ 118.8 MB。→ 相对 go 判据（全树 ≤800 ms、局部 ≤200 ms）**余量 47× / 133×**。编辑区 = `ControlType.Document` + ClassName **`RichEditD2DPT`** + Name `文本编辑器`，**AutomationId 为空**，父级 `Pane` ClassName **`NotepadTextBox`** 可作锚点；`ValuePattern` 可读写（IsReadOnly=False）且 **`TextPattern` 也支持、返回文本逐字符相同** → 优先 TextPattern（能做范围/选区），ValuePattern 作降级。**go/no-go 尚未给出**：跨进程「另存为」对话框、1 MB 大文件、失败注入、以及 **Rust/COM 路径**都还没测（PowerShell 用托管封装，Rust 走 COM，批量取属性行为不同，耗时可能差一个数量级）。
- [2026-09-17][FACT][src:Spike A 先导实测] **新版记事本是「单窗口 + 多标签 + 多进程」结构，PID 不可用于定位**：`Start-Process notepad <file>` 返回的 PID 与真正持有窗口的 PID **不是同一个**（实测 30464 vs 20304，**事先杀掉所有 Notepad 仍然如此**）；打开第二个文件后系统里有 **3 个 Notepad 进程、但只有 1 个窗口、TabItem 变成 2 个**。→ Adapter 必须**按 ClassName `Notepad` + Name 后缀 `- Notepad` 定位窗口、按 `Tab aid='Tabs'` → `List aid='TabListView'` → `TabItem` 寻址内容**，绝不能按"我启动的那个 PID"找窗口；"打开文件"的真实语义是**在当前窗口新增标签页**。TabItem 的 Name 形如 `<文件名>. 未修改。`，**自带脏标记**，但属本地化可见文本，按 AGENTS.md 只能作辅助信号、不能作主 selector。
- [2026-09-17][FACT][src:Spike A 先导实测] **记事本的 AutomationId 是英文、Name 是本地化文本，且 AutomationId 不唯一**：`aid='File'`↔`name='文件'`、`aid='CloseButton'`↔`name='关闭标签页'`、`aid='AddButton'`/`'MenuBar'`/`'SettingsButton'`/`'FREButton'` 同理 → 实证支持 AGENTS.md「禁止用控件可见文本作主 selector」。状态栏有 **6 个** `Text aid='ContentTextBlock'`（行/列、字符数、空、缩放、` Windows (CRLF)`、` UTF-8`）→ 状态栏只能靠**索引 + Name 模式**区分。另有**瞬态** `TeachingTip` 节点（`PrivacyTeachingTip`、`PsDownloadDetailTeachingTip`、`PsDownloadLightTeachingTip`，后者提到"本地 AI 模型必须完成下载"）→ **UIA 树不稳定**，架构 v2 §17.4 的录制回放 fixture 必须在 UI settled 后采集、且比对要容忍瞬态节点。

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
- [2026-09-17][REJECTED][src:探针实测 2026-09-17][supersedes:2026-09-16] **在第三方 Responses 端点上用 Codex 内置 automation（heartbeat / cron 皆然）做无人值守** —— 否决理由与旧条目**不同**：不是因为 cron 建不起来（它建得起来，旧条目记载有误），而是因为两者**共用同一投递机制**、都产生畸形 `function_call_output`。cron 首轮投递时 Codex 会临时生成非法 `at_<uuid>` id（**从不落盘** → 等长补丁法无效），两次探针均在 2.6s 内零产出失败。heartbeat 更糟：毒项沉入长驻会话 + `disable_response_storage=true` 全量重放 → 该会话**永久损坏**，连白天交互也受牵连。改为 `codex exec` CLI + Windows 任务计划程序（已实测可行，见 §2 FACT 2026-09-17）。

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
- [2026-09-17][PITFALL][src:探针实测] Codex automation 的 prompt **不是** user message，而是合成 `function_call_output`（`name=automation_update`、`namespace=codex_app`、`id=None`、`call_id=None`）。官方端点容忍，第三方严格端点 400。**两种表现要分清**：作为**历史重放** → `Invalid 'call_id': call_id is required`（落盘 id 是合法的 `fco_…`，可等长补丁修）；作为**首轮投递** → `Invalid 'id': … must start with 'msg_', got 'at_<uuid>'`（id 序列化请求时临时生成、从不落盘，**无法补丁**）。→ 换 provider 或改用 CLI，别试图修 rollout。
- [2026-09-17][PITFALL][src:探针实测] `disable_response_storage=true` + 一条畸形历史项 = **会话永久损坏**：每轮重放全量历史，毒项每次都被重发，该 thread 之后每次提问都失败，而新开 thread 完全正常。→ 症状是"只有这一个会话坏、别的都好"，别误判成账号 / 网络 / 模型问题。
- [2026-09-17][PITFALL][src:探针实测] 改 Codex rollout 必须**严格等字节长度**：`thread_history_1.sqlite` 的 `next_rollout_byte_offset` 记的是字节偏移，长度一变 UI 历史投影就错位。腾位办法：把对模型无用的 `,"namespace":"codex_app"`（24B）原地换成 `,"call_id":"hb_…"`。写完必须校验 `next_rollout_byte_offset == 文件实际大小`。写入用**原地 seek+write** 而非 temp+replace（Codex 运行时持有句柄，Windows 下 `os.replace` 会 PermissionError）。参考实现 `D:\csart\fix_codex_callid.py`。
- [2026-09-17][PITFALL][src:探针实测] Codex automation 调度有约 **+2 分钟 jitter**（`~/.codex/automations/.run-jitter-salt`）：约定 11:16 实际 11:17:54 触发。→ 定时间表要留余量；外部调度（任务计划程序 + `codex exec`）无 jitter。
- [2026-09-17][PITFALL][src:探针实测] `automation_update` 建 cron 缺必填项时只回 `Failed to create automation.`，**不说是哪个字段**（本次因此误判成"cron 需 ChatGPT 鉴权、本环境不可用"，白烧一晚）。必填：`name`/`prompt`/`rrule`/`status`/`projectId`/`model`/`reasoningEffort`/`executionEnvironment="local"`。→ 探 schema 的办法是故意只传 `kind`，让校验器把缺失字段一次性全列出来。
- [2026-09-17][PITFALL][src:探针实测] 上下文压缩会把补丁过的自动化残留报成 `Orphan function call output for call id: hb_…`（ERROR 级，来自 `run_auto_compact`）。这是**非致命噪音**：Codex 记录后丢弃该项、turn 照常继续。→ 看到它不等于出错，但等于"这个会话里有补丁过的 automation 残留"。
- [2026-09-17][PITFALL][src:本机实测] **Codex 桌面应用会自动重写 `config.toml`**（把 `notify` 数组展开成多行、增删 `[projects.*]`、写 `[shell_environment_policy]`、改 pipe 名等），重写过程中**可能丢失非标准键** —— 本次就丢了 `model_catalog_json`，直接导致上下文窗口缩水 72%。→ 每次改完 config 或发现窗口/模型行为异常，先 `Compare-Object` 与备份 diff 一遍，确认关键键还在；改动前先做 `.bak`。
- [2026-09-17][PITFALL][src:本机实测] `config.toml` 里 `model_context_window` 写得再大也**不一定生效**：桌面 app 路径以模型元数据（catalog 或 fallback）为准。→ 判断真实可用窗口不要看 config，要看 rollout 里 `task_started.model_context_window` 的实测值。
- [2026-09-17][PITFALL][src:本机实测] CI 用 `EmbarkStudios/cargo-deny-action@v1`（该 action 最新已是 **v2.1.1**）且**未钉 cargo-deny 版本**，本地装的是 0.20.2 —— 两边版本不同会让"本地红、CI 绿"或反之。本次 `db-path` 数组写法就是被 CI 掩盖的。→ 建议 CI 显式钉版本，并把 deny.toml 能否解析纳入本地验收（PL-016）。
- [2026-09-17][PITFALL][src:本机实测] `git remote add` 报 `fatal: not a git repository` 与网络 / 代理 / 账号 / 仓库存不存在**完全无关** —— 它是**纯本地**操作，报错只因 cwd 不在仓库内（本次在 `C:\Windows\System32` 跑的，`Test-Path C:\Windows\System32\.git` = False；仓库在 `D:\csart\ai-assistant`）。→ 任何 git 命令前先 `git rev-parse --show-toplevel` 确认。同批踩到的另两个坑：① 从文档复制命令时**把占位符的尖括号一起打进去**（`git@github.com:<账号>/...`），git 会把尖括号当字面 URL 存下来；② 本机 `~/.ssh` 只有 `known_hosts`、**没有密钥对**，且 global 配的 `http.https://github.com/.proxy` **只对 HTTP/HTTPS 生效、不覆盖 SSH 的 22 端口** → 一律走 **HTTPS**，不要走 SSH。
- [2026-09-17][PITFALL][src:本机实测] **在非交互会话里跑 `git ls-remote` / `git push` 会挂死**：Git Credential Manager 被调起后等待 GUI/浏览器授权，命令行侧既不返回也不报错（本次挂 40s+，只能 `Stop-Process -Name git,git-credential-manager,git-remote-https -Force` 清掉）。→ 自动化脚本必须先设 `GIT_TERMINAL_PROMPT=0` 并加超时；**首次 push 交人类在自己的终端里跑**。补充事实：本机 Windows 凭据管理器里**已有** `git:https://github.com` 条目（`cmdkey /list` 可见），所以挂住更可能是 GCM 的 UI 交互流程而非"没有凭据"。
- [2026-09-17][PITFALL][src:本机实测 cargo-deny 0.20.2] cargo-deny 的 CLI 有两个反直觉点：① **`-c` 是 `--color` 而不是 `--config`** —— 写 `cargo deny -c x.toml` 会得到 `invalid value 'x.toml' for '--color <COLOR>'`（exit 2），必须写全称 `--config`；② `cargo deny check` **不接受 `--offline`**（会提示 `to pass '--offline' as a value, use '-- --offline'`，exit 2），但 `check licenses bans sources` 本身**不需要网络**（只有 `advisories` 要 clone advisory-db）→ 离线自检直接去掉该 flag 即可。另注：正向检查会打印一条 **warning** `unmatched license allowance: BSD-3-Clause`（白名单里有但当前无依赖使用），**不影响 exit 0**，不要误判为失败。
- [2026-09-17][PITFALL][src:本机实测] **`HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion` 的 `ProductName` 在 Windows 11 上仍谎报 "Windows 10 Pro"**（本机实际 25H2 / build 26200.9457，`ProductName` 却返回 `Windows 10 Pro`）。→ 判定 Windows 版本**必须用 `DisplayVersion` + `CurrentBuild`（+ `UBR`）**；`ProductName` 与 `[Environment]::OSVersion` 都不可靠。这条对本项目是**产品级**陷阱：架构 v2 的平台基线是「Windows 11 24H2+」，若拿 `ProductName` 做能力探测会把 25H2 误判成 Win10 并错误降级功能。
- [2026-09-17][PITFALL][src:本机实测，**supersedes:2026-09-17** 上文「非交互会话里 `git ls-remote`/`git push` 会挂死」那条的推断部分] 挂住的**真因已确认**：当时远端仓库**还不存在**，GitHub 对不存在仓库的 `info/refs` 返回 **401**（不是 404，以免泄露仓库是否存在）→ GCM 转入交互式授权并等待 GUI，命令行侧永久阻塞。仓库建好后，**同一条 `git push` 在 `GIT_TERMINAL_PROMPT=0` + `GCM_INTERACTION=never` 下 3 秒内 exit 0**，凭据取自 Windows 凭据管理器既有的 `git:https://github.com` 条目。→ 结论修正：**不是「非交互会话一定挂」，而是「遇到 401 一定挂」**。自动化脚本除设这两个变量外，应先 `curl https://api.github.com/repos/<owner>/<repo>` 确认 200 再动 git。
- [2026-09-17][PITFALL][src:仓库配置实测] **`spikes/` 被 workspace `exclude`（`Cargo.toml`：`exclude = ["spikes", "fixtures/apps", "tools"]`）→ deny / fmt / clippy / test 四道硬门禁全都覆盖不到 spike 代码**。阶段 0 这是设计意图，但意味着 spike 里加依赖只有政策约束、没有机器约束（PL-019）。→ spike 卡开工前先确认依赖已写进 `docs/DEPENDENCIES.md`；TASK-002 要用的 `windows` crate 已在「计划首批」清单里，但 **`uiautomation` 不在**，需补行并走批准。
- [2026-09-17][PITFALL][src:本机实测] **仓库里有文件的末尾缺换行**（本次发现 `docs/DEPENDENCIES.md` 末行无 `\n`）。后果：任何以「整行 + `\n`」为锚点的批量编辑脚本都会 `count==0` 而断言失败，且失败信息与真实原因（缺换行）完全无关，极易误判成"锚点写错了"。→ 已补齐该文件末尾换行；建议把「文本文件必须以单个 `\n` 结尾」做成 `xtask hygiene` 规则（与 PL-011 的 CRLF 规则同属 gov §5.4 家族，可一并裁决）。
- [2026-09-17][PITFALL][src:Spike A 先导实测] **记事本的 `ValuePattern` / `TextPattern` 返回的文本用裸 `\r`（CR）分行，而磁盘文件是 `\r\n`（CRLF）** —— 62 字符的 CRLF 文件读回来只有 59 字符。→ 任何「写入后读回校验」必须先把 `\r\n`、`\r`、`\n` **三者全部**归一化，否则必然假阴性；只做 `Replace("\r\n","\n")` **不够**，还要再 `Replace("\r","\n")`。第一次探针就因此得到 `value == file content ? False` 的错误结论。
- [2026-09-17][PITFALL][src:Spike A 先导实测] **打开一个「已经在标签页里」的文件，记事本不会重新加载磁盘内容**（只切到那个标签页）→ 探针读到旧内容、产生假阴性。→ 探针必须**先杀干净所有 Notepad 进程**并使用**带时间戳的唯一文件名**。这条对产品同样关键：外部程序改了 `.txt` 之后，记事本内存态**不会自动更新** → 架构里的「文件契约（L1）」与「UI 态（L3）」可能不一致，撤销/校验逻辑必须显式处理这个偏差。
- [2026-09-17][PITFALL][src:Spike A 先导实测] **`SetValue` 走 UIA Pattern、不经过键盘，因此 IME 开/关对它没有任何影响** → 卡面步骤 5「中文经 `SetValue` 写入，分别在 IME 开/关两态测」对 `SetValue` 路径是**空操作**；IME 两态对照真正针对的是 **L4 合成键盘输入**。→ 已记为对卡面措辞的疑问（`docs/spike-reports/SPIKE-A.md` §5.3），由 TASK-002 正式会话记 `DRIFT-002-x` 交裁决，**本次侦察未改卡面**。
- [2026-09-17][PITFALL][src:Spike A 先导实测] **PowerShell 控制台输出中文会乱码**（控制台代码页问题），但**进程内的字符串比较不受影响**。→ 判定一律以脚本内的 `True/False` 与长度为准，不要用肉眼读控制台就断定"中文被写坏了"（本次差点因此误判 `SetValue` 破坏了中文）。同类问题也会让 Python 脚本 `print` 中文时抛 `UnicodeEncodeError: 'gbk' codec`——需要输出中文时改为写文件再读，或 `sys.stdout.reconfigure(encoding='utf-8')`。

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
- [2026-09-16][OPEN][M8]（**2026-09-17 更新：前两条已裁决关闭，见 §2；仅第三条待裁决**）TASK-001 的三条 DRIFT 待裁决（hygiene 范围 / CI 硬门禁数量 / 执行记录落盘位置），其中第三条会牵动 gov §3.2 任务卡模板与所有后续卡的 write scope → 阶段 0 内裁决。

**ASSUMPTION（假设，未验证，不得当作结论使用）**
- [2026-09-16][ASSUMPTION][src:feasibility §2.3] 同花顺/通达信/东方财富的行情与交易界面以 GDI 自绘为主，UIA 覆盖差 → 需 Spike 实测后升级为 FACT 或推翻。
- [2026-09-16][ASSUMPTION][src:feasibility §2.3] 上述软件普遍支持"键盘精灵 + 数据导出/复制到剪贴板"作为可行只读通道 → 需实测。
- [2026-09-16][ASSUMPTION][src:v2 §13.4.6] Java Swing/AWT 在 Linux 上需要 `libatk-wrapper` 或 JVM 参数才暴露 AT-SPI → 需实测。
- [2026-09-16][ASSUMPTION][src:v2 §15.4] 树快照 zstd 压缩比 5~10x、内容寻址去重收益显著 → 需 Spike H 实测。

---

## §7 归档索引

（暂无。超过 300 行时，最老的 §2/§5 条目移入 `docs/memory/archive-YYYYMM.md` 并在此登记。）
