# MEMORY.md — 项目长期记忆（**L0 索引层**）

> **本文件只是索引**（ADR-0021），硬上限 **150 行**。全部具体条目在 `docs/memory/`。
> **用途**：让任何新会话的 agent（或新加入的人）在 **2 分钟内**知道"该去哪个文件找什么"，
> 而不是每次全量读入 200+ 行、其中真正相关的只有 2~5 条。
> **超过 150 行 = 有内容该下沉到 L1/L2 了，不要提高上限**（ADR-0021 明确否决过"提到 600 行"）。

## 四种记录文件的分工（不可混用）

| 文件 | 记录什么 | 时间性 | 写法 |
|---|---|---|---|
| **`MEMORY.md` + `docs/memory/*`** | **认知**：已知事实、已否决方案、踩过的坑、应用档案 | 长期有效 | 条目式，带日期与来源；**只追加** |
| `LEDGER.md` | **事件**：哪张卡、哪个 commit、验收结果 | 流水 | 只追加，一行一事件 |
| `PLAN.md` + `plans/*` | **意图**：现在做什么、不做什么 | 当前阶段 | 可覆写，带变更历史 |
| `docs/adr/*` | **决策理由**：为什么这么定 | 永久 | 只增不改，可被新 ADR 取代 |

## 读取路由（★ 按这张表读，**不要全读**；详版见 `docs/memory/README.md`）

| 你要做什么 | 读什么 | 读多少 |
|---|---|---|
| 任何工作开始前 | 本文件 | 全量（≤150 行） |
| **动手前防止重复辩论** | `docs/memory/rejected.md` | **全量**（最短的一个） |
| 接某个应用（如记事本） | `docs/memory/apps/<app>.md` + `target-apps-feasibility.md` §3 | 全量 |
| 查某类事实 / 坑 | `docs/memory/facts.md` 或 `pitfalls.md` | **先 grep，再读命中节** |
| 查"为什么这么定" | `docs/memory/decisions.md` → `docs/adr/NNNN-*.md` | 索引 → 单条 ADR |
| 查"还有什么没定" | `docs/memory/open.md` | 全量 |

## 各文件当前规模（Orchestrator 每次归档/大改后更新本表）

| 文件 | 行数 | 条目数 | 读法 |
|---|---|---|---|
| `facts.md` | 162 | 111 | 按主题分节；**grep 优先**，不必全读 |
| `pitfalls.md` | 208 | 102 | 按主题分节；**grep 优先**，不必全读 |
| `rejected.md` | 53 | 33 | ★ **动手前全量读**（防止同一方案被反复重新提出） |
| `decisions.md` | 110 | 56 | 索引 → `docs/adr/NNNN-*.md` |
| `open.md` | 57 | 26 | `[OPEN]` 待实测/裁决 ＋ `[ASSUMPTION]` **不得当结论用** |
| `apps/notepad.md` | 340 | 0 | 接记事本时**全量读**；8 个固定小节 |
| win32-input-research.md | 193 | 0 | 接 Notepad Adapter + 任何 Win32 输入操作时**全量读**（SendInput / keybd_event / SendKeys / AttachThreadInput / BlockInput / SetForegroundWindow / WindowPattern.Close / UIPI / 推荐 pipeline） |


> **迁移核对（ADR-0021 验证方式 2）—— 历史快照：下面三个数字是「迁移当时值」，不随后续追加变化**：
> 2026-09-18 从单体 `MEMORY.md` 逐条复制到 L1。**L1 当前条目数与各文件行数由 `cargo run -p xtask -- memory-counts` 机器校验**（ADR-0030 D1/D2）—— 禁止在本文件手写派生合计数字（PL-022 根因）
> （≥155，差额为当日新增）→ **零丢失**；当日后续又新增 **7** 条（EOL 补测 ×2、编码假阳性与路径不一致 ×2、
> ADR-0025 与 §10.1 决策 ×2、ADR 编号冲突 N8 ×1）。核对已记入 `LEDGER.md`。
> **当前规模只看上面那张表** —— 它是唯一落点，由 `cargo run -p xtask -- memory-counts` 机器校验；
> 本文件正文**禁止**再出现会随追加漂移的合计数字（规则 `memory/derived-total-in-prose`，ADR-0030 D2）。
> **归档触发**：任一文件 **> 400 行** → 按主题拆入 `docs/memory/archive/`（**按体积不按时间**）；
> 该触发也已机器化（规则 `memory/archive-threshold`，Warning 级）—— 此前它因行数手写过期而**永远不会被触发**。

## 条目格式（单行、可 grep）

