# ADR-0028　文件改写互斥锁协议（`xtask guard`）

状态：**Accepted**（2026-09-18，人类指示 #1 的追加硬性要求）　日期：2026-09-18　Supersedes：—　Superseded by：—
来源：2026-09-18 人类会话。原话：「**所有文件在改写时需要加锁和等待以及超时放弃，放弃通报机制，
避免几个进程同时改。**」
关联：`AGENTS.md` §3（会话协议）/ §8（write scope）、`docs/overnight-automation-charter.md` §11.2（`.nightly.lock`）、
`docs/subagent-orchestration.md`（并行 agent 分工）、`xtask/README.md`、`docs/adr/0025-hygiene-rule-count-unification.md`（仓库文件形态约定）

---

## 背景（为什么现在要决定）

本项目的执行方式是「多个 AI agent 会话（Codex / opencode / Claude Code）+ subagent 并行 + 夜间无人值守」，
且 `AGENTS.md` §8 明确允许**并行度 ≤3**。这意味着「两个进程同时改同一个文件」不是理论风险，而是默认工况。

现有防线只有两道，且都不够：

| 防线 | 粒度 | 缺口 |
|---|---|---|
| `AGENTS.md` §8 的 write scope 表 | **文档约定**（每个 agent 声明自己只改哪些文件） | 没有运行时强制。agent 读文档 → 编辑 → 写回之间**没有任何原子性**；两个 agent 各自基于旧内容写回 = 经典的 **lost update**。更糟的是 git **不会报冲突**（两次写都「成功」了），丢失的内容无声无息 |
| 章程 §11.2 的 `.nightly.lock` | **整个仓库**，且只在夜间路径生效 | ① 粒度太粗：夜间任务一跑，白天谁都不能动仓库；② 覆盖不全：白天两个交互式会话同时改 `MEMORY.md`，它完全不管；③ 它是**手写 JSON + PowerShell 脚本**，没有等待、没有超时、没有通报，只有「跳过」 |

而风险最高的恰恰是三个**只追加的公共热点文件**：`LEDGER.md`、`docs/memory/*.md`、`docs/PARKING_LOT.md`。
它们是「所有会话都必须写」的文件，因此是最可能撞车的文件；而「只追加」的语义又让覆盖**特别难被发现** ——
别人追加的那一行消失了，没有任何工具会报错，直到几天后有人发现记忆断层。

**已经真实发生过的近似事故**：2026-09-18 本会话与上一会话交接时，两边都在改 `MEMORY.md` 的计数表；
最终靠人工逐行核对才发现 4 处数字过期（见 `LEDGER.md` 2026-09-18 末行）。那次的根因是「同一数字手写多处」
（PL-022，另见 ADR-0030），但**如果两个会话同时写回，就会直接丢条目**。

## 决策（一句话）

**在 `xtask` 增加 `guard` 子命令族，提供「按文件」的协作式互斥锁：获取时等待，等待超时即放弃，
放弃必须通报（专用退出码 + 机器可读结果行 + 放弃日志 + 台账义务）；锁文件放 `target/locks/`（不入库）。**

拆成八条：

- **D1　落点 = `xtask guard`**（Rust，零第三方依赖）。不新建 `tools/` 顶层目录，不放仓库外脚本。
  子命令：`acquire` / `release` / `status` / `reap`。
- **D2　锁粒度 = 单个仓库相对路径**。锁文件路径 =
  `target/locks/<slug>.lock`，其中 `slug` = 相对路径**先按小写折叠**，再把 `/` 换成 `__`、
  非字母数字换成 `-`、截断到 **64** 个可见字符，最后拼上 8 位 **FNV-1a** 十六进制摘要（防重名与超长）。
  slug 与摘要都是**纯函数**，可测试、跨平台一致。
  - **为什么按小写折叠**：Windows / macOS 的默认文件系统**大小写不敏感**，`MEMORY.md` 与 `memory.md`
    指向同一个文件；若分别派生两把锁，互斥就形同虚设。代价是 Linux（大小写敏感）上两个**真的**只差
    大小写的文件会共用一把锁 —— 这是**刻意接受的过锁**（宁可少并行，不可 lost update）。
  - 锁记录里的 `target` 字段**不做大小写归一**（要能原样回显给人看），折叠只发生在派生锁名时。
