# 跨平台本地 AI 助理（Cross-Platform Local AI Assistant）

让大模型**受控地**操作你电脑上已有的常用应用（记事本、画图、Edge/Chrome、Excel、
Photoshop…）：模型负责理解与规划，所有动作都通过**注册的工具**执行，
全过程可审计、可撤销、可回放。**默认拒绝**，不可逆动作必须人工确认。

> 状态：**阶段 1（三试点闭环：Notepad → Paint → Edge/Chrome）** —— 阶段 0（文档与 Spike）已于 2026-09-20 closeout；TASK-022 `crates/task-engine` 已 Done（12 状态机 / Plan+Step DAG / 检查点 / 恢复 / 取消 / 预算 / 看门狗 / 87.03% 行覆盖）；**TASK-023** `crates/verify` 已 Done（11 种后置断言 + 状态指纹 + 幂等判定 + `on_violation` 分派；**验证失败绝不返回 ok**）；**TASK-024** `crates/undo` 已 Done（L0~L3 + 内容快照 / 影子副本 / undo 预算锚点 + 回滚剧本 + 默认最保守冲突检测 + incident 上报；L0 支持 L1 fallback）；**TASK-025** `crates/lease` 已 Done（`exclusive/shared/intent` 三模式 + TTL / 续租 / 用户抢占 + 规范 key 排序与零提交批量获取；20 个契约测试 / 89.53% 行覆盖）；**TASK-026** `crates/model-gateway` 已 Done（流式 Provider / 1 s 可观察取消 / 类型化路由 / 重试退避 + 降级链 / prompt cache 提示 / 整数成本记录；21 个测试入口 / 75.23% 行覆盖）；跨阶段治理卡 **TASK-200~204** 全部 Done（**TASK-204** = `tool-bus` draft-07 关键字判据硬化：三张拒绝表 + `$schema` 方言校验 + `format`/`pattern` 永久放弃）；
> 产品代码自阶段 1 起才落地（`crates/protocol` / `crates/storage` / `crates/audit` / `crates/core` 骨架 / `crates/secrets` /
> `xtask` 护栏 / **`crates/platform/api`**（平台抽象层：4 个纯类型 + 3 个 trait 形状 + 能力矩阵）/
> **`crates/platform/windows`**（Win32 / UIA provider：树快照 / selector 链解析 / 读写 / 指纹 / 窗口枚举）已完成，
> TASK-017 的 7 条 DRIFT 已全部裁决落地（**ADR-0043** 元素解析加 scope / **ADR-0044** 歧义策略收敛为唯一
> `ErrorAndAsk` / **ADR-0045** 非宿主平台编译门禁进 `AGENTS.md` §6；PL-068 / 069 / 070 闭环），
> 同 crate 的合成输入（`SendInput`）+ 坐标归一化（DPI / 多屏）+ IME 也已落地（TASK-018 —— 铁律 5 的 **L4** 层；
> 真机验收 2/2；新提 PL-074）；`apps/automation-host` + `crates/ipc`（TASK-019：帧 / 握手 token / NamedPipe / 对端身份白名单 / 双向心跳 / 看门狗）也已落地并经真实进程 kill 断连验收；**`crates/tool-bus`**（TASK-020：MCP client(`rmcp`) + **同进程** MCP server + draft-07 子集参数校验（不支持即拒绝） + 统一返回信封（`untrusted` / `truncated`） + 工具集指纹 + 动态挂载（> 40 告警））、**`crates/policy`**（TASK-021：唯一放行点 / 默认拒绝 / deny 优先 / DSL v0 / 参数护栏）、**`crates/undo`**（TASK-024：L0~L3 + 三类锚点 + 回滚剧本 + 冲突检测 + incident）、**`crates/lease`**（TASK-025：三模式矩阵 + TTL / 续租 / 用户抢占 + 零提交批量获取）与 **`crates/model-gateway`**（TASK-026：同步拉取式流 / 路由 / fallback / backoff / cache hint / 成本）均已落地，下一张是 TASK-027。**当前阶段详情以 `PLAN.md` 为准**。

---

## 这个项目最特别的一点

它主要由 **AI coding agent**（Codex / opencode / Claude Code）实现，人类负责规划、审阅与裁决。
因此仓库里有一半的"代码"其实是**约束 agent 的机制**：
任务卡、write scope、约束回执、漂移触发器、机器护栏、长期记忆文件。
如果你只想看架构，直接读架构文档；如果你想让 agent 在这个仓库里干活，**必须先读 `AGENTS.md`**。

---

## 产品层 vs 工程元层（读任何文档前先分清这一条）

本项目由 AI coding agent 实现、人类审阅，因此仓库里有大量**不属于产品**的东西。
判定标准只有一句话：**「删掉它，产品的行为会变吗？」不会 → 工程元层。**

| 层 | 包含 | 会被构建进发布物吗 | 受什么约束 |
|---|---|---|---|
| **产品层** | `crates/`、`protocol/`、`adapters/`、`adapters-private/`、`apps/`、`fixtures/`、`eval/` | ✅ 是 | `docs/spec/*` 的契约 ＋ `AGENTS.md` |
| **工程元层** | `AGENTS.md`、`docs/governance-ai-agent-execution.md`、`docs/subagent-orchestration.md`、`docs/overnight-automation-charter.md`、`docs/nightly/*`、`MEMORY.md` ＋ `docs/memory/*`、`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/adr/*`、`xtask/`、`.github/workflows/*` | ❌ 否 | 只受 `AGENTS.md` 约束 |

**为什么要正式区分**（ADR-0029 D5）：这条边界此前只存在于口头，造成三个真实症状 ——
① 文档地图把「夜间自动化章程」与「存储设计」并列，读者（尤其是将来开源后的外部读者）
分不清哪些是产品的设计、哪些是「我们怎么干活」；② 每次讨论夜间自动化都要重新解释一遍
「它不影响 `crates/` 里的任何代码」；③ 护栏工具与治理文档的体积已接近产品文档，
若不分类，「阶段 0 零产品代码」这个事实会被掩盖（此前只能靠一句免责声明打补丁，那是补丁不是结构）。
边界不清的真实代价是**注意力错配**：工程元层的讨论会被误当成产品需求变更，反之亦然。