```text
- [YYYY-MM-DD][标签][src:来源] 一句话结论 → 因此怎么做
标签：FACT 已验证事实 ｜ DECISION 决策(指向 ADR) ｜ REJECTED 已否决方案(含理由)
      PITFALL 踩坑 ｜ OPEN 未决 ｜ ASSUMPTION 假设(待验证)
更正：不删原条目，追加新条目并在其中写 [supersedes:YYYY-MM-DD]
```

## 更新职责

- **Implementer**：完成任务卡时若产生新 FACT / PITFALL / REJECTED，**必须**追加到对应 L1 文件
  （应用专属的进 `apps/<app>.md`）—— 这是 DoD 的一部分。
- **Orchestrator**：阶段末更新 §1 快照、维护上面的「规模」表、按体积归档、核对路由是否失效。
- **禁止**：删除或改写他人条目（更正用新条目 + `[supersedes:日期]` 标注）。
- **禁止**：把项目结论写进 `~/.codex/memories/`（不版本化、不跨 agent、无法开源 → ADR-0021 已否决）。

---

## §1 项目当前状态快照（可覆写，最近更新：**2026-09-24**）

```text
阶段        ：**阶段 1（三试点闭环：Notepad → Paint → Edge/Chrome）**
              阶段 0 已于 2026-09-20 closeout（`docs/audits/stage-0-closeout-2026-09-20.md`）；阶段 0 的产出是 SPIKE_REPORT.md，不是产品代码（AGENTS.md §6）
git         ：main 与 origin 同步（**哈希不写进本快照** —— 它是每次提交都变的派生值，看 `git log -1`）
              提交身份 = snofin18 (via Codex) <snofin@gmail.com>（仓库本地 config，人类裁决方案 B）
              github.com 经本地代理 http://127.0.0.1:30000（global config；**只对 HTTP/HTTPS 生效**）
已产出文档  ：架构 v2.2、应用可行性 v1.1、AGENTS.md、gov、PLAN.md、LEDGER.md、README.md、
              plans/stage-0-spikes.md ＋ plans/stage-1-pilots.md（**阶段索引与批次表**）、
              tasks/TASK-*.md（**卡片正文 ＋ 执行记录，一卡一文件；ADR-0031**，stage-0 的 10 张 + stage-1 的 48 张已就位，其中 011 / 012 / 013 已展开完整正文、035 = 仅要点摘录（未展开），其余 44 张 = 批次表占位、派单前由 Orchestrator 按 gov §3.2 模板展开实际正文）、
              docs/{governance-ai-agent-execution, subagent-orchestration, storage-design,
              wbs-overview, overnight-automation-charter, DEPENDENCIES, PARKING_LOT}、
              docs/spec/**（**8 份**：`naming.md` + TASK-072 生成的 7 份 —— tool-schema / envelope /
              error-codes / capability-matrix / audit-event / ipc-protocol / testing；后 7 份有系统性结构
              缺陷（空 §1/§2 + 重复 §4/§5）→ **TASK-200 已修完，PL-038 已闭环**）、
              docs/adr/**README.md（编号登记表：已存在文件 / 待建号 /
              已退役编号 / 下一个可用编号；ADR-0026 建立，ADR-0030 改为机器校验）**
              ＋ 各 ADR 文件（**清单以登记表 §1 为准，本快照不再抄一遍** —— 抄一份就是第二个事实源）、
              docs/memory/*（ADR-0021 分层，2026-09-18 落地）、
              docs/nightly/{codex-automations-operations, scheduler-acceptance-test}.md、
              docs/spike-reports/SPIKE-A.md（**PARTIAL**）
已产出代码  ：xtask（零第三方依赖的护栏工具）。**子命令清单以 `cargo run -p xtask -- help` 输出为权威** —— 2026-09-24 起**不再在此手抄子命令个数与名单**：手抄的派生值必然漂移（PL-022 的根因；2026-09-24 TASK-015 新增 `check-ledger` / `check-migrations` 时当场证实这句「9 个」已过时）。另有 exemptions helper module（不暴露）。
              **行数与测试数不写进本快照** —— 看 `cargo test -p xtask` 与 `xtask hygiene` 的输出）
              + CI（三平台矩阵；**硬/软门禁清单与数量以 gov §5.1 表为准**，2026-09-18 新增
                #8b spike-deny 与 #12b doc-consistency 两道硬门禁）
              + spikes/spike-a-notepad：probe-01~04 + `src/bin/uia_dep_proof.rs`（Rust COM，E1~E6 全 PASS）
              ⚠ `.ps1` 纯 ASCII 合规情况（ADR-0024 D4）：probe-03 / probe-04 = **0** 非 ASCII 字节 ✅；
                probe-01 / probe-02 中文注释已清扫（**PL-026 已关闭**，probe-01 788→10 / probe-02 941→35 字节；剩 45 字节 = 3 条 STR-LIT 测试数据的意外非 ASCII，已以 ADR-0032 豁免登记 E-021/022/023，移除触发 = 永不）
工具链      ：rustc/cargo 1.98.1 stable-msvc ✅；cargo-deny 0.20.2 ✅；cargo-llvm-cov 0.9.1 ✅（行覆盖 96.31%）
              inspect.exe ✅（Windows Kits 10.0.26100.0）；Accessibility Insights ❌ 未装（**已否决补装**，ADR-0024 D3）
平台基线    ：Windows 11 24H2/25H2（唯一正式基线）；本机实测 25H2 build 26200.9457，3200×2000 @200%
试点顺序    ：Notepad → Paint → Edge/Chrome（阶段 1）→ Excel（阶段 2）→ Photoshop（阶段 3）
下一步      ：① **stage-0 已正式 closeout**（2026-09-20，TASK-073）；详 `docs/audits/stage-0-closeout-2026-09-20.md`
              ② **stage-1 的逐卡进度不在此处手抄** —— 唯一落点是 `PLAN.md` 的「当前状态」块（ADR-0039 D4）与
                 `plans/stage-1-pilots.md` 的「当前进度」句（ADR-0041 D1）；手抄派生进度 = PL-022 的根因（**PL-065 本次闭环**）。
                 本快照只保留**不随卡 Done 漂移**的约束：依赖列里只有 `013 ← 012` 真串行，`014 ← 011` / `015 ← 011` 在 011 完成后即解锁；
                 实际顺序 011→012→013→014→015，瓶颈 = 人类审阅带宽（AGENTS.md §3：并行度 ≤3、一会话 1~2 张卡）；
                 ✅ **PL-037 已闭环（2026-09-24，人类裁决选项 ③ → TASK-201）**：`crates/core` 骨架已提前落地（零依赖 / 零 `pub` 项）；
                 ✅ **跨阶段治理卡 TASK-200 / 201 / 202 / 203 均已 Done**；**PL-047 已闭环**（`check-migrations` 子命令，归 TASK-015，2026-09-24）；
                 ✅ **`cargo test -p assistant-core arch::` 已由 TASK-015 转为真断言**（`crates/core/tests/arch_layering.rs`；条数以命令输出为准，不写进本快照）
              ③ TASK-002 续做补完 SPIKE-A PARTIAL 仍 Blocked（`open.md N3` create_thread 上游 #36315/#36250 未关闭）→ 人类手工建会话
              ④ 夜间自动化 **GATE-0 已通过（2026-09-24）**（`open.md N9` 已关闭：cron + heartbeat 两形态实测全绿、投递形态已正常化）→ **尚未创建正式排期**（人类另行安排）；探针 automation 已删除
              ⑤ 改公共热点文件（LEDGER / `docs/memory/*` / PARKING_LOT / MEMORY.md / `plans/*`）前必须 `xtask guard acquire`（ADR-0028）；
                  超时放弃（退出码 5）后须 LEDGER 追加一行 + 不得 `--force` 硬抢
              ⑥ 仓库根乱码 0 字节文件名：**TASK-083 已 2026-09-24 收尾**（实测目标文件已不存在、`git status -uall` 干净；详 `tasks/TASK-083-*.md` §5）
              ⑦ **第三方依赖必须先批准 + 登记**（漂移触发器 ①）：`rusqlite`+bundled / `zstd` / `sha2`（TASK-012）、`keyring` / `zeroize`（TASK-014）、`serde` 的新使用方 `crates/platform/api`（TASK-016）均已登记于 `docs/DEPENDENCIES.md`
待产出文档  ：docs/OPEN_SOURCE_CHECKLIST.md（**PL-005 未落地**）、
              docs/dev-env-setup.md（**PL-017 已落地**，2026-09-18）、其余 7 份 Spike 报告（PL-026 的非 ASCII 注释已清扫，probe-01/02 现在纯 ASCII 仅 3 条 STR-LIT 豁免）
执行方式    ：AI coding agent（Codex/opencode/Claude Code）实现，人类规划+审阅+裁决；
              一个会话最多 1~2 张卡（AGENTS.md §3）；并行度 ≤3（人类审阅速度决定项目速度）
```

---

## §7 归档索引

（**暂无**。`docs/memory/` 下还没有任何文件超过 400 行。归档规则见 `docs/memory/archive/README.md`：
按**体积**触发、按**主题**拆分、拆出后原处留指针 + 本表登记一行 + `LEDGER.md` 追加一行。）
