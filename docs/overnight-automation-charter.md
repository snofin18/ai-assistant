# 夜间自动推进章程（Overnight Automation Charter）

> 状态：Active　版本：**1.3**　日期：**2026-09-18**　变更历史见 §12
> 上位：`AGENTS.md`、`docs/governance-ai-agent-execution.md`（gov）、`docs/subagent-orchestration.md`
> 本文件是夜间自动化任务的**唯一权威规则源**。调度器传给 `codex exec` 的 prompt 只做引用，不复制规则。
> 变更门槛：修改本章程需 ADR。

---

## 0. 为什么需要章程，而不是"让它跑就行"

项目的头号风险是**漂移**（gov §0.1），而夜间无人值守恰好移除了唯一的实时纠偏机制：人类。
因此夜间自动化必须比白天**更严格**，而不是更宽松。核心原则：

> **夜间只做「可机器验证 + 无需人类判断」的工作；一切需要判断的事，停下来留给白天。**

三条不可妥协的底线：

1. **不合并 main**：夜间产出全部落在 `nightly/YYYY-MM-DD` 分支，合并权在人类。
2. **不静默失败**：每轮必须留下报告；验收不通过就停止，不允许"先做完再说"。
3. **不改契约**：schema / trait / ErrorCode / DB / IPC / lint 配置 / AGENTS.md / PLAN.md / spec / 已批准的 ADR —— 夜间一律只读。

---

## 1. 前置门禁（每夜第一轮开始前必须全绿，否则整夜停止并报告）

| # | 门禁 | 检查方式 | 不通过时 |
|---|---|---|---|
| G1 | Rust 工具链可用 | `cargo --version` | 停止整夜，报告"环境未就绪" |
| G2 | **当前 HEAD 是绿的** | `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` | 停止整夜。**必须从绿开始**，否则无法区分"我弄坏的"与"本来就坏的" |
| G3 | 工作区干净 | `git status --porcelain` 为空 | 停止整夜（避免把人类在制品混进夜间提交） |
| G4 | 不在 main 分支 | 当前分支 ≠ `main`；若为 main 则创建 `nightly/<date>` | 自动切到夜间分支 |
| G5 | **有可执行的工作** | 按序检查：① `tasks/*.md` 中有 `状态：Ready` 的卡 → ② §13 夜间工作单有未完成项 → ③ §2 白名单 P2~P8 中有可做项 | 三者全空才停止整夜，报告"无可执行工作"并列出三者各自为空的原因 |
| G6 | 护栏工具可用 | `cargo run -p xtask -- hygiene` 能运行 | 停止整夜 |
| G7 | 磁盘与时间预算充足 | 剩余磁盘 > 2 GB；当前时间 < 05:00 | 停止整夜 |
| G8 | **无并发夜间任务** | 获取互斥锁 `.nightly.lock`（规则见 §11.2） | 锁被占用 → 本次唤醒**直接结束**，只往 `docs/nightly/<date>-skipped.md` 追加一行，不动任何代码 |

---

## 2. 允许的工作（白名单，按优先级）

| 优先级 | 工作类型 | write scope | 说明 |
|---|---|---|---|
| P1 | 执行状态为 Ready 的任务卡（**一轮一张，串行**） | 卡内声明的 write scope | 主线推进 |
| P2 | 为已有代码补白盒测试，把覆盖率提到门槛以上 | `crates/**/tests/**`、`#[cfg(test)]` 模块 | 无 Ready 卡时的默认工作 |
| P3 | 修复既有的 `xtask hygiene` / `check-comments` / clippy 告警 | 告警所在文件 | 只修告警，不顺手改别的 |
| P4 | 补文档：crate README 的**不变量**一节、`docs/spec/*` 的 **Draft** 草案 | `crates/*/README.md`、`docs/spec/*`（仅新建或标注 Draft） | 草案必须标 `状态：Draft（待人类批准）` |
| P5 | ADR **草稿**（编号顺延，状态 Draft） | `docs/adr/NNNN-*.draft.md` | **不得**写 `状态：Accepted` |
| P6 | 汇总代码中的 `PITFALL(...)` 标签进 `MEMORY.md` §5；补录 `LEDGER.md` | `MEMORY.md`、`LEDGER.md`（只追加） | 记忆维护 |
| P7 | 实现靶机应用 fixture | `fixtures/apps/**` | 为白盒/回放测试铺路 |
| P8 | 分析 `docs/PARKING_LOT.md` 条目，产出**建议**（不实现） | `docs/nightly/*.md` | 只给分析 |

---

## 3. 禁止的工作（黑名单，命中即停该卡并记 DRIFT）