> **推论**：工程元层的 ADR（夜间自动化机制、护栏工具口径、ADR 编号治理…）**不构成产品决策**，
> 也不改变任何 `docs/spec/*`。审阅它们不需要产品上下文；反过来，审阅产品 spec 也不需要读它们。

---

## 文档地图（按需读，不要全读）

| 你想做什么 | 读什么 |
|---|---|
| 让 AI agent 在本仓库干活 | **`AGENTS.md`**（唯一入口，含十条铁律与会话协议） |
| 了解整体架构（分层、进程边界、安全、平台差异） | `cross-platform-ai-assistant-architecture-v2.md` |
| 了解"某个应用能不能接、怎么接" | `target-apps-feasibility.md` |
| 了解现在做什么、不做什么 | `PLAN.md` → `plans/<当前阶段>.md` |
| 回忆项目已知什么 / 否决过什么 / 踩过什么坑 | `MEMORY.md`（**L0 索引**，≤150 行，含路由表）→ 按路由跳读 `docs/memory/*` |
| 了解治理机制（任务卡、CI 门禁、代码规范、验收分离） | `docs/governance-ai-agent-execution.md` |
| 了解多 agent 如何分工 | `docs/subagent-orchestration.md` |
| 了解存储方案与性能预算 | `docs/storage-design.md` |
| 了解全项目拆解顺序 | `docs/wbs-overview.md` |
| 了解夜间无人值守自动化的**纪律与边界** | `docs/overnight-automation-charter.md`（章程 v1.4 §11：一主一备 —— 主 = Codex 原生 scheduled tasks，备 = 任务计划程序 + `codex exec`） |
| 查夜间自动化的**具体操作**（建 / 改 / 删 / 立即运行 / 暂停 / 停止 / 恢复） | `docs/nightly/codex-automations-operations.md`（ADR-0029 的主交付物；每条结论都带 `[官方]` / `[实测]` / `[未验证]` 证据标签） |
| 查契约细节 | `docs/spec/`（naming 已就位，其余陆续产出） |
| 查"为什么当初这么决定" | `docs/adr/README.md`（**编号登记表**：哪些号已有文件 / 哪些只是待建）→ 具体 `docs/adr/NNNN-*.md` |
| 查台账 / 停车位 / 依赖登记 | `LEDGER.md` / `docs/PARKING_LOT.md` / `docs/DEPENDENCIES.md` |

---

## 当前阶段

**阶段 1 — 三试点闭环**（Notepad → Paint → Edge/Chrome；TASK-011 ~ TASK-058）。
子阶段 1a 已开工：地基层的 `crates/protocol`（schema + codegen）/ `crates/storage`（SQLite WAL + 迁移注册表 + blob）/
`crates/audit`（append-only hash chain）/ `crates/core` 骨架 / `crates/secrets`（OS keychain 封装）/
`xtask` 护栏（`docscan` 4 条结构规则 + `check-ledger` + `check-migrations` + `crates/core` 分层断言）/
**`crates/platform/api`**（铁律 7 的唯一平台入口：`TargetDescriptor` / `NormalizedPoint` / `Fingerprint` /
`CapabilityMatrix` + `PlatformService` / `WindowProvider` / `UiAutomationProvider` 三个 trait 形状）/
**`crates/platform/windows`**（Win32 / UIA provider：树快照 / selector 链解析 / `read_text` / `set_value` /
`edit_text` / `invoke_action` / `bounds` / `fingerprint` / 窗口枚举与状态 / **合成输入（`SendInput`）+ 坐标归一化（DPI / 多屏）+ IME**）**均已落地**；
跨阶段治理卡 TASK-200 / 201 / 202 / 203 / 204 已 Done；TASK-016 的能力命名歧义已由 **ADR-0042** 定案
（`CapabilityCatalog` = 稳定标识目录 / `CapabilityMatrix` = 运行时探测结果 / `CapabilityEntry` = 风险·审批元数据）；
TASK-017 的 7 条 DRIFT 遗留已由 **ADR-0043 / 0044 / 0045** 裁决落地（元素解析加 scope / 歧义策略收敛为唯一
`ErrorAndAsk` / 非宿主平台编译门禁进 `AGENTS.md` §6；PL-068 / 069 / 070 闭环）；
TASK-018 把铁律 5 的 **L4（合成输入）** 层补齐（`SendInput` VK 路径 + `KEYEVENTF_UNICODE` 文本路径 + 发送前 100% 前台校验 +
显示器枚举 / DPI 换算 / 虚拟屏幕归一化 + `is_ime_open`；真机验收 2/2 —— 坐标误差 0 px、记事本 Unicode + Ctrl+S 磁盘回读），
新提 **PL-074**（`pointer_action` 不带目标窗口 → 混合 DPI 多屏下逻辑点无法唯一归属显示器）。
TASK-019 把 Host IPC 传输层落地：纯函数帧编解码（magic / 16 MiB 上限 / CRC32）+ 版本化认证握手（token 常数时间比较）+
Windows NamedPipe 传输（`PIPE_REJECT_REMOTE_CLIENTS` + 客户端 PID / 镜像路径白名单）+ 双向心跳 / 看门狗；
真实验收启动 host、完成 `ServerHello` 与心跳后 kill host，客户端在 2 s 内检测断连。
TASK-020 把**工具通道**落地（架构 v2 §5.4「MCP 是唯一工具协议」）：`crates/tool-bus` 用 `rmcp` 在**同一进程**内起 MCP server、
`tokio::io::duplex` 作传输（无 socket / 无子进程 / 无网络），`tools/list` 与 `tools/call` 全链路走**真实 MCP 往返**；调用身份经 MCP `_meta` 跨边界传递。
参数按 `ToolSchema.input` 做 draft-07 **子集**校验（**不支持即拒绝注册**），不合法直接拒（`ToolInvalidArgs`）且**不进 handler**；返回统一为 `ToolEnvelope`（`untrusted` / `source` / `truncated` / `metrics` / `error` 齐全），超预算截断显式标注、截不动则 fail-closed；
工具集指纹（SHA-256，与挂载顺序无关）+ `toolset.list` / `toolset.search` 两个元工具 + 单次挂载 > 40 个工具的结构化告警（含未串链审计事件）。
TASK-021 把**唯一策略放行点**落地：`crates/policy` 用 JSON DSL v0 完整表达架构 v2 §12.2 的五条示例规则，
求值为无 IO / 无时钟 / 无随机的纯函数；deny 优先，未命中一律 `default_deny`，且 `L3 + unattended` 与
污点上下文中的 L3 / High / Critical 由不可绕过的硬底线拒绝。路径穿越 / UNC / 设备路径 / 符号链接逃逸、
URL scheme-host-IP、文本长度与行数、数值边界、正则 ReDoS 形状均走 fail-closed 护栏；每个 deny 都带
非空 `rule_id` 与 reason。新增 29 个测试 + 1 个 doctest，`policy` 行覆盖 **93.60%**，判定中位数
**3.6~5.6 µs**。`assistant_protocol::PolicyDecision` 无法携带 confirmation scope / `show_diff`，故完整决策
保留在 policy 内、协议映射只作审计摘要，协议扩展记为 **PL-082** / `DRIFT-021-1`。
TASK-022 把任务编排层落地：12 个 Task 状态与完整 Step 生命周期、确定性 DAG Frontier、每次成功迁移先写检查点、SQLite 重开恢复、暂停 / 取消 / 接管、四维预算和分阶段看门狗；恢复证据缺失或 `Unknown` 一律进入 `NeedsHuman`，绝不猜测写操作是否发生过。新增 43 个测试 + 1 个 doctest，行覆盖 **87.03%**。
TASK-023 把**后置断言引擎**落地：架构 v2 §7.4 的 11 种断言（= 12 种减 `visual_assert`，后者归 TASK-042）全部 fail-closed 解析与三值求值 —— `Verified` 要求全部满足，任一 `Falsified` 即 `Violated`，无 `Falsified` 但有 `NotEvaluable` 即 `Inconclusive`，**两种非成功结果都带 `VerifyFailed`**，结构上不可能出现「验证失败却 ok」。另含 §7.3 状态指纹（canonical form + SHA-256；忽略字段由 per-adapter 显式给出，无全局默认）、§8.5 幂等判定（证据不全一律 `Unknown`，应用自报优先）与 `on_violation` 分派（`retry_once` 只对 `Transient` / `TargetNotFound`）。新增 66 个测试 + 1 个 doctest。
TASK-024 把**撤销闭环**落地：`assistant-undo` 用纯逻辑建模 L0~L3、内容快照 / 影子副本 / undo 预算 / 补偿剧本 / evidence-only 锚点；L0 可带 L1 snapshot fallback。冲突检测默认最保守：当前状态偏离 post 指纹即阻断，缺 post 或当前指纹时即使 `RestoreOverall` 也不放行。回滚动作通过注入 `RollbackExecutor` 执行，只有最终指纹等于锚点 pre 指纹才算成功；执行失败、指纹不一致与冲突都变成带 expected/observed/evidence/recovery guidance 的结构化 incident，上报失败显式返回 `Fatal`。新增 23 个测试。

