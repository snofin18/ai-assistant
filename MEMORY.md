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
| `facts.md` | 109 | 71 | 按主题分节；**grep 优先**，不必全读 |
| `pitfalls.md` | 90 | 61 | 按主题分节；**grep 优先**，不必全读 |
| `rejected.md` | 51 | 31 | ★ **动手前全量读**（防止同一方案被反复重新提出） |
| `decisions.md` | 90 | 50 | 索引 → `docs/adr/NNNN-*.md` |
| `open.md` | 51 | 25 | `[OPEN]` 待实测/裁决 ＋ `[ASSUMPTION]` **不得当结论用** |
| `apps/notepad.md` | 243 | 0 | 接记事本时**全量读**；8 个固定小节 |

> **迁移核对（ADR-0021 验证方式 2）—— 历史快照：下面三个数字是「迁移当时值」，不随后续追加变化**：
> 2026-09-18 从单体 `MEMORY.md`（256 行 / **155** 条）逐条复制到 L1，迁移后 L1 合计 **200** 条
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

## §1 项目当前状态快照（可覆写，最近更新：**2026-09-18**）

```text
阶段        ：阶段 0（纯文档 + xtask 护栏 + spike 探针，**零产品代码**）
              阶段 0 的产出是 SPIKE_REPORT.md，不是产品代码（AGENTS.md §6）
git         ：main 与 origin 同步（**哈希不写进本快照** —— 它是每次提交都变的派生值，看 `git log -1`）
              提交身份 = snofin18 (via Codex) <snofin@gmail.com>（仓库本地 config，人类裁决方案 B）
              github.com 经本地代理 http://127.0.0.1:30000（global config；**只对 HTTP/HTTPS 生效**）
已产出文档  ：架构 v2.2、应用可行性 v1.1、AGENTS.md、gov、PLAN.md、LEDGER.md、README.md、
              plans/stage-0-spikes.md ＋ plans/stage-1-pilots.md（**阶段索引与批次表**）、
              tasks/TASK-*.md（**卡片正文 ＋ 执行记录，一卡一文件；ADR-0031**，stage-0 的 10 张已就位）、
              docs/{governance-ai-agent-execution, subagent-orchestration, storage-design,
              wbs-overview, overnight-automation-charter, DEPENDENCIES, PARKING_LOT}、
              docs/spec/naming.md、docs/adr/**README.md（编号登记表：已存在文件 / 待建号 /
              已退役编号 / 下一个可用编号；ADR-0026 建立，ADR-0030 改为机器校验）**
              ＋ 各 ADR 文件（**清单以登记表 §1 为准，本快照不再抄一遍** —— 抄一份就是第二个事实源）、
              docs/memory/*（ADR-0021 分层，2026-09-18 落地）、
              docs/nightly/{codex-automations-operations, scheduler-acceptance-test}.md、
              docs/spike-reports/SPIKE-A.md（**PARTIAL**）
已产出代码  ：xtask（零第三方依赖的护栏工具；子命令 = hygiene / memory-counts / adr-index / guard。
              **行数与测试数不写进本快照** —— 看 `cargo test -p xtask` 与 `xtask hygiene` 的输出）
              + CI（三平台矩阵；**硬/软门禁清单与数量以 gov §5.1 表为准**，2026-09-18 新增
                #8b spike-deny 与 #12b doc-consistency 两道硬门禁）
              + spikes/spike-a-notepad：probe-01~04 + `src/bin/uia_dep_proof.rs`（Rust COM，E1~E6 全 PASS）
              ⚠ `.ps1` 纯 ASCII 合规情况（ADR-0024 D4）：probe-03 / probe-04 = **0** 非 ASCII 字节 ✅；
                probe-01 / probe-02 仍含中文注释（788 / 941 字节，D4 之前提交）→ **PL-026 待清扫**
工具链      ：rustc/cargo 1.98.1 stable-msvc ✅；cargo-deny 0.20.2 ✅；cargo-llvm-cov 0.9.1 ✅（行覆盖 96.31%）
              inspect.exe ✅（Windows Kits 10.0.26100.0）；Accessibility Insights ❌ 未装（**已否决补装**，ADR-0024 D3）
平台基线    ：Windows 11 24H2/25H2（唯一正式基线）；本机实测 25H2 build 26200.9457，3200×2000 @200%
试点顺序    ：Notepad → Paint → Edge/Chrome（阶段 1）→ Excel（阶段 2）→ Photoshop（阶段 3）
下一步      ：① **TASK-002 正式开工**（Spike A 剩余：写路径/菜单/跨进程另存为/大文件/失败注入/接口考古）
                —— `create_thread` 不可用（见 rejected.md 2026-09-18），**由人类手工新建会话**，
                第一句贴 AGENTS.md §3 的约束回执模板
              ② 夜间自动化：**ADR-0029 已把机制改回 Codex 原生 scheduled tasks**（ADR-0018 降级为
                回退方案），章程 §11 重写为 **v1.4**；操作手册已落档但 **GATE-0 未执行**
                （open.md **N9**）→ 需人类专门安排调试时段，在此之前**禁止创建真实 automation**
              ③ 改写公共热点文件（LEDGER / docs/memory/* / PARKING_LOT）**前必须先 `xtask guard acquire`**
                （ADR-0028）；超时放弃（退出码 5）后必须在 LEDGER 追加一行说明
              ④ **DRIFT-001-3 已裁决关闭**（ADR-0031「一卡一文件」Accepted，stage-0 的 10 张卡已迁移，
                零丢失核对通过）；`xtask card-check` 的四条判据已写死在 D6 / gov §3.4，**实现仍归 PL-002**
              ⑤ 待人类裁决：M5 许可证**已裁决关闭**（MIT OR Apache-2.0、`LICENSE` + `NOTICE` 均已落地）、`docs/PARKING_LOT.md` 未关闭项（护栏增强类）、
                gov §5.4 行数口径矛盾**已裁决**（ADR-0033：写作规范 400/600 vs CI 门禆 600/900 各管一摊）、豁免机制**已裁决**（ADR-0032 Accepted，20 条基线豁免已登记于···/docs/adr/0032-doc-rule-exemption-registry.md，机器读入实现归 PL-002 / PL-015）
待产出文档  ：docs/spec/*（其余 6 份，含 testing.md）、docs/OPEN_SOURCE_CHECKLIST.md、
              docs/dev-env-setup.md（**PL-017 已落地**，2026-09-18）、其余 7 份 Spike 报告（PL-026 的非 ASCII 注释已清扫，probe-01/02 现在纯 ASCII 仅 3 条 STR-LIT 豁免）
执行方式    ：AI coding agent（Codex/opencode/Claude Code）实现，人类规划+审阅+裁决；
              一个会话最多 1~2 张卡（AGENTS.md §3）；并行度 ≤3（人类审阅速度决定项目速度）
```

---

## §7 归档索引

（**暂无**。`docs/memory/` 下还没有任何文件超过 400 行。归档规则见 `docs/memory/archive/README.md`：
按**体积**触发、按**主题**拆分、拆出后原处留指针 + 本表登记一行 + `LEDGER.md` 追加一行。）