1. ❌ 任何**契约变更**：`protocol/**` 的 schema、任何 `pub trait`、`ErrorCode`、DB schema、IPC 方法签名
2. ❌ 新增第三方依赖、新增 crate、新增顶层目录（`Cargo.toml` 的 `members`/`workspace.dependencies`）
3. ❌ 修改 `AGENTS.md`、`PLAN.md`、`plans/*`、`docs/spec/*`（已存在的）、`docs/adr/*`（状态为 Accepted 的）、`MEMORY.md` §3/§4（决策与否决项）
4. ❌ **放宽任何护栏**：加 `#[allow(...)]`、改 `deny.toml` / `clippy.toml` / `rustfmt.toml` / `Cargo.toml` 的 `[workspace.lints]`、改 `.github/workflows/**`
5. ❌ 修改测试断言以让测试通过（gov §4.3 触发器 #7）
6. ❌ 删除或重命名既有公共 API；单卡 diff > 400 行
7. ❌ 操作**任何真实应用**（包括 Notepad/Paint/Edge）：夜间不得启动、点击、输入、截图任何 GUI 应用
8. ❌ 任何真实网络请求（除拉取 crates.io 依赖与 git 操作）、任何凭据读取
9. ❌ `git push`、创建/修改远端分支、修改分支保护、合并到 main
10. ❌ 执行需要**真机 GUI 交互**的 Spike 步骤（Spike A/A2/G 的实测部分）→ 跳过并在报告中说明
11. ❌ 自行裁决 DRIFT、自行决定范围调整、自行"优化"设计
12. ❌ 任何 L3 不可逆动作、任何资金/发送/对外发布类操作（v2 §9.7）
13. ❌ 跨卡工作：一轮只做一张卡；发现别的卡的问题 → 记 PARKING_LOT，不动手

> 第 7 条的理由：夜间自动操作真实 GUI 应用既无法验证结果（无人看着屏幕），又可能干扰用户次日的桌面状态，属于"看起来能做但绝不该做"的类别。

---

## 4. 每轮协议（一轮 = 一张任务卡）

```text
① 读 AGENTS.md 全文 → PLAN.md → plans/<当前阶段>.md 的相关批次 → 目标卡全文
   → MEMORY.md §2/§4/§5 → LEDGER.md 末 10 行 → 目标 crate README 的不变量
② 输出【约束回执】到本轮报告（AGENTS.md §3 格式）
③ 黑名单自检：本卡是否触及 §3 任一项？
     命中 → 跳过该卡，记 DRIFT-<卡号>-NIGHT，取下一张卡（当夜累计 2 次命中 → 停止整夜）
④ 实现（严格在卡内 write scope）
⑤ 跑卡内全部验收命令 + 全局门禁（fmt / clippy -D warnings / test）→ 必须全绿
⑥ 跑 xtask hygiene 与 check-comments → 必须无**新增** Error（与本轮开始前的基线对比）
⑦ 白盒测试要求（§5）→ 覆盖率不低于门槛且新代码有测试
⑧ 提交到 nightly 分支：Conventional Commits + 页脚 `Task:` / `Drift:` / `Night-round: N`
⑨ 追加 LEDGER.md 一行；若有新 FACT/PITFALL → 追加 MEMORY.md
⑩ 写 docs/nightly/<date>-round-N.md（本轮报告）
⑪ 预算判断（§6）→ 继续下一轮或收尾
```

---

## 5. 白盒测试的夜间硬性要求

夜间产出的代码**必须**满足（否则该轮视为失败并回滚提交）：

1. 新增/修改的**纯函数**必须有表驱动单元测试，覆盖：正常路径、边界、错误路径（≥3 个用例）
2. 新增/修改的公共 API 必须有文档注释，且**包含错误语义**（何时返回哪个 `ErrorCode`）
3. 覆盖率不得低于门槛：workspace ≥ 75%；`core`/`policy`/`task-engine`/`verify`/`undo` ≥ 85%
4. **不允许**通过降低断言强度来"让测试通过"（例如把精确断言改成 `is_ok()`）
5. 涉及状态机/策略/验证逻辑的改动，必须补**负向测试**（证明它会拒绝该拒绝的东西）
6. 测试命名遵循 `test_<unit>_<condition>_<expected>`（`docs/spec/naming.md` N 系列）
7. 新增测试不得依赖真实 IO/网络/GUI（用注入的 trait 替身，见 `docs/spec/testing.md`）

---

## 6. 预算与停止条件

| 项 | 上限 |
|---|---|
| 每夜轮数 | **≤3 轮**（每轮一张卡；每次调度运行内 ≤2 轮，见 §11.3） |
| 结束时间 | **05:00** 前必须收尾（写晨间报告） |
| 单卡 diff | ≤ 400 行；超出 → 停止该卡并建议拆卡 |
| 每夜总 diff | ≤ 1200 行（**人类半天可审阅完**，这是硬约束） |
| 连续验收失败 | 2 次 → 停止整夜 |
| 黑名单命中 | 当夜累计 2 次 → 停止整夜 |
| 环境/工具链异常 | 立即停止整夜 |
| 发现 spec 矛盾 | 记 DRIFT → **停止整夜**（后续卡可能都依赖同一处矛盾） |