TASK-025 把**目标租约与并发控制**落地：`assistant-lease` 建模 `Shared` / `Intent` / `Exclusive` 三模式，跨 owner 只允许一个写租约；`Intent` 可并发规划但阻止另一 owner 的 `Exclusive`。TTL 以 `now == expires_at_ms` 为失效边界，支持显式续租、懒清理和 `reap_expired`；用户抢占按 target key 强制释放全部 agent 租约，并把自然过期项单列报告。`acquire_many` 在任何状态写入前按规范 key 排序、去重并完成冲突 / TTL / 溢出预检，任一冲突即零提交，消除“先持有再回滚”窗口。新增 20 个测试，行覆盖 89.53%，零新增第三方依赖。
TASK-026 把**模型网关**落地：`assistant-model-gateway` 定义同步拉取式 Provider 流契约（每轮 poll 必须检查取消并遵守 timeout）、类型化 stage/context/sensitivity/budget/tool-count 路由、指数退避与注入 jitter、可审计降级链、prompt-cache 稳定前缀提示透传，以及按模型聚合的整数 micro-USD 成本记录。第一个非空 text / tool-call delta 之后失败一律返回 `PartialOutput`，禁止重试或降级以避免重复输出；成功流必须恰有一次 usage + finish。新增 20 个契约测试 + 1 doctest，行覆盖 75.23%，零新增第三方依赖。
TASK-204 把 `tool-bus` 的 draft-07 判据**显式化**：三张带理由的拒绝表（永久放弃 12 条 / 暂未实现 4 条 / 非 draft-07 方言 15 条）+ 未知关键字兜底 + `$schema` **方言校验**（声明非 draft-07 = 拒绝），报错信息直接给出「为何不可用 / 该用什么代替」。
＋ Notepad 的 3 个任务闭环。
阶段 0（文档与 Spike）已于 2026-09-20 closeout —— 它的产出是 Spike 报告，**不是**产品代码。
详见 `plans/stage-1-pilots.md`。

平台基线：**Windows 11 24H2+**（唯一正式基线；Windows 10 已 EOL，仅 C 级尽力）。
Linux 侧 **Wayland-first**（GNOME 50 已移除 X11 后端）。

---

## 构建与验证

前置：Rust stable（由 `rust-toolchain.toml` 固定）。Windows 需要 MSVC 生成工具 + Windows SDK。

```powershell
# 安装工具链（Windows）
winget install Rustlang.Rustup

# 三项基本门禁
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace

# 仓库卫生护栏（gov §5.4；当前实现 3/13 项，工具会自己声明 —— ADR-0025）
cargo run -p xtask -- hygiene
cargo run -p xtask -- --list-deferred   # 查看"还缺哪些检查、归属哪张卡"

# 文档一致性护栏（ADR-0030；派生计数与 ADR 编号不再靠人记）
cargo run -p xtask -- memory-counts     # MEMORY.md 规模表 ↔ docs/memory/ 实测
cargo run -p xtask -- adr-index         # ADR 登记表 ↔ docs/adr/*.md ↔ decisions.md

# 改写公共热点文件（LEDGER.md / docs/memory/* / docs/PARKING_LOT.md）前先取锁（ADR-0028）
cargo run -p xtask -- guard acquire MEMORY.md --owner <会话级唯一标识> --intent "<要干什么>"
cargo run -p xtask -- guard status
cargo run -p xtask -- guard release MEMORY.md --owner <同上>
```