- **D3　原子获取原语 = `OpenOptions::create_new(true)`**（POSIX 的 `O_CREAT|O_EXCL`、Windows 的
  `CREATE_NEW`）。这是两个平台上**都保证原子**的「不存在才创建」，不需要任何第三方锁库。
- **D4　锁记录 = 行式 `key=value` 文本**（不是 JSON，避免手写解析器的转义坑），字段：
  `target` / `owner` / `pid` / `task` / `acquired_at_unix` / `acquired_at_iso` / `intent`。
  **必须带人类可读的诊断信息**：超时放弃时，等待方要能说出「谁、什么时候、为了什么」拿着这把锁。
- **D5　等待 / 超时 / 放弃 / 通报**：默认 `--timeout 30`（秒），退避轮询 100 ms。超时即**放弃**，并且：
  ① 退出码 **5**（新增，见 D8）；② stdout 打印机器可读行
  `-- guard-result: ABANDONED target=… held_by=… held_age_secs=… waited_ms=…`；
  ③ 往 `target/locks/abandonments.log` 追加一行（取证用，不入库）；
  ④ **协议义务**：调用方必须在 `LEDGER.md` 追加一行说明放弃了什么、为什么。
  ①②③ 是工具做的，④ 是 `AGENTS.md` 规定的 —— 合起来才叫「通报」，缺 ④ 就只是「日志」。
- **D6　陈旧锁 = 纯时间判据**。`--stale-after`（默认 **900** 秒）到期即判为陈旧，`acquire` 可直接接管
  （`TAKEOVER`），但**必须打印被接管者的完整锁记录**并写进 `abandonments.log`。
  **刻意不做 PID 存活探测**：那需要 `windows` crate 或 `libc`，违反 xtask 不变量 1（零第三方依赖），
  而 PID 复用本身还会造成误判（旧 PID 被无关进程占用 → 判定「还活着」→ 永久死锁）。
  人工强制接管用 `--force`（不受 `--stale-after` 限制，同样要打印与记录）。
- **D7　多文件按序获取（死锁避免）**。`acquire` 接受多个路径时，**先按仓库相对路径排序再逐个获取**；
  任何一个失败（含超时）→ **回滚已获取的全部锁**再放弃。这样两个 agent 以不同顺序请求同一组文件时
  不会互相等待到死。
- **D8　xtask「只读」不变量的受限放宽 + 新退出码 5**。`xtask/src/main.rs` 的不变量 3 原文是
  「只读：本工具不创建、修改或删除任何文件」。改为：
  「只读**仓库内容**：不创建、修改或删除任何**受版本控制**的文件；唯一例外是
  `target/locks/` 下的临时锁文件与放弃日志（ADR-0028），它们不入库、由 `guard` 自己管理生命周期。」
  退出码表新增 **`5` = 锁获取超时（放弃）**；`release` 时 owner 不匹配且未给 `--force` → 退出码 **1**。

## 考虑过的选项