---

## 7. 晨间报告（人类醒来看到的第一份东西）

路径：`docs/nightly/<YYYY-MM-DD>-report.md`；同时把摘要写进本次运行的日志文件（`docs/nightly/logs/`）
与 `-o/--output-last-message` 的输出文件 —— **CLI 路径没有应用内通知**，可见性全靠文件（§11.6）。

```markdown
# 夜间报告 <date>
## ① 环境门禁
G1~G8 结果（任一失败则本报告只有这一节 + 原因）
## ② 昨夜完成
| 轮次 | 卡号 | commit | 验收 | diff 行数 | 覆盖率变化 |
## ③ 未完成与原因
## ④ 需要人类裁决（按优先级）
| DRIFT/问题 | 影响 | 我的建议 | 阻塞哪些卡 |
## ⑤ MEMORY 增量（新增 FACT / PITFALL / REJECTED 条目）
## ⑥ 今日建议审阅顺序（风险从高到低）+ 预估审阅行数
## ⑦ 护栏趋势（hygiene / check-comments / clippy / coverage 的数字对比）
## ⑧ 声明
- 所有改动在 `nightly/<date>` 分支，**未合并 main、未 push**
- 未操作任何真实应用、未发起任何非依赖拉取的网络请求、未触碰任何凭据
```

**通知策略**：automation 设为**仅失败时通知**（`failed_runs_only`）——夜间各轮不打扰人类，运行异常立刻通知。
晨间"通知"由报告文件本身承担（heartbeat 无法只对某一次唤醒单独开通知），详见 §11.6。

---

## 8. 人类的早晨流程（建议 15 分钟）

```text
① 读晨间报告 §④（需裁决项）→ 逐条回复"同意/否决/改法"
② 读 §⑥ 的审阅顺序 → 按序 review（或先派 review agent，再抽查）
③ 逐卡决定：合并 main / 退回修改 / 废弃
④ 合并后：更新 PLAN.md 任务队列状态（人类或 Orchestrator 做）
⑤ 若夜间产出质量下降（连续退回）→ 收紧白名单或暂停夜间自动化
```

---

## 9. 什么工作**不该**放到夜间（诚实清单）

| 工作 | 为什么 |
|---|---|
| 需要真机 GUI 验证的 Spike（A/A2/G 实测） | 无人确认屏幕状态；且会干扰用户桌面 |
| 契约与架构设计 | 需要人类判断与裁决，错了代价极大 |
| Adapter 的 App Map 编写 | 需要对着真实应用逐个控件确认 |
| 性能调优 | 需要真实负载与实测数据，夜间无法采集 |
| 涉及许可/登录的应用（Excel/Photoshop） | 需要人工处理弹窗与登录态 |
| 安全机制的设计变更 | 反注入/DLP/策略属于"必须人类想清楚"的部分 |
| 任何"顺手优化" | 漂移的主要来源 |

---

## 10. 与既有治理的对应关系（自检表）

| 治理规则 | 夜间如何满足 |
|---|---|
| 作者不自证（gov §7.1） | 夜间 agent 只提交到独立分支；验收由**机器命令**判定；合并由人类裁决 |
| 契约先行（铁律 10） | 契约类文件夜间只读（§3.1/§3.3） |
| 无静默失败（铁律 1） | 每轮必留报告；失败即停止；未实现的检查显式报错 |
| 小卡 + write scope（gov §3） | 一轮一卡，严格按卡内 scope |
| 重开会话是正常操作（gov §4.4） | **机制上已保证**每次调度运行都是全新会话（§11.1：`codex exec` 不带 `resume`，每次新建 session id）。重锚要求**仍然保留** —— 它防的是「同一轮内的上下文漂移」，与「跨轮复用会话」是两件不同的事 |
| 人类审阅是瓶颈（subagent §9.3） | 每夜总 diff ≤ 1200 行，控制在半天可审完 |
| 不放宽护栏（gov §5.2） | lint/CI/deny 配置夜间只读（§3.4） |
| 记忆可回查（MEMORY.md） | 每轮追加 LEDGER + MEMORY 增量（§4 ⑨） |

---

## 11. 投递机制：Windows 任务计划程序 + `codex exec`（**v1.3 全章重写** — ADR-0018）

> v1.1/v1.2 的本章围绕 Codex 桌面应用的 **heartbeat automation** 写成，**整章作废**。
> 作废理由与实测证据见 **ADR-0018**。一句话：automation 不把 prompt 当 user message 投递，
> 而是注入一条 `call_id: None, name: "automation_update"` 的合成工具结果；本机用第三方
> Responses 兼容端点（阿里云百炼，`qwen3.8-max`）会 400 拒绝，且因 `disable_response_storage = true`
> 每轮重放全量历史 → **所挂 thread 永久损坏**，连白天交互一并受牵连。

### 11.1 为什么两条 Codex automation 路径都不用