`xtask` 是**零第三方依赖**的只读护栏工具。它对未实现的子命令**显式返回失败**
（退出码 3）并指出归属卡号 —— 本项目不允许任何形式的静默失败。

CI：`.github/workflows/ci.yml`（三平台矩阵；**硬门禁 10 项**已上线 —— fmt / clippy / test / verify-schemas(#6) /
codegen --check(#7) / deny / build / hygiene / spike-deny(#8b) / doc-consistency(#12b)；另有 **7 项软门禁**标
`continue-on-error` 并注明启用卡号；#6/#7 的注入式负向验证在 `gate-negative` job，单测侧负向用例在
`xtask/src/{verify_schemas,codegen}.rs`）。

---

## 最近进展（2026-09-26：TASK-026 —— 模型网关 `crates/model-gateway`；TASK-025 —— 目标租约与并发控制 `crates/lease`；TASK-024 —— 撤销与补偿闭环 `crates/undo`；2026-09-25：TASK-023 —— 后置断言引擎 `crates/verify`；TASK-204 —— `crates/tool-bus` draft-07 关键字判据硬化（三张拒绝表 + `$schema` 方言校验）；TASK-022 —— 任务引擎 `crates/task-engine`（12 状态机 + DAG + 检查点 / 恢复 / 预算 / 看门狗）；TASK-021 —— 唯一策略放行点 `crates/policy`；TASK-020 —— 工具通道 `crates/tool-bus`；TASK-019 —— Host IPC；TASK-018 —— 合成输入 + 坐标；2026-09-24：地基层 + 平台抽象 + Windows UIA + 治理池 = TASK-011 ~ 021 / 200 ~ 204）

**2026-09-26 —— TASK-026（`crates/model-gateway`：Provider 流 / 路由 / fallback / 成本）**

新增 `assistant-model-gateway`：Provider 契约统一流式事件、取消、usage、能力与定价；路由器按 stage / context tokens / sensitivity / remaining budget / tool count 的优先级规则选择 primary，并按配置降级链切换。重试使用指数退避和注入 jitter，错误分类明确区分“可同模型重试”与“可降级”；一旦产生 text 或 tool-call delta，任何后续失败都返回 `PartialOutput` 且不重试。prompt-cache 稳定前缀只透传、不改写；成本按 fresh/cached/output 三类整数 micro-USD 分量向上取整，记录 model/tokens/cost/latency/cache_hit。新增 20 个契约测试 + 1 doctest，零新增第三方依赖。

**2026-09-26 —— TASK-025（`crates/lease`：目标租约 + TTL / 续租 / 用户抢占 + 零提交批量获取）**

新增 `assistant-lease`：`Shared` / `Intent` / `Exclusive` 三模式，跨 owner 只允许一个写租约；TTL 到期边界明确，支持续租、懒清理与 `reap_expired`；用户接管按 target key 强制释放全部 agent 租约并把自然过期项单列。`acquire_many` 在任何 ID / 状态写入前完成规范 key 排序、去重、冲突与溢出预检，失败零提交，消除部分持锁与锁顺序死锁窗口。新增 20 个测试，零新增第三方依赖。

**2026-09-26 —— TASK-024（`crates/undo`：可逆性四级 + 锚点 + 回滚剧本 + 冲突检测 + incident）**

新增 `assistant-undo`：L0~L3 可逆性、内容快照 / 影子副本 / undo 预算 / 补偿剧本锚点、L0→L1 自动 fallback、默认最保守的冲突检测和结构化 incident。回滚动作经 `RollbackExecutor` 注入执行，最终指纹必须等于锚点 pre 指纹；缺证据绝不猜测，用户改动只有显式 `RestoreOverall` 才可越过。新增 23 个测试，零新增第三方依赖。

**2026-09-25 —— TASK-023（`crates/verify`：后置断言引擎 + 状态指纹 + 幂等判定）**

新增 `assistant-verify`：架构 v2 §7.4 的 **11 种断言**（= §7.4 的 12 种减 `visual_assert` —— 截图 / 感知哈希 / 容差归 TASK-042；附录 A 的 `target_resolvable` / `capability` 是 §5.3 的 **precondition**，解析期拒绝并说明归属）全部 fail-closed 解析：未知 `kind`、多余字段（拼写错误）、缺字段、类型错误、坏指纹、空 selector、`min > max`、`within_ms = 0` 一律拒绝，绝不静默跳过。求值是**三值**的 —— `Satisfied` / `Falsified` / `NotEvaluable`；只有全部满足才是 `Verified`，任一 `Falsified` 即 `Violated`，无 `Falsified` 但有 `NotEvaluable` 即 `Inconclusive`，**`Violated` 与 `Inconclusive` 都带 `ErrorCode::VerifyFailed`**，因此「验证失败却返回 ok」在类型上不可表达（§7.4 第一红线）。未观测（未探测元素 / 未记录文件 / 缺 previous 指纹 / 缺 elapsed）一律 `NotEvaluable`，绝不当作「不存在 / 未变化 / 满足」；字符串断言对上数值观测是「问错了量」→ `NotEvaluable` 而非 `Falsified`。§7.3 状态指纹用长度前缀 canonical form + SHA-256（分隔符注入无法伪造碰撞），忽略字段由 per-adapter 显式给出、**无全局默认**（否则光标闪烁会让每个动作都「有变化」）；§8.5 幂等判定缺 before/after 一律 `Unknown`、应用自报（L1）优先于指纹差异；`on_violation` 的 `retry_once` 收窄到 `Transient` / `TargetNotFound`，其余升级给用户（`VerifyFailed` **不**在此重试：`ErrorCode::retryable()` 回答的是「任务能否重试」，不是「同一调用能否重发」）。新增 66 个测试 + 1 个 doctest；`serde` / `sha2` / `thiserror` 只新增使用方并同批登记。`assert` 自由字符串不实现（= 表达式语言 = 新抽象层），记 `DRIFT-023-4` / **PL-086**。

**2026-09-25 —— TASK-204（`crates/tool-bus`：draft-07 关键字判据硬化）**

把「除白名单外一律拒绝」这条**隐式**判据**显式化**：新增三张带理由的表 —— `REJECTED_FOREVER_KEYWORDS`（12 条**永久放弃**：`$ref`/`definitions`、`pattern`/`patternProperties`/`format`、`if`-`then`-`else`/`dependencies`、元组 `items`/`additionalItems`、`contentEncoding`/`contentMediaType`）、`REJECTED_FOR_NOW_KEYWORDS`（4 条**暂未实现**：`multipleOf`/`uniqueItems`/`contains`/`propertyNames`）、`NON_DRAFT07_KEYWORDS`（15 条**其它方言**：`$defs`/`$dynamicRef`/`prefixItems`/`unevaluatedProperties` …），加未知关键字兜底。报错分四类标签（`never-supported` / `not-yet-supported` / `other-dialect` / `unknown`）且每条带 JSON pointer。

`$schema` 由「注解」改为**方言校验**：缺省或 draft-07 = 放行，声明**别的方言 = 拒绝**（消息含实际声明的方言）—— 否则会拿 draft-07 的语义去校验一份 2020-12 文档，那正是铁律 1 禁止的「用自己的语义冒充别人的语义」。`format` / `pattern` 按人类 2026-09-25 裁决归入**永久放弃**并写明替代（`enum` / `const` 收窄取值，或**由 handler 校验值**）。`SUPPORTED_KEYWORDS` 的 21 条**一字未改**，零新增依赖、零 `unsafe`、零 `#[allow]`。新增 9 个测试（表两两不相交 / 表内不重复 / 四类标签与理由 / 嵌套 pointer / 方言 4 接受 3 拒绝 / 未知关键字 typo 提示 / 深度上限仍 64）；`cargo test --workspace` = **371 passed**，`hygiene` 0 error。同批把 automation 调度器的 `COUNT=1` **实测反转**为「**真一次性**」（`docs/nightly/codex-automations-operations.md` §4.1.a 更正块：**必须同时带 `BYHOUR` + `BYMINUTE`**，且**不要**用 `COUNT≠1` / `UNTIL`）。

**2026-09-25 —— TASK-022（`crates/task-engine`：任务状态机与恢复）**

新增 `assistant-task-engine`：架构 v2 §8.1 的 12 个 Task 状态与 Step 生命周期均有显式迁移表，非法 `(state, event)` 直接拒绝。Plan DAG 校验重复 id / 缺失依赖 / 环 / 非法工具名 / 写步骤缺 postcondition / L3 未标 point-of-no-return，并返回确定性 ready frontier。每次成功迁移先持久化完整快照再返回；`MemoryCheckpointStore` 供纯逻辑测试，`SqliteCheckpointStore` 只走 `assistant-storage` 公开记录 API，并在同一事务写入初始 task 与 checkpoint。崩溃恢复对 `Executing` / `Verifying` 只接受 `Completed` / `NotCompleted` / `Unknown` 外部证据：完成则提交、未完成则重做、`Unknown` 或缺证据则 `NeedsHuman`。预算覆盖步数 / 时长 / token / 成本，看门狗覆盖 resolve / execute / verify 三段超时。新增 43 个测试 + 1 个 doctest，`cargo llvm-cov -p assistant-task-engine --fail-under-lines 85` 实测 **87.03%**。storage 暂无 task 行更新 API，最新状态以 checkpoint 为准，遗留记为 **PL-083**。

**2026-09-25 —— TASK-021（`crates/policy`：唯一策略放行点）**

新增 `assistant-policy`：规则集用 JSON DSL v0 表达架构 v2 §12.2 的五条示例规则，求值全程无 IO、时钟、随机与全局可变状态。任一命中的 deny 压过 allow / confirmation；`L3 + unattended` 与污点上下文中的 L3 / High / Critical 由安全底线先行拒绝，自定义规则不可覆盖；空规则集或未命中一律 `default_deny`。参数护栏覆盖路径穿越 / 编码逃逸 / UNC / 设备名 / resolved-root containment、URL scheme / host / IP / port、文本长度 / 行数 / 控制字符、数值范围和正则 lookaround / 回引 / 嵌套量词；不支持即拒绝。新增 29 个测试 + 1 个 doctest，`cargo llvm-cov -p assistant-policy --fail-under-lines 85` 实测行覆盖 **93.60%**，判定中位数 **3.6~5.6 µs**。`PolicyDecision` 的 confirmation 投影缺口已记录为 `DRIFT-021-1` / PL-082。

**2026-09-25 —— TASK-020（`crates/tool-bus`：MCP 工具通道）**

新增 `assistant-tool-bus`：内置工具以**同进程 MCP server** 暴露，Agent Core 侧是 MCP client，传输用 `tokio::io::duplex` 的内存管道（无 socket / 无子进程 / 无网络），因此「内部工具也以 MCP 表达」不需要 stdio 或 http。`tools/list` 与 `tools/call` 全链路有测试证据（`test_mcp_round_trip_lists_and_calls_tool`）；调用身份经 MCP `_meta` 跨边界传递，服务端从 `RequestContext.meta` 读回 `task_id` / `step_id` 写进信封。
参数按 `ToolSchema.input` 走手写的 draft-07 **子集**校验（Q2 选 (c)；(a) `jsonschema` 会把传递依赖 `borrow-or-share`(MIT-0) 带进依赖图，而 `deny.toml` 白名单不含它 —— 放宽白名单是漂移触发器 ⑥），**不在支持清单里的关键字一律拒绝注册**（fail-closed）；非法参数返回 `ErrorCode::ToolInvalidArgs` 且**永不进入 handler**（负向用例断言计数器仍为 0）。
返回一律是 `assistant_protocol::ToolEnvelope`：`untrusted` 内容强制带 `source`，超 `max_bytes` 截断并显式写 `reason` / `original_bytes`，截不动（如键名开销就超预算）则 fail-closed 成 `Fatal` 失败信封。工具集指纹对「名字 + 版本 + 描述 + 风险级 + 规范化 schema」取 SHA-256（顺序无关），两个元工具 `toolset.list` / `toolset.search` 提供按需检索，单次挂载 > 40 个工具产出结构化 `ToolsetOversizeWarning` + 未串链 `AuditEvent`。
本卡引入 `rmcp` 3.4 / `tokio` 1 / `thiserror` 2 并逐条登记 `docs/DEPENDENCIES.md`（Q1 / Q4 / Q5）；`cargo deny check` 四项全 ok。新增 `crates/tool-bus/README.md`（职责 / 边界 / 不变量 / **已知限制**）与 8 个集成测试（新基线：`cargo test --workspace` = 43 target / 644 passed）。

**2026-09-25 —— TASK-019（`crates/ipc` + `apps/automation-host`：Host IPC 传输层）**

新增 `assistant-ipc` 与 `assistant-automation-host`：线上帧按 spec 固定为 `magic + envelope_size + payload + CRC32`，
超长 / magic / CRC / 版本 / token 五类错误均带 `ErrorCategory`；握手用 `ClientHello` / `ServerHello` / `Heartbeat`，
token 只作为传输层认证材料伴随发送，不改协议定义。Windows 端以 NamedPipe 实现传输，拒绝远端客户端，并以
`GetNamedPipeClientProcessId` + `QueryFullProcessImageNameW` 做默认拒绝的对端镜像白名单；双向心跳与看门狗在静默时明确报错。
真实验收由测试启动 host 子进程，完成握手、心跳，再 kill host 并确认客户端在 2 s 内检测断连；非 Windows 两个目标的 clippy 同样全绿。
新增 `handle_discipline` 契约测试，禁止平台实现依赖与元素句柄类型进入 IPC 载荷层。

**2026-09-25 —— TASK-018（`crates/platform/windows`：合成输入 + 坐标归一化 + IME）**

TASK-018 把铁律 5 的 **L4（合成输入）** 层补齐：`SendInput` 封装（VK 路径 + `KEYEVENTF_UNICODE` 文本路径 —— 后者把
UTF-16 码元直接投进输入队列，**绕过 IME 组字与键盘布局**，所以「IME 开着也能正确写入」）、**发送前 100% 校验前台窗口**
（不一致 → `SetForegroundWindow` + **回读**，仍不一致 → `TargetUnresponsive`，**绝不盲发**）、`SendInput` 返回值逐次校验
（UIPI `ERROR_ACCESS_DENIED` → `PlatformPermission` / 87 → `ToolInvalidArgs` / 队列被阻塞 → `TargetUnresponsive` / 未识别码 → `Fatal`）、
显示器枚举 + `GetDpiForMonitor` / `GetDpiForWindow` → **实际**显示器的 `CoordinateSpace`、`NormalizedPoint` → `PhysicalPoint`
（**只经** `CoordinateSpace::to_physical`）、虚拟屏幕 `0..=65535` 归一化、`is_ime_open` 两级查询。
真机验收 **2/2**：坐标精度 **0 px**（判据 ≤ 2 px）、真实记事本写入「中文abc」+ `Ctrl+S` **磁盘回读**通过。
纯逻辑（键名映射 / 组合键顺序 / UTF-16 代理对 / DPI → 缩放 / 显示器半开区间命中 / 归一化 / 失败分类）刻意留在
**三平台编译**的 `mod.rs` → **ADR-0045** 的两条非宿主 `--target` clippy 也覆盖得到（这是 PL-070 教训的第一次兑现）。
新提 **PL-074**（`pointer_action` 签名不带目标窗口 → 混合 DPI 多屏下逻辑点无法唯一归属显示器；跨显示器拖拽终点偏差）。

**2026-09-24 —— 阶段 1 地基层 + 平台抽象层 + Windows UIA provider + 治理池收口 + 护栏补齐 + TASK-016 / TASK-017 遗留裁决**

阶段 1 的地基层已经落地（含密钥层），治理池把 TASK-013 现场撞出的三个**结构性**缺陷一次性收口，
`xtask` 护栏同批补齐并把两条 CI 门禁由软转硬。
同批把 TASK-016 留下的 4 项治理遗留收口：**ADR-0042**（能力命名三分：`CapabilityCatalog` / `CapabilityMatrix` / `CapabilityEntry`）+
`MEMORY.md` §1 快照指针化（手抄派生进度改为指向 `PLAN.md`）+ `plans/*` 头部进度句纳入 `AGENTS.md` §11.1 的更新职责 +
4 份 schema 的悬空 `TASK-103` 前缀清除（PL-064 / 065 / 066 / 067 全部闭环）。

| 卡 | 内容 | 状态 |
|---|---|---|
| TASK-011 | `crates/protocol`：JSON Schema → Rust/TS 类型 codegen；`codegen --check` 变成**真门禁**（drift 会 exit 1） | ✅ Done |
| TASK-012 | `crates/storage`：SQLite WAL + 迁移框架 + blob 池（zstd + sha256 内容寻址） | ✅ Done |
| TASK-013 | `crates/audit`：append-only + SHA-256 hash chain + ring buffer 批量 flush（摊销 < 1 ms/条） | ✅ Done |
| TASK-200 | `docs/spec/*` 7 份契约草案的系统性结构缺陷（外来模板块 / 空节 / 断链引用）—— PL-038 闭环 | ✅ Done |
| TASK-201 | `crates/core` 骨架提前（PL-037 闭环） | ✅ Done |
| TASK-202 | 存储**迁移注册表**：storage 只提供机制、各 crate 自持迁移 + 唯一装配点（**ADR-0038**，PL-046 闭环） | ✅ Done |
| TASK-203 | `audit_logs` 列语义去重 + 显式链序：删与 `id` 同义的 `hash`、加 `sequence`（**ADR-0040**，PL-043 / PL-045 闭环） | ✅ Done |
| TASK-014 | `crates/secrets`：OS keychain 封装（`keyring` 4.2 / DPAPI·Keychain·Secret Service）+ `zeroize` + 访问审计注入点（fail-closed） | ✅ Done |
| TASK-016 | `crates/platform/api`：**铁律 7 的唯一平台入口** —— 4 个纯类型（`TargetDescriptor` / `NormalizedPoint` / `Fingerprint` / `CapabilityMatrix`）+ 3 个 trait 形状（RPITIT，**零 `async-trait` 依赖**）+ 能力矩阵 4 条不变量的机器校验；`ResolvedWindow` / `ResolvedElement` = 不透明句柄（铁律 8 有源码扫描断言，含负向样本） | ✅ Done |
| TASK-015 | `xtask` 护栏补齐：`docscan` 4 条结构规则（裸 NUL / 数字节号重复 / 整节为空 / 标题文字重复）+ `crates/core/tests/arch*` 分层断言（`arch::` 由 0 → 5 个测试）+ `check-ledger`（ADR-0039 D3）+ `check-migrations`（PL-047）；同批修 PL-048（flaky）与 PL-051（裸 NUL） | ✅ Done |
| TASK-017 | `crates/platform/windows`：**铁律 7 在 Windows 侧的落地** —— `UiAutomationProvider` 全 9 方法 + `WindowProvider` 全 5 方法（UIA 树快照 / selector 链解析 / `read_text` / `set_value` / `edit_text` / `invoke_action` / `bounds` / `fingerprint` / 窗口枚举与状态）；句柄纪律（铁律 8）有源码扫描断言 + 5 个负向样本；全 crate 零 `serde`。7 条 DRIFT 已全部裁决接受（含 crate 级 `unsafe_code` 放行、`crates/platform/api` 补 `SelectorChain` re-export），PL-068 / 069 / 070 闭环（**ADR-0043 / 0044 / 0045**）| ✅ Done |
| TASK-018 | `crates/platform/windows`：**铁律 5 的 L4（合成输入）落地** —— `SendInput` 封装（VK 路径 + `KEYEVENTF_UNICODE` 文本路径，后者绕过 IME 与键盘布局）、发送前 **100% 前台窗口校验**（不一致 → 置前 + 回读，仍不一致 → `TargetUnresponsive`，绝不盲发）、`SendInput` 返回值逐次校验（UIPI / 87 / 队列被阻塞 / 未识别码四分类）、显示器枚举 + `GetDpiForMonitor` / `GetDpiForWindow` → **实际**显示器的 `CoordinateSpace`、`NormalizedPoint` → `PhysicalPoint`（只经 `CoordinateSpace::to_physical`）、虚拟屏幕归一化、`is_ime_open` 两级查询；纯逻辑留在三平台编译的 `mod.rs`（ADR-0045 的两条非宿主 `--target` clippy 覆盖得到）；真机验收 2/2（坐标误差 0 px / 记事本 Unicode + Ctrl+S 磁盘回读）；新提 PL-074 | ✅ Done |

TASK-016 把**平台抽象层**立起来：`core` / `Host` 从此只能经 `assistant-platform-api` 的 trait 用平台能力
（铁律 7），且**该 crate 全 crate 零 `#[allow]`、零 `unsafe`、零第三方依赖**（除已登记的 `serde`）——
`f64 → i32` 这类"`as` 会饱和 + pedantic 会拦"的转换改用整数域逐位合成，越界一律报 `TargetNotFound`。

治理机制同步前进：**ADR-0039** 把「卡 Done = 同一 PR 内同步 `PLAN.md` + `README.md`」写成硬契约
（DRIFT-202-2 闭环），其机器判据 `check-ledger` 已由 **TASK-015** 落地并**由软门禁转硬**（CI 的 `[HARD #16]`）；
同批把 gov §5.1 **#5 arch test** 也转硬 —— 它此前是 `running 0 tests` 的假绿，现在是真的 5 个断言。

TASK-017 把**平台抽象层**在 Windows 侧兑现：`assistant-platform-windows` 提供 UIA 树快照、selector 链解析、
读写与动作、窗口枚举与指纹，错误一律映射成带 `ErrorCode` 的 `PlatformError`（铁律 1）；
**L4 合成输入（`SendInput`）与 IME 仍留空并显式返回 `CapabilityMissing`**，归 TASK-018（铁律 5 的层次不能跳）。
真机实测暴露一个**架构层缺口**（记 **PL-068**）：受 TASK-016 冻结的 trait 形状所限，`resolve_element` 只能
从**桌面根**发 `FindAll(TreeScope_Descendants)` → 中位数 **1.53 s**（窗口子树内同一操作只要 30~40 ms）。
本卡**没有**偷偷改搜索策略去掩盖它，而是把证据写进 `docs/PARKING_LOT.md` 等裁决。

TASK-017 的遗留裁决同批收口：**ADR-0043**（元素解析必须有 scope —— `resolve_element` / `wait_for` 的首参改为
`&ResolvedWindow`，搜索起点 = 该窗口的 UIA 根；桌面根搜索在元素解析里彻底消失，PL-068 / DRIFT-017-7 闭环）、
**ADR-0044**（歧义策略收敛为唯一 `ErrorAndAsk`，删掉在候选链模型里不可实现的 `HighestScore`；架构 v2 §6.6 其余
3 种策略写明归属层，PL-069 闭环）、**ADR-0045**（把「非宿主平台」`--target` clippy 写进 `AGENTS.md` §6 验收清单 ——
`clippy` 不链接故不需要 C 交叉编译器，但 `--workspace` 会撞 `cc-rs`，须按 crate 指定；PL-070 闭环）。
同批新提 PL-071 / PL-072 / PL-073（ADR-0035 allow 表缺口 / `*Regex` 命名与子串语义不一致 / 卡片状态行归属机制）。

### 历史小节（按时间倒序）

- **2026-09-19 TASK-059/060/061/062/063/064（xtask 护栏升级六连发）**：
  `xtask` 从 4 个子命令扩到 7 个（新增 `refscan` / `docscan` / `card-check`），
  原 `hygiene / memory-counts / adr-index / guard` 保持；全部在 CI 硬门禁 #12b（`doc-consistency`）下统一跑过。
  `docscan` 扫破表（PL-031）+ setext 风险 + UTF-8 BOM/CRLF/缺末行 LF；`card-check` 是 ADR-0031 D6 的机器化；
  同一批清掉了 `xtask/src` 生产代码里的全部 per-line `#[allow]`（**ADR-0034** / **ADR-0035**）。
### 历史小节（按时间倒序）

- **2026-09-19 TASK-064**（promote `extension_is` helper + 收 repowalk.rs:172 per-line allow）:
    - 新增 `pub fn extension_is(path: &Path, expected: &str) -> bool` 到 `xtask/src/repowalk.rs`（紧邻 `has_rust_extension`）；同步把 refscan.rs 的 local `ext_is` closure 删掉、import 此 helper
    - `collect_repo_files_recursively` 内 2 处 `lower.ends_with(...)` 改 `extension_is(&path, ...)`；删函数级 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`
    - 加 4 条 `extension_is` 单元测试：positive match / case-insensitive / 非 UTF-8 返回 false / 无扩展名返回 false（cargo test 279 → 283 passed）
    - **副作用**（行为变化）：`.ends_with(&format!(".{ext}"))` 在 caller 传大写 ext（如 `"MD"`）时会漏匹配，`extension_is` 修复了此 bug —— **现状** caller 全部传小写（card_check `["md"]` / docscan `["md"]` / refscan `["md","rs","ps1"]`）故无回归；**测试覆盖** test_extension_is_is_case_insensitive 显式断言 4 种大小写组合
    - ADR-0035 §1 baseline 表补登 `repowalk.rs:172` 已清 + §决策 2 加一条「抽到 `pub fn` 替代 per-line allow 必须同步登记」（教训：TASK-063 提升到 helper 后才发现 repowalk.rs:172 还有同型 allow）
    - **真正的最终态**：全 xtask/src 生产代码 per-line allow = 0（refscan:0 + repowalk:0 + docscan:0 + card_check:0 + exemptions:0；保留 = test wrapper 4 处合法 + card_check 9 + exemptions 1 共 10 处 `#[allow(dead_code)]` 占位常量）
    - 详见 `tasks/TASK-064-promote-extension-is-helper.md` 执行记录 9 节 + LEDGER.md 2026-09-19 行
- **2026-09-19 TASK-063**（xtask refscan 清最后 3 处 per-line allow — 达到「生产代码 per-line allow = 0」）:
  - refscan.rs:94 `#[allow(clippy::single_char_pattern)]` → `replace("\r", "\n")` 改 `replace('\r', "\n")`（char 字面量，触发 Pattern impl 即可）
  - refscan.rs:101/103 `#[allow(clippy::case_sensitive_file_extension_comparisons)]` → `lower.ends_with(".md"/".rs"/".ps1")` 改 `Path::extension().and_then(to_str).is_some_and(eq_ignore_ascii_case)`（closure `ext_is` 复用）
  - 删除 `let lower = rel_path.to_ascii_lowercase();` 与配套注释（5 处出现 → 0）
  - 行为不变：refscan 仍报 151 errors（baseline 一致）；UTF-8 非合法扩展名按 `to_str` 失败语义 = 与原本 `ends_with`(`&str`) 在非 UTF-8 路径上失败同形
  - **复核 GAP**：repowalk.rs:336 `#[allow(clippy::case_sensitive_file_extension_comparisons)]` **未在本卡 scope 内**（卡面标题与 In scope 严格限制 refscan.rs），且 ADR-0035 baseline 表也漏列；建议下一卡 `TASK-064` 或新卡处理（统一改 `Path::extension` 模式）
  - 详见 `tasks/TASK-063-clear-last-3-per-line-allows.md` 执行记录 9 节 + LEDGER.md 2026-09-19 行
- **2026-09-19 TASK-062**（xtask render() 返回 Result — 消除 per-line allow）:
  - `pub fn render(findings: &[Finding]) -> String` → `pub fn render(findings: &[Finding]) -> Result<String, std::fmt::Error>`
  - 删除 `#[allow(clippy::unwrap_used, clippy::expect_used)]` per-line allow（TASK-060 遗留）
  - `writeln!(..).expect("...")` → `writeln!(..)?` + `.map_err(|e| e.to_string())?` 在 `run()` 中
  - 行为不变（writeln! to String 永不失败；Result 类型强制 caller 处理 = 编译期保证）
- **2026-09-19 TASK-061**（xtask lint cleanup pass 2 — TASK-060 遗留 5 处 str[Range] + 1 处 stale dead_code + 3 处 test f[0]）:
  - card_check.rs 5 处 str[Range] → 全替换为 `.get(range)` + `?` operator 重构
  - refscan.rs:290 stale `#[allow(dead_code)]` 删除（render 已被 run() 调用）
  - docscan/card_check 测试代码 `f[0]` → `f.first().expect("non-empty")`（3 处）
- **2026-09-19 TASK-059 / TASK-060**（xtask 护栏升级 + 纯重构）：
  - TASK-059 cherry-pick 悬空 commit `10f78db` 收回 → 4 子命令可执行 + ADR-0034 注册
  - TASK-060 人类裁决 B 路（漂移不可忍受）= 纯重构 = 4 模块顶部 `#![allow(...)` 块全清
    （**DoD 硬证据**：`grep '^#![allow' xtask/src/{refscan,docscan,card_check,exemptions}.rs` = 0 命中）
    + 32 处 `indexing_slicing` 用 `while let Some + .get(n).expect()` 全替换
    + `docs/PARKING_LOT.md` PL-NEW 关闭
  - 防御性 PITFALL：`docs/memory/pitfalls.md` 追加「禁止 sub-card 后缀；commit 标题必须对应 `tasks/TASK-NNN-*.md`」
  - 详见 `LEDGER.md` 2026-09-19 三行 + `tasks/TASK-059-...md` / `tasks/TASK-060-...md` 执行记录
- **2026-09-17 TASK-001**：仓库骨架 + CI 8 硬门禁 + ADR 编号登记表建立

## 许可证

`MIT`（见 `LICENSE`）。

> 说明：内部项目，暂不公开。计划中的双许可 `MIT OR Apache-2.0` 尚未最终确认
> （`docs/memory/open.md` 的 **M5** 待裁决项）；当前先以 MIT 落地，**该决定可逆**。
> 开源前需要完成脱敏，清单待建（见 `docs/PARKING_LOT.md` PL-005）。