| # | 方案 | 结论 | 理由 |
|---|---|---|---|
| 1 | 只在 `AGENTS.md` 写协议，不提供工具 | ❌ 否决 | 约定没有强制力，这正是当前缺口本身。而且没有工具就没有「等待/超时/放弃」的统一语义，每个 agent 会各写一套 |
| 2 | 仓库外 Python 工具（给 `D:\csart\patchkit.py` 加锁） | ❌ 否决 | 仓库外的东西**其他 agent、CI、未来的开源用户都拿不到** → 机制在纸面上存在、实际上为空。且不受版本控制、不可审计 |
| 3 | 新建 `tools/` 顶层目录放锁脚本 | ❌ 否决 | 新顶层目录要先进 ADR 白名单（gov §5.4 第 5 项，尚未实现）；与 Rust-first 栈相悖；`xtask` 已有覆盖全部分支的白盒测试、已接进 CI、已在所有文档的路由表里 —— 复用它成本最低、可发现性最高 |
| 4 | 用 git 自身做互斥（临时分支 / `index.lock` / `git update-index`） | ❌ 否决 | git 只在**提交时刻**保护，覆盖不到「读文件 → 想 → 写回」这个真正的危险窗口（可能长达几十分钟）。抢占 `index.lock` 更糟：那是 git 自己在用的锁，抢它会让随机的 git 命令失败 |
| 5 | 第三方锁 crate（`fs2` / `fd-lock`） | ❌ 否决 | 违反 xtask 不变量 1（零第三方依赖 = 护栏工具自身无供应链风险）；且 OS 级建议锁在 Windows 与 POSIX 上语义不同（Windows 的字节范围锁会**阻止读**），跨平台一致性还得自己兜。`create_new` 已经足够，且语义在两平台上完全一致 |
| 6 | 数据库锁（SQLite） | ❌ 否决 | 阶段 0 还没有 `rusqlite` 依赖（Spike H 才引入）；为开发期工具提前引入产品级依赖是本末倒置 |
| 7 | **`xtask guard`（协作式文件锁）** | ✅ 采纳 | 仓库内、零依赖、纯逻辑可白盒测试、已在 CI 与文档路由里、跨 agent 可发现。缺点（协作式 = 不能技术强制）由 D5 ④ 的台账义务与 review agent 兜住 |

## 影响（需要改的文档 / 代码 / 任务卡）

| 对象 | 变更 | 本次是否执行 |
|---|---|---|
| `xtask/src/guard.rs` | **新建**：锁记录模型、slug/摘要、`decide_acquire` 纯决策函数、结果渲染（全部纯函数，25 个测试） | ✅ |
| `xtask/src/guard_model.rs` | **新建**：请求 / 失败 / 三态读锁模型（15 个测试） | ✅ |
| `xtask/src/guard_store.rs` | **新建**：`LockStore` trait + 文件系统实现 + ISO-8601 时钟（7 个测试） | ✅ |
| `xtask/src/guard_runner.rs` | **新建**：分派 + `acquire`（等待 / 放弃 / 接管 / 回滚）（18 个测试） | ✅ |
| `xtask/src/guard_release.rs` | **新建**：`release` / `status` / `reap`（14 个测试） | ✅ |
| `xtask/src/guard_testkit.rs` | **新建**：内存锁存储替身（仅 `cfg(test)`，让上面 5 个模块的测试不需要临时目录） | ✅ |
| `xtask/src/guard_tests.rs` / `guard_runner_tests.rs` | **新建**：`guard` / `guard_runner` 的私有单测用 `#[path]` 外置，以满足 gov §5.4 的单文件 600 行硬上限（可见性语义不变，仍是父模块的私有子模块） | ✅ |
| `xtask/src/cli.rs` | `Invocation` 增 `operands: Vec<String>` 与 guard 专用选项；不变量 3 措辞按 D7 放宽（子命令之后的位置参数是操作数，不是第二个子命令） | ✅ |
| `xtask/src/main.rs` | 分派 guard；锁文件 IO 集中在本文件；退出码 5；模块表与不变量 3 按 D8 改写 | ✅ |
| `xtask/Cargo.toml` | 顶部注释「不修改任何文件」→ 按 D8 修正 | ✅ |
| `xtask/README.md` | 增 guard 一节（用法、退出码、协议义务） | ✅ |
| `AGENTS.md` | §3 会话协议增「改写公共热点文件前必须 `guard acquire`」；§8 write scope 表增一行说明锁与 scope 的关系 | ✅（本 ADR 即授权） |
| `docs/overnight-automation-charter.md` §11.2 | 说明**两层锁**：`.nightly.lock`（粗：整个仓库、夜间 vs 白天）+ `guard`（细：单文件、任意两个进程） | ✅ |
| `docs/subagent-orchestration.md` | 并行 agent 的锁协议（write scope 互不重叠**仍然必须**，guard 是第二道保险，不是替代） | ✅ |
| `LEDGER.md` / `docs/PARKING_LOT.md` / `docs/memory/*.md` | 只追加（事件行 / 处置行 / 决策与事实条目） | ✅ |
| `.gitignore` | **不改**（`/target/` 已覆盖 `target/locks/`） | — |
| `.github/workflows/ci.yml` | **不加 job**：guard 是开发期协作工具，不是产品门禁；它的正确性由 `cargo test -p xtask` 覆盖（已在硬门禁 #4 内） | — |
| 产品代码 / spec / schema / 任务卡 | **不改**（本 ADR 属工程元层，零产品影响） | — |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| **锁泄漏**（agent 崩溃、被 kill、断电） | D6 的时间型陈旧判定（默认 900 s）+ `guard reap` 批量清理 + `--force` 人工接管；三者都会**打印被接管者的原始记录**，不静默 |
| **agent 绕过 guard 直接写文件** | 协作式锁**无法技术强制**（这是本方案唯一的真实弱点）。缓解三层：① `AGENTS.md` 把它写成铁律级义务；② review agent 的 PR 检查项；③ 事后由 `xtask memory-counts` / `adr-index`（ADR-0030）发现「条目数变少」这类覆盖症状 |
| `cargo clean` 删掉 `target/locks/` → 锁全丢（fail-open） | 明确接受：guard 是**降低概率**的第二道防线，不是唯一防线；write scope + review 仍是主防线。且 `cargo clean` 通常发生在无人编辑时。锁目录**刻意**放在 `target/`（不入库）而不是仓库根，就是为了不污染版本控制 |
| 超时太短 → 频繁放弃、夜间空转 | 默认 30 s 对「读-改-写」足够（一次写回是毫秒级）；长任务应显式传更大的 `--timeout`，而不是把默认值调大 |
| 两个 agent 用**相同的 `--owner`** → 被误判为同一持有者而直接复用锁 | `AGENTS.md` 规定 owner 必须是**会话级唯一**：`<agent>-<thread 前 8 位>` 或 `TASK-NNN`。`status` 会打印 owner，review 时可查 |
| Windows 上删除被占用的锁文件失败 | `release` 返回 IO 错误（退出码 4）并打印路径，**不静默**；锁写完后立即关闭句柄，正常情况不会触发 |
| 时钟回拨导致 `acquired_at_unix` 在未来 | `decide_acquire` 显式识别为 **clock anomaly**：判为「等待」而非「陈旧」，并在输出里标注，避免一次时钟调整就把所有锁判成陈旧而集体接管 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `cargo run -p xtask -- guard status`（空锁目录）→ 打印 `NONE`、退出码 **0**。
2. **并发实证**：两个进程同时 `guard acquire` 同一文件 → 恰好一个 `OK`，另一个在超时后
   `ABANDONED` + 退出码 **5** + `abandonments.log` 增一行。（`xtask/src/guard_tests.rs` 的单测用
   「先手工放置一个新鲜锁记录再 acquire」的方式等价复现，避免测试依赖真实多进程时序。）