| 路径 | 结论 | 依据 |
|---|---|---|
| `kind = "heartbeat"`（挂在某个 thread 上定时唤醒） | ❌ **否决** | 注入无 `call_id` 的合成项 → 端点 400；毒项沉入长驻会话 + 全量重放 → **会话永久损坏**。四个候选里唯一会**污染主线工作**的 |
| `kind = "cron"`（独立项目任务，每次全新 chat） | ❌ **否决** | 与 heartbeat **共用同一投递机制**；且首轮投递时 Codex 临时生成非法 id `at_<uuid>`（端点要求 `msg_` 前缀），该 id **从不落盘** → 等长补丁法无效。探针两次运行均 2.6 s 内零产出失败 |
| 换官方 OpenAI 端点跑 automation | ⏸ 备选 | 官方端点容忍这些畸形项；但需要官方额度与密钥，且一旦切回第三方端点问题复现 → 不作主方案 |
| **`codex exec` CLI + Windows 任务计划程序** | ✅ **采纳** | 走标准 user message 投递：rollout 中 `function_call_output` 计数 **0**、无 `automation_update`、无 `at_` 前缀 id、模型正常回复、退出码 0。额外好处：① **不依赖 Codex 桌面应用是否开着**；② **无 jitter**（automation 有 +2 分钟随机抖动，来自 `~/.codex/automations/.run-jitter-salt`），定时更准；③ 每次天然是全新会话，顺带解决上下文累积漂移 |

> **附带更正（v1.1 的记载有误）**：v1.1 §11.1 写「cron 模式在当前环境不可用（需 ChatGPT 鉴权）」。
> 实测 cron **能创建也能触发**，真因是创建时缺 `model` / `projectId` / `executionEnvironment`
> 三个必填项，而报错只回一句 `Failed to create automation.` 不指明字段 —— 这本身就是
> 「无静默失败」的反面教材，已记入 `docs/memory/pitfalls.md`。它最终仍被否决，但**否决理由变了**：
> 不是「创建不了」，而是「投递机制与 heartbeat 同源」。

### 11.2 互斥锁（G8）—— **保留，措辞按新机制调整**

- 锁文件：`<repo>/.nightly.lock`（已在 `.gitignore` 中，**不得**提交）
- 内容：单行 JSON
  `{"run_started_at":"<ISO8601>","round":N,"branch":"nightly/<date>","pid":<int>,"note":"<可选>"}`
  （v1.3 新增 `pid`：包装脚本被硬超时 kill 时，人类可据此确认 `codex` 子进程是否也已终止）
- **获取**：锁不存在 → 创建并继续。锁存在且 `run_started_at` 距今 **< 3 小时** → 判定
  「上一次运行仍在进行，或异常退出未清理」→ **本次运行直接结束**（只在
  `docs/nightly/<date>-skipped.md` 追加一行，不改任何代码、不提交）。
- **陈旧锁**：锁存在但距今 ≥ 3 小时 → 把旧锁内容原样抄进 skipped 记录，然后覆盖为新锁并继续。
- **释放**：包装脚本用 `try/finally` 释放，**硬超时 kill 之后也必须释放**（配合 §11.7 ④ 的 `ExecutionTimeLimit`）。
- 为什么必须有：**人类会话与夜间任务可能同时存在**；两个 agent 同时改同一仓库会产生
  无法归因的混合改动，这是不可恢复的（比「少做一夜」严重得多）。
- **双保险**：任务计划的 `MultipleInstances` 默认 **`IgnoreNew`**（本机实测），即同一任务的
  上一次实例未结束时新实例被忽略。但**它挡不住「人类在交互式会话里改仓库」** → G8 锁不可省。

### 11.3 「当夜」的定义与轮次编号 —— **保留**（「每次唤醒」改「每次运行」）

- **当夜归属**：运行开始时刻在本地 00:00~11:59 → 归属**前一天**的夜（例：09-17 02:30 属于
  `2026-09-16` 夜）。所有夜间产物文件名用**当夜日期**。
- **轮次 N** = `docs/nightly/<当夜日期>-round-*.md` 的文件数 + 1。
- **每次运行内最多做 2 轮**；第 2 轮开始前必须重锚（重读 `AGENTS.md` / `PLAN.md` / 新卡）。
- 当 `N > 3`（当夜已做满 3 轮）→ 本次运行**不写代码**，只补/校晨间报告后结束。

### 11.4 谁写晨间报告 —— **保留，触发条件按新机制调整**

满足任一条件，即在本次运行结束前写 `docs/nightly/<当夜日期>-report.md`：

1. 本次运行是当夜的**最后一次计划运行**（当前计划为 23:30 与 02:30 两次，即 02:30 那次）；
2. 当夜轮数已达 3；
3. §6 的任一停止条件被触发（预算耗尽 / 连续验收失败 / 黑名单命中 / 环境异常 / spec 矛盾）。

