# 夜间自动推进章程（Overnight Automation Charter）

> 状态：Active　版本：1.2　日期：2026-09-16　变更历史见 §12
> 上位：`AGENTS.md`、`docs/governance-ai-agent-execution.md`（gov）、`docs/subagent-orchestration.md`
> 本文件是夜间自动化任务的**唯一权威规则源**。automation 的 prompt 只做引用，不复制规则。
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
| 每夜轮数 | **≤3 轮**（每轮一张卡；每次 automation 唤醒内 ≤2 轮，见 §11.3） |
| 结束时间 | **05:00** 前必须收尾（写晨间报告） |
| 单卡 diff | ≤ 400 行；超出 → 停止该卡并建议拆卡 |
| 每夜总 diff | ≤ 1200 行（**人类半天可审阅完**，这是硬约束） |
| 连续验收失败 | 2 次 → 停止整夜 |
| 黑名单命中 | 当夜累计 2 次 → 停止整夜 |
| 环境/工具链异常 | 立即停止整夜 |
| 发现 spec 矛盾 | 记 DRIFT → **停止整夜**（后续卡可能都依赖同一处矛盾） |

---

## 7. 晨间报告（人类醒来看到的第一份东西）

路径：`docs/nightly/<YYYY-MM-DD>-report.md`；同时在该 automation 的运行结果里给出摘要。

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
| 重开会话是正常操作（gov §4.4） | 机制上**做不到**每轮新开会话（§11.1）→ 改为**每轮强制重锚**：重新读 AGENTS.md / PLAN.md / 任务卡 / LEDGER，且 thread 历史一律不视为事实源 |
| 人类审阅是瓶颈（subagent §9.3） | 每夜总 diff ≤ 1200 行，控制在半天可审完 |
| 不放宽护栏（gov §5.2） | lint/CI/deny 配置夜间只读（§3.4） |
| 记忆可回查（MEMORY.md） | 每轮追加 LEDGER + MEMORY 增量（§4 ⑨） |

---

## 11. heartbeat 机制适配（v1.1 新增 —— 实现约束，不是理想设计）

### 11.1 为什么是 heartbeat 而不是独立 cron 任务

本章程初稿假设"每轮都是一个全新会话"。实测（2026-09-16）推翻了该假设：

- `automation_update` 创建 `kind = "cron"`（独立项目任务，每次全新会话）时后端返回
  `Failed to create automation.`；日志显示本机非 ChatGPT 账号鉴权（使用自定义 model
  provider），而 cron/远端调度依赖该鉴权 → **cron 模式在当前环境不可用**。
- `kind = "heartbeat"`（挂在某个 thread 上的定时唤醒）可正常创建，但**只能挂在已存在的
  thread 上**，且**一个 thread 只允许一个 heartbeat**。

因此实际机制是：**同一个 thread 被定时唤醒**（当前为每日 23:30 与 02:30 两次）。后果与对策：

| 后果 | 对策（硬性） |
|---|---|
| 上下文随夜数累积 → 漂移风险上升 | 每轮开始**必须**重新读 `AGENTS.md` + `PLAN.md` + 任务卡 + `LEDGER.md` 末 10 行；**thread 历史不是事实源**（AGENTS.md §1 裁决顺序） |
| 上下文过大导致成本上升与质量下降 | 每夜 ≤3 轮、每次唤醒 ≤2 轮；一旦触发 auto-compact 或出现"引用了仓库中不存在的事实" → 人类新建专用夜间 thread 并把 automation 重新指向它（§11.5） |
| 无法保证只有一个夜间 agent 在跑（人类可能同时在用仓库） | G8 互斥锁（§11.2） |
| 唤醒次数固定，轮次编号不能靠"第几次唤醒" | 轮次编号由已存在的报告文件数推导（§11.3） |

### 11.2 互斥锁（G8）

- 锁文件：`<repo>/.nightly.lock`（已在 `.gitignore` 中，**不得**提交）
- 内容：单行 JSON
  `{"wake_started_at":"<ISO8601>","round":N,"branch":"nightly/<date>","note":"<可选>"}`
- **获取**：锁不存在 → 创建并继续。锁存在且 `wake_started_at` 距今 **< 3 小时** → 判定
  "上一轮仍在运行，或异常退出未清理" → **本次唤醒直接结束**（只在
  `docs/nightly/<date>-skipped.md` 追加一行，不改任何代码、不提交）。