3. **陈旧接管**：手工造一个 `acquired_at_unix` = 现在 − 3600 的锁 → `--stale-after 60` 时返回 `TAKEOVER`
   且输出含被接管者的 owner。
4. **时钟异常**：`acquired_at_unix` = 现在 + 3600 → 返回等待 + 标注 clock anomaly，**不接管**。
5. **死锁避免**：`acquire B A` 与 `acquire A B` 的获取顺序在工具内部**都变成 A → B**（排序可断言）。
6. `git status --porcelain` 在持锁期间**不显示** `target/locks/`（已被 `/target/` 忽略）。
7. `cargo test --workspace` 全绿，且 `guard` 的纯逻辑分支（Grant / Reuse / Wait / TakeOver / ClockAnomaly）
   各有 ≥1 个测试；`xtask hygiene` 仍 PASSED。
8. **重新评估触发条件**：① 若将来引入真正的多进程产品编排（Host / Skill 分进程），进程间互斥应改用
   OS 级建议锁或数据库锁，`guard` 退回为纯开发期工具；② 若 `xtask` 被允许写仓库文件
   （`codegen`，TASK-011），D8 的「只读例外」范围必须重述；③ 若出现「锁被绕过导致真实丢条目」的事故，
   应评估把 guard 从协作式升级为**强制式**（例如让所有文档写入都必须经过一个网关命令）。