若报告已存在 → **更新**它（不新建副本），并在末尾追加一行「最后更新时刻 + 本次运行做了什么」。
**新增（v1.3）**：包装脚本还必须在报告 §① 写入本次的**退出码**与**实际上下文窗口**（§11.8 第 6 条）。

### 11.5 必须上报人类的机制性事件（写进晨间报告 §④）—— **重写**

v1.1 的四条里，「thread 上下文触发 auto-compact」与「需把 automation 重新指向新 thread」
**已不适用**（不再有长驻 thread）。v1.3 的清单：

- **CLI 非零退出**（任何原因）→ 报告 §① 必须给出退出码 + 日志尾部 50 行；
- **包装脚本未释放锁**（下一次运行按陈旧锁处理时，必须把上一次的锁内容原样抄进报告）；
- **硬超时被触发**（运行超过脚本设定的 90 分钟上限被 kill）；
- **任务计划本身没有触发**（计划时刻已过但没有日志文件）→ 这一条**只能由人类发现**，
  因为 CLI 没跑起来就没人写报告。排查顺序见 §11.7 ③；
- 连续两次运行因 G8 锁冲突而跳过；
- 连续两夜 0 产出（门禁一直不绿）→ 建议**暂停夜间自动化**，白天先修环境。

### 11.6 通知策略的现实 —— **重写（这是本次机制切换最大的能力损失）**

**CLI 路径没有应用内通知。** v1.1 依赖的「automation 仅失败时通知」**不存在了**。
失败可见性改为**三层文件信号**，全部由包装脚本负责：

| 层 | 载体 | 人类怎么看 |
|---|---|---|
| ① 退出码 | 任务计划的「上次运行结果」 | `Get-ScheduledTaskInfo -TaskName <name>` → `LastTaskResult`（0 = 成功） |
| ② 运行日志 | `docs/nightly/logs/<当夜日期>-<HHMM>.log` | 首行必须写 `codex --version`、退出码、开始/结束时刻、实际上下文窗口 |
| ③ 晨间报告 | `docs/nightly/<当夜日期>-report.md` §① | 报告 §①「环境门禁」必须含本次退出码 |

**明确放弃**：邮件 / 事件日志告警。理由：`Microsoft-Windows-TaskScheduler/Operational`
事件通道在本机**默认关闭**（`wevtutil gl` 实测 `enabled: false`），启用它需要管理员权限，
且会记录**全系统**所有计划任务 → 为一个实验性项目打开系统级审计通道，代价与收益不匹配。
若将来夜间自动化成为常态可重新评估（已登记 `docs/memory/open.md`）。

> **待裁决（原 M6，仍然有效）**：人类是否希望「晨间显式提醒」？当前策略是**只有文件、没有推送**，
> 靠人类自己打开报告。若要推送，最轻的做法是包装脚本在**非零退出时**弹一次 Toast 或 `msg *`
> —— 但那是「失败提醒」，不是「晨间提醒」。不自行更改。

### 11.7 运行前提（诚实说明）—— **重写**

调度者是 **Windows 任务计划程序**，不再是 Codex 桌面应用。因此：

1. **电脑关机 / 睡眠 → 当夜不运行。** `StartWhenAvailable` 默认 **`False`**（本机实测），
   即「错过就不补跑」。**本项目刻意保持 False**：夜间任务的价值在于「人类睡觉时干活」，
   补跑会在人类上班时突然抢锁并占用机器，反而制造 §11.5 的锁冲突。
   → 因此夜间任务必须「**跳过也无害**」：所有产出都在独立分支 `nightly/<date>`，缺一夜不影响主线。
2. **不需要 Codex 桌面应用运行**，也不严格要求用户已登录交互；但**建议**配置为
   「仅在用户登录时运行」—— 无人登录运行需要存凭据，且 GUI 类 spike 在无会话时行为不同 → **不建议**。
3. **人类发现某夜没有报告时的排查顺序**（对应 §11.5 第 4 条）：
   ① `Get-ScheduledTaskInfo` 看 `LastRunTime` / `LastTaskResult`；
   ② 看 `docs/nightly/logs/` 有没有对应文件；
   ③ 两者都没有 → 是**计划没触发**（关机 / 睡眠 / 任务被停用），不是代码问题；
   ④ 日志存在但报告不存在 → 是 agent 中途失败，读日志尾部。
4. **`ExecutionTimeLimit` 默认 `PT72H`（72 小时！）** —— 本机实测。必须显式设为 **`PT1H30M`**，
   否则一次卡死会占住锁三天，§11.2 的「3 小时陈旧锁」判定会被它击穿。

### 11.8 本机适配要点（**全部为本机/官方实测，不是推断**）