- **陈旧锁**：锁存在但距今 ≥ 3 小时 → 把旧锁内容原样抄进 skipped 记录，然后覆盖为新锁并继续。
- **释放**：本次唤醒结束前**必须**删除锁文件。异常未删除的情况由下一次唤醒按"陈旧锁"处理。
- 为什么必须有：heartbeat 与人类会话可能同时存在；两个 agent 同时改同一仓库会产生
  无法归因的混合改动，这是不可恢复的（比"少做一夜"严重得多）。

### 11.3 "当夜"的定义与轮次编号

- **当夜归属**：唤醒时刻在本地 00:00~11:59 → 归属**前一天**的夜（例：09-17 02:30 属于
  `2026-09-16` 夜）。所有夜间产物文件名用**当夜日期**。
- **轮次 N** = `docs/nightly/<当夜日期>-round-*.md` 的文件数 + 1。
- 每次唤醒内最多做 **2 轮**；第 2 轮开始前必须重锚（重读 PLAN 与新卡）。
- 当 `N > 3`（当夜已做满 3 轮）→ 本次唤醒**不写代码**，只补/校晨间报告后结束。

### 11.4 谁写晨间报告

满足任一条件，即在本次唤醒结束前写 `docs/nightly/<当夜日期>-report.md`：

1. 本次唤醒时刻 ≥ 02:30（当夜最后一次唤醒）；
2. 当夜轮数已达 3；
3. §6 的任一停止条件被触发（预算耗尽 / 连续验收失败 / 黑名单命中 / 环境异常 / spec 矛盾）。

若报告已存在 → **更新**它（不新建副本），并在末尾追加一行"最后更新时刻 + 本次唤醒做了什么"。

### 11.5 必须上报人类的机制性事件（写进晨间报告 §④）

- thread 上下文触发 auto-compact；
- 连续两次唤醒因 G8 锁冲突而跳过（锁未被正常释放，或人类与夜间任务撞车）；
- 连续两夜 0 产出（门禁一直不绿）→ 建议暂停 automation，白天先修环境；
- 需要把 automation 重新指向一个新的专用夜间 thread。

### 11.6 通知策略的现实

automation 的通知策略为 **仅失败时通知**。因此：夜间各轮不打扰人类；运行异常会立刻通知；
**"晨间提醒"由报告文件承担**（人类早晨打开 `docs/nightly/<date>-report.md`）。
若人类希望早晨收到一次显式提醒，唯一办法是把策略改为"每次都通知"，代价是 23:30 与 02:30
也各响一次 → 列为**待裁决项**，不自行更改。

### 11.7 运行前提（诚实说明）

heartbeat 由 Codex 桌面应用调度：**电脑关机 / 应用退出 / 系统睡眠 → 当夜不运行，且不补跑**。
所以：① 夜间任务必须"跳过也无害"——所有产出都在独立分支，缺一夜不影响主线；
② 人类发现某夜没有报告时，先确认应用当时是否在运行，再怀疑代码。

---

## 12. 变更历史

| 版本 | 日期 | 变更 | 理由 |
|---|---|---|---|
| 1.0 | 2026-09-16 | 初稿 | — |
| 1.1 | 2026-09-16 | 新增 §11（heartbeat 适配：G8 互斥锁、当夜与轮次编号、晨间报告触发条件、机制性上报事件、通知现实、运行前提）；§1 增 G8；§6 改"每夜 ≤3 轮 / 每次唤醒 ≤2 轮"、收尾 05:30→05:00；§7 通知策略与机制对齐；§10"每轮新会话"改"每轮强制重锚" | 实测 cron 模式在本环境不可用（需 ChatGPT 鉴权），只能用挂在本 thread 的 heartbeat；原假设"每轮新会话"不成立 → 必须补并发锁与上下文累积对策，否则夜间自动化本身会成为漂移源 |
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
  cargo run -p xtask -- --list-deferred       # 未实现规则应从 8 条降到 5 条
  cargo run -p xtask -- hygiene               # 对本仓库仍须 PASSED（0 error 0 warning）
  ```
  每条新规则 ≥5 个用例，其中**至少 2 个负向**（恰好等于阈值不告警、差一行不告警）。
  若本仓库某个函数因此告警：**不许改阈值**，要么拆函数，要么在报告里记 DRIFT 交人类裁决。
- **与 TASK-015 的重叠**：TASK-015 的验收含"hygiene 12 项检查全部生效"。W1+W2 完成后
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