| # | 事实 | 实测方式 | 因此怎么做 |
|---|---|---|---|
| 1 | `codex exec` **没有** `--ask-for-approval` | `codex exec --help`（`codex-cli 0.154.0-alpha.6.2`） | 批准与沙箱由 `-s/--sandbox {read-only｜workspace-write｜danger-full-access}` 与 `--approve-for-me` 控制。夜间默认 **`-s workspace-write`**；纯侦察轮可用 `read-only` |
| 2 | 存在 `--dangerously-bypass-approvals-and-sandbox` | 同上 | **永久禁止**在夜间脚本里使用（AGENTS.md §7 精神：不放宽护栏）。它绕过一切确认，与「无人值守不做不可逆动作」直接冲突 |
| 3 | `-C/--cd` 指定工作根；`--skip-git-repo-check` 允许在非 git 目录运行；另有 `--worktree` | 同上 | 脚本用 `-C <repo>`；本仓库确实是 git 库，故 `--skip-git-repo-check` **只在探测轮**用 |
| 4 | `--ephemeral` = 不落盘 session 文件 | 同上 | **不使用**。夜间运行必须留 rollout 供事后取证（§11.5 的退出码与 `function_call_output` 计数都要从 rollout 查） |
| 5 | `--json` 输出 JSONL 事件流；`-o/--output-last-message <FILE>` 写最后一条消息 | 同上 | 两者都开：JSONL 进日志文件（可被脚本断言），最后消息单独落一个文件（晨间报告直接引用） |
| 6 | `-c key=value` 可覆盖任意配置项 | 同上 | 用 `-c model_context_window=1000000` 兜住 PL-014（`qwen3.8-max` metadata 缺失时被 fallback 成 828400）；**首跑后必须核对日志里的实际值** |
| 7 | `New-ScheduledTaskSettingsSet` 默认：`StartWhenAvailable:False`、`ExecutionTimeLimit:PT72H`、`RestartCount:0`、`MultipleInstances:IgnoreNew` | 本机 PowerShell 实测 | `ExecutionTimeLimit` 显式设 `PT1H30M`；`StartWhenAvailable` 保持 `False`（§11.7 ①）；`RestartCount` 保持 **0**（**失败不自动重试**：夜间的失败几乎都需要人类判断，自动重试只会重复消耗预算并制造混合改动） |
| 8 | `Microsoft-Windows-TaskScheduler/Operational` 事件通道 **`enabled: false`** | `wevtutil gl` 实测 | 不启用（§11.6）。失败可见性只走「退出码 + 日志 + 报告」三层文件信号 |
| 9 | `schtasks.exe /tr` **没有**「工作目录」参数 | 命令行已知限制 | 注册任务一律用 `New-ScheduledTaskAction -Execute powershell.exe -Argument "..." -WorkingDirectory <repo>` + `Register-ScheduledTask`，**不用 `schtasks`** |
| 10 | **`codex exec` 的退出码语义没有官方文档** | 官方文档查无 | **自建基线**：首次冒烟把观察到的退出码记进 `docs/nightly/scheduler-acceptance-test.md` 的「基线」表，此后该表就是本项目的事实源（已登记 `docs/memory/open.md` N5） |

### 11.9 验收测试（**启用前必须全绿**）

清单与断言见 **`docs/nightly/scheduler-acceptance-test.md`**。核心 5 条（沿用 ADR-0018 的验收）：

1. 任务计划的 `LastTaskResult` = **0**；
2. `docs/nightly/logs/<date>-<HHMM>.log` 存在，且含 session id 与模型回复；
3. 该会话 rollout 中 `function_call_output` 计数为 **0**（`fix_codex_callid.py` 干跑扫描全库仍为 `total 0`）；
4. `docs/nightly/<date>-round-1.md` 与 `LEDGER.md` 追加行存在；
5. `.nightly.lock` 在运行结束后**不存在**。

**当前状态（2026-09-18）：⏳ 未验收。** 本机还**没有**跑过一次完整的
「任务计划触发 → 包装脚本 → `codex exec` → 报告」端到端运行（`docs/memory/open.md` N4）。
在此之前，**不得**注册正式任务计划；只允许在人类在场时做手工冒烟。

---

## 12. 变更历史

| 版本 | 日期 | 变更 | 理由 |
|---|---|---|---|
| 1.0 | 2026-09-16 | 初稿 | — |
| 1.1 | 2026-09-16 | 新增 §11（heartbeat 适配：G8 互斥锁、当夜与轮次编号、晨间报告触发条件、机制性上报事件、通知现实、运行前提）；§1 增 G8；§6 改"每夜 ≤3 轮 / 每次唤醒 ≤2 轮"、收尾 05:30→05:00；§7 通知策略与机制对齐；§10"每轮新会话"改"每轮强制重锚" | 实测 cron 模式在本环境不可用（需 ChatGPT 鉴权），只能用挂在本 thread 的 heartbeat；原假设"每轮新会话"不成立 → 必须补并发锁与上下文累积对策，否则夜间自动化本身会成为漂移源 |
| 1.3 | 2026-09-18 | **§11 全章重写**：投递机制由 Codex heartbeat automation 改为 **Windows 任务计划程序 + `codex exec`**（ADR-0018 转 Accepted）。§11.1 改为「两条 automation 路径为何都否决」并**更正 v1.1 对 cron 的错误记载**；§11.2 锁内容新增 `pid`；§11.3「每次唤醒」→「每次运行」；§11.4 新增「报告 §① 必须写退出码与实际上下文窗口」；§11.5 删 2 条不适用事件、新增 4 条（CLI 非零退出 / 未释放锁 / 硬超时 / 计划未触发）；**§11.6 重写**（CLI 无应用内通知 → 退出码+日志+报告三层文件信号，明确放弃邮件与事件日志告警）；**§11.7 重写**（关机/睡眠不补跑、`StartWhenAvailable` 刻意保持 False、`ExecutionTimeLimit` 必须显式设 90 分钟、给出 4 步排查顺序）；**新增 §11.8**（10 条本机/官方实测适配要点）与 **§11.9**（验收测试；当前状态 ⏳ 未验收）。同步改动：文件头版本/日期、§6、§7、**§10「重开会话是正常操作」一行由「机制上做不到」改为「机制上已保证」** | 人类指示 #10（2026-09-18）：夜间自动化本质是定时任务问题，要求查官方文档 + 分析本地原因 + 专门留时间测试。ADR-0018 的实测已否决两条 automation 路径；本版把结论落进章程，并把所有「靠推断」的部分替换为本机实测值 |
| 1.2 | 2026-09-16 | G5 从"必须有 Ready 任务卡，否则停止整夜"放宽为**三级回退**（Ready 卡 → §13 工作单 → 白名单 P2~P8）；新增 §13「当前夜间工作单」 | 原 G5 与 §2 白名单的 P2~P8 直接矛盾：白名单说"无 Ready 卡时做这些"，G5 却说"无 Ready 卡就停"。且阶段 0 的 10 张卡**没有一张是夜间安全的**（Spike 都要真机 GUI 或新增依赖），照原 G5 会导致夜间自动化永久空转 |

---

## 13. 当前夜间工作单（人类维护 · G5 的第 ② 级回退）

> **为什么需要工作单**：阶段 0 的 10 张卡里，**没有一张是夜间安全的** ——
> Spike A/A2/G 要真机 GUI，Spike B/C/E/F 要新增依赖或前端工程，Spike H 要 `rusqlite`，
> Spike D-lite 要 Linux 虚拟机；而 §3 黑名单禁止夜间新增依赖与操作真实应用。
> 如果没有工作单，夜间自动化只能做 P2~P8 里"补测试、修告警"这类边角料，价值很低。
>
> **规则**：
> 1. 工作单由**人类在白天维护**；夜间 agent 只能执行，**不得新增、改写或重排工作单**。
> 2. 每张工作单都必须自带 write scope 与**可机器验证**的验收命令（否则夜间无法自证完成）。
> 3. 一轮只做一张工作单，按编号顺序取第一张未完成的。
> 4. 完成后：把本节的 `[ ]` 勾成 `[x]`，**并在同一行追加** commit 短哈希与轮次
>    （这是本节唯一允许的夜间写入，属于"进度标记"而非"改契约"）。
> 5. 工作单与既有任务卡**范围重叠**时，必须在轮次报告里写明重叠部分，供人类合并时核对。

### W1　`rustscan` 函数扫描器（为函数级卫生规则铺路）

- [ ] 未完成
- **目标**：在 `rustscan.rs` 增加 `find_functions(code: &str) -> Vec<FunctionSpan>`，
  基于已有的降噪视图 `Scan::code` 定位每个函数的 **名字、起止行、参数列表、函数体行数**。
  trait/extern 里无函数体的声明（以 `;` 结束）要能识别并标记为"无函数体"。
- **write scope**：`xtask/src/rustscan.rs`（仅此一个文件）
- **禁止**：改 `hygiene.rs`（那是 W2）、改任何阈值、改 `deferred.rs`、动其他文件
- **验收**（全部必须通过）：
  ```powershell
  cargo fmt --all --check
  cargo clippy --all-targets -- -D warnings
  cargo test --workspace                      # 原有 99 个测试必须仍然全绿
  cargo run -p xtask -- hygiene               # 仍须 PASSED（含 rustscan.rs 自身）
  ```
  另需新增 ≥10 个针对 `find_functions` 的表驱动测试，**必须覆盖**：无函数体的 trait 声明、
  泛型参数 `fn f<T: Trait>(...)`、多行签名、`pub(crate) async unsafe extern "C" fn`、
  嵌套闭包中的 `fn`、行号与 `Scan::code` 一致（不变量 1）、空文件、只有注释的文件。
- **规模上限**：diff ≤ 400 行；超出就停下并在报告里建议拆单

### W2　hygiene 函数级规则三条（gov §5.4 的第 2/3/4 项）

- [ ] 未完成　**依赖 W1**
- **目标**：实现 `hygiene/function-too-long`（>80 行 Warning）、`hygiene/too-many-params`
  （>6 个 Warning）、`hygiene/high-cyclomatic-complexity`（>15 Warning）。
  阈值**必须复用** `clippy.toml` 里已定的数字（15 / 6 / 80），不得另立一套。
  同步把 `deferred.rs` 里对应的三条移出未实现表，并把
  `IMPLEMENTED_HYGIENE_RULE_COUNT` 从 3 改为 6。
- **write scope**：`xtask/src/hygiene.rs`、`xtask/src/deferred.rs`、`xtask/README.md`
- **禁止**：改 `rustscan.rs`（W1 的产物）、改 `clippy.toml`/`Cargo.toml` 的 lint 配置、
  为了让本仓库通过而调高阈值
- **验收**：W1 的四条命令 + 以下三条
  ```powershell
  cargo test -p xtask                         # deferred 的"数量自洽"测试必须仍然通过
  cargo run -p xtask -- --list-deferred       # 未实现规则应从 10 条降到 7 条（总数 13，ADR-0025）
  cargo run -p xtask -- hygiene               # 对本仓库仍须 PASSED（0 error 0 warning）
  ```
  每条新规则 ≥5 个用例，其中**至少 2 个负向**（恰好等于阈值不告警、差一行不告警）。
  若本仓库某个函数因此告警：**不许改阈值**，要么拆函数，要么在报告里记 DRIFT 交人类裁决。
- **与 TASK-015 的重叠**：TASK-015 的验收含"hygiene 13 项检查全部生效"。W1+W2 完成后
  其中 3 项已生效，人类合并时应在 TASK-015 卡里勾掉对应项（夜间 agent 只报告，不改 plans/*）。

### W3　`docs/spec/testing.md` 草案（白盒测试策略）

- [ ] 未完成
- **目标**：写出白盒测试契约草案，**文件头必须标注** `状态：Draft（待人类批准）`。
  至少覆盖：① 四类可测试性接缝（纯函数规则 / 输出注入 `&mut dyn Write` / IO 集中在边界 /
  阈值为 `pub const`）② 测试命名 `test_<unit>_<condition>_<expected>` ③ 允许在
  `#[cfg(test)] mod tests` 上 `#[allow]` 的 lint 白名单（`unwrap_used`/`expect_used`/`panic`）
  及其理由 ④ 负向测试的强制要求 ⑤ 覆盖率门槛与测量工具（`cargo-llvm-cov`）
  ⑥ trait 替身（test double）的注入方式 ⑦ 禁止真实 IO/网络/GUI ⑧ 与录制回放（TASK-034）的分工。
  以 `xtask` 的现有测试为**实证范例**引用（它们已经满足上述大部分要求）。
- **write scope**：`docs/spec/testing.md`（新建，仅此一个文件）
- **禁止**：修改任何既有 spec、修改 `AGENTS.md`、把状态写成 `Accepted`
- **验收**：文件存在且首屏含 `状态：Draft`；Markdown 代码围栏成对；
  `cargo run -p xtask -- hygiene` 仍 PASSED；`git diff --stat` 只含该文件 + LEDGER + 夜间报告。
- **附带收益**：解除 `docs/PARKING_LOT.md` PL-003（`report.rs` 引用了尚不存在的 testing spec）。

### W4　ADR 草稿 0016~0020

- [ ] 未完成
- **目标**：把 `MEMORY.md` §3 中 ADR 待建 0016~0020 五条决策，按 gov §9.3 模板落成
  `docs/adr/0016-*.draft.md` ~ `docs/adr/0020-*.draft.md`，状态一律 **Draft**。
  每条必须有：背景、决策一句话、**≥2 个考虑过的选项含被否理由**、影响、风险与缓解、验证方式。
  素材来源：`MEMORY.md` §3/§4、`tasks/TASK-001-repo-skeleton.md` §5.4、
  `docs/overnight-automation-charter.md` §11/§12。**不得引入新的决策或改变已有决策的语义** ——
  这是"把已做的决定写成 ADR"，不是"重新做决定"。
- **write scope**：`docs/adr/0016-*.draft.md` ~ `docs/adr/0020-*.draft.md`（新建 5 个文件）
- **禁止**：写 `状态：Accepted`、修改 `MEMORY.md` §3/§4、新建 0021 及以后的编号
- **验收**：5 个文件都存在且都含 `状态：Draft`；每个都含"考虑过的选项"且 ≥2 个选项；
  Markdown 围栏成对；`cargo run -p xtask -- hygiene` 仍 PASSED。

### 优先级与排序理由

`W1 → W2 → W3 → W4`。

代码优先（W1/W2）：它们**可机器验证到近乎绝对**（测试通过就是通过），且直接加强护栏本身 ——
护栏越强，之后每一夜和每一张卡的质量下限越高。
文档其次（W3/W4）：价值高但"写得好不好"需要人类判断，夜间产出只能是 Draft。
